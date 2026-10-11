//! Line stitches along a path: single / triple run, backstitch, stem stitch,
//! blanket, zigzag and motifs, sewn once or in several passes.

use super::out::Out;
use crate::geom::{Contour, FLATTEN_TOLERANCE, Point, polyline};
use crate::model::{MAX_PASSES, Motif, RunKind, RunParams};
use crate::pattern::Stitch;

/// Turn angle (radians) above which a path vertex is treated as a corner that
/// must receive a needle penetration.
pub(crate) const CORNER_ANGLE: f64 = 0.35;

/// Prepares the path polyline: chooses the start nearest to `hint` (closed paths
/// start there, open paths are reversed if their far end is closer). Without a
/// hint, an open path ends near `exit`, or starts there when the stitching comes
/// back to its start (`back_to_start`: an even number of passes).
fn oriented_path(path: &Contour, hint: Option<Point>, exit: Option<Point>, back_to_start: bool) -> Vec<Point> {
    let pts = path.polyline(FLATTEN_TOLERANCE);
    if pts.len() < 2 {
        return pts;
    }
    if path.closed {
        let ring = match hint {
            Some(h) => polyline::ring_starting_near(&pts, h),
            None => pts,
        };
        polyline::closed(&ring)
    } else {
        let mut pts = pts;
        let (first, last) = (pts[0], pts[pts.len() - 1]);
        let reverse = match (hint, exit) {
            (Some(h), _) => h.distance(last) < h.distance(first),
            (None, Some(e)) if back_to_start => e.distance(last) < e.distance(first),
            (None, Some(e)) => e.distance(first) < e.distance(last),
            _ => false,
        };
        if reverse {
            pts.reverse();
        }
        pts
    }
}

pub(crate) fn generate(path: &Contour, params: &RunParams, hint: Option<Point>, exit: Option<Point>) -> Vec<Stitch> {
    let passes = params.passes.clamp(1, MAX_PASSES);
    let pts = oriented_path(path, hint, exit, passes.is_multiple_of(2));
    if pts.is_empty() {
        return Vec::new();
    }
    if pts.len() == 1 {
        return super::out::as_stitches([pts[0]]);
    }
    let step = params.stitch_length.max(0.3);
    let once = match params.kind {
        RunKind::Single => polyline::resample(&pts, step, CORNER_ANGLE),
        RunKind::Triple => triple(&polyline::resample(&pts, step, CORNER_ANGLE)),
        RunKind::Blanket => blanket(&pts, step, params.width, params.spacing, params.flip),
        RunKind::Zigzag => zigzag(&pts, params.width, params.spacing),
        RunKind::Motif => motif(&pts, params.motif, params.width, params.spacing, step, params.flip),
        RunKind::Back => backstitch(&pts, step, 0.0),
        RunKind::Stem => backstitch(&pts, step, (params.width * 0.5).clamp(0.2, 3.0) * if params.flip { -1.0 } else { 1.0 }),
    };
    let mut out = Out::new();
    out.stitches(repeat_passes(&once, passes));
    out.finish()
}

/// The stitching sewn `passes` times, every other pass backwards over the same
/// needle points (2 passes = a backtrack to the start).
fn repeat_passes(once: &[Point], passes: u32) -> Vec<Point> {
    let mut out = once.to_vec();
    for k in 1..passes {
        if k % 2 == 1 {
            out.extend(once.iter().rev().skip(1));
        } else {
            out.extend(once.iter().skip(1));
        }
    }
    out
}

/// Backstitch: needle points every half stitch along the line; each stitch goes
/// forward two of them and back one, so consecutive stitches overlap by half.
/// With `slant` ≠ 0 (stem stitch), the back points sit that far to one side of
/// the line, so the overlapping stitches lie slanted like a twisted rope.
pub(crate) fn backstitch(pts: &[Point], step: f64, slant: f64) -> Vec<Point> {
    let q = polyline::resample(pts, step * 0.5, CORNER_ANGLE);
    let n = q.len();
    if n < 3 {
        return q;
    }
    let normals = polyline::vertex_normals(&q, false);
    let mut out = Vec::with_capacity(n * 2);
    out.push(q[0]);
    let mut i = 0;
    while i + 2 < n {
        out.push(q[i + 2]);
        if i + 3 < n {
            let back = q[i + 1] + normals.get(i + 1).copied().unwrap_or(Point::ZERO) * slant;
            out.push(back);
        }
        i += 1;
    }
    out
}

