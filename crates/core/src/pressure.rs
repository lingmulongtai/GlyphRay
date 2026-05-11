use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum PressureCurve {
    Linear,
    Soft,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PressureMapper {
    curve: PressureCurve,
    min_pressure: f32,
    max_pressure: f32,
}

impl PressureMapper {
    pub fn new(curve: PressureCurve, min_pressure: f32, max_pressure: f32) -> Self {
        let min_pressure = min_pressure.clamp(0.0, 1.0);
        let max_pressure = max_pressure.clamp(min_pressure.max(0.001), 1.0);
        Self {
            curve,
            min_pressure,
            max_pressure,
        }
    }

    pub fn curve(&self) -> PressureCurve {
        self.curve
    }

    pub fn normalize(&self, raw_pressure: f32) -> f32 {
        let normalized =
            ((raw_pressure - self.min_pressure) / (self.max_pressure - self.min_pressure))
                .clamp(0.0, 1.0);
        match self.curve {
            PressureCurve::Linear => normalized,
            PressureCurve::Soft => normalized.sqrt(),
            PressureCurve::Hard => normalized * normalized,
        }
    }

    pub fn to_windows_pressure(&self, raw_pressure: f32) -> u32 {
        const WINDOWS_PRESSURE_MAX: f32 = 1024.0;
        (self.normalize(raw_pressure) * WINDOWS_PRESSURE_MAX).round() as u32
    }
}

impl Default for PressureMapper {
    fn default() -> Self {
        Self::new(PressureCurve::Linear, 0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_pressure_maps_to_windows_range() {
        let mapper = PressureMapper::default();
        assert_eq!(mapper.to_windows_pressure(0.0), 0);
        assert_eq!(mapper.to_windows_pressure(0.5), 512);
        assert_eq!(mapper.to_windows_pressure(1.0), 1024);
    }

    #[test]
    fn hard_curve_reduces_light_pressure() {
        let mapper = PressureMapper::new(PressureCurve::Hard, 0.0, 1.0);
        assert_eq!(mapper.to_windows_pressure(0.5), 256);
    }

    #[test]
    fn soft_curve_boosts_light_pressure() {
        let mapper = PressureMapper::new(PressureCurve::Soft, 0.0, 1.0);
        assert!(mapper.to_windows_pressure(0.25) > 256);
    }

    #[test]
    fn pressure_deadzone_is_respected() {
        let mapper = PressureMapper::new(PressureCurve::Linear, 0.2, 0.8);
        assert_eq!(mapper.normalize(0.2), 0.0);
        assert!((mapper.normalize(0.5) - 0.5).abs() < 0.001);
        assert_eq!(mapper.normalize(0.8), 1.0);
    }
}

