//! The appearance model: a stack of fills and strokes, each with its own opacity, blend mode and
//! effects, plus object-level effects.

use serde::{Deserialize, Serialize};
use vectorcraft_color::{BlendMode, Color, Paint};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StrokeAlign {
    #[default]
    Center,
    Inside,
    Outside,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Dash {
    /// Dash, gap, dash, gap… (up to 6 values in the Stroke panel).
    pub pattern: Vec<f64>,
    #[serde(default)]
    pub offset: f64,
    /// "Aligns dashes to corners and path ends, adjusting lengths to fit".
    #[serde(default)]
    pub align_corners: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Arrowhead {
    Triangle,
    TriangleOpen,
    Circle,
    CircleOpen,
    Square,
    SquareOpen,
    Bar,
    Diamond,
    Arrow,
    ArrowOpen,
}

impl Arrowhead {
    pub const ALL: [Arrowhead; 10] = [
        Arrowhead::Arrow,
        Arrowhead::ArrowOpen,
        Arrowhead::Triangle,
        Arrowhead::TriangleOpen,
        Arrowhead::Circle,
        Arrowhead::CircleOpen,
        Arrowhead::Square,
        Arrowhead::SquareOpen,
        Arrowhead::Diamond,
        Arrowhead::Bar,
    ];
}

/// Variable-width profile: (position 0..1 along the path, left width factor, right width factor).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WidthProfile {
    pub points: Vec<(f64, f64, f64)>,
}

impl WidthProfile {
    /// Width factor at `t` (average of both sides), linear between points.
    pub fn at(&self, t: f64) -> (f64, f64) {
        let p = &self.points;
        if p.is_empty() {
            return (1.0, 1.0);
        }
        if t <= p[0].0 {
            return (p[0].1, p[0].2);
        }
        for w in p.windows(2) {
            if t <= w[1].0 {
                let u = (t - w[0].0) / (w[1].0 - w[0].0).max(1e-9);
                return (w[0].1 + (w[1].1 - w[0].1) * u, w[0].2 + (w[1].2 - w[0].2) * u);
            }
        }
        let l = p.last().unwrap();
        (l.1, l.2)
    }
    /// Illustrator's "Width Profile 1" (lens shape) analogue.
    pub fn lens() -> Self {
        Self { points: vec![(0.0, 0.0, 0.0), (0.5, 1.0, 1.0), (1.0, 0.0, 0.0)] }
    }
    pub fn taper_end() -> Self {
        Self { points: vec![(0.0, 1.0, 1.0), (1.0, 0.0, 0.0)] }
    }
    pub fn taper_start() -> Self {
        Self { points: vec![(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)] }
    }
}

/// A live effect in an appearance stack. Parameters are interpreted by `vectorcraft-effects`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// Stable id, e.g. `stylize.dropShadow`, `distort.roughen`, `path.offsetPath`, `warp.arc`.
    pub id: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default = "yes")]
    pub visible: bool,
}

fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FillLayer {
    pub paint: Paint,
    #[serde(default = "one", skip_serializing_if = "crate::skip::is_one")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub blend: BlendMode,
    #[serde(default = "yes", skip_serializing_if = "crate::skip::is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

impl FillLayer {
    pub fn new(paint: Paint) -> Self {
        Self { paint, opacity: 1.0, blend: BlendMode::Normal, visible: true, effects: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StrokeLayer {
    pub paint: Paint,
    /// Weight in points.
    pub width: f64,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub cap: LineCap,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub join: LineJoin,
    #[serde(default = "ten", skip_serializing_if = "is_ten")]
    pub miter_limit: f64,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub align: StrokeAlign,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<Dash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_arrow: Option<Arrowhead>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_arrow: Option<Arrowhead>,
    /// Arrowhead scale in percent (start, end).
    #[serde(default = "hundreds", skip_serializing_if = "is_hundreds")]
    pub arrow_scale: (f64, f64),
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<WidthProfile>,
    /// Brush applied to the stroke (by brush name).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brush: Option<String>,
    #[serde(default = "one", skip_serializing_if = "crate::skip::is_one")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub blend: BlendMode,
    #[serde(default = "yes", skip_serializing_if = "crate::skip::is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

fn ten() -> f64 {
    10.0
}
fn hundreds() -> (f64, f64) {
    (100.0, 100.0)
}
fn is_ten(v: &f64) -> bool {
    *v == 10.0
}
fn is_hundreds(v: &(f64, f64)) -> bool {
    *v == (100.0, 100.0)
}

impl StrokeLayer {
    pub fn new(paint: Paint, width: f64) -> Self {
        Self {
            paint,
            width,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 10.0,
            align: StrokeAlign::Center,
            dash: None,
            start_arrow: None,
            end_arrow: None,
            arrow_scale: (100.0, 100.0),
            profile: None,
            brush: None,
            opacity: 1.0,
            blend: BlendMode::Normal,
            visible: true,
            effects: vec![],
        }
    }
    /// The box an unplaced gradient on this stroke fits to: the object's geometric bounds grown by
    /// half the weight (as the renderers resolve it).
    pub fn paint_bounds(&self, geometric: vectorcraft_geom::Rect) -> vectorcraft_geom::Rect {
        geometric.inflate(self.width / 2.0, self.width / 2.0)
    }
}

/// One entry of the appearance stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AppearanceItem {
    Fill(FillLayer),
    Stroke(StrokeLayer),
}

/// Appearance attributes. `items` is in paint order: `items[0]` is painted first (the bottom of the
/// Appearance panel list).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Appearance {
    #[serde(default)]
    pub items: Vec<AppearanceItem>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
}

impl Appearance {
    /// Illustrator's "basic appearance": one fill below one stroke.
    pub fn basic(fill: Paint, stroke: Paint, width: f64) -> Self {
        Self { items: vec![AppearanceItem::Fill(FillLayer::new(fill)), AppearanceItem::Stroke(StrokeLayer::new(stroke, width))], effects: vec![] }
    }
    /// White fill, 1 pt black stroke (the default for new art).
    pub fn default_art() -> Self {
        Self::basic(Paint::solid(Color::WHITE), Paint::solid(Color::BLACK), 1.0)
    }
    /// The topmost fill (what the Fill proxy shows).
    pub fn fill(&self) -> Option<&FillLayer> {
        self.items.iter().rev().find_map(|i| if let AppearanceItem::Fill(f) = i { Some(f) } else { None })
    }
    pub fn fill_mut(&mut self) -> Option<&mut FillLayer> {
        self.items.iter_mut().rev().find_map(|i| if let AppearanceItem::Fill(f) = i { Some(f) } else { None })
    }
    /// The topmost stroke (what the Stroke proxy shows).
    pub fn stroke(&self) -> Option<&StrokeLayer> {
        self.items.iter().rev().find_map(|i| if let AppearanceItem::Stroke(s) = i { Some(s) } else { None })
    }
    pub fn stroke_mut(&mut self) -> Option<&mut StrokeLayer> {
        self.items.iter_mut().rev().find_map(|i| if let AppearanceItem::Stroke(s) = i { Some(s) } else { None })
    }
    pub fn fill_paint(&self) -> Paint {
        self.fill().map(|f| f.paint.clone()).unwrap_or(Paint::None)
    }
    pub fn stroke_paint(&self) -> Paint {
        self.stroke().map(|s| s.paint.clone()).unwrap_or(Paint::None)
    }
    /// Set the top fill's paint, creating a fill if there is none.
    pub fn set_fill(&mut self, p: Paint) {
        match self.fill_mut() {
            Some(f) => f.paint = p,
            None => self.items.insert(0, AppearanceItem::Fill(FillLayer::new(p))),
        }
    }
    /// Set the top stroke's paint, creating a 1 pt stroke if there is none.
    pub fn set_stroke(&mut self, p: Paint) {
        match self.stroke_mut() {
            Some(s) => s.paint = p,
            None => self.items.push(AppearanceItem::Stroke(StrokeLayer::new(p, 1.0))),
        }
    }
    pub fn stroke_width(&self) -> f64 {
        self.stroke().filter(|s| !s.paint.is_none()).map(|s| s.width).unwrap_or(0.0)
    }
    /// Is this the basic one-fill-one-stroke appearance without effects?
    pub fn is_basic(&self) -> bool {
        self.effects.is_empty()
            && self.items.len() <= 2
            && self.items.iter().filter(|i| matches!(i, AppearanceItem::Fill(_))).count() <= 1
            && self.items.iter().all(|i| match i {
                AppearanceItem::Fill(f) => f.effects.is_empty() && f.opacity == 1.0 && f.blend == BlendMode::Normal,
                AppearanceItem::Stroke(s) => s.effects.is_empty() && s.opacity == 1.0 && s.blend == BlendMode::Normal,
            })
    }
    /// Largest distance the painted area extends beyond the geometry (for visual bounds).
    pub fn outset(&self) -> f64 {
        self.items
            .iter()
            .filter_map(|i| match i {
                AppearanceItem::Stroke(s) if s.visible && !s.paint.is_none() => Some(match s.align {
                    StrokeAlign::Center => s.width / 2.0 * if s.join == LineJoin::Miter { s.miter_limit.min(4.0) } else { 1.0 },
                    StrokeAlign::Outside => s.width,
                    StrokeAlign::Inside => 0.0,
                }),
                _ => None,
            })
            .fold(0.0, f64::max)
    }
    /// Scale stroke weights (Scale Strokes & Effects).
    pub fn scale_strokes(&mut self, s: f64) {
        for i in &mut self.items {
            if let AppearanceItem::Stroke(st) = i {
                st.width *= s;
                if let Some(d) = &mut st.dash {
                    for v in &mut d.pattern {
                        *v *= s;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_appearance() {
        let a = Appearance::default_art();
        assert!(a.is_basic());
        assert_eq!(a.fill_paint(), Paint::solid(Color::WHITE));
        assert_eq!(a.stroke_width(), 1.0);
    }

    #[test]
    fn set_creates_missing() {
        let mut a = Appearance::default();
        a.set_fill(Paint::solid(Color::BLACK));
        a.set_stroke(Paint::solid(Color::WHITE));
        assert_eq!(a.items.len(), 2);
        assert!(matches!(a.items[0], AppearanceItem::Fill(_)));
    }

    #[test]
    fn profile_interp() {
        let p = WidthProfile::lens();
        assert_eq!(p.at(0.5), (1.0, 1.0));
        assert_eq!(p.at(0.25), (0.5, 0.5));
        assert_eq!(p.at(2.0), (0.0, 0.0));
    }

    #[test]
    fn outset_depends_on_align() {
        let mut a = Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 10.0);
        a.stroke_mut().unwrap().join = LineJoin::Round;
        assert_eq!(a.outset(), 5.0);
        a.stroke_mut().unwrap().align = StrokeAlign::Outside;
        assert_eq!(a.outset(), 10.0);
        a.stroke_mut().unwrap().align = StrokeAlign::Inside;
        assert_eq!(a.outset(), 0.0);
    }
}
