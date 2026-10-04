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
}

/// One entry of the appearance stack.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AppearanceItem {
    Fill(FillLayer),
    Stroke(StrokeLayer),
}

impl AppearanceItem {
    pub fn is_fill(&self) -> bool {
        matches!(self, AppearanceItem::Fill(_))
    }
    /// `"fill"` or `"stroke"` (the serialized `kind`).
    pub fn kind_name(&self) -> &'static str {
        if self.is_fill() { "fill" } else { "stroke" }
    }
    pub fn paint(&self) -> &Paint {
        match self {
            AppearanceItem::Fill(f) => &f.paint,
            AppearanceItem::Stroke(s) => &s.paint,
        }
    }
    pub fn visible(&self) -> bool {
        match self {
            AppearanceItem::Fill(f) => f.visible,
            AppearanceItem::Stroke(s) => s.visible,
        }
    }
    pub fn opacity(&self) -> f32 {
        match self {
            AppearanceItem::Fill(f) => f.opacity,
            AppearanceItem::Stroke(s) => s.opacity,
        }
    }
    pub fn blend(&self) -> BlendMode {
        match self {
            AppearanceItem::Fill(f) => f.blend,
            AppearanceItem::Stroke(s) => s.blend,
        }
    }
    /// The item's own live effects (applied to this fill or stroke only).
    pub fn effects(&self) -> &Vec<Effect> {
        match self {
            AppearanceItem::Fill(f) => &f.effects,
            AppearanceItem::Stroke(s) => &s.effects,
        }
    }
    pub fn effects_mut(&mut self) -> &mut Vec<Effect> {
        match self {
            AppearanceItem::Fill(f) => &mut f.effects,
            AppearanceItem::Stroke(s) => &mut s.effects,
        }
    }
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
    /// Fill item `index`, or the topmost fill for `None`. `None` when that item is not a fill.
    pub fn fill_at(&self, index: Option<usize>) -> Option<&FillLayer> {
        match index {
            None => self.fill(),
            Some(i) => match self.items.get(i)? {
                AppearanceItem::Fill(f) => Some(f),
                AppearanceItem::Stroke(_) => None,
            },
        }
    }
    pub fn fill_at_mut(&mut self, index: Option<usize>) -> Option<&mut FillLayer> {
        match index {
            None => self.fill_mut(),
            Some(i) => match self.items.get_mut(i)? {
                AppearanceItem::Fill(f) => Some(f),
                AppearanceItem::Stroke(_) => None,
            },
        }
    }
    /// Stroke item `index`, or the topmost stroke for `None`. `None` when that item is not a stroke.
    pub fn stroke_at(&self, index: Option<usize>) -> Option<&StrokeLayer> {
        match index {
            None => self.stroke(),
            Some(i) => match self.items.get(i)? {
                AppearanceItem::Stroke(s) => Some(s),
                AppearanceItem::Fill(_) => None,
            },
        }
    }
    pub fn stroke_at_mut(&mut self, index: Option<usize>) -> Option<&mut StrokeLayer> {
        match index {
            None => self.stroke_mut(),
            Some(i) => match self.items.get_mut(i)? {
                AppearanceItem::Stroke(s) => Some(s),
                AppearanceItem::Fill(_) => None,
            },
        }
    }
    /// `index` when it names a fill (`fill`) or stroke item, else `None` (the topmost one): the
    /// item a fill or stroke edit changes while item `index` is the Appearance panel's target.
    pub fn item_of_kind(&self, index: Option<usize>, fill: bool) -> Option<usize> {
        index.filter(|i| self.items.get(*i).is_some_and(|it| it.is_fill() == fill))
    }
    /// The fill the Fill proxy shows while item `index` is targeted: that item when it is a fill,
    /// else the topmost fill.
    pub fn fill_for(&self, index: Option<usize>) -> Option<&FillLayer> {
        self.fill_at(self.item_of_kind(index, true))
    }
    /// The stroke the Stroke proxy and panel show while item `index` is targeted.
    pub fn stroke_for(&self, index: Option<usize>) -> Option<&StrokeLayer> {
        self.stroke_at(self.item_of_kind(index, false))
    }
    /// Set the paint of fill item `index` (`None`: the top fill, created if missing). False when
    /// `index` is not a fill.
    pub fn set_fill_at(&mut self, index: Option<usize>, p: Paint) -> bool {
        if index.is_none() {
            self.set_fill(p);
            return true;
        }
        self.fill_at_mut(index).map(|f| f.paint = p).is_some()
    }
    /// Set the paint of stroke item `index` (`None`: the top stroke, created if missing). False
    /// when `index` is not a stroke.
    pub fn set_stroke_at(&mut self, index: Option<usize>, p: Paint) -> bool {
        if index.is_none() {
            self.set_stroke(p);
            return true;
        }
        self.stroke_at_mut(index).map(|s| s.paint = p).is_some()
    }
    /// The paint of fill (`fill`) or stroke item `index` (`None`: the topmost one).
    pub fn paint_at(&self, index: Option<usize>, fill: bool) -> Option<&Paint> {
        if fill { self.fill_at(index).map(|f| &f.paint) } else { self.stroke_at(index).map(|s| &s.paint) }
    }
    /// [`Self::set_fill_at`] or [`Self::set_stroke_at`].
    pub fn set_paint_at(&mut self, index: Option<usize>, fill: bool, p: Paint) -> bool {
        if fill { self.set_fill_at(index, p) } else { self.set_stroke_at(index, p) }
    }
    /// The effects of item `index`, or the object-level effects for `None`.
    pub fn effects_at(&self, index: Option<usize>) -> Option<&Vec<Effect>> {
        match index {
            None => Some(&self.effects),
            Some(i) => self.items.get(i).map(AppearanceItem::effects),
        }
    }
    pub fn effects_mut(&mut self, index: Option<usize>) -> Option<&mut Vec<Effect>> {
        match index {
            None => Some(&mut self.effects),
            Some(i) => self.items.get_mut(i).map(AppearanceItem::effects_mut),
        }
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
    fn items_by_index() {
        let mut a = Appearance::default_art();
        a.items.push(AppearanceItem::Fill(FillLayer::new(Paint::None)));
        // [Fill white, Stroke black, Fill none]
        assert_eq!(a.fill_at(None).unwrap().paint, Paint::None);
        assert_eq!(a.fill_at(Some(0)).unwrap().paint, Paint::solid(Color::WHITE));
        assert!(a.fill_at(Some(1)).is_none() && a.stroke_at(Some(0)).is_none() && a.fill_at(Some(9)).is_none());
        assert_eq!(a.item_of_kind(Some(1), false), Some(1));
        assert_eq!(a.item_of_kind(Some(1), true), None);
        assert_eq!(a.fill_for(Some(1)).unwrap().paint, Paint::None);
        assert!(a.set_fill_at(Some(0), Paint::solid(Color::BLACK)));
        assert!(!a.set_stroke_at(Some(0), Paint::None));
        assert_eq!(a.fill_at(Some(0)).unwrap().paint, Paint::solid(Color::BLACK));
        a.effects_mut(Some(1)).unwrap().push(Effect { id: "distort.roughen".into(), params: serde_json::Value::Null, visible: true });
        assert_eq!(a.effects_at(Some(1)).unwrap().len(), 1);
        assert!(a.effects_at(None).unwrap().is_empty() && a.effects_mut(Some(3)).is_none());
        assert_eq!((a.items[1].kind_name(), a.items[2].kind_name()), ("stroke", "fill"));
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
