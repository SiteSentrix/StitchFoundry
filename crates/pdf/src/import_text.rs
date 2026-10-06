//! Text as editable point type: glyphs drawn one after another on the same baseline make one
//! point text object of their Unicode text, a run per font, size and paint (a gap wider than a
//! fifth of the size reads as a space). Fonts are named from the file's base font name (its
//! subset prefix dropped, the family matched against the fonts available); a font that isn't
//! available keeps its name, and the text shows in the fallback font until it is.
//!
//! Glyphs each turned a little further along a curve (type set on a path: apps write each glyph
//! with its own placement) make one type-on-a-path object, its path through the glyphs' baseline.

use std::collections::HashMap;

use kurbo::{Affine, BezPath, Point, Vec2};
use vectorcraft_color::Paint;
use vectorcraft_doc::{CharStyle, TextKind, TextObject, TextRun};

/// One glyph's placement: its baseline origin, advance direction, size and horizontal scale.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Placement {
    pub origin: Point,
    /// Unit vector along the baseline.
    pub dir: Vec2,
    /// Font size in points (the em's height).
    pub size: f64,
    /// Width of the em ÷ its height, in percent.
    pub h_scale: f64,
}

impl Placement {
    /// Where a glyph drawn with `m` (glyph space, 1000 units per em, y up → document) sits;
    /// `None` when it is mirrored, degenerate or not finite (those keep their outlines).
    pub fn of(m: Affine) -> Option<Self> {
        let origin = m * Point::ORIGIN;
        let ex = m * Point::new(1000.0, 0.0) - origin;
        let ey = m * Point::new(0.0, 1000.0) - origin;
        let (w, size) = (ex.hypot(), ey.hypot());
        // Upright in y-down document space: x to the right of up.
        let upright = ex.cross(ey) < 0.0;
        (origin.is_finite() && w.is_finite() && size.is_finite() && w > 0.01 && size > 0.01 && size < 1e5 && upright).then(|| Self {
            origin,
            dir: ex / w,
            size,
            h_scale: w / size * 100.0,
        })
    }

    fn up(&self) -> Vec2 {
        Vec2::new(self.dir.y, -self.dir.x)
    }
}

/// What a run of type is drawn with: its font (cache key, family and style), size, horizontal
/// scale, fill and stroke (paint and width).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Look {
    pub font: u128,
    pub family: String,
    pub style: String,
    pub size: f64,
    pub h_scale: f64,
    pub fill: Option<Paint>,
    pub stroke: Option<(Paint, f64)>,
}

impl Look {
    /// Can a glyph drawn with `other` join a run drawn with this? A fill joins a run whose
    /// glyphs were also stroked over (fill and stroke rendering draws each glyph twice).
    fn takes(&self, other: &Look) -> bool {
        self.font == other.font
            && (self.size - other.size).abs() <= self.size * 0.01
            && (self.h_scale - other.h_scale).abs() < 0.5
            && self.fill == other.fill
            && (other.stroke.is_none() || self.stroke == other.stroke)
    }

    fn style(&self) -> CharStyle {
        let (stroke, stroke_width) = self.stroke.clone().unwrap_or((Paint::None, 0.0));
        CharStyle {
            font_family: self.family.clone(),
            font_style: self.style.clone(),
            size: round(self.size),
            h_scale: round(self.h_scale),
            fill: self.fill.clone().unwrap_or(Paint::None),
            stroke,
            stroke_width,
            ..CharStyle::default()
        }
    }
}

/// A line of type being gathered: runs of glyphs on one baseline.
pub(crate) struct TextLine {
    /// The first glyph's placement.
    at: Placement,
    opacity: f32,
    /// Where the next glyph would start without spacing.
    next: Point,
    /// The last glyph's origin.
    last: Point,
    /// The last glyph's baseline direction.
    last_dir: Vec2,
    /// Each glyph's baseline origin, then where the last one ends.
    baseline: Vec<Point>,
    runs: Vec<(Look, String)>,
}

/// The most a glyph on a curve turns from the one before it (about 20°).
const CURVE_TURN_COS: f64 = 0.94;

impl TextLine {
    pub fn new(at: Placement, opacity: f32) -> Self {
        Self { at, opacity, next: at.origin, last: at.origin, last_dir: at.dir, baseline: vec![], runs: vec![] }
    }

    /// Add glyph `text` drawn with `look` at `at` (advancing `advance` points) at `opacity` if
    /// it continues this line; `false`: it starts another.
    pub fn push(&mut self, look: &Look, at: Placement, opacity: f32, advance: f64, text: &str) -> bool {
        if let Some((last, run)) = self.runs.last_mut() {
            let size = self.at.size.max(at.size);
            let gap = (at.origin - self.next).dot(self.last_dir);
            let on_line = opacity == self.opacity
                && at.dir.dot(self.at.dir) > 0.9995
                && (at.origin - self.at.origin).dot(self.at.up()).abs() < size * 0.15
                && gap > -size * 0.3
                && gap < size * 3.0;
            // Or on a curve: turned a little from the glyph before it, and starting about where
            // that one ends.
            let on_curve = !on_line
                && opacity == self.opacity
                && at.dir.dot(self.last_dir) > CURVE_TURN_COS
                && at.dir.dot(self.last_dir) < 0.99999
                && (at.origin - self.next).hypot() < size * 0.6;
            if !on_line && !on_curve {
                return false;
            }
            // A gap wider than a fifth of an em reads as a space.
            if gap > size * 0.2 && !run.ends_with(' ') && !text.starts_with(' ') {
                run.push(' ');
            }
            if last.takes(look) {
                run.push_str(text);
            } else {
                self.runs.push((look.clone(), text.to_string()));
            }
        } else {
            self.runs.push((look.clone(), text.to_string()));
        }
        self.next = at.origin + at.dir * advance;
        self.last = at.origin;
        self.last_dir = at.dir;
        self.baseline.push(at.origin);
        true
    }

