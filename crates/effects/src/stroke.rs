//! Stroke geometry shared by the canvas, the PDF/SVG exporters and Outline Stroke, so every
//! consumer paints the same shapes:
//!
//! - [`stroke_pieces`]: the centre line and the arrowheads as filled outlines;
//! - [`dash`]: the dash pattern; zero-length dashes become [`Dot`]s that a round or projecting cap
//!   turns into discs or squares ([`dot_outline`]);
//! - [`width_outline`]: variable-width (profile) strokes;
//! - [`line_outline`]: the line part of a stroke as one filled outline (profile, dashes, dots,
//!   caps and joins).

mod arrow;
mod dash;
mod width;

use std::borrow::Cow;

use kurbo::{BezPath, ParamCurve, ParamCurveDeriv, PathEl, PathSeg, Vec2};
use vectorcraft_doc::{LineCap, LineJoin, StrokeAlign, StrokeLayer};

pub use arrow::Arrow;
pub use dash::{Dashed, Dot, dash, dot_outline};
pub use width::width_outline;

/// Arc-length accuracy for dashing and trimming (document points).
const ARCLEN_ACCURACY: f64 = 1e-6;

/// The geometry one stroke paints, in the coordinate space of its path.
#[derive(Clone, Debug)]
pub struct StrokePieces<'a> {
    /// The centre line to stroke.
    pub line: Cow<'a, BezPath>,
    /// The arrowheads, start first.
    pub heads: Vec<Arrow>,
}

/// The centre line and arrowheads of stroke `st` along `bp`.
pub fn stroke_pieces<'a>(bp: &'a BezPath, st: &StrokeLayer) -> StrokePieces<'a> {
    if st.start_arrow.is_none() && st.end_arrow.is_none() {
        return StrokePieces { line: Cow::Borrowed(bp), heads: vec![] };
    }
    arrow::pieces(bp, st)
}

/// Does `bp` end with a `ClosePath`?
pub fn is_closed(bp: &BezPath) -> bool {
    bp.elements().last().is_some_and(|e| matches!(e, PathEl::ClosePath))
}

/// The width actually stroked: inside/outside strokes of closed paths are drawn twice as wide
/// and clipped to one side; open paths always stroke centred.
pub fn aligned_width(st: &StrokeLayer, closed: bool) -> f64 {
    match st.align {
        StrokeAlign::Inside | StrokeAlign::Outside if closed => st.width * 2.0,
        _ => st.width,
    }
}

/// The stroke style (cap, join, miter limit) of `st` at `width`, without dashes.
pub fn style(st: &StrokeLayer, width: f64) -> kurbo::Stroke {
    kurbo::Stroke::new(width)
        .with_join(match st.join {
            LineJoin::Miter => kurbo::Join::Miter,
            LineJoin::Round => kurbo::Join::Round,
            LineJoin::Bevel => kurbo::Join::Bevel,
        })
        .with_caps(match st.cap {
            LineCap::Butt => kurbo::Cap::Butt,
            LineCap::Round => kurbo::Cap::Round,
            LineCap::Square => kurbo::Cap::Square,
        })
        .with_miter_limit(st.miter_limit)
}

/// Does the dash pattern of `st` draw anything other than a solid line?
pub fn is_dashed(st: &StrokeLayer) -> bool {
    st.dash.as_ref().is_some_and(|d| d.pattern.iter().any(|v| *v > 0.0))
}

/// The filled outline of the line part of `st` along `line` (usually [`StrokePieces::line`]) at
/// `width` (see [`aligned_width`]), flattened/fitted to `tol`: the width profile, or the dash
/// pattern with its dots, with the stroke's caps and joins. Fill it with the non-zero rule.
/// Arrowheads are not included.
pub fn line_outline(line: &BezPath, st: &StrokeLayer, width: f64, tol: f64) -> BezPath {
    if line.elements().is_empty() || width <= 0.0 || !width.is_finite() {
        return BezPath::new();
    }
    let tol = tol.max(1e-4);
    if let Some(profile) = st.profile.as_ref().filter(|_| !is_dashed(st)) {
        return width_outline(line, width, profile, st.cap, tol);
    }
    let style = style(st, width);
    match st.dash.as_ref().and_then(|d| dash(line, d)) {
        Some(d) => {
            let mut out = kurbo::stroke(d.path.iter(), &style, &kurbo::StrokeOpts::default(), tol);
            out.extend(dot_outline(&d.dots, width, st.cap, tol).iter());
            out
        }
        None => kurbo::stroke(line.iter(), &style, &kurbo::StrokeOpts::default(), tol),
    }
}

/// Unit tangent of `s` at `t`, robust at ends where control points coincide with the end point.
pub(crate) fn tangent(s: &PathSeg, t: f64) -> Vec2 {
    let d = match s {
        PathSeg::Line(l) => l.p1 - l.p0,
        PathSeg::Quad(q) => {
            let v = q.deriv().eval(t).to_vec2();
            if v.hypot() > 1e-9 { v } else { q.p2 - q.p0 }
        }
        PathSeg::Cubic(c) => {
            let v = c.deriv().eval(t).to_vec2();
            if v.hypot() > 1e-9 {
                v
            } else if t < 0.5 {
                // The first control point that differs from the start gives the direction.
                [c.p2, c.p3].into_iter().map(|p| p - c.p0).find(|v| v.hypot() > 1e-9).unwrap_or_default()
            } else {
                [c.p1, c.p0].into_iter().map(|p| c.p3 - p).find(|v| v.hypot() > 1e-9).unwrap_or_default()
            }
        }
    };
    unit(d)
}

/// `v` normalised, or +x when it has no length.
pub(crate) fn unit(v: Vec2) -> Vec2 {
    let h = v.hypot();
    if h > 1e-12 { v / h } else { Vec2::new(1.0, 0.0) }
}

/// Append `seg` to `out` (which must already have a current point at its start).
pub(crate) fn push_seg(out: &mut BezPath, seg: &PathSeg) {
    match seg {
        PathSeg::Line(l) => out.line_to(l.p1),
        PathSeg::Quad(q) => out.quad_to(q.p1, q.p2),
        PathSeg::Cubic(c) => out.curve_to(c.p1, c.p2, c.p3),
    }
}

/// Element ranges of the subpaths of `bp` (each with whether it is closed).
pub(crate) fn subpaths(bp: &BezPath) -> Vec<(std::ops::Range<usize>, bool)> {
    let els = bp.elements();
    let mut out = vec![];
    let mut start = 0;
    for (i, el) in els.iter().enumerate() {
        match el {
            PathEl::MoveTo(_) if i > start => {
                out.push((start..i, false));
                start = i;
            }
            PathEl::ClosePath => {
                out.push((start..i + 1, true));
                start = i + 1;
            }
            _ => {}
        }
    }
    if start < els.len() {
        out.push((start..els.len(), false));
    }
    // A lone MoveTo (or a MoveTo straight after a ClosePath) draws nothing.
    out.retain(|(r, _)| r.len() > 1);
    out
}

/// The segments of one subpath (`els` starts with its MoveTo; a ClosePath adds the closing line).
pub(crate) fn segments(els: &[PathEl]) -> Vec<PathSeg> {
    kurbo::segments(els.iter().copied()).collect()
}

#[cfg(test)]
mod tests;
