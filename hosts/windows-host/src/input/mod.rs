use glyphray_core::{CoordinateMapper, PressureMapper};
use glyphray_protocol::{KeyboardInput, StylusAction, StylusInputBatch, StylusSample};

#[cfg(all(windows))]
mod win32_keyboard;
#[cfg(all(windows))]
mod win32_pen;

#[cfg(not(windows))]
mod stub;

#[cfg(not(windows))]
pub use stub::{PlatformKeyboardInjector, PlatformPenInjector};

#[cfg(all(windows))]
pub use win32_keyboard::PlatformKeyboardInjector;
#[cfg(all(windows))]
pub use win32_pen::PlatformPenInjector;

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

pub fn create_pen_injector() -> Result<Box<dyn PenInjector>, InputError> {
    Ok(Box::new(PlatformPenInjector::open()?))
}

pub fn create_keyboard_injector() -> Result<Box<dyn KeyboardInjector>, InputError> {
    Ok(Box::new(PlatformKeyboardInjector::open()?))
}

pub struct StylusInputBridge<I> {
    injector: I,
    mapper: CoordinateMapper,
    pressure: PressureMapper,
}

impl<I> StylusInputBridge<I>
where
    I: PenInjector,
{
    pub fn new(injector: I, mapper: CoordinateMapper, pressure: PressureMapper) -> Self {
        Self {
            injector,
            mapper,
            pressure,
        }
    }

    pub fn inject_remote_batch(
        &mut self,
        batch: &StylusInputBatch,
    ) -> Result<InjectionReport, InputError> {
        self.injector
            .inject_batch(batch, &self.mapper, &self.pressure)
    }
}

pub struct KeyboardInputBridge<I> {
    injector: I,
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
    use glyphray_protocol::{KeyboardInput, StylusAction, StylusToolType};

    #[derive(Default)]
    struct RecordingInjector {
        samples: usize,
    }

    impl PenInjector for RecordingInjector {
        fn inject_batch(
            &mut self,
            batch: &StylusInputBatch,
            _mapper: &CoordinateMapper,
            _pressure: &PressureMapper,
        ) -> Result<InjectionReport, InputError> {
            self.samples += batch.samples.len();
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
}
