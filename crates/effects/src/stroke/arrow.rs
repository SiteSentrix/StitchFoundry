//! Arrowheads. A head of weight `hw` (stroke weight × scale) is `4·hw` long and `4·hw` wide,
//! with its tip on the end point of the path.

use std::borrow::Cow;

use kurbo::{BezPath, ParamCurve, PathSeg, Point, Shape, Vec2};
use vectorcraft_doc::{Arrowhead, StrokeLayer};

use super::{StrokePieces, tangent};

/// One arrowhead.
#[derive(Clone, Debug, PartialEq)]
pub struct Arrow {
    pub kind: Arrowhead,
    /// The filled outline (non-zero rule).
    pub outline: BezPath,
    pub tip: Point,
    /// Unit direction the head points in.
    pub dir: Vec2,
}

/// [`super::stroke_pieces`] for a stroke with at least one arrowhead.
pub(super) fn pieces<'a>(bp: &'a BezPath, st: &StrokeLayer) -> StrokePieces<'a> {
    let segs: Vec<PathSeg> = bp.segments().collect();
    let mut heads = vec![];
    if let (Some(first), Some(last)) = (segs.first(), segs.last()) {
        let weight = |pct: f64| (st.width * pct / 100.0).max(0.25);
        if let Some(kind) = st.start_arrow {
            let (tip, dir) = (first.start(), -tangent(first, 0.0));
            heads.push(Arrow { kind, outline: shape(kind, tip, dir, weight(st.arrow_scale.0)), tip, dir });
        }
        if let Some(kind) = st.end_arrow {
            let (tip, dir) = (last.end(), tangent(last, 1.0));
            heads.push(Arrow { kind, outline: shape(kind, tip, dir, weight(st.arrow_scale.1)), tip, dir });
        }
    }
    StrokePieces { line: Cow::Borrowed(bp), heads }
}

/// Outline of a `kind` head of weight `hw` with its tip at `tip`, pointing along `dir`.
fn shape(kind: Arrowhead, tip: Point, dir: Vec2, hw: f64) -> BezPath {
    let n = Vec2::new(-dir.y, dir.x);
    let len = 4.0 * hw;
    let half = len / 2.0;
    let base = tip - dir * len;
    let centre = tip - dir * half;
    let mut p = BezPath::new();
    match kind {
        Arrowhead::Triangle | Arrowhead::TriangleOpen => polygon(&mut p, &[tip, base + n * half, base - n * half]),
        Arrowhead::Arrow | Arrowhead::ArrowOpen => polygon(&mut p, &[tip, base + n * half, tip - dir * (len * 0.7), base - n * half]),
        Arrowhead::Circle | Arrowhead::CircleOpen => p.extend(kurbo::Circle::new(centre, half).path_elements(0.01)),
        Arrowhead::Square | Arrowhead::SquareOpen => polygon(&mut p, &[tip + n * half, base + n * half, base - n * half, tip - n * half]),
        Arrowhead::Diamond => polygon(&mut p, &[tip, centre + n * half, base, centre - n * half]),
        Arrowhead::Bar => {
            let t = 0.75 * hw;
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
