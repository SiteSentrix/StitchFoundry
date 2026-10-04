//! Stroke geometry shared by the canvas, the PDF/SVG exporters and Outline Stroke, so every
//! consumer paints the same shapes:
//!
//! - [`stroke_pieces`]: the centre line and the arrowheads as filled outlines;
//! - [`width_outline`]: variable-width (profile) strokes;
//! - [`line_outline`]: the line part of a stroke as one filled outline (profile or dashes, caps
//!   and joins).

mod arrow;
mod width;

use std::borrow::Cow;

use kurbo::{BezPath, ParamCurve, ParamCurveDeriv, PathEl, PathSeg, Vec2};
use vectorcraft_doc::{LineCap, LineJoin, StrokeAlign, StrokeLayer};

pub use arrow::Arrow;
pub use width::width_outline;

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
/// pattern, with the stroke's caps and joins. Fill it with the non-zero rule. Arrowheads are not
/// included.
pub fn line_outline(line: &BezPath, st: &StrokeLayer, width: f64, tol: f64) -> BezPath {
    if line.elements().is_empty() || width <= 0.0 || !width.is_finite() {
        return BezPath::new();
    }
    let tol = tol.max(1e-4);
    if let Some(profile) = st.profile.as_ref().filter(|_| !is_dashed(st)) {
        return width_outline(line, width, profile, st.cap, tol);
    }
    let mut style = style(st, width);
    if let Some(d) = st.dash.as_ref().filter(|_| is_dashed(st)) {
        // An odd pattern repeats with dashes and gaps swapped (as in SVG and PDF).
        let mut pat = d.pattern.clone();
        if pat.len() % 2 == 1 {
            pat.extend_from_within(..);
        }
        style = style.with_dashes(d.offset, pat);
    }
    kurbo::stroke(line.iter(), &style, &kurbo::StrokeOpts::default(), tol)
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

#[cfg(test)]
mod tests;