/// Bean stitch: every stitch is sewn forward, back and forward again.
pub(crate) fn triple(pts: &[Point]) -> Vec<Point> {
    let mut out = Vec::with_capacity(pts.len() * 3);
    if let Some(&first) = pts.first() {
        out.push(first);
    }
    for w in pts.windows(2) {
        out.push(w[1]);
        out.push(w[0]);
        out.push(w[1]);
    }
    out
}

/// Frame (point, unit normal) at arc length `s` along the polyline.
fn frame(pts: &[Point], s: f64) -> (Point, Point) {
    (polyline::point_at(pts, s), polyline::tangent_at(pts, s).perp())
}

/// Blanket / E-stitch: running stitch with prongs every `spacing` mm.
pub(crate) fn blanket(pts: &[Point], step: f64, prong: f64, spacing: f64, flip: bool) -> Vec<Point> {
    let side = if flip { -1.0 } else { 1.0 };
    let anchors = polyline::resample(pts, spacing.max(0.5), CORNER_ANGLE);
    let normals = polyline::vertex_normals(&anchors, false);
    let mut out = Vec::new();
    for (i, (&a, &n)) in anchors.iter().zip(&normals).enumerate() {
        if i > 0 {
            // base line from the previous anchor, subdivided if long
            let prev = anchors[i - 1];
            let k = ((prev.distance(a) / step).ceil() as usize).max(1);
            for j in 1..=k {
                out.push(prev.lerp(a, j as f64 / k as f64));
            }
        } else {
            out.push(a);
        }
        out.push(a + n * (prong * side));
        out.push(a);
    }
    out
}

/// Open zigzag of total `width` with peaks every `spacing` mm.
pub(crate) fn zigzag(pts: &[Point], width: f64, spacing: f64) -> Vec<Point> {
    let len = polyline::length(pts);
    let half = (spacing.max(0.2)) / 2.0;
    let n = ((len / half).ceil() as usize).max(1);
    let mut out = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let s = len * k as f64 / n as f64;
        let (p, nrm) = frame(pts, s);
        let side = if k % 2 == 0 { 0.5 } else { -0.5 };
        out.push(p + nrm * (width * side));
    }
    out
}

/// Control points of one motif repeat in (along, across) units, starting and
/// ending on the line. `along` is a fraction of the repeat length, `across` a
/// fraction of the motif width.
fn motif_shape(m: Motif) -> Vec<(f64, f64)> {
    match m {
        Motif::Wave => (0..=8)
            .map(|i| {
                let t = i as f64 / 8.0;
                (t, 0.5 * (t * std::f64::consts::TAU).sin())
            })
            .collect(),
        Motif::Scallop => (0..=8)
            .map(|i| {
                let t = i as f64 / 8.0;
                (t, (t * std::f64::consts::PI).sin())
            })
            .collect(),
        Motif::Diamond => vec![(0.0, 0.0), (0.5, 0.5), (1.0, 0.0), (0.5, -0.5), (0.0, 0.0), (1.0, 0.0)],
        Motif::Cross => vec![(0.0, 0.0), (0.0, -0.5), (1.0, 0.5), (1.0, -0.5), (0.0, 0.5), (0.0, 0.0), (1.0, 0.0)],
        Motif::Feather => vec![(0.0, 0.0), (0.3, 0.5), (0.5, 0.0), (0.8, -0.5), (1.0, 0.0)],
    }
}

