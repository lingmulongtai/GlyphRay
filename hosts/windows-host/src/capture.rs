use glyphray_protocol::DisplayDescriptor;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    #[error("screen capture is not implemented for this platform yet")]
    UnsupportedPlatform,
    #[error("display {0} was not found")]
    DisplayNotFound(u32),
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

pub struct WindowsGraphicsCaptureBackend;

impl ScreenCapture for WindowsGraphicsCaptureBackend {
    fn list_displays(&self) -> Result<Vec<DisplayDescriptor>, CaptureError> {
        platform::list_displays()
    }

    fn capture_frame(&mut self, display_id: u32) -> Result<CapturedFrame, CaptureError> {
        Err(CaptureError::DisplayNotFound(display_id))
    }
}

#[cfg(not(windows))]
mod platform {
    use super::CaptureError;
    use glyphray_protocol::DisplayDescriptor;

    pub fn list_displays() -> Result<Vec<DisplayDescriptor>, CaptureError> {
        Ok(Vec::new())
    }
}

#[cfg(windows)]
mod platform {
    use super::CaptureError;
    use glyphray_protocol::DisplayDescriptor;
    use std::mem::size_of;
    use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO, MONITORINFOEXW,
        MONITORINFOF_PRIMARY,
    };

    pub fn list_displays() -> Result<Vec<DisplayDescriptor>, CaptureError> {
        let mut displays = Vec::<DisplayDescriptor>::new();
        let data = LPARAM(&mut displays as *mut Vec<DisplayDescriptor> as isize);
        let ok = unsafe { EnumDisplayMonitors(HDC::default(), None, Some(enum_monitor), data) };
        if !ok.as_bool() {
            return Err(CaptureError::UnsupportedPlatform);
        }
        Ok(displays)
    }

    unsafe extern "system" fn enum_monitor(
        monitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        data: LPARAM,
    ) -> BOOL {
        let displays = &mut *(data.0 as *mut Vec<DisplayDescriptor>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;

        let ok = GetMonitorInfoW(
            monitor,
            &mut info as *mut MONITORINFOEXW as *mut MONITORINFO,
        );
        if ok.as_bool() {
            let rect = info.monitorInfo.rcMonitor;
            let name = utf16_device_name(&info.szDevice)
                .unwrap_or_else(|| format!("Display {}", displays.len() + 1));
            displays.push(DisplayDescriptor {
                id: displays.len() as u32,
                name,
                origin_x: rect.left,
                origin_y: rect.top,
                width_px: (rect.right - rect.left).max(0) as u32,
                height_px: (rect.bottom - rect.top).max(0) as u32,
                scale_factor: 1.0,
                rotation_degrees: 0,
                refresh_hz: 60.0,
                primary: (info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY) != 0,
            });
        }

        BOOL(1)
    }

    fn utf16_device_name(raw: &[u16]) -> Option<String> {
        let len = raw.iter().position(|value| *value == 0).unwrap_or(raw.len());
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&raw[..len]))
    }
}
