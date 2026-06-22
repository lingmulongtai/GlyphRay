use super::{InputError, TouchInjectionReport, TouchInjector};
use glyphray_core::CoordinateMapper;
use glyphray_protocol::{TouchAction, TouchInputBatch};
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::UI::Input::Pointer::{
    InitializeTouchInjection, InjectTouchInput, POINTER_FLAG_CANCELED, POINTER_FLAG_DOWN,
    POINTER_FLAG_INCONTACT, POINTER_FLAG_INRANGE, POINTER_FLAG_PRIMARY, POINTER_FLAG_UP,
    POINTER_FLAG_UPDATE, POINTER_INFO, POINTER_TOUCH_INFO, TOUCH_FEEDBACK_NONE,
};
use windows::Win32::UI::WindowsAndMessaging::{
    PT_TOUCH, TOUCH_MASK_CONTACTAREA, TOUCH_MASK_ORIENTATION, TOUCH_MASK_PRESSURE,
};

pub struct PlatformTouchInjector;

impl PlatformTouchInjector {
    pub fn open() -> Result<Self, InputError> {
        unsafe {
            InitializeTouchInjection(10, TOUCH_FEEDBACK_NONE)
                .map_err(|_| InputError::DeviceCreationFailed)?;
        }
        Ok(Self)
    }
}

impl TouchInjector for PlatformTouchInjector {
    fn inject_touch_batch(
        &mut self,
        batch: &TouchInputBatch,
        mapper: &CoordinateMapper,
    ) -> Result<TouchInjectionReport, InputError> {
        for sample in &batch.samples {
            let mapped = mapper.map(sample.x, sample.y);
            let x = mapped.x.round() as i32;
            let y = mapped.y.round() as i32;
            let radius_x = (sample.major.max(8.0) * 0.5).round() as i32;
            let radius_y = (sample.minor.max(8.0) * 0.5).round() as i32;

            let mut pointer_info = POINTER_INFO {
                pointerType: PT_TOUCH,
                pointerId: sample.pointer_id,
                ptPixelLocation: POINT { x, y },
                pointerFlags: POINTER_FLAG_PRIMARY | POINTER_FLAG_INRANGE,
                ..Default::default()
            };
            pointer_info.pointerFlags |= match sample.action {
                TouchAction::Down => POINTER_FLAG_DOWN | POINTER_FLAG_INCONTACT,
                TouchAction::Move => POINTER_FLAG_UPDATE | POINTER_FLAG_INCONTACT,
                TouchAction::Up => POINTER_FLAG_UP,
                TouchAction::Cancel => POINTER_FLAG_UP | POINTER_FLAG_CANCELED,
            };

            let touch_info = POINTER_TOUCH_INFO {
                pointerInfo: pointer_info,
                touchFlags: 0,
                touchMask: TOUCH_MASK_CONTACTAREA | TOUCH_MASK_ORIENTATION | TOUCH_MASK_PRESSURE,
                rcContact: RECT {
                    left: x - radius_x,
                    top: y - radius_y,
                    right: x + radius_x,
                    bottom: y + radius_y,
                },
                rcContactRaw: RECT::default(),
                orientation: sample.orientation_degrees.rem_euclid(360.0).round() as u32,
                pressure: (sample.pressure.clamp(0.0, 1.0) * 1024.0).round() as u32,
            };

            unsafe {
                InjectTouchInput(&[touch_info]).map_err(|_| InputError::TouchInjectionFailed)?;
            }
        }

        Ok(TouchInjectionReport {
            injected_samples: batch.samples.len(),
        })
    }
}
