//! Arrowheads. A head of weight `hw` (stroke weight × scale) is `4·hw` long and `4·hw` wide.
//! Hollow kinds are rings (`hw/2` walls) or, for the open arrow, a chevron with `hw` arms. The
//! stroke ends under the head at the head's *inset* from the tip: halfway into solid heads, in
//! the back wall of hollow ones (with the cap kept out of the hole) and at the chevron's inner
//! corner. With [`ArrowAlign::Tip`] the tip sits on the end point and the stroke is shortened by
//! the inset; with [`ArrowAlign::Extend`] the path keeps its length and the tip sits the inset
//! past the end point.

use std::borrow::Cow;

use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg, Point, Shape, Vec2};
use vectorcraft_doc::{ArrowAlign, Arrowhead, StrokeLayer};

use super::{ARCLEN_ACCURACY, StrokePieces, cap_extent, push_seg, segments, subpaths, tangent, unit};

/// One arrowhead.
#[derive(Clone, Debug, PartialEq)]
pub struct Arrow {
    pub kind: Arrowhead,
    /// The filled outline (non-zero rule; hollow kinds are rings).
    pub outline: BezPath,
    pub tip: Point,
    /// Unit direction the head points in.
    pub dir: Vec2,
    /// Distance from the tip back to where the stroke ends under the head.
    pub inset: f64,
}

/// [`super::stroke_pieces`] for a stroke with at least one arrowhead.
pub(super) fn pieces<'a>(bp: &'a BezPath, st: &StrokeLayer) -> StrokePieces<'a> {
    let subs = subpaths(bp);
    let els = bp.elements();
    // Heads go on the start of the first subpath and the end of the last one, when open.
    let first = subs.first().filter(|s| !s.1).map(|s| s.0.clone());
    let last = subs.last().filter(|s| !s.1).map(|s| s.0.clone());
    let cap = cap_extent(st.cap, st.width);
    let weight = |pct: f64| (st.width * pct / 100.0).max(0.25);
    let start = st.start_arrow.zip(first).map(|(k, r)| (k, r, inset(k, weight(st.arrow_scale.0), cap)));
    let end = st.end_arrow.zip(last).map(|(k, r)| (k, r, inset(k, weight(st.arrow_scale.1), cap)));
    if start.is_none() && end.is_none() {
        return StrokePieces { line: Cow::Borrowed(bp), heads: vec![] };
    }
    let tip_mode = st.arrow_align == ArrowAlign::Tip;
    let mut heads = vec![];
    // Trims per subpath index (start, end), applied in tip mode only.
    let mut trims: Vec<(f64, f64)> = vec![(0.0, 0.0); subs.len()];
    let mut place = |kind: Arrowhead, range: std::ops::Range<usize>, at_start: bool, inset: f64, pct: f64| {
        let segs = segments(&els[range]);
        let (end_pt, outward) = end_of(&segs, at_start);
        let (tip, dir) = if tip_mode {
            // Point along the chord from where the stroke now ends to the end point.
            let dir = cut_point(&segs, at_start, inset).map(|p| end_pt - p).filter(|v| v.hypot() > 1e-9).map(unit).unwrap_or(outward);
            (end_pt, dir)
        } else {
            (end_pt + outward * inset, outward)
        };
        heads.push(Arrow { kind, outline: shape(kind, tip, dir, weight(pct)), tip, dir, inset });
    };
    if let Some((k, r, i)) = start {
        place(k, r, true, i, st.arrow_scale.0);
        if tip_mode {
            trims[0].0 = i;
        }
    }
    if let Some((k, r, i)) = end {
        place(k, r, false, i, st.arrow_scale.1);
        if tip_mode {
            trims[subs.len() - 1].1 = i;
        }
    }
    if !tip_mode {
        return StrokePieces { line: Cow::Borrowed(bp), heads };
    }
    let mut line = BezPath::new();
    for ((range, _), &(s0, s1)) in subs.iter().zip(&trims) {
        if s0 == 0.0 && s1 == 0.0 {
            line.extend(els[range.clone()].iter().copied());
            continue;
        }
        // Only open subpaths get heads, so a trimmed subpath stays open.
        let segs = trim(&segments(&els[range.clone()]), s0, s1);
        if let Some(first) = segs.first() {
            line.move_to(first.start());
            for s in &segs {
                push_seg(&mut line, s);
            }
        }
    }
    StrokePieces { line: Cow::Owned(line), heads }
}

