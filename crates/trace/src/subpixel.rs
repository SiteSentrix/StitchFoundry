//! Outlines placed to a fraction of a pixel.
//!
//! A boundary loop runs along the cracks between pixels, so on its own it is a staircase that is
//! right only to half a pixel. But an anti-aliased or slightly blurred image says where the edge
//! really is: the pixels beside it are part one colour and part the other. Each crack between an
//! inside and an outside pixel is moved along its normal by what the four pixels across it say (see
//! [`Crack`]), which gives one point per crack; a corner of the loop that is a true corner of the
//! shape (not a step of a staircase) is kept, moved with the two edges that meet there. The same
//! crack gives the same point from either side, so two regions that touch get the same points along
//! their shared edge.
//!
//! An image with hard edges (a GIF, a 1-bit scan) says nothing more than the cracks do: every crack
//! moves by the same amount. There the staircase is read the classic way: a one-pixel step between
//! two runs is a point of a sloped edge (its middle is on the edge, the corners beside it are not).

use kurbo::Vec2;
use vectorcraft_geom::Point;

use crate::contour::Loop;

/// One straight run of a loop, from one of its vertices to the next.
struct Edge {
    dir: (i32, i32),
    len: usize,
    /// Index of its first crack's shift.
    first: usize,
}

/// A crack between an inside and an outside pixel of a `w × h` image, with the column of pixels
/// across it. Read across the edge, the column says where the edge lies: a straight edge that
/// crosses it leaves in its pixels, summed, exactly as much of the inside colour as its distance
/// along the column (which is how anti-aliasing draws it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Crack {
    inside: (i32, i32),
    /// From the inside pixel to the outside one.
    step: (i32, i32),
    w: usize,
    h: usize,
}

impl Crack {
    /// The index of the pixel `k` steps out from the inside pixel (0: the inside pixel, 1: the
    /// outside one, −1: the one behind the inside pixel), if the image has it.
    pub fn at(&self, k: i32) -> Option<usize> {
        pixel((self.inside.0 + k * self.step.0, self.inside.1 + k * self.step.1), self.w, self.h)
    }
}

fn pixel(p: (i32, i32), w: usize, h: usize) -> Option<usize> {
    let (x, y) = (usize::try_from(p.0).ok()?, usize::try_from(p.1).ok()?);
    (x < w && y < h).then(|| y * w + x)
}

