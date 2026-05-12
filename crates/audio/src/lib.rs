use glyphray_protocol::AudioFrame;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCodec {
    Pcm16,
    Opus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u16,
    pub codec: AudioCodec,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
            frame_duration_ms: 10,
            codec: AudioCodec::Opus,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioError {
    #[error("audio payload is empty")]
    EmptyPayload,
}

#[derive(Debug, Clone)]
pub struct AudioPacketizer {
    config: AudioConfig,
    next_sequence: u64,
}

impl AudioPacketizer {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            config,
            next_sequence: 1,
        }
    }

    pub fn packetize(
        &mut self,
        capture_timestamp_us: u64,
        payload: Vec<u8>,
    ) -> Result<AudioFrame, AudioError> {
        if payload.is_empty() {
            return Err(AudioError::EmptyPayload);
        }
        let sequence = self.next_sequence;
        self.next_sequence += 1;
        Ok(AudioFrame {
            sequence,
            capture_timestamp_us,
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_packetizer_assigns_sequences() {
        let mut packetizer = AudioPacketizer::new(AudioConfig::default());
        let first = packetizer.packetize(10, vec![1, 2]).expect("first");
        let second = packetizer.packetize(20, vec![3, 4]).expect("second");
        assert_eq!(first.sequence, 1);
        assert_eq!(second.sequence, 2);
        assert_eq!(first.sample_rate, 48_000);
    }
}
