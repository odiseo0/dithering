use super::Palette;
use crate::components::raster::{Rgb8, Scale};
use thiserror::Error;

pub const MIN_HALFTONE_CELL_SIZE: u8 = 2;
pub const MAX_HALFTONE_CELL_SIZE: u8 = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EffectKind {
    ErrorDiffusion,
    Ordered,
    Halftone,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorDiffusionAlgorithm {
    FloydSteinberg,
    Atkinson,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BayerMatrix {
    Two,
    Four,
    Eight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DotShape {
    Circle,
    Square,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ErrorDiffusionConfig {
    algorithm: ErrorDiffusionAlgorithm,
    scale: Scale,
    palette: Palette,
}

impl ErrorDiffusionConfig {
    #[must_use]
    pub const fn new(algorithm: ErrorDiffusionAlgorithm, scale: Scale, palette: Palette) -> Self {
        Self {
            algorithm,
            scale,
            palette,
        }
    }

    #[must_use]
    pub const fn algorithm(&self) -> ErrorDiffusionAlgorithm {
        self.algorithm
    }

    #[must_use]
    pub const fn scale(&self) -> Scale {
        self.scale
    }

    #[must_use]
    pub const fn palette(&self) -> &Palette {
        &self.palette
    }
}

impl Default for ErrorDiffusionConfig {
    fn default() -> Self {
        Self::new(
            ErrorDiffusionAlgorithm::FloydSteinberg,
            Scale::MIN,
            Palette::default(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OrderedConfig {
    matrix: BayerMatrix,
    scale: Scale,
    palette: Palette,
}

impl OrderedConfig {
    #[must_use]
    pub const fn new(matrix: BayerMatrix, scale: Scale, palette: Palette) -> Self {
        Self {
            matrix,
            scale,
            palette,
        }
    }

    #[must_use]
    pub const fn matrix(&self) -> BayerMatrix {
        self.matrix
    }

    #[must_use]
    pub const fn scale(&self) -> Scale {
        self.scale
    }

    #[must_use]
    pub const fn palette(&self) -> &Palette {
        &self.palette
    }
}

impl Default for OrderedConfig {
    fn default() -> Self {
        Self::new(BayerMatrix::Four, Scale::MIN, Palette::default())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HalftoneConfig {
    background: Rgb8,
    dot: Rgb8,
    shape: DotShape,
    cell_size: u8,
    max_size: u8,
    inverted: bool,
}

impl HalftoneConfig {
    /// # Errors
    ///
    /// Returns [`ConfigError`] if the cell size or maximum dot size is outside its range.
    pub fn new(
        background: Rgb8,
        dot: Rgb8,
        shape: DotShape,
        cell_size: u8,
        max_size: u8,
        inverted: bool,
    ) -> Result<Self, ConfigError> {
        if !(MIN_HALFTONE_CELL_SIZE..=MAX_HALFTONE_CELL_SIZE).contains(&cell_size) {
            return Err(ConfigError::InvalidCellSize { value: cell_size });
        }

        if max_size == 0 || max_size > cell_size {
            return Err(ConfigError::InvalidMaximumDotSize {
                value: max_size,
                cell_size,
            });
        }

        Ok(Self {
            background,
            dot,
            shape,
            cell_size,
            max_size,
            inverted,
        })
    }

    #[must_use]
    pub const fn background(self) -> Rgb8 {
        self.background
    }

    #[must_use]
    pub const fn dot(self) -> Rgb8 {
        self.dot
    }

    #[must_use]
    pub const fn shape(self) -> DotShape {
        self.shape
    }

    #[must_use]
    pub const fn cell_size(self) -> u8 {
        self.cell_size
    }

    #[must_use]
    pub const fn max_size(self) -> u8 {
        self.max_size
    }

    #[must_use]
    pub const fn inverted(self) -> bool {
        self.inverted
    }
}

impl Default for HalftoneConfig {
    fn default() -> Self {
        Self {
            background: Rgb8::WHITE,
            dot: Rgb8::BLACK,
            shape: DotShape::Circle,
            cell_size: 8,
            max_size: 8,
            inverted: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EffectConfig {
    ErrorDiffusion(ErrorDiffusionConfig),
    Ordered(OrderedConfig),
    Halftone(HalftoneConfig),
}

impl EffectConfig {
    #[must_use]
    pub const fn kind(&self) -> EffectKind {
        match self {
            Self::ErrorDiffusion(_) => EffectKind::ErrorDiffusion,
            Self::Ordered(_) => EffectKind::Ordered,
            Self::Halftone(_) => EffectKind::Halftone,
        }
    }
}

impl Default for EffectConfig {
    fn default() -> Self {
        Self::ErrorDiffusion(ErrorDiffusionConfig::default())
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ConfigError {
    #[error("halftone cell size must be from 2 through 128, got {value}")]
    InvalidCellSize { value: u8 },
    #[error("maximum dot size must be from 1 through the cell size")]
    InvalidMaximumDotSize { value: u8, cell_size: u8 },
}

#[cfg(test)]
mod tests {
    use super::{
        ConfigError, DotShape, EffectConfig, ErrorDiffusionAlgorithm, HalftoneConfig,
        MAX_HALFTONE_CELL_SIZE,
    };
    use crate::components::raster::Rgb8;

    #[test]
    fn default_effect_matches_the_product_contract() {
        let EffectConfig::ErrorDiffusion(config) = EffectConfig::default() else {
            panic!("default effect must be error diffusion");
        };

        assert_eq!(config.algorithm(), ErrorDiffusionAlgorithm::FloydSteinberg);
        assert_eq!(config.scale().get(), 1);
        assert_eq!(config.palette().colors(), &[Rgb8::BLACK, Rgb8::WHITE]);
    }

    #[test]
    fn validates_halftone_cell_size() {
        assert_eq!(
            HalftoneConfig::new(Rgb8::WHITE, Rgb8::BLACK, DotShape::Circle, 1, 1, false),
            Err(ConfigError::InvalidCellSize { value: 1 })
        );
        assert!(
            HalftoneConfig::new(
                Rgb8::WHITE,
                Rgb8::BLACK,
                DotShape::Circle,
                MAX_HALFTONE_CELL_SIZE,
                MAX_HALFTONE_CELL_SIZE,
                false,
            )
            .is_ok()
        );
    }

    #[test]
    fn validates_halftone_maximum_dot_size() {
        assert_eq!(
            HalftoneConfig::new(Rgb8::WHITE, Rgb8::BLACK, DotShape::Square, 8, 9, false),
            Err(ConfigError::InvalidMaximumDotSize {
                value: 9,
                cell_size: 8,
            })
        );
    }
}
