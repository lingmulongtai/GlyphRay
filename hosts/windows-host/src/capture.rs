use glyphray_protocol::DisplayDescriptor;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    #[error("screen capture is not implemented for this platform yet")]
    UnsupportedPlatform,
    #[error("display {0} was not found")]
    DisplayNotFound(u32),
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

pub struct WindowsGraphicsCaptureBackend;

impl ScreenCapture for WindowsGraphicsCaptureBackend {
    fn list_displays(&self) -> Result<Vec<DisplayDescriptor>, CaptureError> {
        platform::list_displays()
    }

    fn capture_frame(&mut self, display_id: u32) -> Result<CapturedFrame, CaptureError> {
        platform::capture_display(display_id)
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
    use super::{CaptureError, CapturedFrame};
    use glyphray_protocol::DisplayDescriptor;
    use std::mem::size_of;
    use std::ptr::null_mut;
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject,
        EnumDisplayMonitors, GetDC, GetDIBits, GetMonitorInfoW, ReleaseDC, SelectObject,
        BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ, HMONITOR,
        MONITORINFO, MONITORINFOEXW, RGBQUAD, SRCCOPY,
    };
    use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

    pub fn list_displays() -> Result<Vec<DisplayDescriptor>, CaptureError> {
        let mut displays = Vec::<DisplayDescriptor>::new();
        let data = LPARAM(&mut displays as *mut Vec<DisplayDescriptor> as isize);
        let ok = unsafe { EnumDisplayMonitors(HDC::default(), None, Some(enum_monitor), data) };
        if !ok.as_bool() {
            return Err(CaptureError::UnsupportedPlatform);
        }
        Ok(displays)
    }

    pub fn capture_display(display_id: u32) -> Result<CapturedFrame, CaptureError> {
        let display = list_displays()?
            .into_iter()
            .find(|display| display.id == display_id)
            .ok_or(CaptureError::DisplayNotFound(display_id))?;

        capture_rect(display)
    }

    fn capture_rect(display: DisplayDescriptor) -> Result<CapturedFrame, CaptureError> {
        let width = display.width_px as i32;
        let height = display.height_px as i32;
        if width <= 0 || height <= 0 {
            return Err(CaptureError::Backend(
                "display has no capture area".to_string(),
            ));
        }

        let hwnd = HWND::default();
        let screen_dc = unsafe { GetDC(hwnd) };
        if screen_dc.0 == null_mut() {
            return Err(last_error("GetDC"));
        }

        let mem_dc = unsafe { CreateCompatibleDC(screen_dc) };
        if mem_dc.0 == null_mut() {
            unsafe {
                ReleaseDC(hwnd, screen_dc);
            }
            return Err(last_error("CreateCompatibleDC"));
        }

        let bitmap = unsafe { CreateCompatibleBitmap(screen_dc, width, height) };
        if bitmap.0 == null_mut() {
            unsafe {
                let _ = DeleteDC(mem_dc);
                ReleaseDC(hwnd, screen_dc);
            }
            return Err(last_error("CreateCompatibleBitmap"));
        }

        let old_object = unsafe { SelectObject(mem_dc, HGDIOBJ(bitmap.0)) };
        let bitblt_ok = unsafe {
            BitBlt(
                mem_dc,
                0,
                0,
                width,
                height,
                screen_dc,
                display.origin_x,
                display.origin_y,
                SRCCOPY,
            )
        };
        if bitblt_ok.is_err() {
            cleanup_gdi(hwnd, screen_dc, mem_dc, bitmap, old_object);
            return Err(last_error("BitBlt"));
        }

        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: (width * height * 4) as u32,
                ..Default::default()
            },
            bmiColors: [RGBQUAD::default(); 1],
        };
        let mut pixels = vec![0_u8; (width * height * 4) as usize];
        let rows = unsafe {
            GetDIBits(
                mem_dc,
                bitmap,
                0,
                height as u32,
                Some(pixels.as_mut_ptr() as *mut _),
                &mut bitmap_info,
                DIB_RGB_COLORS,
            )
        };

        cleanup_gdi(hwnd, screen_dc, mem_dc, bitmap, old_object);

        if rows == 0 {
            return Err(last_error("GetDIBits"));
        }

        Ok(CapturedFrame {
            display_id: display.id,
            width: display.width_px,
            height: display.height_px,
            capture_timestamp_us: now_us(),
            bgra: pixels,
        })
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
        let len = raw
            .iter()
            .position(|value| *value == 0)
            .unwrap_or(raw.len());
        if len == 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&raw[..len]))
    }

    fn cleanup_gdi(hwnd: HWND, screen_dc: HDC, mem_dc: HDC, bitmap: HBITMAP, old_object: HGDIOBJ) {
        unsafe {
            if old_object.0 != null_mut() {
                SelectObject(mem_dc, old_object);
            }
            let _ = DeleteObject(HGDIOBJ(bitmap.0));
            let _ = DeleteDC(mem_dc);
            ReleaseDC(hwnd, screen_dc);
        }
    }

    fn now_us() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_micros() as u64)
            .unwrap_or_default()
    }

    fn last_error(api: &str) -> CaptureError {
        let code = unsafe { windows::Win32::Foundation::GetLastError() };
        CaptureError::Backend(format!("{api} failed with Win32 error {}", code.0))
    }
}