    /// Has the baseline turned (more than about 3° from the first glyph to the last)?
    fn curved(&self) -> bool {
        self.last_dir.dot(self.at.dir) < 0.9986
    }

    /// A smooth path through the glyphs' baseline origins and the end of the last glyph
    /// (Catmull-Rom through the points, as cubic Béziers).
    fn baseline_path(&self) -> Option<BezPath> {
        let mut pts = self.baseline.clone();
        pts.push(self.next);
        let first = *pts.first()?;
        if pts.len() < 3 {
            return None;
        }
        let mut bp = BezPath::new();
        bp.move_to(first);
        for (i, seg) in pts.windows(2).enumerate() {
            let &[p1, p2] = seg else { continue };
            let p0 = *pts.get(i.wrapping_sub(1)).unwrap_or(&p1);
            let p3 = *pts.get(i + 2).unwrap_or(&p2);
            bp.curve_to(p1 + (p2 - p0) / 6.0, p2 - (p3 - p1) / 6.0, p2);
        }
        Some(bp)
    }

    /// A stroke (`stroke`: paint and width) over the glyph just drawn at `at` in `font` (fill
    /// and stroke rendering): the run is stroked. `false`: it isn't that glyph.
    pub fn stroke_last(&mut self, font: u128, at: Placement, stroke: (Paint, f64)) -> bool {
        let Some((look, _)) = self.runs.last_mut().filter(|(l, _)| l.font == font && (at.origin - self.last).hypot() < self.at.size * 1e-3) else {
            return false;
        };
        look.stroke = Some(stroke);
        true
    }

    /// The point type object and its opacity.
    pub fn finish(mut self) -> Option<(TextObject, f32)> {
        // Turned along a curve: type on a path through the glyphs.
        let path = if self.curved() { self.baseline_path() } else { None };
        if let Some((_, last)) = self.runs.last_mut() {
            last.truncate(last.trim_end().len());
        }
        self.runs.retain(|(_, t)| !t.is_empty());
        if self.runs.iter().all(|(_, t)| t.trim().is_empty()) {
            return None;
        }
        let mut runs = self.runs.into_iter().map(|(look, text)| TextRun { text, style: look.style() });
        let first = runs.next()?;
        let mut t = TextObject::point(Point::ORIGIN, &first.text, first.style);
        t.runs.extend(runs);
        t.xf = Affine::translate(self.at.origin.to_vec2()) * Affine::rotate(self.at.dir.atan2());
        if let Some(path) = path {
            t.kind = TextKind::OnPath { path: vectorcraft_geom::PathData::from_bezpath(&path), start: 0.0 };
            t.xf = Affine::IDENTITY;
        }
        Some((t, self.opacity))
    }
}

fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

/// Lower-case letters and digits only (`"Source Sans 3"` → `"sourcesans3"`).
fn norm(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// `"TimesNewRoman"` → `"Times New Roman"`, `"SourceSans3"` → `"Source Sans 3"`.
fn spaced(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 4);
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if let Some(p) = prev
            && ((p.is_ascii_lowercase() && c.is_ascii_uppercase()) || (p.is_ascii_alphabetic() && c.is_ascii_digit()))
        {
            out.push(' ');
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

/// `name` without a PostScript name's trailing `PSMT`, `MT` or `PS`.
fn strip_ps(name: &str) -> &str {
    ["PSMT", "MT", "PS"].iter().find_map(|s| name.strip_suffix(s).filter(|r| !r.is_empty())).unwrap_or(name)
}

/// The families available, by [`norm`]ed name.
pub(crate) struct Families(HashMap<String, String>);

impl Families {
    pub fn available() -> Self {
        Self(vectorcraft_text::FontDb::global().families().into_iter().map(|f| (norm(&f), f)).collect())
    }

    /// The family and style of a font with base (PostScript) name `name`, or of weight `weight`
    /// and slant `italic` when the name has no style; whether the family is available.
    pub fn resolve(&self, name: &str, weight: Option<u32>, italic: bool) -> (String, String, bool) {
        // A subset's six-letter tag.
        let name = match name.split_once('+') {
            Some((tag, rest)) if tag.len() == 6 && tag.chars().all(|c| c.is_ascii_uppercase()) => rest,
            _ => name,
        };
        // An installed face of that exact PostScript name: its own family and style.
        if let Some((family, style)) = vectorcraft_text::FontDb::global().by_postscript_name(name) {
            return (family, style, true);
        }
        let (fam, style) = name.split_once(['-', ',']).unwrap_or((name, ""));
        let found = [fam, strip_ps(fam)].iter().find_map(|f| self.0.get(&norm(f)).cloned());
        let style = match spaced(strip_ps(style)).as_str() {
            "" | "Roman" | "Book" | "Normal" | "Plain" => match (weight.is_some_and(|w| w >= 600) || style.contains("Bold"), italic) {
                (true, true) => "Bold Italic".to_string(),
                (true, false) => "Bold".to_string(),
                (false, true) => "Italic".to_string(),
                (false, false) => "Regular".to_string(),
            },
            s => s.to_string(),
        };
        match found {
            Some(f) => (f, style, true),
            None => (spaced(strip_ps(fam)).trim().to_string(), style, false),
        }
    }
}
