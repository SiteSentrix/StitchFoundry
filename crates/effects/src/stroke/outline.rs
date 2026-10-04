//! Strokes as filled geometry outside the canvas:
//!
//! - [`outline_region`]: the area a stroke paints as one clean path (Outline Stroke and the live
//!   Outline Stroke effect).

use vectorcraft_doc::{StrokeAlign, StrokeLayer};
use vectorcraft_geom::{FillRule, PathData};
use vectorcraft_pathops::BoolOp;

use super::{aligned_width, is_closed, line_outline, stroke_pieces};

/// Flattening/fitting tolerance of outlined strokes (document points).
pub const OUTLINE_TOL: f64 = 0.01;

/// The area stroke `st` paints along `path` (fill rule `rule`) as one clean path, as on the
/// canvas: the line (width profile, dashes, caps, joins) united with the arrowheads, and kept
/// inside or outside a closed path for inside or outside alignment. Brushes are not drawn.
pub fn outline_region(path: &PathData, rule: FillRule, st: &StrokeLayer) -> PathData {
    if !(st.width > 0.0 && st.width.is_finite()) || path.is_empty() {
        return PathData::default();
    }
    let bp = path.to_bezpath();
    let closed = is_closed(&bp);
    let pieces = stroke_pieces(&bp, st);
    let mut parts = vec![line_outline(&pieces.line, st, aligned_width(st, closed), OUTLINE_TOL)];
    parts.extend(pieces.heads.into_iter().map(|h| h.outline));
    let region = vectorcraft_pathops::stroke_region(&parts);
    match st.align {
        StrokeAlign::Inside if closed => vectorcraft_pathops::boolean(&region, FillRule::NonZero, path, rule, BoolOp::Intersect),
        StrokeAlign::Outside if closed => vectorcraft_pathops::boolean(&region, FillRule::NonZero, path, rule, BoolOp::Difference),
        _ => region,
    }
}
