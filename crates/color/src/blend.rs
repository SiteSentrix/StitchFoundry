//! Blend modes (the 16 modes of Illustrator's Transparency panel, same as PDF's).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlendMode {
    #[default]
    Normal,
    Darken,
    Multiply,
    ColorBurn,
    Lighten,
    Screen,
    ColorDodge,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    /// In Transparency-panel order (grouped as Illustrator groups them).
    pub const ALL: [BlendMode; 16] = [
        BlendMode::Normal,
        BlendMode::Darken,
        BlendMode::Multiply,
        BlendMode::ColorBurn,
        BlendMode::Lighten,
        BlendMode::Screen,
        BlendMode::ColorDodge,
        BlendMode::Overlay,
        BlendMode::SoftLight,
        BlendMode::HardLight,
        BlendMode::Difference,
        BlendMode::Exclusion,
        BlendMode::Hue,
        BlendMode::Saturation,
        BlendMode::Color,
        BlendMode::Luminosity,
    ];

    pub fn label(self) -> &'static str {
        match self {
            BlendMode::Normal => "Normal",
            BlendMode::Darken => "Darken",
            BlendMode::Multiply => "Multiply",
            BlendMode::ColorBurn => "Color Burn",
            BlendMode::Lighten => "Lighten",
            BlendMode::Screen => "Screen",
            BlendMode::ColorDodge => "Color Dodge",
            BlendMode::Overlay => "Overlay",
            BlendMode::SoftLight => "Soft Light",
            BlendMode::HardLight => "Hard Light",
            BlendMode::Difference => "Difference",
            BlendMode::Exclusion => "Exclusion",
            BlendMode::Hue => "Hue",
            BlendMode::Saturation => "Saturation",
            BlendMode::Color => "Color",
            BlendMode::Luminosity => "Luminosity",
        }
    }
    /// Index of the separator group (the panel draws a divider between groups).
    pub fn group(self) -> u8 {
        match self {
            BlendMode::Normal => 0,
            BlendMode::Darken | BlendMode::Multiply | BlendMode::ColorBurn => 1,
            BlendMode::Lighten | BlendMode::Screen | BlendMode::ColorDodge => 2,
            BlendMode::Overlay | BlendMode::SoftLight | BlendMode::HardLight => 3,
            BlendMode::Difference | BlendMode::Exclusion => 4,
            _ => 5,
        }
    }
    /// Whether the mode works on each colour channel separately (all but Hue, Saturation, Color
    /// and Luminosity).
    pub fn is_separable(self) -> bool {
        self.group() != 5
    }
    /// Parse a label or identifier, case/space-insensitively.
    pub fn parse(s: &str) -> Option<Self> {
        let norm = |x: &str| x.to_ascii_lowercase().replace([' ', '_', '-'], "");
        let want = norm(s);
        Self::ALL.into_iter().find(|m| norm(m.label()) == want)
    }
}

/// Luminance weights of the non-separable modes (Hue, Saturation, Color, Luminosity), as the PDF
/// and W3C compositing specifications define them.
pub const LUM: [f32; 3] = [0.3, 0.59, 0.11];

/// Luminance weights of opacity masks (Rec. 709, applied to the sRGB values as SVG luminance masks
/// with `color-interpolation="sRGB"` apply them).
pub const MASK_LUM: [f32; 3] = [0.2126, 0.7152, 0.0722];

