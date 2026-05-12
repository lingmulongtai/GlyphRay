use super::{
    InjectionReport, InputError, KeyboardInjectionReport, KeyboardInjector, MouseInjectionReport,
    MouseInjector, PenInjector, TouchInjectionReport, TouchInjector,
};
use glyphray_core::{CoordinateMapper, PressureMapper};
use glyphray_protocol::{KeyboardInput, MouseInput, StylusInputBatch, TouchInputBatch};

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

pub struct PlatformTouchInjector;

impl PlatformTouchInjector {
    pub fn open() -> Result<Self, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}

impl TouchInjector for PlatformTouchInjector {
    fn inject_touch_batch(
        &mut self,
        _batch: &TouchInputBatch,
        _mapper: &CoordinateMapper,
    ) -> Result<TouchInjectionReport, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}

pub struct PlatformMouseInjector;

impl PlatformMouseInjector {
    pub fn open() -> Result<Self, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}

impl MouseInjector for PlatformMouseInjector {
    fn inject_mouse(
        &mut self,
        _input: &MouseInput,
        _mapper: &CoordinateMapper,
    ) -> Result<MouseInjectionReport, InputError> {
        Err(InputError::UnsupportedPlatform)
    }
}
