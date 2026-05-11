use super::{map_action_to_contact, InjectionReport, InputError, PenInjector};
use glyphray_core::{CoordinateMapper, PressureMapper};
use glyphray_protocol::{StylusAction, StylusInputBatch};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Controls::{
    CreateSyntheticPointerDevice, DestroySyntheticPointerDevice, HSYNTHETICPOINTERDEVICE,
    POINTER_FEEDBACK_DEFAULT, POINTER_TYPE_INFO, POINTER_TYPE_INFO_0,
};
use windows::Win32::UI::Input::Pointer::{
    InjectSyntheticPointerInput, POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE,
    POINTER_FLAG_PRIMARY, POINTER_FLAG_UPDATE, POINTER_FLAG_UP, POINTER_INFO, POINTER_PEN_INFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    PEN_MASK_PRESSURE, PEN_MASK_ROTATION, PEN_MASK_TILT_X, PEN_MASK_TILT_Y, PT_PEN,
};

pub struct PlatformPenInjector {
    device: HSYNTHETICPOINTERDEVICE,
}

impl PlatformPenInjector {
    pub fn open() -> Result<Self, InputError> {
        let device = unsafe {
            CreateSyntheticPointerDevice(PT_PEN, 1, POINTER_FEEDBACK_DEFAULT)
                .map_err(|_| InputError::DeviceCreationFailed)?
        };
        if device.is_invalid() {
            return Err(InputError::DeviceCreationFailed);
        }
        Ok(Self { device })
    }
}

impl Drop for PlatformPenInjector {
    fn drop(&mut self) {
        unsafe {
            DestroySyntheticPointerDevice(self.device);
        }
    }
}

impl PenInjector for PlatformPenInjector {
    fn inject_batch(
        &mut self,
        batch: &StylusInputBatch,
        mapper: &CoordinateMapper,
        pressure: &PressureMapper,
    ) -> Result<InjectionReport, InputError> {
        for sample in &batch.samples {
            let mapped = mapper.map(sample.x, sample.y);
            let mut pointer_info = POINTER_INFO::default();
            pointer_info.pointerType = PT_PEN;
            pointer_info.pointerId = sample.pointer_id;
            pointer_info.ptPixelLocation = POINT {
                x: mapped.x.round() as i32,
                y: mapped.y.round() as i32,
            };
            pointer_info.pointerFlags = POINTER_FLAG_PRIMARY | POINTER_FLAG_INRANGE;

            if map_action_to_contact(sample.action, sample) {
                pointer_info.pointerFlags |= POINTER_FLAG_INCONTACT;
            }

            pointer_info.pointerFlags |= match sample.action {
                StylusAction::Down => POINTER_FLAG_DOWN,
                StylusAction::Up => POINTER_FLAG_UP,
                StylusAction::Cancel => POINTER_FLAG_UP,
                _ => POINTER_FLAG_UPDATE,
            };

            let pen_info = POINTER_PEN_INFO {
                pointerInfo: pointer_info,
                penFlags: Default::default(),
                penMask: PEN_MASK_PRESSURE
                    | PEN_MASK_TILT_X
                    | PEN_MASK_TILT_Y
                    | PEN_MASK_ROTATION,
                pressure: pressure.to_windows_pressure(sample.pressure),
                rotation: sample.orientation_degrees.rem_euclid(360.0).round() as u32,
                tiltX: sample.tilt_x_degrees.round().clamp(-90.0, 90.0) as i32,
                tiltY: sample.tilt_y_degrees.round().clamp(-90.0, 90.0) as i32,
            };

            let pointer_type_info = POINTER_TYPE_INFO {
                r#type: PT_PEN,
                Anonymous: POINTER_TYPE_INFO_0 { penInfo: pen_info },
            };

            if unsafe { InjectSyntheticPointerInput(self.device, &[pointer_type_info]) }.is_err() {
                return Err(InputError::InjectionFailed);
            }
        }

        Ok(InjectionReport {
            injected_samples: batch.samples.len(),
            used_pen_path: true,
        })
    }
}
