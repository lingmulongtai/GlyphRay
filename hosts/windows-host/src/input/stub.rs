use super::{InjectionReport, InputError, PenInjector};
use glyphray_core::{CoordinateMapper, PressureMapper};
use glyphray_protocol::StylusInputBatch;

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

