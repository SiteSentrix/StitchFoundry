//! Line stitches along a path: single / triple run, backstitch, stem stitch,
//! blanket, zigzag and motifs, sewn once or in several passes.

use super::out::Out;
use crate::geom::{Contour, FLATTEN_TOLERANCE, Point, polyline};
use crate::model::{MAX_PASSES, Motif, RunKind, RunParams};
use crate::pattern::Stitch;

/// Turn angle (radians) above which a path vertex is treated as a corner that
/// must receive a needle penetration.
pub(crate) const CORNER_ANGLE: f64 = 0.35;

/// Prepares the path polyline and whether to sew it backwards: closed paths start
/// nearest to `hint`; open paths keep their drawn direction and are sewn backwards
/// if their far end is closer to it. Without a hint, an open path ends near `exit`,
/// or starts there when the stitching comes back to its start (`back_to_start`: an
/// even number of passes).
fn oriented_path(path: &Contour, hint: Option<Point>, exit: Option<Point>, back_to_start: bool) -> (Vec<Point>, bool) {
    let pts = path.polyline(FLATTEN_TOLERANCE);
    if pts.len() < 2 {
        return (pts, false);
    }
    if path.closed {
        let ring = match hint {
            Some(h) => polyline::ring_starting_near(&pts, h),
            None => pts,
        };
        (polyline::closed(&ring), false)
    } else {
        let (first, last) = (pts[0], pts[pts.len() - 1]);
        let reverse = match (hint, exit) {
            (Some(h), _) => h.distance(last) < h.distance(first),
            (None, Some(e)) if back_to_start => e.distance(last) < e.distance(first),
            (None, Some(e)) => e.distance(first) < e.distance(last),
            _ => false,
        };
        (pts, reverse)
    }
}

pub(crate) fn generate(path: &Contour, params: &RunParams, hint: Option<Point>, exit: Option<Point>) -> Vec<Stitch> {
    let passes = params.passes.clamp(1, MAX_PASSES);
    // Stitches are laid out along the path as drawn (so prongs, motifs and their grading stay
    // on their side and in their order), then sewn backwards when it starts at the far end.
    let (pts, backwards) = oriented_path(path, hint, exit, passes.is_multiple_of(2));
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
        RunKind::Motif => motif(&pts, params.motif, (params.width, params.width_end), params.spacing, step, params.flip),
        RunKind::Back => backstitch(&pts, step, 0.0),
        RunKind::Stem => backstitch(&pts, step, (params.width * 0.5).clamp(0.2, 3.0) * if params.flip { -1.0 } else { 1.0 }),
        RunKind::Hand => hand(&pts, step, params.spacing),
    };
    let mut once = once;
    if backwards {
        once.reverse();
    }
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

/// Most stitches a hand stitch run is cut into.
const MAX_HAND_STITCHES: usize = 100_000;