/// The blend function B(backdrop, source) of `mode` on straight (not premultiplied) RGB in 0..=1:
/// the colour that replaces the backdrop where an opaque source covers an opaque backdrop. Every
/// output (screen, SVG, PDF and transparency flattening) follows these formulas.
pub fn blend_rgb(mode: BlendMode, backdrop: [f32; 3], source: [f32; 3]) -> [f32; 3] {
    use BlendMode as B;
    let separable = |f: fn(f32, f32) -> f32| [0, 1, 2].map(|i| f(backdrop[i].clamp(0.0, 1.0), source[i].clamp(0.0, 1.0)));
    match mode {
        B::Normal => separable(|_, s| s),
        B::Multiply => separable(|b, s| b * s),
        B::Screen => separable(screen),
        B::Overlay => separable(|b, s| hard_light(s, b)),
        B::Darken => separable(f32::min),
        B::Lighten => separable(f32::max),
        B::ColorDodge => separable(|b, s| {
            if b <= 0.0 {
                0.0
            } else if s >= 1.0 {
                1.0
            } else {
                (b / (1.0 - s)).min(1.0)
            }
        }),
        B::ColorBurn => separable(|b, s| {
            if b >= 1.0 {
                1.0
            } else if s <= 0.0 {
                0.0
            } else {
                1.0 - ((1.0 - b) / s).min(1.0)
            }
        }),
        B::HardLight => separable(hard_light),
        B::SoftLight => separable(|b, s| {
            if s <= 0.5 {
                b - (1.0 - 2.0 * s) * b * (1.0 - b)
            } else {
                let d = if b <= 0.25 { ((16.0 * b - 12.0) * b + 4.0) * b } else { b.sqrt() };
                b + (2.0 * s - 1.0) * (d - b)
            }
        }),
        B::Difference => separable(|b, s| (b - s).abs()),
        B::Exclusion => separable(|b, s| b + s - 2.0 * b * s),
        B::Hue => set_lum(set_sat(source, sat(backdrop)), lum(backdrop)),
        B::Saturation => set_lum(set_sat(backdrop, sat(source)), lum(backdrop)),
        B::Color => set_lum(source, lum(backdrop)),
        B::Luminosity => set_lum(backdrop, lum(source)),
    }
}

/// Source-over compositing of `source` onto `backdrop` with `mode` (straight RGBA in 0..=1, the
/// result straight too): the source colour is mixed with B(backdrop, source) by the backdrop's
/// alpha, then laid over the backdrop by its own alpha.
pub fn composite(mode: BlendMode, backdrop: [f32; 4], source: [f32; 4]) -> [f32; 4] {
    let (ab, as_) = (backdrop[3].clamp(0.0, 1.0), source[3].clamp(0.0, 1.0));
    let ao = as_ + ab * (1.0 - as_);
    if ao <= 0.0 {
        return [0.0; 4];
    }
    let mixed = blend_rgb(mode, [backdrop[0], backdrop[1], backdrop[2]], [source[0], source[1], source[2]]);
    let c = |i: usize| ((1.0 - ab) * source[i] + ab * mixed[i]) * as_ + (1.0 - as_) * ab * backdrop[i];
    [c(0) / ao, c(1) / ao, c(2) / ao, ao]
}

fn screen(b: f32, s: f32) -> f32 {
    b + s - b * s
}

fn hard_light(b: f32, s: f32) -> f32 {
    if s <= 0.5 { b * 2.0 * s } else { screen(b, 2.0 * s - 1.0) }
}

fn lum(c: [f32; 3]) -> f32 {
    LUM[0] * c[0] + LUM[1] * c[1] + LUM[2] * c[2]
}

fn sat(c: [f32; 3]) -> f32 {
    c[0].max(c[1]).max(c[2]) - c[0].min(c[1]).min(c[2])
}

/// Bring a colour moved off the gamut by [`set_lum`] back in, keeping its luminance.
fn clip_color(c: [f32; 3]) -> [f32; 3] {
    let l = lum(c);
    let (n, x) = (c[0].min(c[1]).min(c[2]), c[0].max(c[1]).max(c[2]));
    let mut c = c;
    if n < 0.0 {
        c = c.map(|v| l + (v - l) * l / (l - n));
    }
    if x > 1.0 {
        c = c.map(|v| l + (v - l) * (1.0 - l) / (x - l));
    }
    c
}

fn set_lum(c: [f32; 3], l: f32) -> [f32; 3] {
    let d = l - lum(c);
    clip_color(c.map(|v| v + d))
}

