use serde::{Deserialize, Serialize};

pub const MAGIC: [u8; 4] = *b"GLYR";
pub const WIRE_VERSION: u16 = 1;
pub const HEADER_LEN: usize = 24;
const MAX_PAYLOAD_LEN: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ProtocolError {
    #[error("frame is shorter than the GlyphRay header")]
    ShortFrame,
    #[error("invalid frame magic")]
    InvalidMagic,
    #[error("unsupported protocol version {0}")]
    UnsupportedVersion(u16),
    #[error("payload length exceeds configured maximum")]
    PayloadTooLarge,
    #[error("declared payload length does not match frame length")]
    LengthMismatch,
    #[error("message kind {header_kind:?} does not match payload kind {payload_kind:?}")]
    KindMismatch {
        header_kind: MessageKind,
        payload_kind: MessageKind,
    },
    #[error("payload checksum mismatch")]
    ChecksumMismatch,
    #[error("serialization failed: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum MessageKind {
    ClientHello = 1,
    HostHello = 2,
    AuthChallenge = 3,
    AuthResponse = 4,
    PairingRequest = 5,
    PairingResult = 6,
    DisplayInfo = 7,
    EncoderConfig = 8,
    VideoFrame = 9,
    AudioFrame = 10,
    StylusInputBatch = 11,
    MouseInput = 12,
    KeyboardInput = 13,
    ClipboardMessage = 14,
    LatencyPing = 15,
    LatencyPong = 16,
    ErrorMessage = 17,
    Disconnect = 18,
}

