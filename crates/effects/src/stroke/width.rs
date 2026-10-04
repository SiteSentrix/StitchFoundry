//! Variable-width strokes (Width tool / width profiles).
//!
//! A stroke with a [`WidthProfile`] is drawn as a filled outline: each subpath is flattened, and
//! at every sample the left and right offsets `width/2 · factor(t)` (t = fraction of the
//! subpath's length) are placed along the (mitred) normal. Open subpaths get butt, round or
//! projecting caps (the round cap blends the two side widths); closed subpaths become two loops
//! of opposite orientation filled with the non-zero rule.

use kurbo::{BezPath, PathEl, Point, Vec2};
use vectorcraft_doc::{LineCap, WidthProfile};

use super::unit;

/// Filled outline of `bp` stroked with `width` scaled by `profile`, flattened to `tol`.
pub fn width_outline(bp: &BezPath, width: f64, profile: &WidthProfile, cap: LineCap, tol: f64) -> BezPath {
    let mut out = BezPath::new();
    for (pts, closed) in subpaths(bp, tol.max(1e-4)) {
        outline_subpath(&mut out, &pts, closed, width, profile, cap);
    }
    out
}

/// Flattened subpaths (consecutive duplicates removed; closed ones don't repeat the start).
fn subpaths(bp: &BezPath, tol: f64) -> Vec<(Vec<Point>, bool)> {
    let mut out: Vec<(Vec<Point>, bool)> = vec![];
    let mut cur: Vec<Point> = vec![];
    let push = |cur: &mut Vec<Point>, p: Point| {
        if cur.last().is_none_or(|l| l.distance(p) > 1e-9) {
            cur.push(p);
        }
    };
    kurbo::flatten(bp.iter(), tol, |el| match el {
        PathEl::MoveTo(p) => {
            if cur.len() > 1 {
                out.push((std::mem::take(&mut cur), false));
            }
            cur.clear();
            cur.push(p);
        }
        PathEl::LineTo(p) => push(&mut cur, p),
        PathEl::ClosePath => {
            if cur.len() > 1 && cur.first().unwrap().distance(*cur.last().unwrap()) <= 1e-9 {
                cur.pop();
            }
            if cur.len() > 1 {
                let start = cur[0];
                out.push((std::mem::take(&mut cur), true));
                cur.push(start);
            } else {
                cur.clear();
            }
        }
        _ => {}
    });
    if cur.len() > 1 {
        out.push((cur, false));
    }
    out
}

/// Left normal in y-down space (left of the direction of travel).
fn left(t: Vec2) -> Vec2 {
    Vec2::new(t.y, -t.x)
}

/// Insert samples where the profile has width points (so the piecewise-linear profile is exact
/// along straight runs).
fn densify(pts: &[Point], closed: bool, profile: &WidthProfile) -> Vec<Point> {
    let n = pts.len();
    let seg_count = if closed { n } else { n - 1 };
    let total: f64 = (0..seg_count).map(|i| pts[i].distance(pts[(i + 1) % n])).sum();
    if total <= 1e-12 {
        return pts.to_vec();
    }
    let mut out = Vec::with_capacity(n + profile.points.len());
    let mut acc = 0.0;
    for i in 0..seg_count {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let len = a.distance(b);
        out.push(a);
        for (t, _, _) in &profile.points {
            let d = t * total;
            if d > acc + 1e-9 && d < acc + len - 1e-9 {
                out.push(a.lerp(b, (d - acc) / len));
            }
        }
        acc += len;
    }
    if !closed {
        out.push(pts[n - 1]);
    }
    out
}

fn outline_subpath(out: &mut BezPath, pts: &[Point], closed: bool, width: f64, profile: &WidthProfile, cap: LineCap) {
    let pts = &densify(pts, closed, profile)[..];
    let n = pts.len();
    let seg_count = if closed { n } else { n - 1 };
    let seg_dir: Vec<Vec2> = (0..seg_count).map(|i| unit(pts[(i + 1) % n] - pts[i])).collect();
    let mut cum = vec![0.0; n + 1];
    for i in 0..seg_count {
        cum[i + 1] = cum[i] + pts[i].distance(pts[(i + 1) % n]);
    }
    let total = cum[seg_count];
    if total <= 1e-12 {
        return;
    }
    // Vertex normals (mitred, limited to 4× so sharp corners don't spike).
    let normal_at = |i: usize| -> (Vec2, f64) {
        let (a, b) = if closed {
            (seg_dir[(i + seg_count - 1) % seg_count], seg_dir[i % seg_count])
        } else if i == 0 {
            (seg_dir[0], seg_dir[0])
        } else if i == n - 1 {
            (seg_dir[seg_count - 1], seg_dir[seg_count - 1])
        } else {
            (seg_dir[i - 1], seg_dir[i])
        };
        let (na, nb) = (left(a), left(b));
        let nm = unit(na + nb);
        let c = nm.dot(nb).max(0.25);
        (nm, 1.0 / c)
    };
    let half = width / 2.0;
    let mut lpts = Vec::with_capacity(n);
    let mut rpts = Vec::with_capacity(n);
    for (i, p) in pts.iter().enumerate() {
        let (nm, miter) = normal_at(i);
        let (fl, fr) = profile.at(cum[i] / total);
        lpts.push(*p + nm * (miter * half * fl.max(0.0)));
        rpts.push(*p - nm * (miter * half * fr.max(0.0)));
    }
    if closed {
        polygon(out, lpts.iter().copied());
        polygon(out, rpts.iter().rev().copied());
        return;
    }
    let (t0, t1) = (seg_dir[0], seg_dir[seg_count - 1]);
    let (fl0, fr0) = profile.at(0.0);
    let (fl1, fr1) = profile.at(1.0);
    let (wl0, wr0, wl1, wr1) = (half * fl0.max(0.0), half * fr0.max(0.0), half * fl1.max(0.0), half * fr1.max(0.0));
    let mut ring: Vec<Point> = lpts.clone();
    // End cap: from the left side around the end to the right side.
    cap_points(&mut ring, pts[n - 1], left(t1), t1, wl1, wr1, cap);
    ring.extend(rpts.iter().rev());
    // Start cap: from the right side around the start back to the left side.
    cap_points(&mut ring, pts[0], -left(t0), -t0, wr0, wl0, cap);
    polygon(out, ring.into_iter());
}

