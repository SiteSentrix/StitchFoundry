//! Dash patterns. Dashes follow the path's arc length, restart on every subpath and join across
//! the start of a closed subpath. A dash of zero length is a [`Dot`]: with a round cap it paints
//! a disc, with a projecting cap a square turned along the path, with a butt cap nothing.

use kurbo::{BezPath, ParamCurve, ParamCurveArclen, PathSeg, Point, Shape, Vec2};
use vectorcraft_doc::{Dash, LineCap};

use super::{ARCLEN_ACCURACY, push_seg, segments, subpaths, tangent};

/// More dashes than this (a tiny pattern on a long path) draws the line solid instead.
const MAX_DASHES: f64 = 200_000.0;
/// A pattern whose longest entry is shorter than this can't advance along the path (each entry
/// ends within the generator's tolerance of where it starts), so it draws the line solid.
const MIN_ENTRY: f64 = 1e-6;

/// A zero-length dash: where it sits and the path's direction there.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Dot {
    pub at: Point,
    pub dir: Vec2,
}

/// A dashed path: the dashes (open subpaths to stroke without a dash pattern) and the dots.
#[derive(Clone, Debug, Default)]
pub struct Dashed {
    pub path: BezPath,
    pub dots: Vec<Dot>,
}

/// Apply dash pattern `d` to `bp`. `None` when the pattern is invalid (a negative or non-finite
/// entry, as in SVG and PDF) or has no positive length (the line is solid), or is too fine to draw
/// (then the line is drawn solid too).
pub fn dash(bp: &BezPath, d: &Dash) -> Option<Dashed> {
    if d.pattern.iter().any(|v| !v.is_finite() || *v < 0.0) || !d.offset.is_finite() {
        return None;
    }
    let mut pat = d.pattern.clone();
    if !pat.iter().any(|v| *v > MIN_ENTRY) {
        return None;
    }
    // An odd pattern repeats with dashes and gaps swapped (as in SVG and PDF).
    if pat.len() % 2 == 1 {
        pat.extend_from_within(..);
    }
    let period: f64 = pat.iter().sum();
    let subs: Vec<(Vec<PathSeg>, bool)> = subpaths(bp).into_iter().map(|(r, closed)| (segments(&bp.elements()[r]), closed)).collect();
    let lens: Vec<Vec<f64>> = subs.iter().map(|(segs, _)| segs.iter().map(|s| s.arclen(ARCLEN_ACCURACY)).collect()).collect();
    let total: f64 = lens.iter().flatten().sum();
    if total / period * pat.len() as f64 > MAX_DASHES {
        return None;
    }
    // Where the pattern starts: `offset` into it.
    let mut off = d.offset.rem_euclid(period);
    let mut ix0 = 0;
    while off > pat[ix0] {
        off -= pat[ix0];
        ix0 = (ix0 + 1) % pat.len();
    }
    let phase = Phase { ix: ix0, rem: pat[ix0] - off, on: ix0 % 2 == 0 };
    let mut out = Dashed::default();
    for ((segs, closed), lens) in subs.iter().zip(&lens) {
        dash_subpath(&mut out, segs, lens, *closed, &pat, phase);
    }
    Some(out)
}

/// Position in the pattern: entry index, length left in it, and whether it is a dash.
#[derive(Clone, Copy)]
struct Phase {
    ix: usize,
    rem: f64,
    on: bool,
}

fn dash_subpath(out: &mut Dashed, segs: &[PathSeg], lens: &[f64], closed: bool, pat: &[f64], mut ph: Phase) {
    const EPS: f64 = 1e-9;
    let first_dot = out.dots.len();
    let starts_on = ph.on && ph.rem > 0.0;
    let mut dashes: Vec<Vec<PathSeg>> = vec![];
    let mut cur: Vec<PathSeg> = vec![];
    let mut cuts = 0;
    for (seg, &len) in segs.iter().zip(lens) {
        let mut s = 0.0;
        loop {
            let left = len - s;
            if ph.rem > left + EPS {
                if ph.on && left > EPS {
                    cur.push(sub(seg, len, s, len));
                }
                ph.rem -= left;
                break;
            }
            let end = (s + ph.rem).min(len);
            if ph.on {
                if pat[ph.ix] > 0.0 {
                    if end > s {
                        cur.push(sub(seg, len, s, end));
                    }
                    if !cur.is_empty() {
                        dashes.push(std::mem::take(&mut cur));
                    }
                } else {
                    let t = param(seg, len, end);
                    out.dots.push(Dot { at: seg.eval(t), dir: tangent(seg, t) });
                }
            }
            cuts += 1;
            s = end;
            ph.ix = (ph.ix + 1) % pat.len();
            ph.rem = pat[ph.ix];
            ph.on = !ph.on;
        }
    }
    let ends_on = !cur.is_empty();
    if ends_on {
        dashes.push(cur);
    }
    if closed {
        if cuts == 0 && ends_on {
            // One dash all the way round: keep it closed so the start gets a join, not caps.
            let segs = dashes.pop().unwrap_or_default();
            write(&mut out.path, &segs);
            out.path.close_path();
            return;
        }
        if starts_on && ends_on && dashes.len() > 1 {
            // The dash running through the start point is one dash.
            let mut last = dashes.pop().unwrap_or_default();
            last.append(&mut dashes[0]);
            dashes[0] = last;
        }
        // A dot on the start point is reached again at the end.
        if out.dots.len() > first_dot + 1 && out.dots[first_dot].at.distance(out.dots[out.dots.len() - 1].at) < 1e-6 {
            out.dots.pop();
        }
    }
    for d in &dashes {
        write(&mut out.path, d);
    }
}

fn write(out: &mut BezPath, segs: &[PathSeg]) {
    let Some(first) = segs.first() else { return };
    out.move_to(first.start());
    for s in segs {
        push_seg(out, s);
    }
}

/// Curve parameter at arc length `s` along `seg` (of total length `len`).
fn param(seg: &PathSeg, len: f64, s: f64) -> f64 {
    if s <= 0.0 {
        0.0
    } else if s >= len {
        1.0
    } else if let PathSeg::Line(_) = seg {
        s / len
    } else {
        seg.inv_arclen(s, ARCLEN_ACCURACY)
    }
}

/// The part of `seg` between arc lengths `s0` and `s1`.
fn sub(seg: &PathSeg, len: f64, s0: f64, s1: f64) -> PathSeg {
    seg.subsegment(param(seg, len, s0)..param(seg, len, s1))
}

/// Outlines of `dots` for a stroke of `width` with `cap`: discs (round), squares turned along the
/// path (projecting) or nothing (butt). Each outline winds like kurbo's stroke outlines, so a dot
/// touching a dash doesn't cancel it under the non-zero rule.
pub fn dot_outline(dots: &[Dot], width: f64, cap: LineCap, tol: f64) -> BezPath {
    let mut out = BezPath::new();
    let r = width / 2.0;
    if r <= 0.0 {
        return out;
    }
    for d in dots {
        match cap {
            LineCap::Butt => {}
            LineCap::Round => out.extend(kurbo::Circle::new(d.at, r).path_elements(tol)),
            LineCap::Square => {
                let (u, n) = (d.dir * r, Vec2::new(-d.dir.y, d.dir.x) * r);
                out.move_to(d.at - u - n);
                out.line_to(d.at + u - n);
                out.line_to(d.at + u + n);
                out.line_to(d.at - u + n);
                out.close_path();
            }
        }
    }
    out
}