/// Hand stitch: stitches about `step` long, each sewn three times (forward, back and forward)
/// so it lies thick, joined by single stitches about `gap` long that hardly show, every length
/// varied a little at random (the same way each time), like a running stitch sewn by hand.
/// The line starts and ends with a thick stitch.
pub(crate) fn hand(pts: &[Point], step: f64, gap: f64) -> Vec<Point> {
    let total = polyline::length(pts);
    let Some(&first) = pts.first() else { return Vec::new() };
    if !(total.is_finite() && total > 1e-6) {
        return pts.to_vec();
    }
    let step = step.clamp(0.5, 12.0);
    let gap = if gap.is_finite() { gap.clamp(0.3, 2.0 * step) } else { step * 0.5 };
    // n thick stitches and n − 1 gaps, scaled to the line's length.
    let n = ((total + gap) / (step + gap)).round().clamp(1.0, MAX_HAND_STITCHES as f64) as usize;
    let mut rng = super::special::Rng::new(total.to_bits() ^ first.x.to_bits().rotate_left(17) ^ first.y.to_bits().rotate_left(41));
    let lens: Vec<f64> = (0..2 * n - 1)
        .map(|i| {
            let base = if i % 2 == 0 { step } else { gap };
            base * (0.8 + 0.4 * rng.unit())
        })
        .collect();
    let k = total / lens.iter().sum::<f64>();
    let mut out = Vec::with_capacity(4 * n);
    out.push(first);
    let mut s = 0.0;
    for (i, len) in lens.iter().enumerate() {
        let from = s;
        s = (s + len * k).min(total);
        let to = polyline::point_at(pts, s);
        if i % 2 == 0 {
            out.extend([to, polyline::point_at(pts, from), to]);
        } else {
            out.push(to);
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
/// A motif's line in a repeat one long (along the run) and one wide (across it), from `(0, 0)`
/// to `(1, 0)` so repeats join.
pub(crate) fn motif_shape(m: Motif) -> Vec<(f64, f64)> {
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
        // A heart standing on its point, reached from the line and left back to it.
        Motif::Heart => vec![
            (0.0, 0.0),
            (0.5, -0.45),
            (0.15, 0.05),
            (0.13, 0.25),
            (0.22, 0.4),
            (0.36, 0.43),
            (0.5, 0.3),
            (0.64, 0.43),
            (0.78, 0.4),
            (0.87, 0.25),
            (0.85, 0.05),
            (0.5, -0.45),
            (1.0, 0.0),
        ],
        // A five-pointed star round the middle of the repeat.
        Motif::Star => {
            let mut v = vec![(0.0, 0.0)];
            v.extend((0..=10).map(|k| {
                let r = if k % 2 == 0 { 0.48 } else { 0.2 };
                let a = std::f64::consts::FRAC_PI_2 + f64::from(k) * std::f64::consts::PI / 5.0;
                (0.5 + r * a.cos(), r * a.sin())
            }));
            v.push((1.0, 0.0));
            v
        }
        // A leaf: its upper edge, its lower edge back, then the vein.
        Motif::Leaf => {
            let edge = |side: f64| (0..=8).map(move |i| f64::from(i) / 8.0).map(move |t| (t, side * 0.4 * (t * std::f64::consts::PI).sin()));
            let mut v: Vec<(f64, f64)> = edge(1.0).collect();
            v.extend(edge(-1.0).collect::<Vec<_>>().into_iter().rev().skip(1));
            v.push((1.0, 0.0));
            v
        }
        // A loop like a handwritten l.
        Motif::Loop => {
            let mut v = vec![(0.0, 0.0)];
            v.extend((0..=12).map(|i| {
                let a = f64::from(i) / 12.0 * std::f64::consts::TAU;
                (0.55 + 0.25 * a.sin(), 0.25 - 0.25 * a.cos())
            }));
            v.push((1.0, 0.0));
            v
        }
    }
}

/// Most motif repeats along one line.
const MAX_MOTIFS: usize = 50_000;

/// Repeating decorative motif along the path, `widths.0` wide; with `widths.1`, the motifs
/// grow or shrink to that width at the end (graded), their repeats `spacing` long at the start
/// and keeping their proportions.
pub(crate) fn motif(pts: &[Point], m: Motif, widths: (f64, Option<f64>), spacing: f64, step: f64, flip: bool) -> Vec<Point> {
    let len = polyline::length(pts);
    let width = widths.0;
    let end = widths.1.filter(|w| w.is_finite() && *w > 0.0 && width > 1e-9);
    let rep = spacing.max(0.5);
    // Where each repeat starts along the line, and its width.
    let mut repeats: Vec<(f64, f64, f64)> = Vec::new();
    match end {
        None => {
            let n = ((len / rep).round() as usize).clamp(1, MAX_MOTIFS);
            let r = len / n as f64;
            repeats.extend((0..n).map(|k| (k as f64 * r, r, width)));
        }
        Some(w1) => {
            // Lay repeats out at their own sizes, then scale them all to fit the line.
            let mut s = 0.0;
            let mut sizes = Vec::new();
            while s < len && sizes.len() < MAX_MOTIFS {
                let w = width + (w1 - width) * (s / len).clamp(0.0, 1.0);
                let r = (rep * w / width).max(0.5);
                sizes.push((r, w));
                s += r;
            }
            let k = len / sizes.iter().map(|x| x.0).sum::<f64>().max(1e-9);
            let mut at = 0.0;
            for (r, w) in sizes {
                repeats.push((at, r * k, w));
                at += r * k;
            }
        }
    }
    let side = if flip { -1.0 } else { 1.0 };
    let shape = motif_shape(m);
    let mut out: Vec<Point> = Vec::new();
    for (s0, rep, width) in repeats {
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
    fn every_motif_joins_its_repeats_and_graded_runs_change_size() {
        for m in Motif::ALL {
            let shape = motif_shape(m);
            let near = |p: Option<&(f64, f64)>, q: (f64, f64)| p.is_some_and(|p| (p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9);
            assert!(near(shape.first(), (0.0, 0.0)) && near(shape.last(), (1.0, 0.0)), "{m:?}: {shape:?}");
            assert!(shape.iter().all(|(a, c)| (-0.01..=1.01).contains(a) && c.abs() <= 1.0 + 1e-9), "{m:?}");
        }
        // Scallops 2 mm high at the start of a 100 mm line, 6 mm at its end.
        let line = Contour::from_points(&[Point::new(0.0, 0.0), Point::new(100.0, 0.0)], false);
        let p = RunParams { kind: RunKind::Motif, motif: Motif::Scallop, width: 2.0, width_end: Some(6.0), spacing: 4.0, ..Default::default() };
        let pts: Vec<Point> = generate(&line, &p, None, None).iter().map(|s| s.pos()).collect();
        let height = |lo: f64, hi: f64| pts.iter().filter(|q| q.x >= lo && q.x <= hi).map(|q| q.y.abs()).fold(0.0, f64::max);
        assert!(height(0.0, 15.0) < 2.7 && height(85.0, 100.0) > 5.0, "{} {}", height(0.0, 15.0), height(85.0, 100.0));
        assert!(pts.last().unwrap().distance(Point::new(100.0, 0.0)) < 1e-6, "ends on the line's end");
        // Sewn from the far end (the closest join), the motifs still grow along the line as drawn.
        let back: Vec<Point> = generate(&line, &p, Some(Point::new(101.0, 0.0)), None).iter().map(|s| s.pos()).collect();
        assert!(back.first().unwrap().distance(Point::new(100.0, 0.0)) < 1e-6, "starts at the far end");
        let rev: Vec<Point> = pts.iter().rev().copied().collect();
        assert_eq!(back, rev);
        // Without an end width every motif is the same; absurd ones change nothing.
        let even = RunParams { width_end: None, ..p.clone() };
        for w in [f64::NAN, -1.0, 0.0] {
            let odd = RunParams { width_end: Some(w), ..p.clone() };
            assert_eq!(generate(&line, &odd, None, None), generate(&line, &even, None, None), "{w}");
        }
    }

    #[test]
    fn hand_stitches_are_thick_dashes_joined_by_short_single_stitches() {
        let p = RunParams { kind: RunKind::Hand, stitch_length: 3.0, spacing: 1.0, ..Default::default() };
        let pts: Vec<Point> = generate(&line(), &p, None, None).iter().map(|s| s.pos()).collect();
        // first, then (to, back, to) for each thick stitch and one point per gap
        assert_eq!((pts.len() - 1) % 4, 3, "{pts:?}");
        assert_eq!(pts.first(), Some(&Point::new(0.0, 0.0)));
        assert!(pts.last().unwrap().distance(Point::new(20.0, 0.0)) < 1e-9, "ends on the line's end");
        let mut thick = Vec::new();
        let mut gaps = Vec::new();
        let mut i = 0;
        while i + 3 < pts.len() {
            let (a, b) = (pts[i], pts[i + 1]);
            assert!(pts[i + 2].distance(a) < 1e-9 && pts[i + 3].distance(b) < 1e-9, "sewn forward, back and forward");
            thick.push(a.distance(b));
            if let Some(g) = pts.get(i + 4) {
                gaps.push(b.distance(*g));
            }
            i += 4;
        }
        assert_eq!(thick.len(), 5, "20 mm of 3 mm stitches and 1 mm gaps: {thick:?}");
        assert!(thick.iter().all(|l| (2.0..=4.2).contains(l)) && gaps.iter().all(|g| (0.6..=1.4).contains(g)), "{thick:?} {gaps:?}");
        assert!(thick.windows(2).any(|w| (w[0] - w[1]).abs() > 0.05), "the lengths vary: {thick:?}");
        // Short lines are one thick stitch; the same line comes out the same.
        let short = Contour::from_points(&[Point::new(0.0, 0.0), Point::new(1.0, 0.0)], false);
        assert_eq!(generate(&short, &p, None, None).len(), 4);
        assert_eq!(generate(&line(), &p, None, None), generate(&line(), &p, None, None));
        for (len, gap) in [(f64::NAN, f64::NAN), (1e308, -1.0), (0.0, 1e308)] {
            let p = RunParams { kind: RunKind::Hand, stitch_length: len, spacing: gap, ..Default::default() };
            let st = generate(&line(), &p, None, None);
            assert!(st.len() >= 4 && st.iter().all(|s| s.pos().is_finite()), "{len} {gap}");
        }
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
