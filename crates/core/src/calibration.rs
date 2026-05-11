use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CalibrationPoint {
    pub source_x: f32,
    pub source_y: f32,
    pub target_x: f32,
    pub target_y: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationProfile {
    pub display_id: u32,
    pub top_left: CalibrationPoint,
    pub bottom_right: CalibrationPoint,
}

impl CalibrationProfile {
    pub fn identity(display_id: u32, width: f32, height: f32) -> Self {
        Self {
            display_id,
            top_left: CalibrationPoint {
                source_x: 0.0,
                source_y: 0.0,
                target_x: 0.0,
                target_y: 0.0,
            },
            bottom_right: CalibrationPoint {
                source_x: width,
                source_y: height,
                target_x: width,
                target_y: height,
            },
        }
    }

    pub fn apply(&self, source_x: f32, source_y: f32) -> (f32, f32) {
        let source_w = (self.bottom_right.source_x - self.top_left.source_x).max(0.001);
        let source_h = (self.bottom_right.source_y - self.top_left.source_y).max(0.001);
        let target_w = self.bottom_right.target_x - self.top_left.target_x;
        let target_h = self.bottom_right.target_y - self.top_left.target_y;
        let nx = ((source_x - self.top_left.source_x) / source_w).clamp(0.0, 1.0);
        let ny = ((source_y - self.top_left.source_y) / source_h).clamp(0.0, 1.0);

        (
            self.top_left.target_x + nx * target_w,
            self.top_left.target_y + ny * target_h,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_profile_maps_into_calibrated_area() {
        let profile = CalibrationProfile {
            display_id: 1,
            top_left: CalibrationPoint {
                source_x: 10.0,
                source_y: 10.0,
                target_x: 100.0,
                target_y: 200.0,
            },
            bottom_right: CalibrationPoint {
                source_x: 110.0,
                source_y: 210.0,
                target_x: 500.0,
                target_y: 1_000.0,
            },
        };

        let (x, y) = profile.apply(60.0, 110.0);
        assert_eq!(x, 300.0);
        assert_eq!(y, 600.0);
    }
}

