use super::{InputError, MouseInjectionReport, MouseInjector};
use glyphray_core::CoordinateMapper;
use glyphray_protocol::MouseInput;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT,
};
use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;

const ANDROID_BUTTON_PRIMARY: u32 = 1;
const ANDROID_BUTTON_SECONDARY: u32 = 1 << 1;
const ANDROID_BUTTON_TERTIARY: u32 = 1 << 2;

pub struct PlatformMouseInjector {
    last_buttons: u32,
}

impl PlatformMouseInjector {
    pub fn open() -> Result<Self, InputError> {
        Ok(Self { last_buttons: 0 })
    }
}

impl MouseInjector for PlatformMouseInjector {
    fn inject_mouse(
        &mut self,
        input: &MouseInput,
        mapper: &CoordinateMapper,
    ) -> Result<MouseInjectionReport, InputError> {
        let mapped = mapper.map(input.x, input.y);
        unsafe {
            SetCursorPos(mapped.x.round() as i32, mapped.y.round() as i32)
                .map_err(|_| InputError::MouseInjectionFailed)?;
        }

        let mut injected = 1;
        let changed = self.last_buttons ^ input.button_flags;
        for (mask, down_flag, up_flag) in [
            (
                ANDROID_BUTTON_PRIMARY,
                MOUSEEVENTF_LEFTDOWN,
                MOUSEEVENTF_LEFTUP,
            ),
            (
                ANDROID_BUTTON_SECONDARY,
                MOUSEEVENTF_RIGHTDOWN,
                MOUSEEVENTF_RIGHTUP,
            ),
            (
                ANDROID_BUTTON_TERTIARY,
                MOUSEEVENTF_MIDDLEDOWN,
                MOUSEEVENTF_MIDDLEUP,
            ),
        ] {
            if changed & mask != 0 {
                let flag = if input.button_flags & mask != 0 {
                    down_flag
                } else {
                    up_flag
                };
                send_mouse_input(0, flag)?;
                injected += 1;
            }
        }
        self.last_buttons = input.button_flags;

        if input.wheel_delta_y != 0.0 {
            send_mouse_input(
                (input.wheel_delta_y * 120.0).round() as u32,
                MOUSEEVENTF_WHEEL,
            )?;
            injected += 1;
        }
        if input.wheel_delta_x != 0.0 {
            send_mouse_input(
                (input.wheel_delta_x * 120.0).round() as u32,
                MOUSEEVENTF_HWHEEL,
            )?;
            injected += 1;
        }

        Ok(MouseInjectionReport {
            injected_events: injected,
        })
    }
}

fn send_mouse_input(
    mouse_data: u32,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
) -> Result<(), InputError> {
    let input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: mouse_data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&[input], std::mem::size_of::<INPUT>() as i32) };
    if sent != 1 {
        return Err(InputError::MouseInjectionFailed);
    }
    Ok(())
}
