use glyphray_core::{CoordinateMapper, PressureMapper};
use glyphray_protocol::{
    GamepadInput, KeyboardInput, MouseInput, StylusAction, StylusInputBatch, StylusSample,
    TouchInputBatch,
};
use std::collections::HashMap;

#[cfg(windows)]
mod win32_gamepad;
#[cfg(windows)]
mod win32_keyboard;
#[cfg(windows)]
mod win32_mouse;
#[cfg(windows)]
mod win32_pen;
#[cfg(windows)]
mod win32_touch;

#[cfg(not(windows))]
mod stub;

#[cfg(not(windows))]
pub use stub::{
    PlatformGamepadInjector, PlatformKeyboardInjector, PlatformMouseInjector, PlatformPenInjector,
    PlatformTouchInjector,
};

#[cfg(windows)]
pub use win32_gamepad::PlatformGamepadInjector;
#[cfg(windows)]
pub use win32_keyboard::PlatformKeyboardInjector;
#[cfg(windows)]
pub use win32_mouse::PlatformMouseInjector;
#[cfg(windows)]
pub use win32_pen::PlatformPenInjector;
#[cfg(windows)]
pub use win32_touch::PlatformTouchInjector;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("native pen injection is not supported on this platform")]
    UnsupportedPlatform,
    #[error("failed to create a synthetic pen device")]
    DeviceCreationFailed,
    #[error("failed to inject synthetic pen event")]
    InjectionFailed,
    #[error("keyboard input packet did not contain a valid virtual key")]
    InvalidKeyboardInput,
    #[error("failed to inject keyboard input event")]
    KeyboardInjectionFailed,
    #[error("failed to inject native touch input event")]
    TouchInjectionFailed,
    #[error("failed to inject native mouse input event")]
    MouseInjectionFailed,
    #[error("virtual gamepad backend is unavailable: {0}")]
    GamepadBackendUnavailable(&'static str),
    #[error("failed to inject virtual gamepad input event")]
    GamepadInjectionFailed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InjectionReport {
    pub injected_samples: usize,
    pub used_pen_path: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardInjectionReport {
    pub injected_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchInjectionReport {
    pub injected_samples: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseInjectionReport {
    pub injected_events: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GamepadInjectionReport {
    pub updated_controllers: usize,
    pub disconnected_controllers: usize,
}

pub trait PenInjector {
    fn inject_batch(
        &mut self,
        batch: &StylusInputBatch,
        mapper: &CoordinateMapper,
        pressure: &PressureMapper,
    ) -> Result<InjectionReport, InputError>;
}

impl<T> PenInjector for Box<T>
where
    T: PenInjector + ?Sized,
{
    fn inject_batch(
        &mut self,
        batch: &StylusInputBatch,
        mapper: &CoordinateMapper,
        pressure: &PressureMapper,
    ) -> Result<InjectionReport, InputError> {
        self.as_mut().inject_batch(batch, mapper, pressure)
    }
}

pub trait KeyboardInjector {
    fn inject_key(&mut self, input: &KeyboardInput) -> Result<KeyboardInjectionReport, InputError>;
}

impl<T> KeyboardInjector for Box<T>
where
    T: KeyboardInjector + ?Sized,
{
    fn inject_key(&mut self, input: &KeyboardInput) -> Result<KeyboardInjectionReport, InputError> {
        self.as_mut().inject_key(input)
    }
}

pub trait TouchInjector {
    fn inject_touch_batch(
        &mut self,
        batch: &TouchInputBatch,
        mapper: &CoordinateMapper,
    ) -> Result<TouchInjectionReport, InputError>;
}

impl<T> TouchInjector for Box<T>
where
    T: TouchInjector + ?Sized,
{
    fn inject_touch_batch(
        &mut self,
        batch: &TouchInputBatch,
        mapper: &CoordinateMapper,
    ) -> Result<TouchInjectionReport, InputError> {
        self.as_mut().inject_touch_batch(batch, mapper)
    }
}

pub trait MouseInjector {
    fn inject_mouse(
        &mut self,
        input: &MouseInput,
        mapper: &CoordinateMapper,
    ) -> Result<MouseInjectionReport, InputError>;
}

pub trait GamepadInjector {
    fn inject_gamepad(
        &mut self,
        input: &GamepadInput,
    ) -> Result<GamepadInjectionReport, InputError>;
}

impl<T> GamepadInjector for Box<T>
where
    T: GamepadInjector + ?Sized,
{
    fn inject_gamepad(
        &mut self,
        input: &GamepadInput,
    ) -> Result<GamepadInjectionReport, InputError> {
        self.as_mut().inject_gamepad(input)
    }
}

impl<T> MouseInjector for Box<T>
where
    T: MouseInjector + ?Sized,
{
    fn inject_mouse(
        &mut self,
        input: &MouseInput,
        mapper: &CoordinateMapper,
    ) -> Result<MouseInjectionReport, InputError> {
        self.as_mut().inject_mouse(input, mapper)
    }
}

pub fn create_pen_injector() -> Result<Box<dyn PenInjector>, InputError> {
    Ok(Box::new(PlatformPenInjector::open()?))
}

pub fn create_keyboard_injector() -> Result<Box<dyn KeyboardInjector>, InputError> {
    Ok(Box::new(PlatformKeyboardInjector::open()?))
}

pub fn create_touch_injector() -> Result<Box<dyn TouchInjector>, InputError> {
    Ok(Box::new(PlatformTouchInjector::open()?))
}

pub fn create_mouse_injector() -> Result<Box<dyn MouseInjector>, InputError> {
    Ok(Box::new(PlatformMouseInjector::open()?))
}

pub fn create_gamepad_injector() -> Result<Box<dyn GamepadInjector>, InputError> {
    Ok(Box::new(PlatformGamepadInjector::open()?))
}

pub struct StylusInputBridge<I> {
    injector: I,
    mapper: CoordinateMapper,
    pressure: PressureMapper,
    smoothing: StylusSmoothingConfig,
    pressure_state: HashMap<u32, f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylusSmoothingConfig {
    pub pressure_alpha: f32,
}

impl StylusSmoothingConfig {
    pub fn disabled() -> Self {
        Self {
            pressure_alpha: 1.0,
        }
    }
}

impl Default for StylusSmoothingConfig {
    fn default() -> Self {
        Self {
            pressure_alpha: 0.45,
        }
    }
}

impl<I> StylusInputBridge<I>
where
    I: PenInjector,
{
    pub fn new(injector: I, mapper: CoordinateMapper, pressure: PressureMapper) -> Self {
        Self::with_smoothing(injector, mapper, pressure, StylusSmoothingConfig::default())
    }

    pub fn with_smoothing(
        injector: I,
        mapper: CoordinateMapper,
        pressure: PressureMapper,
        smoothing: StylusSmoothingConfig,
    ) -> Self {
        Self {
            injector,
            mapper,
            pressure,
            smoothing,
            pressure_state: HashMap::new(),
        }
    }

    pub fn inject_remote_batch(
        &mut self,
        batch: &StylusInputBatch,
    ) -> Result<InjectionReport, InputError> {
        let prepared = self.prepare_batch(batch);
        self.injector
            .inject_batch(&prepared, &self.mapper, &self.pressure)
    }

    fn prepare_batch(&mut self, batch: &StylusInputBatch) -> StylusInputBatch {
        let mut prepared = batch.clone();
        for sample in &mut prepared.samples {
            sample.pressure = self.smoothed_pressure(sample);
            sample.tilt_x_degrees = sample.tilt_x_degrees.clamp(-90.0, 90.0);
            sample.tilt_y_degrees = sample.tilt_y_degrees.clamp(-90.0, 90.0);
            sample.orientation_degrees = sample.orientation_degrees.rem_euclid(360.0);
        }
        prepared
    }

    fn smoothed_pressure(&mut self, sample: &StylusSample) -> f32 {
        let raw = sample.pressure.clamp(0.0, 1.0);
        if !map_action_to_contact(sample.action, sample) {
            self.pressure_state.remove(&sample.pointer_id);
            return raw;
        }

        let alpha = self.smoothing.pressure_alpha.clamp(0.0, 1.0);
        let previous = self.pressure_state.entry(sample.pointer_id).or_insert(raw);
        let smoothed = *previous + (raw - *previous) * alpha;
        *previous = smoothed;
        smoothed
    }
}

pub struct KeyboardInputBridge<I> {
    injector: I,
}

pub struct TouchInputBridge<I> {
    injector: I,
    mapper: CoordinateMapper,
}

impl<I> TouchInputBridge<I>
where
    I: TouchInjector,
{
    pub fn new(injector: I, mapper: CoordinateMapper) -> Self {
        Self { injector, mapper }
    }

    pub fn inject_remote_touch_batch(
        &mut self,
        batch: &TouchInputBatch,
    ) -> Result<TouchInjectionReport, InputError> {
        self.injector.inject_touch_batch(batch, &self.mapper)
    }
}

pub struct MouseInputBridge<I> {
    injector: I,
    mapper: CoordinateMapper,
}

pub struct GamepadInputBridge<I> {
    injector: I,
}

impl<I> MouseInputBridge<I>
where
    I: MouseInjector,
{
    pub fn new(injector: I, mapper: CoordinateMapper) -> Self {
        Self { injector, mapper }
    }

    pub fn inject_remote_mouse(
        &mut self,
        input: &MouseInput,
    ) -> Result<MouseInjectionReport, InputError> {
        self.injector.inject_mouse(input, &self.mapper)
    }
}

impl<I> KeyboardInputBridge<I>
where
    I: KeyboardInjector,
{
    pub fn new(injector: I) -> Self {
        Self { injector }
    }

    pub fn inject_remote_key(
        &mut self,
        input: &KeyboardInput,
    ) -> Result<KeyboardInjectionReport, InputError> {
        self.injector.inject_key(input)
    }
}

impl<I> GamepadInputBridge<I>
where
    I: GamepadInjector,
{
    pub fn new(injector: I) -> Self {
        Self { injector }
    }

    pub fn inject_remote_gamepad(
        &mut self,
        input: &GamepadInput,
    ) -> Result<GamepadInjectionReport, InputError> {
        let prepared = normalize_gamepad_input(input);
        self.injector.inject_gamepad(&prepared)
    }
}

fn normalize_gamepad_input(input: &GamepadInput) -> GamepadInput {
    GamepadInput {
        sequence: input.sequence,
        timestamp_us: input.timestamp_us,
        controller_id: input.controller_id,
        connected: input.connected,
        buttons: input.buttons,
        left_trigger: input.left_trigger.clamp(0.0, 1.0),
        right_trigger: input.right_trigger.clamp(0.0, 1.0),
        left_stick_x: input.left_stick_x.clamp(-1.0, 1.0),
        left_stick_y: input.left_stick_y.clamp(-1.0, 1.0),
        right_stick_x: input.right_stick_x.clamp(-1.0, 1.0),
        right_stick_y: input.right_stick_y.clamp(-1.0, 1.0),
    }
}

pub(crate) fn map_action_to_contact(action: StylusAction, sample: &StylusSample) -> bool {
    matches!(
        action,
        StylusAction::Down | StylusAction::Move | StylusAction::Up
    ) && !sample.hover
}

#[cfg(test)]
mod tests {
    use super::*;
    use glyphray_core::{DisplayRect, MappingMode, SourceRect};
    use glyphray_protocol::{
        GamepadInput, KeyboardInput, MouseInput, StylusAction, StylusToolType, TouchAction,
        TouchInputBatch, TouchSample,
    };

    #[derive(Default)]
    struct RecordingInjector {
        samples: usize,
        observed_pressures: Vec<f32>,
        observed_tilt_x: Vec<f32>,
        observed_orientation: Vec<f32>,
    }

    impl PenInjector for RecordingInjector {
        fn inject_batch(
            &mut self,
            batch: &StylusInputBatch,
            _mapper: &CoordinateMapper,
            _pressure: &PressureMapper,
        ) -> Result<InjectionReport, InputError> {
            self.samples += batch.samples.len();
            self.observed_pressures
                .extend(batch.samples.iter().map(|sample| sample.pressure));
            self.observed_tilt_x
                .extend(batch.samples.iter().map(|sample| sample.tilt_x_degrees));
            self.observed_orientation.extend(
                batch
                    .samples
                    .iter()
                    .map(|sample| sample.orientation_degrees),
            );
            Ok(InjectionReport {
                injected_samples: batch.samples.len(),
                used_pen_path: true,
            })
        }
    }

    #[derive(Default)]
    struct RecordingKeyboardInjector {
        events: usize,
    }

    impl KeyboardInjector for RecordingKeyboardInjector {
        fn inject_key(
            &mut self,
            _input: &KeyboardInput,
        ) -> Result<KeyboardInjectionReport, InputError> {
            self.events += 1;
            Ok(KeyboardInjectionReport { injected_events: 1 })
        }
    }

    #[derive(Default)]
    struct RecordingTouchInjector {
        samples: usize,
    }

    impl TouchInjector for RecordingTouchInjector {
        fn inject_touch_batch(
            &mut self,
            batch: &TouchInputBatch,
            _mapper: &CoordinateMapper,
        ) -> Result<TouchInjectionReport, InputError> {
            self.samples += batch.samples.len();
            Ok(TouchInjectionReport {
                injected_samples: batch.samples.len(),
            })
        }
    }

    #[derive(Default)]
    struct RecordingMouseInjector {
        events: usize,
    }

    impl MouseInjector for RecordingMouseInjector {
        fn inject_mouse(
            &mut self,
            _input: &MouseInput,
            _mapper: &CoordinateMapper,
        ) -> Result<MouseInjectionReport, InputError> {
            self.events += 1;
            Ok(MouseInjectionReport { injected_events: 1 })
        }
    }

    #[derive(Default)]
    struct RecordingGamepadInjector {
        reports: Vec<GamepadInput>,
    }

    impl GamepadInjector for RecordingGamepadInjector {
        fn inject_gamepad(
            &mut self,
            input: &GamepadInput,
        ) -> Result<GamepadInjectionReport, InputError> {
            self.reports.push(input.clone());
            Ok(GamepadInjectionReport {
                updated_controllers: usize::from(input.connected),
                disconnected_controllers: usize::from(!input.connected),
            })
        }
    }

    #[test]
    fn stylus_bridge_forwards_remote_batches_to_injector() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(100.0, 100.0).expect("source"),
            DisplayRect::new(0.0, 0.0, 100.0, 100.0, 0, 1.0).expect("display"),
            MappingMode::Stretch,
        );
        let injector = RecordingInjector::default();
        let mut bridge = StylusInputBridge::new(injector, mapper, PressureMapper::default());
        let batch = StylusInputBatch {
            batch_sequence: 1,
            monotonic_timestamp_us: 1,
            samples: vec![StylusSample {
                sequence: 1,
                timestamp_us: 1,
                display_id: 0,
                pointer_id: 1,
                tool_type: StylusToolType::Stylus,
                action: StylusAction::Down,
                x: 50.0,
                y: 50.0,
                pressure: 0.5,
                tilt_x_degrees: 0.0,
                tilt_y_degrees: 0.0,
                orientation_degrees: 0.0,
                button_flags: 0,
                hover: false,
                eraser: false,
                predicted: false,
            }],
        };

        let report = bridge.inject_remote_batch(&batch).expect("inject");
        assert_eq!(report.injected_samples, 1);
        assert!(report.used_pen_path);
    }

    #[test]
    fn stylus_bridge_smooths_pressure_and_clamps_pen_axes() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(100.0, 100.0).expect("source"),
            DisplayRect::new(0.0, 0.0, 100.0, 100.0, 0, 1.0).expect("display"),
            MappingMode::Stretch,
        );
        let injector = RecordingInjector::default();
        let mut bridge = StylusInputBridge::with_smoothing(
            injector,
            mapper,
            PressureMapper::default(),
            StylusSmoothingConfig {
                pressure_alpha: 0.5,
            },
        );
        let batch = StylusInputBatch {
            batch_sequence: 1,
            monotonic_timestamp_us: 1,
            samples: vec![
                stylus_sample(1, StylusAction::Down, 0.0, 120.0, -45.0),
                stylus_sample(2, StylusAction::Move, 1.0, 30.0, 725.0),
            ],
        };

        bridge.inject_remote_batch(&batch).expect("inject");

        assert_eq!(bridge.injector.observed_pressures, vec![0.0, 0.5]);
        assert_eq!(bridge.injector.observed_tilt_x, vec![90.0, 30.0]);
        assert_eq!(bridge.injector.observed_orientation, vec![315.0, 5.0]);
    }

    #[test]
    fn keyboard_bridge_forwards_remote_keys_to_injector() {
        let injector = RecordingKeyboardInjector::default();
        let mut bridge = KeyboardInputBridge::new(injector);
        let input = KeyboardInput {
            sequence: 1,
            timestamp_us: 10,
            scan_code: 0,
            virtual_key: 0x5B,
            pressed: true,
            modifiers: 0,
        };

        let report = bridge.inject_remote_key(&input).expect("inject");

        assert_eq!(report.injected_events, 1);
    }

    #[test]
    fn touch_bridge_forwards_remote_touch_batches_to_injector() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(100.0, 100.0).expect("source"),
            DisplayRect::new(0.0, 0.0, 100.0, 100.0, 0, 1.0).expect("display"),
            MappingMode::Stretch,
        );
        let mut bridge = TouchInputBridge::new(RecordingTouchInjector::default(), mapper);
        let batch = TouchInputBatch {
            batch_sequence: 1,
            monotonic_timestamp_us: 1,
            display_id: 0,
            samples: vec![TouchSample {
                sequence: 1,
                timestamp_us: 1,
                pointer_id: 1,
                action: TouchAction::Down,
                x: 10.0,
                y: 20.0,
                pressure: 0.5,
                major: 8.0,
                minor: 8.0,
                orientation_degrees: 0.0,
                flags: 0,
            }],
        };

        let report = bridge.inject_remote_touch_batch(&batch).expect("inject");

        assert_eq!(report.injected_samples, 1);
    }

    #[test]
    fn mouse_bridge_forwards_remote_mouse_to_injector() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(100.0, 100.0).expect("source"),
            DisplayRect::new(0.0, 0.0, 100.0, 100.0, 0, 1.0).expect("display"),
            MappingMode::Stretch,
        );
        let mut bridge = MouseInputBridge::new(RecordingMouseInjector::default(), mapper);
        let input = MouseInput {
            sequence: 1,
            timestamp_us: 1,
            display_id: 0,
            x: 10.0,
            y: 20.0,
            wheel_delta_x: 0.0,
            wheel_delta_y: 0.0,
            button_flags: 1,
        };

        let report = bridge.inject_remote_mouse(&input).expect("inject");

        assert_eq!(report.injected_events, 1);
    }

    #[test]
    fn gamepad_bridge_normalizes_axes_before_injection() {
        let mut bridge = GamepadInputBridge::new(RecordingGamepadInjector::default());
        let input = GamepadInput {
            sequence: 1,
            timestamp_us: 1,
            controller_id: 9,
            connected: true,
            buttons: 1,
            left_trigger: -1.0,
            right_trigger: 2.0,
            left_stick_x: -2.0,
            left_stick_y: 2.0,
            right_stick_x: 0.25,
            right_stick_y: -0.25,
        };

        let report = bridge.inject_remote_gamepad(&input).expect("inject");

        assert_eq!(report.updated_controllers, 1);
        assert_eq!(bridge.injector.reports[0].left_trigger, 0.0);
        assert_eq!(bridge.injector.reports[0].right_trigger, 1.0);
        assert_eq!(bridge.injector.reports[0].left_stick_x, -1.0);
        assert_eq!(bridge.injector.reports[0].left_stick_y, 1.0);
    }

    fn stylus_sample(
        sequence: u64,
        action: StylusAction,
        pressure: f32,
        tilt_x_degrees: f32,
        orientation_degrees: f32,
    ) -> StylusSample {
        StylusSample {
            sequence,
            timestamp_us: sequence,
            display_id: 0,
            pointer_id: 1,
            tool_type: StylusToolType::Stylus,
            action,
            x: 50.0,
            y: 50.0,
            pressure,
            tilt_x_degrees,
            tilt_y_degrees: 0.0,
            orientation_degrees,
            button_flags: 0,
            hover: false,
            eraser: false,
            predicted: false,
        }
    }
}
