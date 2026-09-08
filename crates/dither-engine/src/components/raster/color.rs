#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Rgb8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb8 {
    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255);

    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    #[must_use]
    pub fn to_linear(self) -> LinearRgb {
        LinearRgb {
            r: srgb_channel_to_linear(self.r),
            g: srgb_channel_to_linear(self.g),
            b: srgb_channel_to_linear(self.b),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearRgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl LinearRgb {
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    #[must_use]
    pub fn clamped(self) -> Self {
        Self {
            r: clamp_channel(self.r),
            g: clamp_channel(self.g),
            b: clamp_channel(self.b),
        }
    }

    #[must_use]
    pub fn to_srgb8(self) -> Rgb8 {
        let value = self.clamped();
        Rgb8 {
            r: linear_channel_to_srgb(value.r),
            g: linear_channel_to_srgb(value.g),
            b: linear_channel_to_srgb(value.b),
        }
    }

    #[must_use]
    pub fn luminance(self) -> f32 {
        let value = self.clamped();
        value
            .r
            .mul_add(0.2126, value.g.mul_add(0.7152, value.b * 0.0722))
    }

    #[must_use]
    pub fn distance_squared(self, other: Self) -> f32 {
        let red = self.r - other.r;
        let green = self.g - other.g;
        let blue = self.b - other.b;
        red.mul_add(red, green.mul_add(green, blue * blue))
    }
}

fn clamp_channel(value: f32) -> f32 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(0.0, 1.0)
    }
}

fn srgb_channel_to_linear(value: u8) -> f32 {
    let normalized = f32::from(value) / 255.0;

    if normalized <= 0.040_45 {
        normalized / 12.92
    } else {
        ((normalized + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_channel_to_srgb(value: f32) -> u8 {
    let normalized = if value <= 0.003_130_8 {
        value * 12.92
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = (normalized * 255.0).round() as u8;
    byte
}

#[cfg(test)]
mod tests {
    use super::{LinearRgb, Rgb8};

    const EPSILON: f32 = 0.000_01;

    #[test]
    fn converts_black_and_white_exactly() {
        assert_eq!(Rgb8::BLACK.to_linear(), LinearRgb::new(0.0, 0.0, 0.0));
        assert_eq!(Rgb8::WHITE.to_linear(), LinearRgb::new(1.0, 1.0, 1.0));
        assert_eq!(LinearRgb::new(0.0, 0.0, 0.0).to_srgb8(), Rgb8::BLACK);
        assert_eq!(LinearRgb::new(1.0, 1.0, 1.0).to_srgb8(), Rgb8::WHITE);
    }

    #[test]
    fn converts_a_middle_value_both_ways() {
        let linear = Rgb8::new(128, 128, 128).to_linear();
        assert!((linear.r - 0.215_861).abs() < EPSILON);
        assert_eq!(linear.to_srgb8(), Rgb8::new(128, 128, 128));
    }

    #[test]
    fn clamps_invalid_values_before_conversion() {
        assert_eq!(
            LinearRgb::new(f32::NAN, -1.0, 2.0).to_srgb8(),
            Rgb8::new(0, 0, 255)
        );
    }

    #[test]
    fn uses_rec_709_luminance() {
        let red = LinearRgb::new(1.0, 0.0, 0.0).luminance();
        assert!((red - 0.2126).abs() < EPSILON);
    }
}
