use dither_engine::{
    BayerMatrix, DotShape, EffectConfig, ErrorDiffusionAlgorithm, ErrorDiffusionConfig,
    HalftoneConfig, OrderedConfig, Palette, Rgb8, Scale,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveEffect {
    ErrorDiffusion,
    Ordered,
    Halftone,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Settings {
    pub active_effect: ActiveEffect,
    pub scale: u8,
    pub palette: Vec<Rgb8>,
    pub diffusion_algorithm: ErrorDiffusionAlgorithm,
    pub bayer_matrix: BayerMatrix,
    pub halftone_background: Rgb8,
    pub halftone_dot: Rgb8,
    pub halftone_shape: DotShape,
    pub halftone_cell_size: u8,
    pub halftone_max_size: u8,
    pub halftone_inverted: bool,
}

impl Settings {
    #[must_use]
    pub fn effect_config(&self) -> EffectConfig {
        match self.active_effect {
            ActiveEffect::ErrorDiffusion => {
                EffectConfig::ErrorDiffusion(ErrorDiffusionConfig::new(
                    self.diffusion_algorithm,
                    Scale::new(self.scale).unwrap_or(Scale::MIN),
                    self.valid_palette(),
                ))
            }
            ActiveEffect::Ordered => EffectConfig::Ordered(OrderedConfig::new(
                self.bayer_matrix,
                Scale::new(self.scale).unwrap_or(Scale::MIN),
                self.valid_palette(),
            )),
            ActiveEffect::Halftone => EffectConfig::Halftone(
                HalftoneConfig::new(
                    self.halftone_background,
                    self.halftone_dot,
                    self.halftone_shape,
                    self.halftone_cell_size,
                    self.halftone_max_size.min(self.halftone_cell_size),
                    self.halftone_inverted,
                )
                .unwrap_or_default(),
            ),
        }
    }

    pub fn add_palette_color(&mut self) {
        if self.palette.len() >= 8 {
            return;
        }
        let mut pair = (0, 1);
        let mut distance = 0_u32;
        for first in 0..self.palette.len() {
            for second in (first + 1)..self.palette.len() {
                let candidate = rgb_distance(self.palette[first], self.palette[second]);
                if candidate > distance {
                    distance = candidate;
                    pair = (first, second);
                }
            }
        }
        let first = self.palette[pair.0];
        let second = self.palette[pair.1];
        self.palette.insert(
            pair.1,
            Rgb8::new(
                midpoint(first.r, second.r),
                midpoint(first.g, second.g),
                midpoint(first.b, second.b),
            ),
        );
    }

    pub fn remove_palette_color(&mut self, index: usize) {
        if self.palette.len() > 2 && index < self.palette.len() {
            self.palette.remove(index);
        }
    }

    fn valid_palette(&self) -> Palette {
        Palette::new(self.palette.clone()).unwrap_or_default()
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            active_effect: ActiveEffect::ErrorDiffusion,
            scale: 1,
            palette: vec![Rgb8::BLACK, Rgb8::WHITE],
            diffusion_algorithm: ErrorDiffusionAlgorithm::FloydSteinberg,
            bayer_matrix: BayerMatrix::Four,
            halftone_background: Rgb8::WHITE,
            halftone_dot: Rgb8::BLACK,
            halftone_shape: DotShape::Circle,
            halftone_cell_size: 8,
            halftone_max_size: 8,
            halftone_inverted: false,
        }
    }
}

fn midpoint(first: u8, second: u8) -> u8 {
    let sum = u16::from(first) + u16::from(second);
    u8::try_from(sum / 2).unwrap_or(u8::MAX)
}

fn rgb_distance(first: Rgb8, second: Rgb8) -> u32 {
    let red = i32::from(first.r) - i32::from(second.r);
    let green = i32::from(first.g) - i32::from(second.g);
    let blue = i32::from(first.b) - i32::from(second.b);
    u32::try_from(red * red + green * green + blue * blue).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::{ActiveEffect, Settings};
    use dither_engine::{EffectConfig, ErrorDiffusionAlgorithm, Rgb8};

    #[test]
    fn defaults_match_the_product_contract() {
        let settings = Settings::default();
        assert_eq!(settings.active_effect, ActiveEffect::ErrorDiffusion);
        assert_eq!(
            settings.diffusion_algorithm,
            ErrorDiffusionAlgorithm::FloydSteinberg
        );
        assert_eq!(settings.scale, 1);
        assert_eq!(settings.palette, vec![Rgb8::BLACK, Rgb8::WHITE]);
        assert!(matches!(
            settings.effect_config(),
            EffectConfig::ErrorDiffusion(_)
        ));
    }

    #[test]
    fn adds_the_midpoint_of_the_most_distant_pair() {
        let mut settings = Settings {
            palette: vec![
                Rgb8::new(0, 0, 0),
                Rgb8::new(10, 10, 10),
                Rgb8::new(255, 255, 255),
            ],
            ..Settings::default()
        };
        settings.add_palette_color();
        assert_eq!(settings.palette[2], Rgb8::new(127, 127, 127));
    }

    #[test]
    fn never_removes_below_two_or_adds_above_eight() {
        let mut settings = Settings::default();
        settings.remove_palette_color(0);
        assert_eq!(settings.palette.len(), 2);
        for _ in 0..10 {
            settings.add_palette_color();
        }
        assert_eq!(settings.palette.len(), 8);
    }

    #[test]
    fn every_effect_builds_a_valid_engine_configuration() {
        let mut settings = Settings {
            active_effect: ActiveEffect::Ordered,
            ..Settings::default()
        };
        assert!(matches!(settings.effect_config(), EffectConfig::Ordered(_)));
        settings.active_effect = ActiveEffect::Halftone;
        assert!(matches!(
            settings.effect_config(),
            EffectConfig::Halftone(_)
        ));
    }
}
