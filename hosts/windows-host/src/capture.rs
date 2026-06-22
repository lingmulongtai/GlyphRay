use glyphray_protocol::DisplayDescriptor;

#[cfg(windows)]
mod desktop_duplication;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    #[error("screen capture is not implemented for this platform yet")]
    UnsupportedPlatform,
    #[error("display {0} was not found")]
    DisplayNotFound(u32),
    #[error("screen capture access was lost and must be recreated")]
    AccessLost,
    #[error("screen capture backend failed: {0}")]
    Backend(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedFrame {
    pub display_id: u32,
    pub width: u32,
    pub height: u32,
    pub capture_timestamp_us: u64,
    pub bgra: Vec<u8>,
}

pub trait ScreenCapture {
    fn list_displays(&self) -> Result<Vec<DisplayDescriptor>, CaptureError>;
    fn capture_frame(&mut self, display_id: u32) -> Result<CapturedFrame, CaptureError>;
}

#[derive(Default)]
pub struct WindowsGraphicsCaptureBackend {
    #[cfg(windows)]
    inner: desktop_duplication::DesktopDuplicationCapture,
}

impl WindowsGraphicsCaptureBackend {
    pub fn new() -> Self {
        Self::default()
    }
}

impl ScreenCapture for WindowsGraphicsCaptureBackend {
    fn list_displays(&self) -> Result<Vec<DisplayDescriptor>, CaptureError> {
        platform::list_displays()
    }

    fn capture_frame(&mut self, display_id: u32) -> Result<CapturedFrame, CaptureError> {
        #[cfg(windows)]
        {
            self.inner.capture_display(display_id)
        }
        #[cfg(not(windows))]
        {
            platform::capture_display(display_id)
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::CaptureError;
    use glyphray_protocol::DisplayDescriptor;

    pub fn list_displays() -> Result<Vec<DisplayDescriptor>, CaptureError> {
        Ok(Vec::new())
    }

    pub fn capture_display(_display_id: u32) -> Result<super::CapturedFrame, CaptureError> {
        Err(CaptureError::UnsupportedPlatform)
    }
}

#[cfg(windows)]
mod platform {
    use super::{desktop_duplication, CaptureError};
    use glyphray_protocol::DisplayDescriptor;

    pub fn list_displays() -> Result<Vec<DisplayDescriptor>, CaptureError> {
        desktop_duplication::list_displays()
    }
}
