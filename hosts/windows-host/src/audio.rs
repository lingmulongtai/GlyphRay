use glyphray_audio::AudioConfig;
#[cfg(test)]
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedAudioFrame {
    pub capture_timestamp_us: u64,
    pub sample_rate: u32,
    pub channels: u8,
    pub pcm16: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AudioCaptureError {
    #[error("audio capture is not available: {0}")]
    Unavailable(String),
    #[error("audio frame format {sample_rate} Hz / {channels} ch did not match configured {expected_sample_rate} Hz / {expected_channels} ch")]
    FormatMismatch {
        sample_rate: u32,
        channels: u8,
        expected_sample_rate: u32,
        expected_channels: u8,
    },
    #[error("captured audio payload is empty")]
    EmptyPayload,
}

pub trait AudioCapture {
    fn config(&self) -> AudioConfig;

    fn capture_audio_frame(&mut self) -> Result<CapturedAudioFrame, AudioCaptureError>;
}

#[derive(Debug, Clone)]
pub struct WindowsWasapiAudioCapture {
    config: AudioConfig,
}

impl WindowsWasapiAudioCapture {
    pub fn new(config: AudioConfig) -> Self {
        Self { config }
    }
}

impl Default for WindowsWasapiAudioCapture {
    fn default() -> Self {
        Self::new(AudioConfig {
            codec: glyphray_audio::AudioCodec::Pcm16,
            ..AudioConfig::default()
        })
    }
}

impl AudioCapture for WindowsWasapiAudioCapture {
    fn config(&self) -> AudioConfig {
        self.config
    }

    fn capture_audio_frame(&mut self) -> Result<CapturedAudioFrame, AudioCaptureError> {
        platform::capture_audio_frame(self.config)
    }
}

pub fn validate_audio_frame(
    frame: &CapturedAudioFrame,
    config: AudioConfig,
) -> Result<(), AudioCaptureError> {
    if frame.pcm16.is_empty() {
        return Err(AudioCaptureError::EmptyPayload);
    }
    if frame.sample_rate != config.sample_rate || frame.channels != config.channels {
        return Err(AudioCaptureError::FormatMismatch {
            sample_rate: frame.sample_rate,
            channels: frame.channels,
            expected_sample_rate: config.sample_rate,
            expected_channels: config.channels,
        });
    }
    Ok(())
}

#[cfg(test)]
fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros() as u64)
        .unwrap_or_default()
}

#[cfg(windows)]
mod platform {
    use super::{AudioCaptureError, CapturedAudioFrame};
    use glyphray_audio::AudioConfig;

    pub(super) fn capture_audio_frame(
        _config: AudioConfig,
    ) -> Result<CapturedAudioFrame, AudioCaptureError> {
        Err(AudioCaptureError::Unavailable(
            "WASAPI loopback capture is planned for the production audio worker; packetization and secure Audio-channel routing are implemented".to_string(),
        ))
    }
}

#[cfg(not(windows))]
mod platform {
    use super::{AudioCaptureError, CapturedAudioFrame};
    use glyphray_audio::AudioConfig;

    pub(super) fn capture_audio_frame(
        _config: AudioConfig,
    ) -> Result<CapturedAudioFrame, AudioCaptureError> {
        Err(AudioCaptureError::Unavailable(
            "Windows WASAPI capture is only available on Windows".to_string(),
        ))
    }
}

#[cfg(test)]
#[derive(Debug, Clone)]
pub(crate) struct FakeAudioCapture {
    config: AudioConfig,
    frames: std::collections::VecDeque<Vec<u8>>,
}

#[cfg(test)]
impl FakeAudioCapture {
    pub(crate) fn new(config: AudioConfig, frames: impl IntoIterator<Item = Vec<u8>>) -> Self {
        Self {
            config,
            frames: frames.into_iter().collect(),
        }
    }
}

#[cfg(test)]
impl AudioCapture for FakeAudioCapture {
    fn config(&self) -> AudioConfig {
        self.config
    }

    fn capture_audio_frame(&mut self) -> Result<CapturedAudioFrame, AudioCaptureError> {
        let pcm16 = self
            .frames
            .pop_front()
            .ok_or_else(|| AudioCaptureError::Unavailable("fake capture exhausted".to_string()))?;
        Ok(CapturedAudioFrame {
            capture_timestamp_us: now_us(),
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
            pcm16,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_matching_pcm_frame() {
        let config = AudioConfig {
            codec: glyphray_audio::AudioCodec::Pcm16,
            ..AudioConfig::default()
        };
        let frame = CapturedAudioFrame {
            capture_timestamp_us: 1,
            sample_rate: config.sample_rate,
            channels: config.channels,
            pcm16: vec![0, 1, 2, 3],
        };

        validate_audio_frame(&frame, config).expect("valid frame");
    }

    #[test]
    fn rejects_format_mismatch() {
        let config = AudioConfig {
            codec: glyphray_audio::AudioCodec::Pcm16,
            ..AudioConfig::default()
        };
        let frame = CapturedAudioFrame {
            capture_timestamp_us: 1,
            sample_rate: 44_100,
            channels: config.channels,
            pcm16: vec![0, 1],
        };

        assert!(matches!(
            validate_audio_frame(&frame, config),
            Err(AudioCaptureError::FormatMismatch { .. })
        ));
    }
}