/// `c` with saturation `s`: its largest component becomes `s`, its smallest 0 and the middle one
/// keeps its proportion.
fn set_sat(c: [f32; 3], s: f32) -> [f32; 3] {
    let (n, x) = (c[0].min(c[1]).min(c[2]), c[0].max(c[1]).max(c[2]));
    if x <= n {
        return [0.0; 3];
    }
    c.map(|v| (v - n) * s / (x - n))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    #[test]
    fn separable_modes_follow_the_reference_formulas() {
        let (b, s) = ([0.2, 0.5, 0.9], [0.6, 0.25, 0.5]);
        let cases: [(BlendMode, [f32; 3]); 12] = [
            (BlendMode::Normal, s),
            (BlendMode::Multiply, [0.12, 0.125, 0.45]),
            (BlendMode::Screen, [0.68, 0.625, 0.95]),
            (BlendMode::Darken, [0.2, 0.25, 0.5]),
            (BlendMode::Lighten, [0.6, 0.5, 0.9]),
            (BlendMode::Difference, [0.4, 0.25, 0.4]),
            (BlendMode::Exclusion, [0.56, 0.5, 0.5]),
            // Overlay = Hard Light with the layers swapped: b ≤ ½ multiplies, else screens.
            (BlendMode::Overlay, [0.24, 0.25, 0.9]),
            (BlendMode::HardLight, [0.36, 0.25, 0.9]),
            (BlendMode::ColorDodge, [0.5, 0.6666667, 1.0]),
            (BlendMode::ColorBurn, [0.0, 0.0, 0.8]),
            // s ≤ ½ darkens by b(1−b)(1−2s); s > ½ lightens towards D(b).
            (BlendMode::SoftLight, [0.2 + 0.2 * (((16.0 * 0.2 - 12.0) * 0.2 + 4.0) * 0.2 - 0.2), 0.375, 0.9]),
        ];
        for (m, want) in cases {
            assert!(near(blend_rgb(m, b, s), want), "{m:?}: {:?} ≠ {want:?}", blend_rgb(m, b, s));
        }
    }

    #[test]
    fn non_separable_modes_keep_the_reference_luminance() {
        let (b, s) = ([0.8, 0.3, 0.2], [0.1, 0.4, 0.9]);
        // Color and Hue take the backdrop's luminance; Luminosity the source's.
        for m in [BlendMode::Hue, BlendMode::Saturation, BlendMode::Color] {
            assert!((lum(blend_rgb(m, b, s)) - lum(b)).abs() < 1e-5, "{m:?}");
        }
        assert!((lum(blend_rgb(BlendMode::Luminosity, b, s)) - lum(s)).abs() < 1e-5);
        // Saturation of grey is 0: a grey source desaturates the backdrop to its luminance.
        let l = lum(b);
        assert!(near(blend_rgb(BlendMode::Saturation, b, [0.5; 3]), [l; 3]));
        // Hue of the source with the backdrop's saturation: blue-ish stays blue-ish.
        let h = blend_rgb(BlendMode::Hue, b, s);
        assert!(h[2] > h[1] && h[1] > h[0], "{h:?}");
        assert!((sat(h) - sat(b)).abs() < 1e-5);
    }

    #[test]
    fn composite_lays_the_blend_over_the_backdrop_by_alpha() {
        let (b, s) = ([0.8, 0.4, 0.2, 1.0], [0.5, 0.5, 0.5, 0.5]);
        let o = composite(BlendMode::Multiply, b, s);
        // Half of the multiplied colour over half of the backdrop.
        assert!(near([o[0], o[1], o[2]], [0.6, 0.3, 0.15]) && o[3] == 1.0, "{o:?}");
        // Over a transparent backdrop the source shows unblended.
        let o = composite(BlendMode::Difference, [0.3, 0.3, 0.3, 0.0], s);
        assert!(near([o[0], o[1], o[2]], [0.5; 3]) && (o[3] - 0.5).abs() < 1e-6, "{o:?}");
    }

    #[test]
    fn parse_labels() {
        assert_eq!(BlendMode::parse("color burn"), Some(BlendMode::ColorBurn));
        assert_eq!(BlendMode::parse("ColorBurn"), Some(BlendMode::ColorBurn));
        assert_eq!(BlendMode::parse("soft_light"), Some(BlendMode::SoftLight));
        assert_eq!(BlendMode::parse("nope"), None);
        for m in BlendMode::ALL {
            assert_eq!(BlendMode::parse(m.label()), Some(m));
        }
    }
}