/// Cap points between `c + u·r0` and `c − u·r1`, bulging towards `v`.
fn cap_points(ring: &mut Vec<Point>, c: Point, u: Vec2, v: Vec2, r0: f64, r1: f64, cap: LineCap) {
    match cap {
        LineCap::Butt => {}
        LineCap::Square => {
            let h = (r0 + r1) / 2.0;
            ring.push(c + u * r0 + v * h);
            ring.push(c - u * r1 + v * h);
        }
        LineCap::Round => {
            let steps = 16;
            for k in 1..steps {
                let a = std::f64::consts::PI * k as f64 / steps as f64;
                let r = r0 + (r1 - r0) * (k as f64 / steps as f64);
                ring.push(c + u * (a.cos() * r) + v * (a.sin() * r));
            }
        }
    }
}

fn polygon(out: &mut BezPath, mut pts: impl Iterator<Item = Point>) {
    let Some(first) = pts.next() else { return };
    out.move_to(first);
    for p in pts {
        out.line_to(p);
    }
    out.close_path();
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Shape;

    fn line() -> BezPath {
        let mut b = BezPath::new();
        b.move_to((0.0, 0.0));
        b.line_to((100.0, 0.0));
        b
    }

    fn area(b: &BezPath) -> f64 {
        b.area().abs()
    }

    #[test]
    fn uniform_profile_matches_a_plain_stroke() {
        let p = WidthProfile { points: vec![(0.0, 1.0, 1.0), (1.0, 1.0, 1.0)] };
        assert!((area(&width_outline(&line(), 10.0, &p, LineCap::Butt, 0.01)) - 1000.0).abs() < 1e-6);
        let round = area(&width_outline(&line(), 10.0, &p, LineCap::Round, 0.01));
        let want = 1000.0 + std::f64::consts::PI * 25.0;
        assert!((round - want).abs() / want < 0.01, "{round} vs {want}");
        assert!((area(&width_outline(&line(), 10.0, &p, LineCap::Square, 0.01)) - 1100.0).abs() < 1e-6);
    }

    #[test]
    fn lens_profile_is_a_diamond_and_one_sided_profiles_stay_on_their_side() {
        let a = area(&width_outline(&line(), 10.0, &WidthProfile::lens(), LineCap::Butt, 0.01));
        assert!((a - 500.0).abs() < 1e-6, "{a}");
        let one = WidthProfile { points: vec![(0.0, 1.0, 0.0), (1.0, 1.0, 0.0)] };
        let o = width_outline(&line(), 10.0, &one, LineCap::Butt, 0.01);
        assert!((area(&o) - 500.0).abs() < 1e-6);
        let bb = o.bounding_box();
        // Travelling +x, the left side is up (negative y).
        assert!(bb.y0 < -4.99 && bb.y1 <= 1e-9, "{bb:?}");
    }

    #[test]
    fn closed_paths_make_a_ring() {
        let mut sq = BezPath::new();
        sq.move_to((0.0, 0.0));
        sq.line_to((100.0, 0.0));
        sq.line_to((100.0, 100.0));
        sq.line_to((0.0, 100.0));
        sq.close_path();
        let p = WidthProfile { points: vec![(0.0, 1.0, 1.0), (1.0, 1.0, 1.0)] };
        let a = area(&width_outline(&sq, 10.0, &p, LineCap::Butt, 0.01));
        assert!((a - (110.0 * 110.0 - 90.0 * 90.0)).abs() < 1e-6, "{a}");
    }

    #[test]
    fn curves_are_offset_by_the_profile() {
        // A circle of radius 50 with a uniform 10 pt stroke: ring area 2π·50·10.
        let c = kurbo::Circle::new((0.0, 0.0), 50.0).to_path(1e-4);
        let p = WidthProfile { points: vec![(0.0, 1.0, 1.0), (1.0, 1.0, 1.0)] };
        let a = area(&width_outline(&c, 10.0, &p, LineCap::Butt, 0.001));
        let want = 2.0 * std::f64::consts::PI * 50.0 * 10.0;
        assert!((a - want).abs() / want < 0.01, "{a} vs {want}");
    }
}
