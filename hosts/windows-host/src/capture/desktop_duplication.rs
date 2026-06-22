use super::{CaptureError, CapturedFrame};
use glyphray_protocol::DisplayDescriptor;
use std::collections::HashMap;
use std::slice;
use std::time::{SystemTime, UNIX_EPOCH};
use windows::core::{Error as WindowsError, Interface, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_UNKNOWN, D3D_FEATURE_LEVEL};
use windows::Win32::Graphics::Direct3D11::{
    D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11Texture2D, D3D11_CPU_ACCESS_READ,
    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_MAPPED_SUBRESOURCE, D3D11_MAP_READ, D3D11_SDK_VERSION,
    D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_MODE_ROTATION, DXGI_MODE_ROTATION_IDENTITY,
    DXGI_MODE_ROTATION_ROTATE180, DXGI_MODE_ROTATION_ROTATE270, DXGI_MODE_ROTATION_ROTATE90,
    DXGI_MODE_ROTATION_UNSPECIFIED, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1, IDXGIOutput, IDXGIOutput1,
    IDXGIOutputDuplication, IDXGIResource, DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_NOT_FOUND,
    DXGI_ERROR_WAIT_TIMEOUT, DXGI_OUTDUPL_FRAME_INFO, DXGI_OUTPUT_DESC,
};
use windows::Win32::Graphics::Gdi::{
    EnumDisplaySettingsW, GetMonitorInfoW, DEVMODEW, ENUM_CURRENT_SETTINGS, MONITORINFO,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;

const ACQUIRE_TIMEOUT_MS: u32 = 100;

#[derive(Default)]
pub(super) struct DesktopDuplicationCapture {
    sessions: HashMap<u32, DuplicationSession>,
}

impl DesktopDuplicationCapture {
    pub(super) fn capture_display(
        &mut self,
        display_id: u32,
    ) -> Result<CapturedFrame, CaptureError> {
        if let std::collections::hash_map::Entry::Vacant(entry) = self.sessions.entry(display_id) {
            entry.insert(DuplicationSession::open(display_id)?);
        }

        let result = self
            .sessions
            .get_mut(&display_id)
            .expect("session inserted above")
            .capture();
        if matches!(&result, Err(error) if is_access_lost(error)) {
            let mut replacement = DuplicationSession::open(display_id)?;
            let frame = replacement.capture();
            self.sessions.insert(display_id, replacement);
            return frame;
        }
        result
    }
}

struct OutputBinding {
    display: DisplayDescriptor,
    adapter: IDXGIAdapter1,
    output: IDXGIOutput1,
    rotation: DXGI_MODE_ROTATION,
}

struct DuplicationSession {
    display: DisplayDescriptor,
    rotation: DXGI_MODE_ROTATION,
    _device: ID3D11Device,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    staging: ID3D11Texture2D,
    texture_width: u32,
    texture_height: u32,
    last_pixels: Option<Vec<u8>>,
}

impl DuplicationSession {
    fn open(display_id: u32) -> Result<Self, CaptureError> {
        let binding = enumerate_outputs()?
            .into_iter()
            .find(|binding| binding.display.id == display_id)
            .ok_or(CaptureError::DisplayNotFound(display_id))?;

        let (device, context) = create_device(&binding.adapter)?;
        let duplication = unsafe { binding.output.DuplicateOutput(&device) }
            .map_err(|error| backend_error("IDXGIOutput1::DuplicateOutput", error))?;
        let duplication_desc = unsafe { duplication.GetDesc() };
        let texture_width = duplication_desc.ModeDesc.Width;
        let texture_height = duplication_desc.ModeDesc.Height;
        if texture_width == 0 || texture_height == 0 {
            return Err(CaptureError::Backend(
                "Desktop Duplication reported an empty output".to_string(),
            ));
        }

        let staging_desc = D3D11_TEXTURE2D_DESC {
            Width: texture_width,
            Height: texture_height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            BindFlags: 0,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            MiscFlags: 0,
        };
        let mut staging = None;
        unsafe { device.CreateTexture2D(&staging_desc, None, Some(&mut staging)) }
            .map_err(|error| backend_error("ID3D11Device::CreateTexture2D", error))?;

        Ok(Self {
            display: binding.display,
            rotation: binding.rotation,
            _device: device,
            context,
            duplication,
            staging: staging.expect("CreateTexture2D succeeded without a texture"),
            texture_width,
            texture_height,
            last_pixels: None,
        })
    }

    fn capture(&mut self) -> Result<CapturedFrame, CaptureError> {
        let mut frame_info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut desktop_resource: Option<IDXGIResource> = None;
        let acquired = unsafe {
            self.duplication.AcquireNextFrame(
                ACQUIRE_TIMEOUT_MS,
                &mut frame_info,
                &mut desktop_resource,
            )
        };
        if let Err(error) = acquired {
            if error.code() == DXGI_ERROR_WAIT_TIMEOUT {
                return self.cached_frame().ok_or_else(|| {
                    CaptureError::Backend(
                        "Desktop Duplication timed out before the first frame".to_string(),
                    )
                });
            }
            return Err(backend_error(
                "IDXGIOutputDuplication::AcquireNextFrame",
                error,
            ));
        }

        let copied = self.copy_acquired_frame(desktop_resource);
        let released = unsafe { self.duplication.ReleaseFrame() };
        if let Err(error) = released {
            return Err(backend_error("IDXGIOutputDuplication::ReleaseFrame", error));
        }
        let pixels = copied?;
        self.last_pixels = Some(pixels.clone());
        Ok(self.build_frame(pixels))
    }

    fn copy_acquired_frame(
        &self,
        desktop_resource: Option<IDXGIResource>,
    ) -> Result<Vec<u8>, CaptureError> {
        let desktop_resource = desktop_resource.ok_or_else(|| {
            CaptureError::Backend("Desktop Duplication returned no resource".to_string())
        })?;
        let source: ID3D11Texture2D = desktop_resource
            .cast()
            .map_err(|error| backend_error("IDXGIResource::cast<ID3D11Texture2D>", error))?;

        unsafe {
            self.context.CopyResource(&self.staging, &source);
        }
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        unsafe {
            self.context
                .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
        }
        .map_err(|error| backend_error("ID3D11DeviceContext::Map", error))?;

        let row_bytes = self.texture_width as usize * 4;
        let mut pixels = vec![0_u8; row_bytes * self.texture_height as usize];
        if mapped.pData.is_null() || mapped.RowPitch < row_bytes as u32 {
            unsafe {
                self.context.Unmap(&self.staging, 0);
            }
            return Err(CaptureError::Backend(
                "D3D11 mapped texture has an invalid row pitch".to_string(),
            ));
        }
        for row in 0..self.texture_height as usize {
            let source = unsafe {
                slice::from_raw_parts(
                    (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                    row_bytes,
                )
            };
            pixels[row * row_bytes..(row + 1) * row_bytes].copy_from_slice(source);
        }
        unsafe {
            self.context.Unmap(&self.staging, 0);
        }

        Ok(rotate_bgra(
            &pixels,
            self.texture_width,
            self.texture_height,
            self.rotation,
        ))
    }

    fn cached_frame(&self) -> Option<CapturedFrame> {
        self.last_pixels
            .as_ref()
            .map(|pixels| self.build_frame(pixels.clone()))
    }

    fn build_frame(&self, bgra: Vec<u8>) -> CapturedFrame {
        let (width, height) =
            rotated_dimensions(self.texture_width, self.texture_height, self.rotation);
        CapturedFrame {
            display_id: self.display.id,
            width,
            height,
            capture_timestamp_us: now_us(),
            bgra,
        }
    }
}

pub(super) fn list_displays() -> Result<Vec<DisplayDescriptor>, CaptureError> {
    Ok(enumerate_outputs()?
        .into_iter()
        .map(|binding| binding.display)
        .collect())
}

fn enumerate_outputs() -> Result<Vec<OutputBinding>, CaptureError> {
    let factory: IDXGIFactory1 = unsafe { CreateDXGIFactory1() }
        .map_err(|error| backend_error("CreateDXGIFactory1", error))?;
    let mut bindings = Vec::new();
    let mut adapter_index = 0;
    loop {
        let adapter = match unsafe { factory.EnumAdapters1(adapter_index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => return Err(backend_error("IDXGIFactory1::EnumAdapters1", error)),
        };
        let mut output_index = 0;
        loop {
            let output: IDXGIOutput = match unsafe { adapter.EnumOutputs(output_index) } {
                Ok(output) => output,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(error) => return Err(backend_error("IDXGIAdapter::EnumOutputs", error)),
            };
            let desc: DXGI_OUTPUT_DESC = unsafe { output.GetDesc() }
                .map_err(|error| backend_error("IDXGIOutput::GetDesc", error))?;
            if desc.AttachedToDesktop.as_bool() {
                let rect = desc.DesktopCoordinates;
                let width = (rect.right - rect.left).max(0) as u32;
                let height = (rect.bottom - rect.top).max(0) as u32;
                if width > 0 && height > 0 {
                    let id = bindings.len() as u32;
                    bindings.push(OutputBinding {
                        display: DisplayDescriptor {
                            id,
                            name: utf16_device_name(&desc.DeviceName)
                                .unwrap_or_else(|| format!("Display {}", id + 1)),
                            origin_x: rect.left,
                            origin_y: rect.top,
                            width_px: width,
                            height_px: height,
                            scale_factor: monitor_scale_factor(desc.Monitor),
                            rotation_degrees: rotation_degrees(desc.Rotation),
                            refresh_hz: current_refresh_hz(&desc.DeviceName),
                            primary: monitor_is_primary(desc.Monitor),
                        },
                        adapter: adapter.clone(),
                        output: output.cast().map_err(|error| {
                            backend_error("IDXGIOutput::cast<IDXGIOutput1>", error)
                        })?,
                        rotation: desc.Rotation,
                    });
                }
            }
            output_index += 1;
        }
        adapter_index += 1;
    }
    Ok(bindings)
}

fn create_device(
    adapter: &IDXGIAdapter1,
) -> Result<(ID3D11Device, ID3D11DeviceContext), CaptureError> {
    let mut device = None;
    let mut context = None;
    let mut feature_level = D3D_FEATURE_LEVEL::default();
    unsafe {
        D3D11CreateDevice(
            adapter,
            D3D_DRIVER_TYPE_UNKNOWN,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(&mut device),
            Some(&mut feature_level),
            Some(&mut context),
        )
    }
    .map_err(|error| backend_error("D3D11CreateDevice", error))?;
    Ok((
        device.expect("D3D11CreateDevice succeeded without a device"),
        context.expect("D3D11CreateDevice succeeded without a context"),
    ))
}

fn monitor_is_primary(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> bool {
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool()
        && (info.dwFlags & MONITORINFOF_PRIMARY) != 0
}

fn monitor_scale_factor(monitor: windows::Win32::Graphics::Gdi::HMONITOR) -> f32 {
    let mut dpi_x = 96_u32;
    let mut dpi_y = 96_u32;
    if unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }.is_ok() {
        dpi_x as f32 / 96.0
    } else {
        1.0
    }
}

fn current_refresh_hz(device_name: &[u16; 32]) -> f32 {
    let mut mode = DEVMODEW {
        dmSize: std::mem::size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    let found = unsafe {
        EnumDisplaySettingsW(
            PCWSTR(device_name.as_ptr()),
            ENUM_CURRENT_SETTINGS,
            &mut mode,
        )
    };
    if found.as_bool() && mode.dmDisplayFrequency > 1 {
        mode.dmDisplayFrequency as f32
    } else {
        60.0
    }
}

fn rotate_bgra(source: &[u8], width: u32, height: u32, rotation: DXGI_MODE_ROTATION) -> Vec<u8> {
    if rotation == DXGI_MODE_ROTATION_UNSPECIFIED || rotation == DXGI_MODE_ROTATION_IDENTITY {
        return source.to_vec();
    }
    let mut output = vec![0_u8; source.len()];
    for y in 0..height {
        for x in 0..width {
            let (destination_x, destination_y, destination_width) =
                if rotation == DXGI_MODE_ROTATION_ROTATE90 {
                    (height - 1 - y, x, height)
                } else if rotation == DXGI_MODE_ROTATION_ROTATE180 {
                    (width - 1 - x, height - 1 - y, width)
                } else if rotation == DXGI_MODE_ROTATION_ROTATE270 {
                    (y, width - 1 - x, height)
                } else {
                    (x, y, width)
                };
            let source_offset = ((y * width + x) * 4) as usize;
            let destination_offset =
                ((destination_y * destination_width + destination_x) * 4) as usize;
            output[destination_offset..destination_offset + 4]
                .copy_from_slice(&source[source_offset..source_offset + 4]);
        }
    }
    output
}

fn rotated_dimensions(width: u32, height: u32, rotation: DXGI_MODE_ROTATION) -> (u32, u32) {
    if rotation == DXGI_MODE_ROTATION_ROTATE90 || rotation == DXGI_MODE_ROTATION_ROTATE270 {
        (height, width)
    } else {
        (width, height)
    }
}

fn rotation_degrees(rotation: DXGI_MODE_ROTATION) -> u16 {
    if rotation == DXGI_MODE_ROTATION_ROTATE90 {
        90
    } else if rotation == DXGI_MODE_ROTATION_ROTATE180 {
        180
    } else if rotation == DXGI_MODE_ROTATION_ROTATE270 {
        270
    } else {
        0
    }
}

fn utf16_device_name(raw: &[u16]) -> Option<String> {
    let len = raw
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(raw.len());
    (len > 0).then(|| String::from_utf16_lossy(&raw[..len]))
}

fn now_us() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros() as u64)
        .unwrap_or_default()
}

fn is_access_lost(error: &CaptureError) -> bool {
    matches!(error, CaptureError::AccessLost)
}

fn backend_error(api: &str, error: WindowsError) -> CaptureError {
    if error.code() == DXGI_ERROR_ACCESS_LOST {
        return CaptureError::AccessLost;
    }
    CaptureError::Backend(format!(
        "{api} failed: {} (0x{:08X})",
        error.message(),
        error.code().0 as u32
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(values: &[u8]) -> Vec<u8> {
        values
            .iter()
            .flat_map(|value| [*value, 0, 0, 255])
            .collect()
    }

    fn blue_values(pixels: &[u8]) -> Vec<u8> {
        pixels.chunks_exact(4).map(|pixel| pixel[0]).collect()
    }

    #[test]
    fn rotates_bgra_clockwise_for_portrait_output() {
        let source = pixels(&[1, 2, 3, 4, 5, 6]);
        let rotated = rotate_bgra(&source, 3, 2, DXGI_MODE_ROTATION_ROTATE90);
        assert_eq!(blue_values(&rotated), vec![4, 1, 5, 2, 6, 3]);
        assert_eq!(
            rotated_dimensions(3, 2, DXGI_MODE_ROTATION_ROTATE90),
            (2, 3)
        );
    }

    #[test]
    fn rotates_bgra_counter_clockwise() {
        let source = pixels(&[1, 2, 3, 4, 5, 6]);
        let rotated = rotate_bgra(&source, 3, 2, DXGI_MODE_ROTATION_ROTATE270);
        assert_eq!(blue_values(&rotated), vec![3, 6, 2, 5, 1, 4]);
    }
}