/// The end point of a subpath and the unit direction pointing out of it there.
fn end_of(segs: &[PathSeg], at_start: bool) -> (Point, Vec2) {
    let long = |s: &&PathSeg| s.arclen(ARCLEN_ACCURACY) > 1e-9;
    if at_start {
        let p = segs.first().map_or(Point::ORIGIN, |s| s.start());
        let d = segs.iter().find(long).map_or(Vec2::new(-1.0, 0.0), |s| -tangent(s, 0.0));
        (p, d)
    } else {
        let p = segs.last().map_or(Point::ORIGIN, |s| s.end());
        let d = segs.iter().rev().find(long).map_or(Vec2::new(1.0, 0.0), |s| tangent(s, 1.0));
        (p, d)
    }
}

/// The point `dist` along the subpath from its start (or back from its end), if it is that long.
fn cut_point(segs: &[PathSeg], from_start: bool, dist: f64) -> Option<Point> {
    let (i, t) = if from_start { cut_from_start(segs, dist)? } else { cut_from_end(segs, dist)? };
    Some(segs[i].eval(t))
}

/// (segment, parameter) at arc length `dist` from the start.
fn cut_from_start(segs: &[PathSeg], dist: f64) -> Option<(usize, f64)> {
    let mut acc = 0.0;
    for (i, s) in segs.iter().enumerate() {
        let len = s.arclen(ARCLEN_ACCURACY);
        if acc + len >= dist {
            return Some((i, if len > 0.0 { s.inv_arclen(dist - acc, ARCLEN_ACCURACY) } else { 0.0 }));
        }
        acc += len;
    }
    None
}

/// (segment, parameter) at arc length `dist` back from the end.
fn cut_from_end(segs: &[PathSeg], dist: f64) -> Option<(usize, f64)> {
    let mut acc = 0.0;
    for (i, s) in segs.iter().enumerate().rev() {
        let len = s.arclen(ARCLEN_ACCURACY);
        if acc + len >= dist {
            return Some((i, if len > 0.0 { s.inv_arclen(len - (dist - acc), ARCLEN_ACCURACY) } else { 1.0 }));
        }
        acc += len;
    }
    None
}

/// The segments with `s0` of arc length removed from the start and `s1` from the end (empty when
/// nothing is left).
fn trim(segs: &[PathSeg], s0: f64, s1: f64) -> Vec<PathSeg> {
    let (Some((i0, t0)), Some((i1, t1))) = (cut_from_start(segs, s0), cut_from_end(segs, s1)) else { return vec![] };
    if (i0, t0) >= (i1, t1) {
        return vec![];
    }
    if i0 == i1 {
        return vec![segs[i0].subsegment(t0..t1)];
    }
    let mut out = Vec::with_capacity(i1 - i0 + 1);
    out.push(segs[i0].subsegment(t0..1.0));
    out.extend_from_slice(&segs[i0 + 1..i1]);
    out.push(segs[i1].subsegment(0.0..t1));
    out
}

/// Head length (and width) for weight `hw`.
fn len(hw: f64) -> f64 {
    4.0 * hw
}
/// Wall of the hollow ring heads.
fn ring(hw: f64) -> f64 {
    0.5 * hw
}
/// Arm thickness of the open arrow.
fn arm(hw: f64) -> f64 {
    hw
}
/// Thickness of the bar.
fn bar(hw: f64) -> f64 {
    0.75 * hw
}
/// Where the notch of the (open) arrow sits, as a fraction of the length.
const NOTCH: f64 = 0.7;

