use glyphray_protocol::MessageKind;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::Duration;

pub mod fragment;
pub mod bitrate;
pub mod reconnect;
pub mod relay;
pub mod udp;
pub mod secure;
pub mod video;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelKind {
    Video,
    Audio,
    Input,
    Control,
}

impl ChannelKind {
    pub fn priority(self) -> u8 {
        match self {
            Self::Input => 0,
            Self::Control => 1,
            Self::Audio => 2,
            Self::Video => 3,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportPacket {
    pub sequence: u64,
    pub channel: ChannelKind,
    pub message_kind: MessageKind,
    pub enqueue_timestamp_us: u64,
    pub payload: Vec<u8>,
}

impl TransportPacket {
    pub fn input(sequence: u64, payload: Vec<u8>) -> Self {
        Self {
            sequence,
            channel: ChannelKind::Input,
            message_kind: MessageKind::StylusInputBatch,
            enqueue_timestamp_us: 0,
            payload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TransportConfig {
    pub target_rtt: Duration,
    pub max_jitter_buffer: Duration,
    pub prefer_input_over_video: bool,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            target_rtt: Duration::from_millis(8),
            max_jitter_buffer: Duration::from_millis(16),
            prefer_input_over_video: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ConnectionStats {
    pub rtt_ms: f32,
    pub jitter_ms: f32,
    pub packet_loss_percent: f32,
    pub estimated_bandwidth_kbps: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("transport is disconnected")]
    Disconnected,
    #[error("packet payload exceeds transport maximum")]
    PayloadTooLarge,
    #[error("transport packet could not be decoded: {0}")]
    Decode(String),
    #[error("i/o error: {0}")]
    Io(String),
}

pub trait RealtimeTransport {
    fn send(&mut self, packet: TransportPacket) -> Result<(), TransportError>;
    fn poll_recv(&mut self) -> Result<Option<TransportPacket>, TransportError>;
    fn stats(&self) -> ConnectionStats;
}

#[derive(Debug, Default)]
pub struct PriorityPacketQueue {
    packets: Vec<TransportPacket>,
}

impl PriorityPacketQueue {
    pub fn push(&mut self, packet: TransportPacket) {
        self.packets.push(packet);
    }

    pub fn pop(&mut self) -> Option<TransportPacket> {
        let index = self
            .packets
            .iter()
            .enumerate()
            .min_by_key(|(_, packet)| (packet.channel.priority(), packet.sequence))
            .map(|(index, _)| index)?;
        Some(self.packets.remove(index))
    }
}

#[derive(Debug)]
pub struct SimulatedTransport {
    queue: VecDeque<TransportPacket>,
    rng: StdRng,
    drop_probability: f32,
    stats: ConnectionStats,
}

impl SimulatedTransport {
    pub fn new(seed: u64, drop_probability: f32) -> Self {
        Self {
            queue: VecDeque::new(),
            rng: StdRng::seed_from_u64(seed),
            drop_probability: drop_probability.clamp(0.0, 1.0),
            stats: ConnectionStats {
                rtt_ms: 4.0,
                jitter_ms: 1.0,
                packet_loss_percent: drop_probability * 100.0,
                estimated_bandwidth_kbps: 80_000,
            },
        }
    }
}

impl RealtimeTransport for SimulatedTransport {
    fn send(&mut self, packet: TransportPacket) -> Result<(), TransportError> {
        if self.rng.gen::<f32>() >= self.drop_probability {
            self.queue.push_back(packet);
        }
        Ok(())
    }

    fn poll_recv(&mut self) -> Result<Option<TransportPacket>, TransportError> {
        Ok(self.queue.pop_front())
    }

    fn stats(&self) -> ConnectionStats {
        self.stats
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_packets_are_prioritized_over_video() {
        let mut queue = PriorityPacketQueue::default();
        queue.push(TransportPacket {
            sequence: 1,
            channel: ChannelKind::Video,
            message_kind: MessageKind::VideoFrame,
            enqueue_timestamp_us: 0,
            payload: vec![1],
        });
        queue.push(TransportPacket::input(2, vec![2]));

        let first = queue.pop().unwrap();
        assert_eq!(first.channel, ChannelKind::Input);
    }

    #[test]
    fn simulated_transport_can_drop_packets() {
        let mut transport = SimulatedTransport::new(7, 1.0);
        transport.send(TransportPacket::input(1, vec![1])).unwrap();
        assert!(transport.poll_recv().unwrap().is_none());
        assert_eq!(transport.stats().packet_loss_percent, 100.0);
    }

    #[test]
    fn simulated_transport_delivers_without_loss() {
        let mut transport = SimulatedTransport::new(7, 0.0);
        transport.send(TransportPacket::input(9, vec![9])).unwrap();
        assert_eq!(transport.poll_recv().unwrap().unwrap().sequence, 9);
    }
}
