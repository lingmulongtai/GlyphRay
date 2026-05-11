use crate::{ChannelKind, ConnectionStats, RealtimeTransport, TransportError, TransportPacket};
use glyphray_protocol::MessageKind;
use std::io::ErrorKind;
use std::net::{SocketAddr, UdpSocket};

const DATAGRAM_MAGIC: [u8; 4] = *b"GLYT";
const DATAGRAM_VERSION: u16 = 1;
const HEADER_LEN: usize = 33;
const MAX_DATAGRAM_PAYLOAD: usize = 60_000;

pub struct UdpTransport {
    socket: UdpSocket,
    stats: ConnectionStats,
}

impl UdpTransport {
    pub fn bind(local: SocketAddr, peer: SocketAddr) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(local).map_err(io_error)?;
        socket.connect(peer).map_err(io_error)?;
        socket.set_nonblocking(true).map_err(io_error)?;

        Ok(Self {
            socket,
            stats: ConnectionStats {
                rtt_ms: 0.0,
                jitter_ms: 0.0,
                packet_loss_percent: 0.0,
                estimated_bandwidth_kbps: 0,
            },
        })
    }
}

impl RealtimeTransport for UdpTransport {
    fn send(&mut self, packet: TransportPacket) -> Result<(), TransportError> {
        let datagram = encode_packet(&packet)?;
        self.socket.send(&datagram).map_err(io_error)?;
        Ok(())
    }

    fn poll_recv(&mut self) -> Result<Option<TransportPacket>, TransportError> {
        let mut buffer = vec![0_u8; HEADER_LEN + MAX_DATAGRAM_PAYLOAD];
        match self.socket.recv(&mut buffer) {
            Ok(len) => decode_packet(&buffer[..len]).map(Some),
            Err(err) if err.kind() == ErrorKind::WouldBlock => Ok(None),
            Err(err) => Err(io_error(err)),
        }
    }

    fn stats(&self) -> ConnectionStats {
        self.stats
    }
}

pub fn encode_packet(packet: &TransportPacket) -> Result<Vec<u8>, TransportError> {
    if packet.payload.len() > MAX_DATAGRAM_PAYLOAD {
        return Err(TransportError::PayloadTooLarge);
    }

    let mut out = Vec::with_capacity(HEADER_LEN + packet.payload.len());
    out.extend_from_slice(&DATAGRAM_MAGIC);
    out.extend_from_slice(&DATAGRAM_VERSION.to_le_bytes());
    out.push(channel_to_u8(packet.channel));
    out.extend_from_slice(&(packet.message_kind as u16).to_le_bytes());
    out.extend_from_slice(&packet.sequence.to_le_bytes());
    out.extend_from_slice(&packet.enqueue_timestamp_us.to_le_bytes());
    out.extend_from_slice(&(packet.payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&crc32fast::hash(&packet.payload).to_le_bytes());
    out.extend_from_slice(&packet.payload);
    Ok(out)
}

pub fn decode_packet(bytes: &[u8]) -> Result<TransportPacket, TransportError> {
    if bytes.len() < HEADER_LEN {
        return Err(TransportError::Decode("short datagram".to_string()));
    }
    if bytes[0..4] != DATAGRAM_MAGIC[..] {
        return Err(TransportError::Decode("bad datagram magic".to_string()));
    }

    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != DATAGRAM_VERSION {
        return Err(TransportError::Decode(format!(
            "unsupported datagram version {version}"
        )));
    }

    let channel = channel_from_u8(bytes[6])?;
    let message_kind = MessageKind::try_from(u16::from_le_bytes([bytes[7], bytes[8]]))
        .map_err(|err| TransportError::Decode(err.to_string()))?;
    let sequence = u64::from_le_bytes(bytes[9..17].try_into().expect("slice length"));
    let enqueue_timestamp_us =
        u64::from_le_bytes(bytes[17..25].try_into().expect("slice length"));
    let payload_len = u32::from_le_bytes(bytes[25..29].try_into().expect("slice length")) as usize;
    if payload_len > MAX_DATAGRAM_PAYLOAD {
        return Err(TransportError::PayloadTooLarge);
    }
    if bytes.len() != HEADER_LEN + payload_len {
        return Err(TransportError::Decode("payload length mismatch".to_string()));
    }

    let expected_crc = u32::from_le_bytes(bytes[29..33].try_into().expect("slice length"));
    let payload = bytes[HEADER_LEN..].to_vec();
    if crc32fast::hash(&payload) != expected_crc {
        return Err(TransportError::Decode("payload checksum mismatch".to_string()));
    }

    Ok(TransportPacket {
        sequence,
        channel,
        message_kind,
        enqueue_timestamp_us,
        payload,
    })
}

fn channel_to_u8(channel: ChannelKind) -> u8 {
    match channel {
        ChannelKind::Video => 1,
        ChannelKind::Audio => 2,
        ChannelKind::Input => 3,
        ChannelKind::Control => 4,
    }
}

fn channel_from_u8(value: u8) -> Result<ChannelKind, TransportError> {
    match value {
        1 => Ok(ChannelKind::Video),
        2 => Ok(ChannelKind::Audio),
        3 => Ok(ChannelKind::Input),
        4 => Ok(ChannelKind::Control),
        _ => Err(TransportError::Decode(format!("unknown channel {value}"))),
    }
}

fn io_error(error: std::io::Error) -> TransportError {
    TransportError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datagram_round_trips_packet_metadata_and_payload() {
        let packet = TransportPacket {
            sequence: 99,
            channel: ChannelKind::Input,
            message_kind: MessageKind::StylusInputBatch,
            enqueue_timestamp_us: 123_456,
            payload: vec![1, 2, 3, 4],
        };

        let encoded = encode_packet(&packet).expect("encode");
        let decoded = decode_packet(&encoded).expect("decode");
        assert_eq!(decoded, packet);
    }

    #[test]
    fn datagram_rejects_payload_tampering() {
        let packet = TransportPacket {
            sequence: 1,
            channel: ChannelKind::Control,
            message_kind: MessageKind::LatencyPing,
            enqueue_timestamp_us: 0,
            payload: vec![9, 9, 9],
        };

        let mut encoded = encode_packet(&packet).expect("encode");
        let last = encoded.len() - 1;
        encoded[last] ^= 0x7f;

        assert!(matches!(decode_packet(&encoded), Err(TransportError::Decode(_))));
    }
}

