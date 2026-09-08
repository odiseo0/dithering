use crate::components::raster::{LinearRgb, Rgb8};
use thiserror::Error;

pub const MIN_PALETTE_COLORS: usize = 2;
pub const MAX_PALETTE_COLORS: usize = 8;

/// A validated palette for error diffusion and Bayer dithering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Palette {
    colors: Vec<Rgb8>,
}

impl Palette {
    /// # Errors
    ///
    /// Returns [`PaletteError`] if the list does not contain 2 through 8 colors.
    pub fn new(colors: Vec<Rgb8>) -> Result<Self, PaletteError> {
        if colors.len() < MIN_PALETTE_COLORS {
            Err(PaletteError::TooFewColors {
                actual: colors.len(),
                minimum: MIN_PALETTE_COLORS,
            })
        } else if colors.len() > MAX_PALETTE_COLORS {
            Err(PaletteError::TooManyColors {
                actual: colors.len(),
                maximum: MAX_PALETTE_COLORS,
            })
        } else {
            Ok(Self { colors })
        }
    }

    #[must_use]
    pub fn black_and_white() -> Self {
        Self {
            colors: vec![Rgb8::BLACK, Rgb8::WHITE],
        }
    }

    #[must_use]
    pub fn colors(&self) -> &[Rgb8] {
        &self.colors
    }

    #[must_use]
    pub fn nearest(&self, target: LinearRgb) -> (usize, Rgb8) {
        let target = target.clamped();
        let mut best_index = 0;
        let mut best_distance = f32::INFINITY;

        for (index, color) in self.colors.iter().copied().enumerate() {
            let distance = target.distance_squared(color.to_linear());
            if distance < best_distance {
                best_index = index;
                best_distance = distance;
            }
        }

        (best_index, self.colors[best_index])
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self::black_and_white()
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum PaletteError {
    #[error("palette must have at least {minimum} colors, got {actual}")]
    TooFewColors { actual: usize, minimum: usize },
    #[error("palette must have at most {maximum} colors, got {actual}")]
    TooManyColors { actual: usize, maximum: usize },
}

#[cfg(test)]
mod tests {
    use super::{MAX_PALETTE_COLORS, Palette, PaletteError};
    use crate::components::raster::{LinearRgb, Rgb8};

    #[test]
    fn accepts_two_through_eight_colors_and_repeated_colors() {
        assert!(Palette::new(vec![Rgb8::BLACK; 2]).is_ok());
        assert!(Palette::new(vec![Rgb8::WHITE; MAX_PALETTE_COLORS]).is_ok());
    }

    #[test]
    fn rejects_palette_sizes_outside_the_range() {
        assert_eq!(
            Palette::new(vec![Rgb8::BLACK]),
            Err(PaletteError::TooFewColors {
                actual: 1,
                minimum: 2,
            })
        );
        assert!(Palette::new(vec![Rgb8::BLACK; MAX_PALETTE_COLORS + 1]).is_err());
    }

    #[test]
    fn nearest_color_uses_linear_distance() {
        let palette = Palette::new(vec![Rgb8::BLACK, Rgb8::WHITE]).expect("valid palette");

        assert_eq!(
            palette.nearest(Rgb8::new(128, 128, 128).to_linear()),
            (0, Rgb8::BLACK)
        );
    }

    #[test]
    fn a_tie_uses_the_lower_index() {
        let first = Rgb8::new(10, 20, 30);
        let palette = Palette::new(vec![first, first]).expect("valid palette");

        assert_eq!(palette.nearest(LinearRgb::new(0.5, 0.5, 0.5)).0, 0);
    }
}
