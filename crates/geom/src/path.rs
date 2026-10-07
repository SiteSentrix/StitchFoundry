//! The anchor-based editable path model.

use kurbo::{Affine, BezPath, CubicBez, ParamCurve, ParamCurveNearest, PathEl, Point, Rect, Shape, Vec2};
use serde::{Deserialize, Serialize};

use crate::EPS;

/// Fill rule for paths and compound paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FillRule {
    /// Illustrator's default for compound paths created from overlapping shapes.
    #[default]
    NonZero,
    EvenOdd,
}

/// How an anchor's handles behave when edited.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnchorKind {
    /// Handles move independently (or there are none).
    #[default]
    Corner,
    /// Handles stay collinear (dragging one rotates the other).
    Smooth,
}

/// One anchor point with absolute handle positions. A handle equal to `p` means "no handle".
/// Serialized without handles that equal `p` and without the default `kind` (smaller files).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "AnchorRepr", into = "AnchorRepr")]
pub struct Anchor {
    pub p: Point,
    #[serde(rename = "in")]
    pub h_in: Point,
    #[serde(rename = "out")]
    pub h_out: Point,
    #[serde(default)]
    pub kind: AnchorKind,
}

/// Wire form of [`Anchor`]: points as `[x, y]` (format v2; `{"x", "y"}` maps from v1 files are
/// still read), and absent handles mean "no handle" (equal to `p`).
#[derive(Serialize, Deserialize)]
struct AnchorRepr {
    p: WirePoint,
    #[serde(rename = "in", default, skip_serializing_if = "Option::is_none")]
    h_in: Option<WirePoint>,
    #[serde(rename = "out", default, skip_serializing_if = "Option::is_none")]
    h_out: Option<WirePoint>,
    #[serde(default, skip_serializing_if = "is_corner")]
    kind: AnchorKind,
}

fn is_corner(k: &AnchorKind) -> bool {
    *k == AnchorKind::Corner
}

impl From<AnchorRepr> for Anchor {
    fn from(r: AnchorRepr) -> Self {
        let p = r.p.0;
        Self { p, h_in: r.h_in.map_or(p, |h| h.0), h_out: r.h_out.map_or(p, |h| h.0), kind: r.kind }
    }
}

impl From<Anchor> for AnchorRepr {
    fn from(a: Anchor) -> Self {
        let handle = |h: Point| (h != a.p).then_some(WirePoint(h));
        Self { p: WirePoint(a.p), h_in: handle(a.h_in), h_out: handle(a.h_out), kind: a.kind }
    }
}

/// A point written as `[x, y]`, read from `[x, y]` or `{"x": …, "y": …}`.
struct WirePoint(Point);

impl Serialize for WirePoint {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.0.x, self.0.y].serialize(s)
    }
}