/// Distance from the tip where the stroke ends, for a stroke whose cap reaches `cap` past its end.
fn inset(kind: Arrowhead, hw: f64, cap: f64) -> f64 {
    let l = len(hw);
    // Solid heads: halfway in, far enough that the cap stays behind the tip.
    let solid = |back: f64| (back / 2.0).max(cap);
    // Hollow heads: in the back wall, with the cap kept out of the hole.
    let hollow = |back: f64| (back - ring(hw) / 2.0).max(back - ring(hw) + cap);
    match kind {
        Arrowhead::Triangle | Arrowhead::Circle | Arrowhead::Square | Arrowhead::Diamond => solid(l),
        Arrowhead::Arrow => solid(NOTCH * l),
        Arrowhead::Bar => solid(bar(hw)),
        Arrowhead::TriangleOpen | Arrowhead::CircleOpen | Arrowhead::SquareOpen => hollow(l),
        // The inner corner of the chevron: arm / sin(half angle).
        Arrowhead::ArrowOpen => (arm(hw) * (l * l + l * l / 4.0).sqrt() / (l / 2.0)).max(cap),
    }
}

/// Outline of a `kind` head of weight `hw` with its tip at `tip`, pointing along `dir`.
fn shape(kind: Arrowhead, tip: Point, dir: Vec2, hw: f64) -> BezPath {
    let n = Vec2::new(-dir.y, dir.x);
    let l = len(hw);
    let half = l / 2.0;
    let base = tip - dir * l;
    let mut p = BezPath::new();
    let triangle = [tip, base + n * half, base - n * half];
    let square = [tip + n * half, base + n * half, base - n * half, tip - n * half];
    let centre = tip - dir * half;
    match kind {
        Arrowhead::Triangle => polygon(&mut p, &triangle),
        Arrowhead::TriangleOpen => ring_polygon(&mut p, &triangle, ring(hw)),
        Arrowhead::Arrow => polygon(&mut p, &[tip, base + n * half, tip - dir * (NOTCH * l), base - n * half]),
        Arrowhead::ArrowOpen => {
            // Arms of thickness `arm` from the tip to the base corners, cut square at their ends.
            let t = arm(hw);
            let (a, b) = (base + n * half, base - n * half);
            let (ua, ub) = (unit(a - tip), unit(b - tip));
            // Inward (towards the axis) normals of the two arms.
            let (na, nb) = (Vec2::new(-ua.y, ua.x), Vec2::new(ub.y, -ub.x));
            let inner = tip - dir * (t * (a - tip).hypot() / half);
            polygon(&mut p, &[tip, a, a + na * t, inner, b + nb * t, b]);
        }
        Arrowhead::Circle => p.extend(kurbo::Circle::new(centre, half).path_elements(0.01)),
        Arrowhead::CircleOpen => {
            p.extend(kurbo::Circle::new(centre, half).path_elements(0.01));
            p.extend(kurbo::Circle::new(centre, half - ring(hw)).to_path(0.01).reverse_subpaths());
        }
        Arrowhead::Square => polygon(&mut p, &square),
        Arrowhead::SquareOpen => ring_polygon(&mut p, &square, ring(hw)),
        Arrowhead::Diamond => polygon(&mut p, &[tip, centre + n * half, base, centre - n * half]),
        Arrowhead::Bar => {
            let t = bar(hw);
            polygon(&mut p, &[tip + n * half, tip + n * half - dir * t, tip - n * half - dir * t, tip - n * half]);
        }
    }
    p
}

fn polygon(p: &mut BezPath, pts: &[Point]) {
    let Some((first, rest)) = pts.split_first() else { return };
    p.move_to(*first);
    for q in rest {
        p.line_to(*q);
    }
    p.close_path();
}

/// A convex polygon with a hole: the polygon inset by `wall`, wound the other way.
fn ring_polygon(p: &mut BezPath, pts: &[Point], wall: f64) {
    polygon(p, pts);
    let n = pts.len();
    // Signed area: which side of each edge is inside.
    let area: f64 = (0..n).map(|i| pts[i].to_vec2().cross(pts[(i + 1) % n].to_vec2())).sum();
    let inward = |e: Vec2| if area > 0.0 { Vec2::new(-e.y, e.x) } else { Vec2::new(e.y, -e.x) };
    let inner: Vec<Point> = (0..n)
        .map(|i| {
            let (prev, cur, next) = (pts[(i + n - 1) % n], pts[i], pts[(i + 1) % n]);
            let (n1, n2) = (inward(unit(cur - prev)), inward(unit(next - cur)));
            // The corner where both edges, moved in by `wall`, meet.
            cur + (n1 + n2) * (wall / (1.0 + n1.dot(n2)))
        })
        .rev()
        .collect();
    polygon(p, &inner);
}
