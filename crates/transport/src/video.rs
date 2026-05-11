use crate::fragment::{decode_fragment, encode_fragment, fragment_frame, FrameReassembler};
use crate::{ChannelKind, TransportError, TransportPacket};
use glyphray_protocol::{MessageKind, VideoCodec};

pub const DEFAULT_VIDEO_FRAGMENT_PAYLOAD: usize = 1_200;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedVideoAccessUnit {
    pub sequence: u64,
    pub codec: VideoCodec,
    pub presentation_time_us: u64,
    pub is_keyframe: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoPacketizer {
    max_fragment_payload: usize,
}

impl Default for VideoPacketizer {
    fn default() -> Self {
        Self::new(DEFAULT_VIDEO_FRAGMENT_PAYLOAD)
    }
}

impl VideoPacketizer {
    pub fn new(max_fragment_payload: usize) -> Self {
        Self {
            max_fragment_payload: max_fragment_payload.max(1),
        }
    }

    pub fn packetize(
        &self,
        access_unit: &EncodedVideoAccessUnit,
    ) -> Result<Vec<TransportPacket>, TransportError> {
        let envelope = encode_access_unit(access_unit)?;
        let fragments = fragment_frame(
            access_unit.sequence,
            &envelope,
            self.max_fragment_payload,
        )?;

        fragments
            .into_iter()
            .map(|fragment| {
                Ok(TransportPacket {
                    sequence: access_unit.sequence,
                    channel: ChannelKind::Video,
                    message_kind: MessageKind::VideoFrame,
                    enqueue_timestamp_us: access_unit.presentation_time_us,
                    payload: encode_fragment(&fragment)?,
                })
            })
            .collect()
    }
}

#[derive(Debug, Default)]
pub struct VideoReassembler {
    fragments: FrameReassembler,
}

impl VideoReassembler {
    pub fn push_packet(
        &mut self,
        packet: &TransportPacket,
    ) -> Result<Option<EncodedVideoAccessUnit>, TransportError> {
        if packet.channel != ChannelKind::Video || packet.message_kind != MessageKind::VideoFrame {
            return Err(TransportError::Decode(
                "packet is not a video frame packet".to_string(),
            ));
        }

        let fragment = decode_fragment(&packet.payload)?;
        let Some(envelope) = self.fragments.push(fragment)? else {
            return Ok(None);
        };

        decode_access_unit(&envelope).map(Some)
    }
}

pub fn encode_access_unit(access_unit: &EncodedVideoAccessUnit) -> Result<Vec<u8>, TransportError> {
    let mut out = Vec::with_capacity(22 + access_unit.payload.len());
    out.push(codec_to_u8(access_unit.codec));
    out.push(u8::from(access_unit.is_keyframe));
    out.extend_from_slice(&access_unit.sequence.to_le_bytes());
    out.extend_from_slice(&access_unit.presentation_time_us.to_le_bytes());
    out.extend_from_slice(&(access_unit.payload.len() as u32).to_le_bytes());
    out.extend_from_slice(&access_unit.payload);
    Ok(out)
}

pub fn decode_access_unit(bytes: &[u8]) -> Result<EncodedVideoAccessUnit, TransportError> {
    if bytes.len() < 22 {
        return Err(TransportError::Decode(
            "short encoded video access unit".to_string(),
        ));
    }

    let codec = codec_from_u8(bytes[0])?;
    let is_keyframe = match bytes[1] {
        0 => false,
        1 => true,
        value => {
            return Err(TransportError::Decode(format!(
                "invalid keyframe flag {value}"
            )))
        }
    };
    let sequence = u64::from_le_bytes(bytes[2..10].try_into().expect("slice length"));
    let presentation_time_us = u64::from_le_bytes(bytes[10..18].try_into().expect("slice length"));
    let payload_len = u32::from_le_bytes(bytes[18..22].try_into().expect("slice length")) as usize;
    if bytes.len() != 22 + payload_len {
        return Err(TransportError::Decode(
            "encoded video access unit length mismatch".to_string(),
        ));
    }

    Ok(EncodedVideoAccessUnit {
        sequence,
        codec,
        presentation_time_us,
        is_keyframe,
        payload: bytes[22..].to_vec(),
    })
}

fn codec_to_u8(codec: VideoCodec) -> u8 {
    match codec {
        VideoCodec::H264 => 1,
        VideoCodec::H265 => 2,
        VideoCodec::Av1 => 3,
    }
}

fn codec_from_u8(value: u8) -> Result<VideoCodec, TransportError> {
    match value {
        1 => Ok(VideoCodec::H264),
        2 => Ok(VideoCodec::H265),
        3 => Ok(VideoCodec::Av1),
        _ => Err(TransportError::Decode(format!("unknown video codec {value}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoded_access_unit_round_trips() {
        let unit = EncodedVideoAccessUnit {
            sequence: 8,
            codec: VideoCodec::H264,
            presentation_time_us: 123,
            is_keyframe: true,
            payload: vec![0, 0, 1, 103],
        };

        let decoded = decode_access_unit(&encode_access_unit(&unit).expect("encode"))
            .expect("decode");
        assert_eq!(decoded, unit);
    }

    #[test]
    fn packetizer_reassembles_large_access_unit() {
        let unit = EncodedVideoAccessUnit {
            sequence: 90,
            codec: VideoCodec::H264,
            presentation_time_us: 10_000,
            is_keyframe: false,
            payload: (0..30_000).map(|value| (value % 251) as u8).collect(),
        };
        let packetizer = VideoPacketizer::new(777);
        let packets = packetizer.packetize(&unit).expect("packetize");
        assert!(packets.len() > 1);

        let mut reassembler = VideoReassembler::default();
        let mut completed = None;
        for packet in packets {
            completed = reassembler.push_packet(&packet).expect("push");
        }

        assert_eq!(completed, Some(unit));
    }
}