/// The outline of `l` (a loop of a `w × h` image) as points, and whether its edge is hard.
///
/// `shift` says how far (in pixels, −0.5 to 0.5, positive towards the outside pixel) the edge lies
/// from a crack.
pub(crate) fn outline(l: &Loop, w: usize, h: usize, shift: &dyn Fn(Crack) -> f64) -> (Vec<Point>, bool) {
    let v = &l.pts;
    let n = v.len();
    let at = |i: usize| Point::new(f64::from(v[i % n].0), f64::from(v[i % n].1));
    let raw = || (0..n).map(at).collect::<Vec<_>>();
    if n < 4 {
        return (raw(), true);
    }
    let mut edges = Vec::with_capacity(n);
    let mut shifts: Vec<f64> = Vec::new();
    for i in 0..n {
        let (a, b) = (v[i], v[(i + 1) % n]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let dir = (dx.signum(), dy.signum());
        let len = dx.unsigned_abs().max(dy.unsigned_abs());
        edges.push(Edge { dir, len: len as usize, first: shifts.len() });
        for k in 0..len as i32 {
            // The crack starts at corner `s`; the inside pixel is on its right (y down).
            let (sx, sy) = (a.0 + dir.0 * k, a.1 + dir.1 * k);
            let (inside, outside) = match dir {
                (1, 0) => ((sx, sy), (sx, sy - 1)),
                (0, 1) => ((sx - 1, sy), (sx, sy)),
                (-1, 0) => ((sx - 1, sy - 1), (sx - 1, sy)),
                _ => ((sx, sy - 1), (sx - 1, sy - 1)),
            };
            let crack = Crack { inside, step: (outside.0 - inside.0, outside.1 - inside.1), w, h };
            let s = match crack.at(1) {
                Some(_) => shift(crack),
                // The edge of the image is where it is.
                _ => 0.0,
            };
            shifts.push(if s.is_finite() { s.clamp(-0.5, 0.5) } else { 0.0 });
        }
    }
    // A hard edge: nearly every crack moves by the same amount.
    let mut sorted = shifts.clone();
    let mid = sorted.len() / 2;
    let median = if sorted.is_empty() { 0.0 } else { *sorted.select_nth_unstable_by(mid, f64::total_cmp).1 };
    let hard = shifts.iter().filter(|s| (*s - median).abs() > 0.03).count() * 4 < shifts.len();

    // Which way the loop turns at vertex `i`, and whether edge `i` is a step of a staircase: one
    // pixel long, turning one way where it starts and the other where it ends.
    let turn = |i: usize| {
        let (a, b) = (edges[(i + n - 1) % n].dir, edges[i % n].dir);
        (a.0 * b.1 - a.1 * b.0).signum()
    };
    let step: Vec<bool> = (0..n).map(|i| edges[i].len == 1 && turn(i) * turn(i + 1) < 0).collect();
    let outward = |d: (i32, i32)| Vec2::new(f64::from(d.1), f64::from(-d.0));
    let shift_of = |e: &Edge, k: usize| shifts.get(e.first + k).copied().unwrap_or(0.0);

    let mut out = Vec::with_capacity(if hard { n } else { shifts.len() + n });
    for i in 0..n {
        let (before, e) = (&edges[(i + n - 1) % n], &edges[i]);
        if !step[i] && !step[(i + n - 1) % n] {
            // A true corner: where its two edges, each moved by its own shift, meet.
            out.push(at(i) + outward(before.dir) * shift_of(before, before.len.saturating_sub(1)) + outward(e.dir) * shift_of(e, 0));
        }
        if step[i] || !hard {
            let along = Vec2::new(f64::from(e.dir.0), f64::from(e.dir.1));
            out.extend((0..e.len).map(|k| at(i) + along * (k as f64 + 0.5) + outward(e.dir) * shift_of(e, k)));
        }
    }
    if out.len() < 3 { (raw(), hard) } else { (out, hard) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contour::trace_mask;

    fn loop_of(mask: &[bool], w: usize, h: usize) -> Loop {
        trace_mask(mask, w, h).remove(0).outer
    }

    #[test]
    fn a_hard_rectangle_keeps_its_four_corners() {
        let (w, h) = (8, 6);
        let mask: Vec<bool> = (0..w * h).map(|i| (2..6).contains(&(i % w)) && (1..5).contains(&(i / w))).collect();
        let (pts, hard) = outline(&loop_of(&mask, w, h), w, h, &|_| 0.0);
        assert!(hard);
        assert_eq!(pts, [(2.0, 1.0), (6.0, 1.0), (6.0, 5.0), (2.0, 5.0)].map(Point::from));
    }

    #[test]
    fn a_hard_staircase_becomes_the_sloped_line_through_its_steps() {
        // Below the line y = x / 4 + 2: treads four pixels long.
        let (w, h) = (16, 8);
        let mask: Vec<bool> = (0..w * h).map(|i| (i / w) as f64 + 0.5 > (i % w) as f64 / 4.0 + 2.0).collect();
        let (pts, hard) = outline(&loop_of(&mask, w, h), w, h, &|_| 0.0);
        assert!(hard);
        // Every step's middle lies on one line, a quarter of a pixel's rise per pixel.
        let steps: Vec<Point> = pts.iter().copied().filter(|p| p.y.fract() == 0.5 && p.x > 0.0 && p.x < w as f64).collect();
        assert_eq!(steps.len(), 4, "{pts:?}");
        assert!(steps.windows(2).all(|s| ((s[1].y - s[0].y) / (s[1].x - s[0].x) - 0.25).abs() < 1e-9), "{steps:?}");
    }

    #[test]
    fn a_soft_edge_moves_each_crack_by_its_shift() {
        let (w, h) = (6, 4);
        // The left half is inside; its right edge really lies 0.3 px further right.
        let mask: Vec<bool> = (0..w * h).map(|i| i % w < 3).collect();
        let shift = |c: Crack| if c.at(0).is_some_and(|i| i % w == 2) && c.at(1).is_some_and(|i| i % w == 3) { 0.3 } else { 0.0 };
        let l = loop_of(&mask, w, h);
        let (pts, hard) = outline(&l, w, h, &shift);
        assert!(!hard);
        // Each crack comes with the column of pixels across it.
        outline(&l, w, h, &|c| {
            let (a, b) = (c.at(0).unwrap(), c.at(1).unwrap());
            assert_eq!((c.at(-1), c.at(2)), (Some(a - 1), Some(b + 1)));
            assert_eq!((c.at(-3), c.at(4)), (None, None));
            0.0
        });
        let right: Vec<&Point> = pts.iter().filter(|p| p.x > 2.5).collect();
        // The four cracks' middles and the two corners, all at x = 3.3.
        assert_eq!(right.len(), 6);
        assert!(right.iter().all(|p| (p.x - 3.3).abs() < 1e-12), "{right:?}");
    }

    #[test]
    fn a_shift_that_is_not_a_number_moves_nothing() {
        let mask = [true, true, false, false, true, true, false, false];
        let (pts, _) = outline(&loop_of(&mask, 4, 2), 4, 2, &|_| f64::NAN);
        assert_eq!(pts.len(), 4);
        assert!(pts.iter().all(|p| p.x.is_finite() && p.y.is_finite()));
    }
}