impl TryFrom<u16> for MessageKind {
    type Error = ProtocolError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::ClientHello),
            2 => Ok(Self::HostHello),
            3 => Ok(Self::AuthChallenge),
            4 => Ok(Self::AuthResponse),
            5 => Ok(Self::PairingRequest),
            6 => Ok(Self::PairingResult),
            7 => Ok(Self::DisplayInfo),
            8 => Ok(Self::EncoderConfig),
            9 => Ok(Self::VideoFrame),
            10 => Ok(Self::AudioFrame),
            11 => Ok(Self::StylusInputBatch),
            12 => Ok(Self::MouseInput),
            13 => Ok(Self::KeyboardInput),
            14 => Ok(Self::ClipboardMessage),
            15 => Ok(Self::LatencyPing),
            16 => Ok(Self::LatencyPong),
            17 => Ok(Self::ErrorMessage),
            18 => Ok(Self::Disconnect),
            _ => Err(ProtocolError::Serialization(format!(
                "unknown message kind {value}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Message {
    ClientHello(ClientHello),
    HostHello(HostHello),
    AuthChallenge(AuthChallenge),
    AuthResponse(AuthResponse),
    PairingRequest(PairingRequest),
    PairingResult(PairingResult),
    DisplayInfo(DisplayInfo),
    EncoderConfig(EncoderConfig),
    VideoFrame(VideoFrame),
    AudioFrame(AudioFrame),
    StylusInputBatch(StylusInputBatch),
    MouseInput(MouseInput),
    KeyboardInput(KeyboardInput),
    ClipboardMessage(ClipboardMessage),
    LatencyPing(LatencyPing),
    LatencyPong(LatencyPong),
    ErrorMessage(ErrorMessage),
    Disconnect(Disconnect),
}

impl Message {
    pub fn kind(&self) -> MessageKind {
        match self {
            Self::ClientHello(_) => MessageKind::ClientHello,
            Self::HostHello(_) => MessageKind::HostHello,
            Self::AuthChallenge(_) => MessageKind::AuthChallenge,
            Self::AuthResponse(_) => MessageKind::AuthResponse,
            Self::PairingRequest(_) => MessageKind::PairingRequest,
            Self::PairingResult(_) => MessageKind::PairingResult,
            Self::DisplayInfo(_) => MessageKind::DisplayInfo,
            Self::EncoderConfig(_) => MessageKind::EncoderConfig,
            Self::VideoFrame(_) => MessageKind::VideoFrame,
            Self::AudioFrame(_) => MessageKind::AudioFrame,
            Self::StylusInputBatch(_) => MessageKind::StylusInputBatch,
            Self::MouseInput(_) => MessageKind::MouseInput,
            Self::KeyboardInput(_) => MessageKind::KeyboardInput,
            Self::ClipboardMessage(_) => MessageKind::ClipboardMessage,
            Self::LatencyPing(_) => MessageKind::LatencyPing,
            Self::LatencyPong(_) => MessageKind::LatencyPong,
            Self::ErrorMessage(_) => MessageKind::ErrorMessage,
            Self::Disconnect(_) => MessageKind::Disconnect,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub sequence: u64,
    pub message: Message,
}

pub fn encode_frame(sequence: u64, message: &Message) -> Result<Vec<u8>, ProtocolError> {
    let payload = bincode::serialize(message)
        .map_err(|err| ProtocolError::Serialization(err.to_string()))?;
    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(ProtocolError::PayloadTooLarge);
    }

    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&WIRE_VERSION.to_le_bytes());
    out.extend_from_slice(&(message.kind() as u16).to_le_bytes());
    out.extend_from_slice(&sequence.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&crc32fast::hash(&payload).to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

pub fn decode_frame(bytes: &[u8]) -> Result<Frame, ProtocolError> {
    if bytes.len() < HEADER_LEN {
        return Err(ProtocolError::ShortFrame);
    }
    if bytes[0..4] != MAGIC[..] {
        return Err(ProtocolError::InvalidMagic);
    }

    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    if version != WIRE_VERSION {
        return Err(ProtocolError::UnsupportedVersion(version));
    }

    let header_kind = MessageKind::try_from(u16::from_le_bytes([bytes[6], bytes[7]]))?;
    let sequence = u64::from_le_bytes(bytes[8..16].try_into().expect("slice length"));
    let payload_len = u32::from_le_bytes(bytes[16..20].try_into().expect("slice length")) as usize;
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(ProtocolError::PayloadTooLarge);
    }
    if bytes.len() != HEADER_LEN + payload_len {
        return Err(ProtocolError::LengthMismatch);
    }

    let expected_crc = u32::from_le_bytes(bytes[20..24].try_into().expect("slice length"));
    let payload = &bytes[HEADER_LEN..];
    if crc32fast::hash(payload) != expected_crc {
        return Err(ProtocolError::ChecksumMismatch);
    }

    let message: Message = bincode::deserialize(payload)
        .map_err(|err| ProtocolError::Serialization(err.to_string()))?;
    let payload_kind = message.kind();
    if header_kind != payload_kind {
        return Err(ProtocolError::KindMismatch {
            header_kind,
            payload_kind,
        });
    }

    Ok(Frame { sequence, message })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientHello {
    pub protocol_min: u16,
    pub protocol_max: u16,
    pub app_version: String,
    pub device_name: String,
    pub device_public_key: Vec<u8>,
    pub supports_stylus: bool,
    pub max_refresh_hz: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostHello {
    pub protocol_version: u16,
    pub app_version: String,
    pub host_name: String,
    pub host_public_key: Vec<u8>,
    pub capabilities: HostCapabilities,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCapabilities {
    pub windows_ink: bool,
    pub mouse_keyboard: bool,
    pub h264: bool,
    pub h265: bool,
    pub av1: bool,
    pub max_refresh_hz: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthChallenge {
    pub challenge_id: u64,
    pub nonce: [u8; 32],
    pub issued_at_unix_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthResponse {
    pub challenge_id: u64,
    pub device_id: String,
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingRequest {
    pub device_name: String,
    pub pairing_code_hash: Vec<u8>,
    pub one_time_public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingResult {
    pub accepted: bool,
    pub trusted_device_id: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub displays: Vec<DisplayDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayDescriptor {
    pub id: u32,
    pub name: String,
    pub origin_x: i32,
    pub origin_y: i32,
    pub width_px: u32,
    pub height_px: u32,
    pub scale_factor: f32,
    pub rotation_degrees: u16,
    pub refresh_hz: f32,
    pub primary: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EncoderConfig {
    pub display_id: u32,
    pub codec: VideoCodec,
    pub width: u32,
    pub height: u32,
    pub max_fps: u16,
    pub target_bitrate_kbps: u32,
    pub keyframe_interval_ms: u32,
    pub low_latency: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoCodec {
    H264,
    H265,
    Av1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoFrame {
    pub sequence: u64,
    pub capture_timestamp_us: u64,
    pub encode_done_timestamp_us: u64,
    pub display_id: u32,
    pub is_keyframe: bool,
    pub codec: VideoCodec,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioFrame {
    pub sequence: u64,
    pub capture_timestamp_us: u64,
    pub sample_rate: u32,
    pub channels: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StylusInputBatch {
    pub batch_sequence: u64,
    pub monotonic_timestamp_us: u64,
    pub samples: Vec<StylusSample>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StylusSample {
    pub sequence: u64,
    pub timestamp_us: u64,
    pub display_id: u32,
    pub pointer_id: u32,
    pub tool_type: StylusToolType,
    pub action: StylusAction,
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
    pub tilt_x_degrees: f32,
    pub tilt_y_degrees: f32,
    pub orientation_degrees: f32,
    pub button_flags: u32,
    pub hover: bool,
    pub eraser: bool,
    pub predicted: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StylusToolType {
    Unknown,
    Finger,
    Stylus,
    Eraser,
    Mouse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StylusAction {
    HoverEnter,
    HoverMove,
    HoverExit,
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseInput {
    pub sequence: u64,
    pub timestamp_us: u64,
    pub display_id: u32,
    pub x: f32,
    pub y: f32,
    pub wheel_delta_x: f32,
    pub wheel_delta_y: f32,
    pub button_flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyboardInput {
    pub sequence: u64,
    pub timestamp_us: u64,
    pub scan_code: u32,
    pub virtual_key: u32,
    pub pressed: bool,
    pub modifiers: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardMessage {
    pub sequence: u64,
    pub mime_type: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencyPing {
    pub sequence: u64,
    pub client_send_timestamp_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatencyPong {
    pub sequence: u64,
    pub client_send_timestamp_us: u64,
    pub host_receive_timestamp_us: u64,
    pub host_send_timestamp_us: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorMessage {
    pub code: u32,
    pub retryable: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Disconnect {
    pub reason: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stylus_batch_round_trips_as_binary_frame() {
        let message = Message::StylusInputBatch(StylusInputBatch {
            batch_sequence: 42,
            monotonic_timestamp_us: 1_500_000,
            samples: vec![
                StylusSample {
                    sequence: 1,
                    timestamp_us: 1_500_001,
                    display_id: 7,
                    pointer_id: 2,
                    tool_type: StylusToolType::Stylus,
                    action: StylusAction::Down,
                    x: 120.5,
                    y: 92.25,
                    pressure: 0.45,
                    tilt_x_degrees: -12.0,
                    tilt_y_degrees: 18.0,
                    orientation_degrees: 44.0,
                    button_flags: 0b10,
                    hover: false,
                    eraser: false,
                    predicted: false,
                },
                StylusSample {
                    sequence: 2,
                    timestamp_us: 1_500_004,
                    display_id: 7,
                    pointer_id: 2,
                    tool_type: StylusToolType::Stylus,
                    action: StylusAction::Move,
                    x: 121.0,
                    y: 93.0,
                    pressure: 0.5,
                    tilt_x_degrees: -10.0,
                    tilt_y_degrees: 17.0,
                    orientation_degrees: 45.0,
                    button_flags: 0,
                    hover: false,
                    eraser: false,
                    predicted: false,
                },
            ],
        });

        let encoded = encode_frame(77, &message).expect("encode");
        assert_eq!(&encoded[0..4], &MAGIC);

        let decoded = decode_frame(&encoded).expect("decode");
        assert_eq!(decoded.sequence, 77);
        assert_eq!(decoded.message, message);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut encoded = encode_frame(1, &Message::LatencyPing(LatencyPing {
            sequence: 3,
            client_send_timestamp_us: 10,
        }))
        .expect("encode");
        encoded[0] = b'X';

        assert_eq!(decode_frame(&encoded), Err(ProtocolError::InvalidMagic));
    }

    #[test]
    fn rejects_checksum_mismatch() {
        let mut encoded = encode_frame(1, &Message::Disconnect(Disconnect {
            reason: "test".to_string(),
        }))
        .expect("encode");
        let last = encoded.len() - 1;
        encoded[last] ^= 0x11;

        assert_eq!(decode_frame(&encoded), Err(ProtocolError::ChecksumMismatch));
    }
}
