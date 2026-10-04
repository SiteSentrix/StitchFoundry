//! Colour harmony rules (the Color Guide panel).

use crate::{Color, keep_model};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Harmony {
    Complementary,
    SplitComplementary,
    LeftComplement,
    RightComplement,
    Analogous,
    Triad,
    Tetrad,
    Square,
    Shades,
}

impl Harmony {
    pub const ALL: [Harmony; 9] = [
        Harmony::Complementary,
        Harmony::SplitComplementary,
        Harmony::LeftComplement,
        Harmony::RightComplement,
        Harmony::Analogous,
        Harmony::Triad,
        Harmony::Tetrad,
        Harmony::Square,
        Harmony::Shades,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Harmony::Complementary => "Complementary",
            Harmony::SplitComplementary => "Split Complementary",
            Harmony::LeftComplement => "Left Complement",
            Harmony::RightComplement => "Right Complement",
            Harmony::Analogous => "Analogous",
            Harmony::Triad => "Triad",
            Harmony::Tetrad => "Tetrad",
            Harmony::Square => "Square",
            Harmony::Shades => "Shades",
        }
    }
    /// Colours of the harmony, base colour first, in the base colour's model.
    pub fn apply(self, base: Color) -> Vec<Color> {
        let [h, s, v] = base.to_hsb();
        let hue = |d: f32| keep_model(base, Color::from_hsb(h + d, s, v));
        match self {
            Harmony::Complementary => vec![base, hue(180.0)],
            Harmony::SplitComplementary => vec![base, hue(150.0), hue(210.0)],
            Harmony::LeftComplement => vec![base, hue(150.0)],
            Harmony::RightComplement => vec![base, hue(210.0)],
            Harmony::Analogous => vec![hue(-60.0), hue(-30.0), base, hue(30.0), hue(60.0)],
            Harmony::Triad => vec![base, hue(120.0), hue(240.0)],
            Harmony::Tetrad => vec![base, hue(60.0), hue(180.0), hue(240.0)],
            Harmony::Square => vec![base, hue(90.0), hue(180.0), hue(270.0)],
            Harmony::Shades => (0..5).map(|i| keep_model(base, Color::from_hsb(h, s, (v * (1.0 - i as f32 * 0.18)).max(0.0)))).collect(),
        }
    }
}

/// Tints and shades row for the Color Guide variation grid, in the base colour's model.
pub fn variations(base: Color, steps: usize) -> Vec<Color> {
    let n = steps.max(1) as f32;
    (0..steps)
        .map(|i| {
            let t = (i as f32 + 1.0) / (n + 1.0);
            let c = if i < steps / 2 { base.lerp(&Color::BLACK, 1.0 - t * 2.0) } else { base.lerp(&Color::WHITE, (t - 0.5) * 2.0) };
            keep_model(base, c)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn complementary_red() {
        let c = Harmony::Complementary.apply(Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(c[1].to_hex(), "#00ffff");
        assert_eq!(Harmony::Triad.apply(Color::rgb(1.0, 0.0, 0.0))[1].to_hex(), "#00ff00");
        assert_eq!(variations(Color::rgb(1.0, 0.0, 0.0), 6).len(), 6);
    }

    #[test]
    fn harmonies_keep_the_base_model() {
        let base = Color::cmyk(0.0, 1.0, 1.0, 0.0);
        for h in Harmony::ALL {
            assert!(h.apply(base).iter().all(|c| matches!(c, Color::Cmyk { .. })), "{}", h.label());
        }
        let Color::Cmyk { c, m, y, .. } = Harmony::Complementary.apply(base)[1] else { unreachable!() };
        assert!(c > m && c > y, "the complement of red is a cyan: {c} {m} {y}");
        assert!(variations(Color::gray(0.4), 4).iter().all(|c| matches!(c, Color::Gray { .. })));
    }
}
