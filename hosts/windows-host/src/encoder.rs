use crate::capture::CapturedFrame;
use glyphray_protocol::{ColorSpace, VideoCodec};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderBackend {
    Auto,
    IntelQuickSync,
    NvidiaNvenc,
    AmdAmf,
    Software,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderSettings {
    pub codec: VideoCodec,
    pub color_space: ColorSpace,
    pub width: u32,
    pub height: u32,
    pub fps: u16,
    pub target_bitrate_kbps: u32,
    pub keyframe_interval_ms: u32,
    pub allow_b_frames: bool,
    pub backend: EncoderBackend,
}

impl EncoderSettings {
    pub fn low_latency_h264(width: u32, height: u32, fps: u16) -> Self {
        let pixels = width.saturating_mul(height).max(1);
        let bitrate = ((pixels as f32 / (1920.0 * 1080.0)) * 18_000.0)
            .round()
            .clamp(4_000.0, 60_000.0) as u32;

        Self {
            codec: VideoCodec::H264,
            color_space: ColorSpace::Rec709,
            width,
            height,
            fps,
            target_bitrate_kbps: bitrate,
            keyframe_interval_ms: 1_000,
            allow_b_frames: false,
            backend: EncoderBackend::Auto,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncodedVideoFrame {
    pub sequence: u64,
    pub codec: VideoCodec,
    pub capture_timestamp_us: u64,
    pub encode_done_timestamp_us: u64,
    pub is_keyframe: bool,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EncoderError {
    #[error("encoder backend is not available: {0:?}")]
    BackendUnavailable(EncoderBackend),
    #[error("input frame dimensions do not match encoder settings")]
    DimensionMismatch,
    #[error("encoder has not been started")]
    NotStarted,
}

pub trait VideoEncoder {
    fn settings(&self) -> &EncoderSettings;
    fn start(&mut self) -> Result<(), EncoderError>;
    fn encode(&mut self, frame: &CapturedFrame) -> Result<EncodedVideoFrame, EncoderError>;
    fn request_keyframe(&mut self) -> Result<(), EncoderError>;
}

pub struct PendingHardwareEncoder {
    settings: EncoderSettings,
    started: bool,
    next_sequence: u64,
    force_keyframe: bool,
}

impl PendingHardwareEncoder {
    pub fn new(settings: EncoderSettings) -> Self {
        Self {
            settings,
            started: false,
            next_sequence: 1,
            force_keyframe: true,
        }
    }
}

impl VideoEncoder for PendingHardwareEncoder {
    fn settings(&self) -> &EncoderSettings {
        &self.settings
    }

    fn start(&mut self) -> Result<(), EncoderError> {
        self.started = true;
        Ok(())
    }

    fn encode(&mut self, frame: &CapturedFrame) -> Result<EncodedVideoFrame, EncoderError> {
        if !self.started {
            return Err(EncoderError::NotStarted);
        }
        if frame.width != self.settings.width || frame.height != self.settings.height {
            return Err(EncoderError::DimensionMismatch);
        }

        let sequence = self.next_sequence;
        self.next_sequence += 1;
        let is_keyframe = std::mem::take(&mut self.force_keyframe);

        Ok(EncodedVideoFrame {
            sequence,
            codec: self.settings.codec,
            capture_timestamp_us: frame.capture_timestamp_us,
            encode_done_timestamp_us: frame.capture_timestamp_us,
            is_keyframe,
            payload: Vec::new(),
        })
    }

    fn request_keyframe(&mut self) -> Result<(), EncoderError> {
        self.force_keyframe = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_latency_h264_disables_b_frames() {
        let settings = EncoderSettings::low_latency_h264(1920, 1080, 60);
        assert_eq!(settings.codec, VideoCodec::H264);
        assert_eq!(settings.color_space, ColorSpace::Rec709);
        assert!(!settings.allow_b_frames);
        assert_eq!(settings.keyframe_interval_ms, 1_000);
    }

    #[test]
    fn pending_encoder_requires_start() {
        let settings = EncoderSettings::low_latency_h264(16, 16, 60);
        let mut encoder = PendingHardwareEncoder::new(settings);
        let frame = CapturedFrame {
            display_id: 0,
            width: 16,
            height: 16,
            capture_timestamp_us: 44,
            bgra: vec![0; 16 * 16 * 4],
        };

        assert_eq!(encoder.encode(&frame), Err(EncoderError::NotStarted));
        encoder.start().expect("start");
        assert_eq!(encoder.encode(&frame).expect("encode").sequence, 1);
    }
}