impl<'de> Deserialize<'de> for WirePoint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = WirePoint;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a point as [x, y] or {\"x\": x, \"y\": y}")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<WirePoint, A::Error> {
                let x = a.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let y = a.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                Ok(WirePoint(Point::new(x, y)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<WirePoint, A::Error> {
                let (mut x, mut y) = (None, None);
                while let Some(k) = a.next_key::<std::borrow::Cow<'de, str>>()? {
                    match k.as_ref() {
                        "x" => x = Some(a.next_value()?),
                        "y" => y = Some(a.next_value()?),
                        _ => {
                            a.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(WirePoint(Point::new(
                    x.ok_or_else(|| serde::de::Error::missing_field("x"))?,
                    y.ok_or_else(|| serde::de::Error::missing_field("y"))?,
                )))
            }
        }
        d.deserialize_any(V)
    }
}

impl Anchor {
    /// A corner anchor without handles.
    pub fn corner(p: Point) -> Self {
        Self { p, h_in: p, h_out: p, kind: AnchorKind::Corner }
    }
    /// A smooth anchor with a symmetric out handle `out` (the in handle is mirrored).
    pub fn smooth(p: Point, out: Point) -> Self {
        Self { p, h_in: p - (out - p), h_out: out, kind: AnchorKind::Smooth }
    }
    pub fn with_handles(p: Point, h_in: Point, h_out: Point) -> Self {
        let kind =
            if is_collinear(p, h_in, h_out) && h_in.distance(p) > EPS && h_out.distance(p) > EPS { AnchorKind::Smooth } else { AnchorKind::Corner };
        Self { p, h_in, h_out, kind }
    }
    pub fn has_in(&self) -> bool {
        self.h_in.distance(self.p) > EPS
    }
    pub fn has_out(&self) -> bool {
        self.h_out.distance(self.p) > EPS
    }
    pub fn transform(&self, a: Affine) -> Self {
        Self { p: a * self.p, h_in: a * self.h_in, h_out: a * self.h_out, kind: self.kind }
    }
    pub fn translate(&mut self, d: kurbo::Vec2) {
        self.p += d;
        self.h_in += d;
        self.h_out += d;
    }
    /// Place the end of the outgoing (`out`) or incoming handle at `pos`. `independent` makes the
    /// anchor a corner first; a smooth anchor keeps its other handle in line, at its length.
    pub fn set_handle(&mut self, out: bool, pos: Point, independent: bool) {
        if independent {
            self.kind = AnchorKind::Corner;
        }
        let p = self.p;
        let smooth = self.kind == AnchorKind::Smooth;
        let (moved, other) = if out { (&mut self.h_out, &mut self.h_in) } else { (&mut self.h_in, &mut self.h_out) };
        *moved = pos;
        if smooth {
            let len = (*other - p).hypot();
            let dir = p - pos;
            let l = dir.hypot();
            if l > 1e-9 {
                *other = p + dir * (len / l);
            }
        }
    }
    /// Swap handles (used when reversing path direction).
    pub fn reversed(&self) -> Self {
        Self { p: self.p, h_in: self.h_out, h_out: self.h_in, kind: self.kind }
    }
    /// Remove both handles (convert to a sharp corner).
    pub fn retract(&mut self) {
        self.h_in = self.p;
        self.h_out = self.p;
        self.kind = AnchorKind::Corner;
    }
}

fn anchor_finite(a: &Anchor) -> bool {
    [a.p, a.h_in, a.h_out].into_iter().all(|p| p.x.is_finite() && p.y.is_finite())
}

fn unit(v: Vec2) -> Vec2 {
    let l = v.hypot();
    if l > 1e-9 { v / l } else { Vec2::ZERO }
}

/// Direction of the handle that leaves `prev` toward `mid`. A missing handle takes the
/// tangent of that segment, then the chord.
fn ray_out(prev: &Anchor, mid: &Anchor) -> Vec2 {
    let v = prev.h_out - prev.p;
    if v.hypot() > 1e-9 {
        return unit(v);
    }
    let toward_in = mid.h_in - prev.p;
    if toward_in.hypot() > 1e-9 {
        return unit(toward_in);
    }
    unit(mid.p - prev.p)
}

/// Direction of the handle that arrives at `next` from `mid`.
fn ray_in(next: &Anchor, mid: &Anchor) -> Vec2 {
    let v = next.h_in - next.p;
    if v.hypot() > 1e-9 {
        return unit(v);
    }
    let toward_out = mid.h_out - next.p;
    if toward_out.hypot() > 1e-9 {
        return unit(toward_out);
    }
    unit(mid.p - next.p)
}

/// Points along the two segments that meet at `index`, in order, including both ends.
fn sample_pair(sp: &SubPath, index: usize) -> Vec<Point> {
    let n = sp.anchors.len();
    let left = sp.segment((index + n - 1) % n);
    let right = sp.segment(index);
    const N: usize = 8;
    let mut pts = Vec::with_capacity(N * 2 + 1);
    for i in 0..=N {
        pts.push(left.eval(i as f64 / N as f64));
    }
    for i in 1..=N {
        pts.push(right.eval(i as f64 / N as f64));
    }
    pts
}

fn shape_error(p0: Point, p3: Point, d0: Vec2, d1: Vec2, s0: f64, s1: f64, samples: &[Point]) -> f64 {
    if !s0.is_finite() || !s1.is_finite() {
        return f64::MAX;
    }
    let c = CubicBez::new(p0, p0 + d0 * s0, p3 + d1 * s1, p3);
    samples.iter().map(|p| c.nearest(*p, 1e-3).distance_sq).sum()
}

/// True when `(s0, s1)` matches the samples more closely, or matches them the same and is shorter.
/// Equal shapes keep the retracted handles: a straight line needs no handles.
fn prefer(err: f64, s0: f64, s1: f64, best_e: f64, best: (f64, f64)) -> bool {
    err < best_e - 1e-6 || ((err - best_e).abs() <= 1e-6 && s0 + s1 < best.0 + best.1 - 1e-6)
}

struct ScaleFit {
    cap: f64,
    p0: Point,
    p3: Point,
    d0: Vec2,
    d1: Vec2,
}

impl ScaleFit {
    fn consider(&self, s0: f64, s1: f64, samples: &[Point], best: &mut (f64, f64), best_e: &mut f64) {
        let s0 = s0.clamp(0.0, self.cap);
        let s1 = s1.clamp(0.0, self.cap);
        let err = shape_error(self.p0, self.p3, self.d0, self.d1, s0, s1, samples);
        if prefer(err, s0, s1, *best_e, *best) {
            *best_e = err;
            *best = (s0, s1);
        }
    }
}

/// Lengths along `d0` and `d1`. A grid search on the nearest-point error, then a tighter grid
/// around the winner. The handles already on the path, and retracted handles, are candidates too.
fn best_scales(p0: Point, p3: Point, d0: Vec2, d1: Vec2, keep0: f64, keep1: f64, samples: &[Point]) -> (f64, f64) {
    let fit = ScaleFit { cap: (p3 - p0).hypot().max(1.0) * 4.0, p0, p3, d0, d1 };
    let mut best = (0.0_f64, 0.0_f64);
    let mut best_e = f64::MAX;
    fit.consider(keep0, keep1, samples, &mut best, &mut best_e);
    fit.consider(0.0, 0.0, samples, &mut best, &mut best_e);
    const N: usize = 16;
    for i in 0..=N {
        for j in 0..=N {
            fit.consider(fit.cap * (i as f64 / N as f64), fit.cap * (j as f64 / N as f64), samples, &mut best, &mut best_e);
        }
    }
    let mut window = fit.cap / N as f64;
    for _ in 0..4 {
        let step = window / 6.0;
        if step < 1e-3 {
            break;
        }
        let center = best;
        for i in 0..=12 {
            for j in 0..=12 {
                fit.consider(center.0 + (i as f64 - 6.0) * step, center.1 + (j as f64 - 6.0) * step, samples, &mut best, &mut best_e);
            }
        }
        window *= 0.5;
    }
    best
}

/// Facing handles of the anchors on either side of `index`, as absolute positions.
fn fit_facing_handles(sp: &SubPath, index: usize) -> Option<(Point, Point)> {
    let n = sp.anchors.len();
    let pi = (index + n - 1) % n;
    let ni = (index + 1) % n;
    let prev = sp.anchors.get(pi)?;
    let mid = sp.anchors.get(index)?;
    let next = sp.anchors.get(ni)?;
    let d0 = ray_out(prev, mid);
    let d1 = ray_in(next, mid);
    let samples = sample_pair(sp, index);
    if samples.len() < 3 {
        return None;
    }
    let (p0, p3) = (prev.p, next.p);
    let keep0 = (prev.h_out - p0).hypot();
    let keep1 = (next.h_in - p3).hypot();
    let (s0, s1) = best_scales(p0, p3, d0, d1, keep0, keep1, &samples);
    let cap = (p3 - p0).hypot().max(1.0) * 8.0;
    let s0 = if s0.is_finite() { s0.clamp(0.0, cap) } else { 0.0 };
    let s1 = if s1.is_finite() { s1.clamp(0.0, cap) } else { 0.0 };
    Some((p0 + d0 * s0, p3 + d1 * s1))
}

fn is_collinear(p: Point, a: Point, b: Point) -> bool {
    let va = a - p;
    let vb = b - p;
    let cross = va.cross(vb);
    cross.abs() <= 1e-6 * va.hypot().max(1.0) * vb.hypot().max(1.0) && va.dot(vb) < 0.0
}

/// An open or closed run of anchors.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubPath {
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub closed: bool,
}

impl SubPath {
    pub fn new(anchors: Vec<Anchor>, closed: bool) -> Self {
        Self { anchors, closed }
    }
    /// Polyline from points.
    pub fn polyline(points: &[Point], closed: bool) -> Self {
        Self { anchors: points.iter().map(|&p| Anchor::corner(p)).collect(), closed }
    }
    /// Number of segments (a closed path has one more, back to the start).
    pub fn segment_count(&self) -> usize {
        let n = self.anchors.len();
        if n < 2 {
            0
        } else if self.closed {
            n
        } else {
            n - 1
        }
    }
    /// Segment `i` as a cubic (lines are cubics with handles on the anchors).
    pub fn segment(&self, i: usize) -> CubicBez {
        let n = self.anchors.len();
        let a = &self.anchors[i % n];
        let b = &self.anchors[(i + 1) % n];
        CubicBez::new(a.p, a.h_out, b.h_in, b.p)
    }
    /// True if segment `i` is a straight line.
    pub fn segment_is_line(&self, i: usize) -> bool {
        let n = self.anchors.len();
        let a = &self.anchors[i % n];
        let b = &self.anchors[(i + 1) % n];
        !a.has_out() && !b.has_in()
    }
    pub fn to_bezpath_into(&self, out: &mut BezPath) {
        let Some(first) = self.anchors.first() else { return };
        out.move_to(first.p);
        for i in 0..self.segment_count() {
            let c = self.segment(i);
            if self.segment_is_line(i) {
                out.line_to(c.p3);
            } else {
                out.curve_to(c.p1, c.p2, c.p3);
            }
        }
        if self.closed {
            out.close_path();
        }
    }
    pub fn reverse(&mut self) {
        self.anchors.reverse();
        for a in &mut self.anchors {
            *a = a.reversed();
        }
    }
    /// Signed area, closing an open subpath with a line: positive when it runs clockwise on
    /// screen (y down), negative when counter-clockwise.
    pub fn signed_area(&self) -> f64 {
        use kurbo::ParamCurveArea;
        let a: f64 = (0..self.segment_count()).map(|i| self.segment(i).signed_area()).sum();
        match (self.closed, self.anchors.first(), self.anchors.last()) {
            (false, Some(f), Some(l)) => a + kurbo::Line::new(l.p, f.p).signed_area(),
            _ => a,
        }
    }
    /// Split segment `seg` at parameter `t`, inserting a new anchor. Returns the new anchor index.
    pub fn insert_anchor(&mut self, seg: usize, t: f64) -> usize {
        let n = self.anchors.len();
        let c = self.segment(seg);
        let line = self.segment_is_line(seg);
        let (l, r) = (c.subsegment(0.0..t), c.subsegment(t..1.0));
        let i0 = seg % n;
        let i1 = (seg + 1) % n;
        let mid = if line {
            Anchor::corner(l.p3)
        } else {
            self.anchors[i0].h_out = l.p1;
            self.anchors[i1].h_in = r.p2;
            Anchor { p: l.p3, h_in: l.p2, h_out: r.p1, kind: AnchorKind::Smooth }
        };
        self.anchors.insert(seg + 1, mid);
        seg + 1
    }
    /// Remove anchor `index`. When `fit` is set and the anchor has a neighbour on both sides, the
    /// facing handles are lengthened or shortened along their current direction so one cubic stays
    /// close to the two segments that met at the removed point. Positions of the other anchors do
    /// not move. Returns false when `index` is out of range.
    pub fn remove_anchor(&mut self, index: usize, fit: bool) -> bool {
        let n = self.anchors.len();
        if index >= n {
            return false;
        }
        let interior = self.closed || (index > 0 && index + 1 < n);
        if fit
            && interior
            && n >= 3
            && self.anchors.iter().all(anchor_finite)
            && let Some((hout, hin)) = fit_facing_handles(self, index)
        {
            let pi = (index + n - 1) % n;
            let ni = (index + 1) % n;
            let prev = self.anchors[pi];
            let next = self.anchors[ni];
            self.anchors[pi] = Anchor::with_handles(prev.p, prev.h_in, hout);
            self.anchors[ni] = Anchor::with_handles(next.p, hin, next.h_out);
        }
        self.anchors.remove(index);
        true
    }
    /// Signed area via the shoelace formula on the Bézier path (positive = clockwise in y-down).
    pub fn area(&self) -> f64 {
        let mut bp = BezPath::new();
        self.to_bezpath_into(&mut bp);
        bp.area()
    }
}

/// A (possibly multi-subpath) path.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathData {
    pub subpaths: Vec<SubPath>,
}

