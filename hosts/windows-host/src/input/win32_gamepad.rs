use super::{GamepadInjectionReport, GamepadInjector, InputError};
use glyphray_protocol::GamepadInput;
use std::collections::HashMap;

pub struct PlatformGamepadInjector {
    backend: VirtualGamepadBackend,
}

impl PlatformGamepadInjector {
    pub fn open() -> Result<Self, InputError> {
        match std::env::var("GLYPHRAY_GAMEPAD_BACKEND") {
            Ok(value) if value.eq_ignore_ascii_case("vigem") => Ok(Self {
                backend: VirtualGamepadBackend::Vigem(VigemGamepadBackend::open()?),
            }),
            Ok(value) if value.eq_ignore_ascii_case("virtual-hid") => {
                Err(InputError::GamepadBackendUnavailable(
                    "virtual-hid backend is planned but not implemented yet",
                ))
            }
            Ok(value) if value.eq_ignore_ascii_case("none") || value.eq_ignore_ascii_case("off") => {
                Err(InputError::GamepadBackendUnavailable(
                    "gamepad backend was explicitly disabled",
                ))
            }
            Ok(_) => Err(InputError::GamepadBackendUnavailable(
                "unknown GLYPHRAY_GAMEPAD_BACKEND; expected vigem, virtual-hid, none, or unset",
            )),
            Err(_) => Err(InputError::GamepadBackendUnavailable(
                "set GLYPHRAY_GAMEPAD_BACKEND=vigem after installing a compatible virtual controller driver",
            )),
        }
    }
}

impl GamepadInjector for PlatformGamepadInjector {
    fn inject_gamepad(
        &mut self,
        input: &GamepadInput,
    ) -> Result<GamepadInjectionReport, InputError> {
        self.backend.inject(input)
    }
}

enum VirtualGamepadBackend {
    Vigem(VigemGamepadBackend),
}

impl VirtualGamepadBackend {
    fn inject(&mut self, input: &GamepadInput) -> Result<GamepadInjectionReport, InputError> {
        match self {
            Self::Vigem(backend) => backend.inject(input),
        }
    }
}

struct VigemGamepadBackend {
    controllers: HashMap<u32, NormalizedXInputReport>,
}

impl VigemGamepadBackend {
    fn open() -> Result<Self, InputError> {
        Err(InputError::GamepadBackendUnavailable(
            "ViGEm client bindings are not linked yet; install the driver and keep GLYPHRAY_GAMEPAD_BACKEND unset until the native binding lands",
        ))
    }

    #[allow(dead_code)]
    fn inject(&mut self, input: &GamepadInput) -> Result<GamepadInjectionReport, InputError> {
        let report = NormalizedXInputReport::from_input(input);
        if input.connected {
            self.controllers.insert(input.controller_id, report);
            Ok(GamepadInjectionReport {
                updated_controllers: 1,
                disconnected_controllers: 0,
            })
        } else {
            self.controllers.remove(&input.controller_id);
            Ok(GamepadInjectionReport {
                updated_controllers: 0,
                disconnected_controllers: 1,
            })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NormalizedXInputReport {
    buttons: u16,
    left_trigger: u8,
    right_trigger: u8,
    left_thumb_x: i16,
    left_thumb_y: i16,
    right_thumb_x: i16,
    right_thumb_y: i16,
}

impl NormalizedXInputReport {
    fn from_input(input: &GamepadInput) -> Self {
        Self {
            buttons: input.buttons as u16,
            left_trigger: trigger_to_u8(input.left_trigger),
            right_trigger: trigger_to_u8(input.right_trigger),
            left_thumb_x: axis_to_i16(input.left_stick_x),
            left_thumb_y: axis_to_i16(-input.left_stick_y),
            right_thumb_x: axis_to_i16(input.right_stick_x),
            right_thumb_y: axis_to_i16(-input.right_stick_y),
        }
    }
}

fn trigger_to_u8(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * u8::MAX as f32).round() as u8
}

fn axis_to_i16(value: f32) -> i16 {
    let clamped = value.clamp(-1.0, 1.0);
    if clamped >= 0.0 {
        (clamped * i16::MAX as f32).round() as i16
    } else {
        (clamped * 32768.0).round() as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xinput_report_clamps_and_flips_vertical_axes() {
        let input = GamepadInput {
            sequence: 1,
            timestamp_us: 2,
            controller_id: 3,
            connected: true,
            buttons: 0xffff_1234,
            left_trigger: 1.5,
            right_trigger: -0.5,
            left_stick_x: 2.0,
            left_stick_y: 0.5,
            right_stick_x: -2.0,
            right_stick_y: -0.25,
        };

        let report = NormalizedXInputReport::from_input(&input);

        assert_eq!(report.buttons, 0x1234);
        assert_eq!(report.left_trigger, 255);
        assert_eq!(report.right_trigger, 0);
        assert_eq!(report.left_thumb_x, i16::MAX);
        assert_eq!(report.left_thumb_y, -16_384);
        assert_eq!(report.right_thumb_x, i16::MIN);
        assert_eq!(report.right_thumb_y, 8_192);
    }
}
