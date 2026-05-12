use super::{InjectionReport, InputError, KeyboardInjectionReport, KeyboardInjector, PenInjector};
use glyphray_core::{CoordinateMapper, PressureMapper};
use glyphray_protocol::{KeyboardInput, StylusInputBatch};

pub struct PlatformPenInjector;

impl PlatformPenInjector {
    pub fn open() -> Result<Self, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}

impl PenInjector for PlatformPenInjector {
    fn inject_batch(
        &mut self,
        _batch: &StylusInputBatch,
        _mapper: &CoordinateMapper,
        _pressure: &PressureMapper,
    ) -> Result<InjectionReport, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}

pub struct PlatformKeyboardInjector;

impl PlatformKeyboardInjector {
    pub fn open() -> Result<Self, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}

impl KeyboardInjector for PlatformKeyboardInjector {
    fn inject_key(
        &mut self,
        _input: &KeyboardInput,
    ) -> Result<KeyboardInjectionReport, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}