impl PathData {
    pub fn new(subpaths: Vec<SubPath>) -> Self {
        Self { subpaths }
    }
    pub fn single(sp: SubPath) -> Self {
        Self { subpaths: vec![sp] }
    }
    pub fn is_empty(&self) -> bool {
        self.subpaths.iter().all(|s| s.anchors.is_empty())
    }
    pub fn anchor_count(&self) -> usize {
        self.subpaths.iter().map(|s| s.anchors.len()).sum()
    }
    pub fn to_bezpath(&self) -> BezPath {
        let mut bp = BezPath::new();
        for sp in &self.subpaths {
            sp.to_bezpath_into(&mut bp);
        }
        bp
    }
    /// Convert a kurbo path to anchors. Quadratics are elevated to cubics.
    pub fn from_bezpath(bp: &BezPath) -> Self {
        let mut subpaths: Vec<SubPath> = Vec::new();
        let mut cur: Option<SubPath> = None;
        let mut last = Point::ZERO;
        for el in bp.elements() {
            match *el {
                PathEl::MoveTo(p) => {
                    if let Some(sp) = cur.take()
                        && !sp.anchors.is_empty()
                    {
                        subpaths.push(sp);
                    }
                    cur = Some(SubPath { anchors: vec![Anchor::corner(p)], closed: false });
                    last = p;
                }
                PathEl::LineTo(p) => {
                    let sp = cur.get_or_insert_with(|| SubPath { anchors: vec![Anchor::corner(last)], closed: false });
                    sp.anchors.push(Anchor::corner(p));
                    last = p;
                }
                PathEl::QuadTo(q, p) => {
                    let c1 = last + (q - last) * (2.0 / 3.0);
                    let c2 = p + (q - p) * (2.0 / 3.0);
                    push_curve(&mut cur, last, c1, c2, p);
                    last = p;
                }
                PathEl::CurveTo(c1, c2, p) => {
                    push_curve(&mut cur, last, c1, c2, p);
                    last = p;
                }
                PathEl::ClosePath => {
                    if let Some(mut sp) = cur.take() {
                        // Merge a closing anchor that duplicates the start.
                        if sp.anchors.len() > 1
                            && let (Some(first), Some(lastp)) = (sp.anchors.first().map(|a| a.p), sp.anchors.last().map(|a| a.p))
                            && first.distance(lastp) < 1e-7
                            && let Some(l) = sp.anchors.pop()
                            && let Some(a0) = sp.anchors.first_mut()
                        {
                            a0.h_in = l.h_in;
                        }
                        sp.closed = true;
                        for a in &mut sp.anchors {
                            *a = Anchor::with_handles(a.p, a.h_in, a.h_out);
                        }
                        last = sp.anchors.first().map(|a| a.p).unwrap_or(last);
                        subpaths.push(sp);
                    }
                }
            }
        }
        if let Some(mut sp) = cur
            && !sp.anchors.is_empty()
        {
            for a in &mut sp.anchors {
                *a = Anchor::with_handles(a.p, a.h_in, a.h_out);
            }
            subpaths.push(sp);
        }
        Self { subpaths }
    }
    /// Tight geometric bounds (curve extrema, not control points).
    pub fn bounds(&self) -> Option<Rect> {
        if self.is_empty() {
            return None;
        }
        let bp = self.to_bezpath();
        if self.anchor_count() == 1
            && let Some(a) = self.subpaths.iter().find_map(|s| s.anchors.first())
        {
            return Some(Rect::from_points(a.p, a.p));
        }
        Some(bp.bounding_box())
    }
    /// Bounds including control handles (used for quick culling).
    pub fn control_bounds(&self) -> Option<Rect> {
        let mut it = self.subpaths.iter().flat_map(|s| s.anchors.iter()).flat_map(|a| [a.p, a.h_in, a.h_out]);
        let first = it.next()?;
        Some(it.fold(Rect::from_points(first, first), |r, p| r.union_pt(p)))
    }
    pub fn transform(&mut self, a: Affine) {
        for sp in &mut self.subpaths {
            for an in &mut sp.anchors {
                *an = an.transform(a);
            }
        }
    }
    pub fn transformed(&self, a: Affine) -> Self {
        let mut c = self.clone();
        c.transform(a);
        c
    }
    pub fn is_closed(&self) -> bool {
        !self.subpaths.is_empty() && self.subpaths.iter().all(|s| s.closed)
    }
    /// Nearest point on any segment: (subpath, segment, t, point, distance).
    pub fn nearest(&self, p: Point) -> Option<(usize, usize, f64, Point, f64)> {
        let mut best: Option<(usize, usize, f64, Point, f64)> = None;
        for (si, sp) in self.subpaths.iter().enumerate() {
            for seg in 0..sp.segment_count() {
                let c = sp.segment(seg);
                let n = c.nearest(p, 1e-6);
                let q = c.eval(n.t);
                let d = n.distance_sq.sqrt();
                if best.is_none_or(|b| d < b.4) {
                    best = Some((si, seg, n.t, q, d));
                }
            }
        }
        best
    }
    /// Iterate (subpath index, anchor index, anchor).
    pub fn anchors(&self) -> impl Iterator<Item = (usize, usize, &Anchor)> {
        self.subpaths.iter().enumerate().flat_map(|(si, s)| s.anchors.iter().enumerate().map(move |(ai, a)| (si, ai, a)))
    }
    pub fn anchor_mut(&mut self, si: usize, ai: usize) -> Option<&mut Anchor> {
        self.subpaths.get_mut(si)?.anchors.get_mut(ai)
    }
    pub fn reverse(&mut self) {
        for sp in &mut self.subpaths {
            sp.reverse();
        }
    }
    /// Total length of all segments.
    pub fn length(&self) -> f64 {
        self.to_bezpath().segments().map(|s| kurbo::ParamCurveArclen::arclen(&s, 1e-6)).sum()
    }
}

