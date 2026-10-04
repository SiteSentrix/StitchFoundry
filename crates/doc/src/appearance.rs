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

/// Where an arrowhead sits relative to the end of its path. In both modes the stroke stops under
/// the head, so the line never shows through a hollow head or past its tip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArrowAlign {
    /// The tip extends past the end point (the path keeps its length).
    #[default]
    Extend,
    /// The tip sits on the end point (the stroke is shortened by the head).
    Tip,
}

/// Variable-width profile: (position 0..1 along the path, left width factor, right width factor).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WidthProfile {
    pub points: Vec<(f64, f64, f64)>,
}

/// A built-in width profile (the Stroke panel's Profile list).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProfilePreset {
    /// Stable id used by `stroke.set {profile}`.
    pub id: &'static str,
    /// Menu label.
    pub label: &'static str,
    /// (t, left, right) width points.
    pub points: &'static [(f64, f64, f64)],
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
    /// The built-in profiles, in menu order. "uniform" is the plain stroke (no profile).
    pub const PRESETS: [ProfilePreset; 4] = [
        ProfilePreset { id: "uniform", label: "Uniform", points: &[(0.0, 1.0, 1.0), (1.0, 1.0, 1.0)] },
        ProfilePreset { id: "lens", label: "Lens", points: &[(0.0, 0.0, 0.0), (0.5, 1.0, 1.0), (1.0, 0.0, 0.0)] },
        ProfilePreset { id: "taperStart", label: "Taper Start", points: &[(0.0, 0.0, 0.0), (1.0, 1.0, 1.0)] },
        ProfilePreset { id: "taperEnd", label: "Taper End", points: &[(0.0, 1.0, 1.0), (1.0, 0.0, 0.0)] },
    ];
    /// The built-in profile with this id.
    pub fn preset(id: &str) -> Option<Self> {
        Self::PRESETS.iter().find(|p| p.id == id).map(|p| Self { points: p.points.to_vec() })
    }
    /// The id of the built-in profile these points match, if any.
    pub fn preset_id(&self) -> Option<&'static str> {
        Self::PRESETS.iter().find(|p| p.points == self.points.as_slice()).map(|p| p.id)
    }
    /// The id of a stroke's profile: "uniform" without one, "custom" when it matches no preset.
    pub fn id_of(p: Option<&Self>) -> &'static str {
        p.map_or(Some("uniform"), Self::preset_id).unwrap_or("custom")
    }
    /// The lens profile (thin ends, full width in the middle).
    pub fn lens() -> Self {
        Self::preset("lens").expect("built-in")
    }
    pub fn taper_end() -> Self {
        Self::preset("taperEnd").expect("built-in")
    }
    pub fn taper_start() -> Self {
        Self::preset("taperStart").expect("built-in")
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
    /// Arrowhead placement at both ends.
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub arrow_align: ArrowAlign,
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
            arrow_align: ArrowAlign::Extend,
        }
    }
    /// Weight of the start (`end == false`) or end arrowhead: stroke weight × its scale, at least
    /// a quarter point. A head of weight `hw` is `4·hw` long and wide.
    pub fn arrow_weight(&self, end: bool) -> f64 {
        let pct = if end { self.arrow_scale.1 } else { self.arrow_scale.0 };
        (self.width * pct / 100.0).max(0.25)
    }
    /// How far the arrowheads can reach from the path's end points (0 without heads): the
    /// head's diagonal, or with [`ArrowAlign::Extend`] its length plus the cap past the end.
    pub fn arrow_reach(&self) -> f64 {
        let reach = |head: Option<Arrowhead>, end: bool| head.map_or(0.0, |_| 4.5 * self.arrow_weight(end) + self.width / 2.0);
        reach(self.start_arrow, false).max(reach(self.end_arrow, true))
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
                AppearanceItem::Stroke(s) if s.visible && !s.paint.is_none() => Some(
                    match s.align {
                        StrokeAlign::Center => s.width / 2.0 * if s.join == LineJoin::Miter { s.miter_limit.min(4.0) } else { 1.0 },
                        StrokeAlign::Outside => s.width,
                        StrokeAlign::Inside => 0.0,
                    }
                    .max(s.arrow_reach()),
                ),
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
    fn profile_presets_round_trip_their_ids() {
        for p in WidthProfile::PRESETS {
            let prof = WidthProfile::preset(p.id).unwrap();
            assert_eq!(prof.preset_id(), Some(p.id));
            assert_eq!(WidthProfile::id_of(Some(&prof)), p.id);
        }
        assert_eq!(WidthProfile::id_of(None), "uniform");
        assert_eq!(WidthProfile::id_of(Some(&WidthProfile { points: vec![(0.0, 0.3, 0.3)] })), "custom");
        assert!(WidthProfile::preset("nope").is_none());
        assert_eq!(WidthProfile::lens().points, vec![(0.0, 0.0, 0.0), (0.5, 1.0, 1.0), (1.0, 0.0, 0.0)]);
    }

    #[test]
    fn arrow_align_defaults_to_extend_and_round_trips() {
        let mut st = StrokeLayer::new(Paint::solid(Color::BLACK), 2.0);
        assert_eq!(st.arrow_align, ArrowAlign::Extend);
        // The default is not written, so older readers see the same JSON as before.
        assert!(!serde_json::to_string(&st).unwrap().contains("arrow_align"));
        let old: StrokeLayer = serde_json::from_str(r#"{"paint":{"type":"none"},"width":2.0}"#).unwrap();
        assert_eq!(old.arrow_align, ArrowAlign::Extend);
        st.arrow_align = ArrowAlign::Tip;
        let back: StrokeLayer = serde_json::from_str(&serde_json::to_string(&st).unwrap()).unwrap();
        assert_eq!(back, st);
    }

    #[test]
    fn outset_covers_arrowheads() {
        let mut a = Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 4.0);
        a.stroke_mut().unwrap().join = LineJoin::Round;
        assert_eq!(a.outset(), 2.0);
        let st = a.stroke_mut().unwrap();
        st.end_arrow = Some(Arrowhead::Triangle);
        st.arrow_scale = (100.0, 200.0);
        assert_eq!(st.arrow_weight(false), 4.0);
        assert_eq!(st.arrow_weight(true), 8.0);
        // A 32 pt head whose tip sits up to its length (plus the cap) past the end point.
        assert_eq!(a.outset(), 4.5 * 8.0 + 2.0);
        a.stroke_mut().unwrap().width = 0.01;
        assert_eq!(a.stroke().unwrap().arrow_weight(true), 0.25, "heads keep a minimum size");
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