/// Repeating decorative motif along the path.
pub(crate) fn motif(pts: &[Point], m: Motif, width: f64, spacing: f64, step: f64, flip: bool) -> Vec<Point> {
    let len = polyline::length(pts);
    let rep = spacing.max(0.5);
    let n = ((len / rep).round() as usize).max(1);
    let rep = len / n as f64;
    let side = if flip { -1.0 } else { 1.0 };
    let shape = motif_shape(m);
    let mut out: Vec<Point> = Vec::new();
    for k in 0..n {
        let s0 = k as f64 * rep;
        for &(a, c) in &shape {
            let (p, nrm) = frame(pts, (s0 + a * rep).min(len));
            let q = p + nrm * (c * width * side);
            if let Some(&last) = out.last() {
                let d: f64 = last.distance(q);
                let parts = ((d / step).ceil() as usize).max(1);
                for j in 1..=parts {
                    out.push(last.lerp(q, j as f64 / parts as f64));
                }
            } else {
                out.push(q);
            }
        }
    }
    polyline::dedup(&out, 1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::Cmd;

    fn line() -> Contour {
        Contour::from_points(&[Point::new(0.0, 0.0), Point::new(20.0, 0.0)], false)
    }

    #[test]
    fn single_run_lengths() {
        let st = generate(&line(), &RunParams::default(), None, None);
        assert_eq!(st.len(), 9); // 20 / 2.5 = 8 stitches
        assert!(st.iter().all(|s| s.cmd == Cmd::Stitch));
    }

    #[test]
    fn hint_reverses_open_path() {
        let st = generate(&line(), &RunParams::default(), Some(Point::new(25.0, 0.0)), None);
        assert!(st[0].pos().approx_eq(Point::new(20.0, 0.0), 1e-9));
    }

    #[test]
    fn closed_run_starts_near_hint() {
        let sq = Contour::from_points(&[Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0)], true);
        let st = generate(&sq, &RunParams::default(), Some(Point::new(5.0, 11.0)), None);
        assert!(st[0].pos().approx_eq(Point::new(5.0, 10.0), 1e-9));
        assert!(st.last().unwrap().pos().approx_eq(Point::new(5.0, 10.0), 1e-9));
    }

    #[test]
    fn backstitch_goes_forward_two_and_back_one() {
        let p = RunParams { kind: RunKind::Back, stitch_length: 2.0, ..Default::default() };
        let st = generate(&line(), &p, None, None);
        let xs: Vec<f64> = st.iter().map(|s| s.x).collect();
        // needle points every 1 mm: 0 → 2 → 1 → 3 → 2 → 4 …, ending at 20
        assert_eq!(&xs[..6], &[0.0, 2.0, 1.0, 3.0, 2.0, 4.0]);
        assert_eq!(*xs.last().unwrap(), 20.0);
        for w in xs.windows(2) {
            assert!((w[1] - w[0]).abs() <= 2.0 + 1e-9);
        }
        // stem stitch: the back points are slanted off the line, all on one side
        let p = RunParams { kind: RunKind::Stem, stitch_length: 2.0, width: 2.0, ..Default::default() };
        let st = generate(&line(), &p, None, None);
        let off: Vec<f64> = st.iter().map(|s| s.y).filter(|y| y.abs() > 1e-9).collect();
        assert!(off.len() > 10 && off.iter().all(|y| (y.abs() - 1.0).abs() < 1e-9 && y.signum() == off[0].signum()), "{off:?}");
        let flipped = generate(&line(), &RunParams { flip: true, ..p }, None, None);
        assert!(flipped.iter().any(|s| s.y.signum() == -off[0].signum() && s.y.abs() > 0.5));
    }

    #[test]
    fn passes_backtrack_and_repeat() {
        let once = generate(&line(), &RunParams::default(), None, None);
        let back = generate(&line(), &RunParams { passes: 2, ..Default::default() }, None, None);
        assert_eq!(back.len(), once.len() * 2 - 1);
        assert!(back.last().unwrap().pos().approx_eq(back[0].pos(), 1e-9), "a backtrack ends at the start");
        // the way back uses the same needle points
        for (a, b) in once.iter().zip(back.iter().rev()) {
            assert!(a.pos().approx_eq(b.pos(), 1e-9));
        }
        let three = generate(&line(), &RunParams { passes: 3, ..Default::default() }, None, None);
        assert_eq!(three.len(), once.len() * 3 - 2);
        assert!(three.last().unwrap().pos().approx_eq(Point::new(20.0, 0.0), 1e-9));
        // too many passes are capped
        let many = generate(&line(), &RunParams { passes: 500, ..Default::default() }, None, None);
        assert_eq!(many.len(), (once.len() - 1) * MAX_PASSES as usize + 1);
        // asked to end near x = 20 with a backtrack: starts there and comes back
        let st = generate(&line(), &RunParams { passes: 2, ..Default::default() }, None, Some(Point::new(21.0, 0.0)));
        assert!(st[0].pos().approx_eq(Point::new(20.0, 0.0), 1e-9));
    }

    #[test]
    fn triple_and_styles() {
        let p = RunParams { kind: RunKind::Triple, ..Default::default() };
        let st = generate(&line(), &p, None, None);
        assert_eq!(st.len(), 1 + 8 * 3);
        for kind in RunKind::ALL {
            for m in Motif::ALL {
                let p = RunParams { kind, motif: m, ..Default::default() };
                let st = generate(&line(), &p, None, None);
                assert!(!st.is_empty());
                for w in st.windows(2) {
                    assert!(w[0].pos().distance(w[1].pos()) < 6.0, "{kind:?} {m:?}");
                }
            }
        }
    }
}
