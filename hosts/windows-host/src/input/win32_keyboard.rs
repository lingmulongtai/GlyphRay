use super::{InputError, KeyboardInjectionReport, KeyboardInjector};
use glyphray_protocol::KeyboardInput;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
};

pub struct PlatformKeyboardInjector;

impl PlatformKeyboardInjector {
    pub fn open() -> Result<Self, InputError> {
        Ok(Self)
    }
}

impl KeyboardInjector for PlatformKeyboardInjector {
    fn inject_key(&mut self, input: &KeyboardInput) -> Result<KeyboardInjectionReport, InputError> {
        if input.virtual_key == 0 || input.virtual_key > u16::MAX as u32 {
            return Err(InputError::InvalidKeyboardInput);
        }

        let mut flags = Default::default();
        if !input.pressed {
            flags |= KEYEVENTF_KEYUP;
        }
        if is_extended_key(input.virtual_key) {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }

        let keyboard_input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(input.virtual_key as u16),
                    wScan: input.scan_code.min(u16::MAX as u32) as u16,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };

        let sent = unsafe { SendInput(&[keyboard_input], std::mem::size_of::<INPUT>() as i32) };
        if sent != 1 {
            return Err(InputError::KeyboardInjectionFailed);
        }

        Ok(KeyboardInjectionReport { injected_events: 1 })
    }
}

fn is_extended_key(virtual_key: u32) -> bool {
    matches!(
        virtual_key,
        0x21..=0x28 | // PageUp, PageDown, End, Home, arrows
        0x2C..=0x2E | // PrintScreen, Insert, Delete
        0x5B..=0x5D | // Windows keys and Apps
        0xA3 | 0xA5 // Right Ctrl, Right Alt
    )
}