fn push_curve(cur: &mut Option<SubPath>, last: Point, c1: Point, c2: Point, p: Point) {
    let sp = cur.get_or_insert_with(|| SubPath { anchors: vec![Anchor::corner(last)], closed: false });
    if let Some(a) = sp.anchors.last_mut() {
        a.h_out = c1;
    }
    sp.anchors.push(Anchor { p, h_in: c2, h_out: p, kind: AnchorKind::Corner });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> PathData {
        PathData::single(SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0)], true))
    }

    #[test]
    fn anchors_serialize_compactly_and_read_v1_maps() {
        let corner = Anchor::corner(Point::new(1.0, 2.0));
        assert_eq!(serde_json::to_string(&corner).unwrap(), r#"{"p":[1.0,2.0]}"#);
        let smooth = Anchor { p: Point::new(1.0, 2.0), h_in: Point::new(0.0, 2.0), h_out: Point::new(2.0, 2.0), kind: AnchorKind::Smooth };
        let back: Anchor = serde_json::from_str(&serde_json::to_string(&smooth).unwrap()).unwrap();
        assert_eq!(back, smooth);
        let v1 = r#"{"p":{"x":1.0,"y":2.0},"in":{"x":0.0,"y":2.0},"out":{"x":2.0,"y":2.0},"kind":"Smooth"}"#;
        assert_eq!(serde_json::from_str::<Anchor>(v1).unwrap(), smooth);
        let v1_corner = r#"{"p":{"x":1.0,"y":2.0},"in":{"x":1.0,"y":2.0},"out":{"x":1.0,"y":2.0},"kind":"Corner"}"#;
        assert_eq!(serde_json::from_str::<Anchor>(v1_corner).unwrap(), corner);
    }

    #[test]
    fn square_bounds_and_area() {
        let p = square();
        assert_eq!(p.bounds(), Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!((p.subpaths[0].area().abs() - 100.0).abs() < 1e-9);
        assert_eq!(p.subpaths[0].segment_count(), 4);
    }

    #[test]
    fn bezpath_roundtrip_polyline() {
        let p = square();
        let bp = p.to_bezpath();
        let back = PathData::from_bezpath(&bp);
        assert_eq!(back.subpaths.len(), 1);
        assert_eq!(back.subpaths[0].anchors.len(), 4);
        assert!(back.subpaths[0].closed);
        assert_eq!(back.bounds(), p.bounds());
    }

    #[test]
    fn bezpath_roundtrip_curves() {
        let c = kurbo::Circle::new((50.0, 50.0), 20.0);
        let bp = c.to_path(1e-3);
        let pd = PathData::from_bezpath(&bp);
        assert_eq!(pd.subpaths.len(), 1);
        assert!(pd.subpaths[0].closed);
        let b = pd.bounds().unwrap();
        assert!((b.width() - 40.0).abs() < 1e-3);
        // All anchors of a circle are smooth.
        assert!(pd.subpaths[0].anchors.iter().all(|a| a.kind == AnchorKind::Smooth));
    }

    #[test]
    fn insert_anchor_on_line_keeps_shape() {
        let mut p = square();
        let before = p.bounds();
        let idx = p.subpaths[0].insert_anchor(0, 0.5);
        assert_eq!(idx, 1);
        assert_eq!(p.subpaths[0].anchors[1].p, Point::new(5.0, 0.0));
        assert_eq!(p.bounds(), before);
        assert_eq!(p.anchor_count(), 5);
    }

    #[test]
    fn insert_anchor_on_curve_keeps_shape() {
        let bp = kurbo::Circle::new((0.0, 0.0), 10.0).to_path(1e-3);
        let mut pd = PathData::from_bezpath(&bp);
        let area0 = pd.subpaths[0].area();
        pd.subpaths[0].insert_anchor(1, 0.3);
        assert!((pd.subpaths[0].area() - area0).abs() < 1e-6);
    }

    #[test]
    fn nearest_on_edge() {
        let p = square();
        let (_, seg, _, q, d) = p.nearest(Point::new(5.0, -3.0)).unwrap();
        assert_eq!(seg, 0);
        assert!((q.x - 5.0).abs() < 1e-6 && q.y.abs() < 1e-6);
        assert!((d - 3.0).abs() < 1e-6);
    }

    #[test]
    fn reverse_flips_area_sign() {
        let mut p = square();
        let a = p.subpaths[0].area();
        p.reverse();
        assert!((p.subpaths[0].area() + a).abs() < 1e-9);
    }

    #[test]
    fn transform_moves_bounds() {
        let p = square().transformed(Affine::translate((5.0, 5.0)));
        assert_eq!(p.bounds(), Some(Rect::new(5.0, 5.0, 15.0, 15.0)));
    }

    #[test]
    fn smooth_detection() {
        let a = Anchor::with_handles(Point::new(0.0, 0.0), Point::new(-1.0, 0.0), Point::new(2.0, 0.0));
        assert_eq!(a.kind, AnchorKind::Smooth);
        let c = Anchor::with_handles(Point::new(0.0, 0.0), Point::new(-1.0, 0.0), Point::new(0.0, 2.0));
        assert_eq!(c.kind, AnchorKind::Corner);
    }

    #[test]
    fn length_of_square() {
        assert!((square().length() - 40.0).abs() < 1e-6);
    }

    #[test]
    fn serde_roundtrip() {
        let p = square();
        let s = serde_json::to_string(&p).unwrap();
        let back: PathData = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn smart_remove_restores_a_split_cubic() {
        let mut sp = SubPath::new(
            vec![
                Anchor::with_handles(Point::new(0.0, 0.0), Point::new(0.0, 0.0), Point::new(30.0, 80.0)),
                Anchor::with_handles(Point::new(100.0, 0.0), Point::new(70.0, 80.0), Point::new(100.0, 0.0)),
            ],
            false,
        );
        let original = sp.clone();
        let idx = sp.insert_anchor(0, 0.4);
        assert!(sp.remove_anchor(idx, true));
        assert_eq!(sp.anchors.len(), 2);
        assert!(sp.anchors[0].h_out.distance(original.anchors[0].h_out) < 0.5, "{:?}", sp.anchors[0].h_out);
        assert!(sp.anchors[1].h_in.distance(original.anchors[1].h_in) < 0.5, "{:?}", sp.anchors[1].h_in);
    }

    #[test]
    fn smart_remove_of_a_line_stays_a_line() {
        let mut sp = SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(40.0, 0.0), Point::new(100.0, 0.0)], false);
        assert!(sp.remove_anchor(1, true));
        assert_eq!(sp.anchors.len(), 2);
        assert!(!sp.anchors[0].has_out());
        assert!(!sp.anchors[1].has_in());
    }

    #[test]
    fn smart_remove_of_a_corner_beats_the_straight_chord() {
        let mut sp = SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(50.0, 40.0), Point::new(100.0, 0.0)], false);
        assert!(sp.remove_anchor(1, true));
        let d = sp.segment(0).nearest(Point::new(50.0, 40.0), 1e-4).distance_sq.sqrt();
        assert!(d < 40.0, "curve stays closer to the removed corner than the chord does ({d})");
    }

    #[test]
    fn remove_without_fit_keeps_handles() {
        let mut sp = SubPath::new(
            vec![
                Anchor::with_handles(Point::new(0.0, 0.0), Point::new(0.0, 0.0), Point::new(10.0, 30.0)),
                Anchor::corner(Point::new(50.0, 10.0)),
                Anchor::with_handles(Point::new(100.0, 0.0), Point::new(80.0, 25.0), Point::new(100.0, 0.0)),
            ],
            false,
        );
        let before = sp.clone();
        assert!(sp.remove_anchor(1, false));
        assert_eq!(sp.anchors[0].h_out, before.anchors[0].h_out);
        assert_eq!(sp.anchors[1].h_in, before.anchors[2].h_in);
        assert!(!sp.remove_anchor(9, true));
    }

    #[test]
    fn remove_endpoint_drops_the_point() {
        let mut sp = SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(20.0, 5.0)], false);
        assert!(sp.remove_anchor(0, true));
        assert_eq!(sp.anchors[0].p, Point::new(10.0, 0.0));
        assert_eq!(sp.anchors.len(), 2);
    }
}
