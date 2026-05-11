use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SourceRect {
    pub width: f32,
    pub height: f32,
}

impl SourceRect {
    pub fn new(width: f32, height: f32) -> Result<Self, MappingError> {
        if width <= 0.0 || height <= 0.0 {
            return Err(MappingError::InvalidDimensions);
        }
        Ok(Self { width, height })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DisplayRect {
    pub origin_x: f32,
    pub origin_y: f32,
    pub width: f32,
    pub height: f32,
    pub rotation_degrees: u16,
    pub scale_factor: f32,
}

impl DisplayRect {
    pub fn new(
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        rotation_degrees: u16,
        scale_factor: f32,
    ) -> Result<Self, MappingError> {
        if width <= 0.0 || height <= 0.0 || scale_factor <= 0.0 {
            return Err(MappingError::InvalidDimensions);
        }
        Ok(Self {
            origin_x,
            origin_y,
            width,
            height,
            rotation_degrees,
            scale_factor,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingMode {
    Fit,
    Fill,
    OneToOne,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MappedPoint {
    pub x: f32,
    pub y: f32,
    pub inside_active_area: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum MappingError {
    #[error("source and destination dimensions must be positive")]
    InvalidDimensions,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CoordinateMapper {
    source: SourceRect,
    display: DisplayRect,
    mode: MappingMode,
}

impl CoordinateMapper {
    pub fn new(source: SourceRect, display: DisplayRect, mode: MappingMode) -> Self {
        Self {
            source,
            display,
            mode,
        }
    }

    pub fn map(&self, source_x: f32, source_y: f32) -> MappedPoint {
        let clamped_x = source_x.clamp(0.0, self.source.width);
        let clamped_y = source_y.clamp(0.0, self.source.height);
        let inside_source = source_x == clamped_x && source_y == clamped_y;

        let normalized_x = clamped_x / self.source.width;
        let normalized_y = clamped_y / self.source.height;

        let (active_x, active_y, active_w, active_h, inside_active) = self.active_area();
        let mapped_x = active_x + normalized_x * active_w;
        let mapped_y = active_y + normalized_y * active_h;
        let (rotated_x, rotated_y) = self.apply_rotation(mapped_x, mapped_y);

        MappedPoint {
            x: self.display.origin_x + rotated_x,
            y: self.display.origin_y + rotated_y,
            inside_active_area: inside_source && inside_active,
        }
    }

    fn active_area(&self) -> (f32, f32, f32, f32, bool) {
        match self.mode {
            MappingMode::Stretch => (0.0, 0.0, self.display.width, self.display.height, true),
            MappingMode::OneToOne => {
                let width = self.source.width.min(self.display.width);
                let height = self.source.height.min(self.display.height);
                (
                    (self.display.width - width) * 0.5,
                    (self.display.height - height) * 0.5,
                    width,
                    height,
                    true,
                )
            }
            MappingMode::Fit | MappingMode::Fill => {
                let source_aspect = self.source.width / self.source.height;
                let display_aspect = self.display.width / self.display.height;
                let scale_by_width = source_aspect >= display_aspect;
                let use_width = match self.mode {
                    MappingMode::Fit => scale_by_width,
                    MappingMode::Fill => !scale_by_width,
                    _ => unreachable!(),
                };

                if use_width {
                    let height = self.display.width / source_aspect;
                    (
                        0.0,
                        (self.display.height - height) * 0.5,
                        self.display.width,
                        height,
                        self.mode == MappingMode::Fit,
                    )
                } else {
                    let width = self.display.height * source_aspect;
                    (
                        (self.display.width - width) * 0.5,
                        0.0,
                        width,
                        self.display.height,
                        self.mode == MappingMode::Fit,
                    )
                }
            }
        }
    }

    fn apply_rotation(&self, x: f32, y: f32) -> (f32, f32) {
        match self.display.rotation_degrees % 360 {
            90 => (self.display.height - y, x),
            180 => (self.display.width - x, self.display.height - y),
            270 => (y, self.display.width - x),
            _ => (x, y),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_fit_with_letterbox_offsets() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(1600.0, 1000.0).unwrap(),
            DisplayRect::new(0.0, 0.0, 1920.0, 1080.0, 0, 1.0).unwrap(),
            MappingMode::Fit,
        );

        let top_left = mapper.map(0.0, 0.0);
        assert_eq!(top_left.x, 0.0);
        assert!((top_left.y - 15.0).abs() < 0.01);

        let bottom_right = mapper.map(1600.0, 1000.0);
        assert_eq!(bottom_right.x, 1920.0);
        assert!((bottom_right.y - 1065.0).abs() < 0.01);
    }

    #[test]
    fn maps_one_to_one_centered() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(1000.0, 500.0).unwrap(),
            DisplayRect::new(100.0, 200.0, 2000.0, 1000.0, 0, 1.0).unwrap(),
            MappingMode::OneToOne,
        );

        let center = mapper.map(500.0, 250.0);
        assert!((center.x - 1100.0).abs() < 0.01);
        assert!((center.y - 700.0).abs() < 0.01);
    }

    #[test]
    fn applies_display_rotation() {
        let mapper = CoordinateMapper::new(
            SourceRect::new(100.0, 100.0).unwrap(),
            DisplayRect::new(0.0, 0.0, 100.0, 200.0, 90, 1.0).unwrap(),
            MappingMode::Stretch,
        );

        let point = mapper.map(0.0, 0.0);
        assert_eq!(point.x, 200.0);
        assert_eq!(point.y, 0.0);
    }
}

