//! Smart Guides: snapping to anchors, object bounds (edges/centres), artboards, with the magenta
//! construction lines and labels Illustrator users expect.

use vectorcraft_doc::hit::{HitOptions, hit_test};
use vectorcraft_doc::{Document, NodeId, NodeKind};
use vectorcraft_geom::{Affine, Point, Rect, Vec2};

use crate::bbox::Handle;
use crate::{Overlay, ToolContext};

pub const MAGENTA: [u8; 3] = [0xff, 0x3d, 0xfc];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Anchor,
    Center,
    Edge,
    Artboard,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Anchor => "anchor",
            Kind::Center => "center",
            Kind::Edge => "path",
            Kind::Artboard => "artboard",
        }
    }
}

/// Snap targets gathered from the document (excluding the objects being edited).
#[derive(Default)]
pub struct Targets {
    points: Vec<(Point, Kind)>,
    xs: Vec<(f64, Point, Kind)>,
    ys: Vec<(f64, Point, Kind)>,
}

impl Targets {
    pub fn collect(doc: &Document, exclude: &[NodeId], visible: Option<Rect>) -> Self {
        let mut t = Targets::default();
        let excluded = |id: NodeId| exclude.iter().any(|e| doc.ancestry(id).is_some_and(|a| a.contains(e)));
        let add_rect = |t: &mut Targets, r: Rect, kind: Kind| {
            let c = r.center();
            for p in [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)] {
                t.xs.push((p.x, p, kind));
                t.ys.push((p.y, p, kind));
            }
            t.points.push((c, Kind::Center));
            t.xs.push((c.x, c, Kind::Center));
            t.ys.push((c.y, c, Kind::Center));
        };
        for ab in &doc.artboards {
            add_rect(&mut t, ab.rect, Kind::Artboard);
        }
        let mut budget = 20_000usize;
        doc.walk(|n| {
            if budget == 0 || n.is_container() || !n.visible || excluded(n.id) {
                return;
            }
            let Some(b) = n.geometric_bounds() else { return };
            if let Some(v) = visible
                && b.intersect(v).area() <= 0.0
                && !v.contains(b.center())
            {
                return;
            }
            if let NodeKind::Path { path, .. } = &n.kind {
                for (_, _, a) in path.anchors() {
                    t.points.push((a.p, Kind::Anchor));
                    budget = budget.saturating_sub(1);
                }
            }
            add_rect(&mut t, b, Kind::Edge);
        });
        t
    }

    /// Anchors and centres of the visible leaves under `roots` (the roots included) whose bounds
    /// reach into `near`.
    fn points_near(doc: &Document, roots: &[NodeId], near: Rect) -> Self {
        let mut t = Targets::default();
        for n in roots.iter().filter_map(|id| doc.node(*id)) {
            n.walk(&mut |c| {
                if c.is_container() || !c.visible {
                    return;
                }
                let Some(b) = c.geometric_bounds() else { return };
                if b.x0 > near.x1 || b.x1 < near.x0 || b.y0 > near.y1 || b.y1 < near.y0 {
                    return;
                }
                if let NodeKind::Path { path, .. } = &c.kind {
                    t.points.extend(path.anchors().map(|(_, _, a)| (a.p, Kind::Anchor)));
                }
                t.points.push((b.center(), Kind::Center));
            });
        }
        t
    }

    /// Snap a single point. Returns the snapped point and guide overlays.
    pub fn snap_point(&self, p: Point, tol: f64) -> (Point, Vec<Overlay>) {
        if let Some((q, k)) = self.points.iter().filter(|(q, _)| q.distance(p) <= tol).min_by(|a, b| a.0.distance(p).total_cmp(&b.0.distance(p))) {
            return (*q, vec![Overlay::Label { p: *q, text: k.label().into(), color: MAGENTA }]);
        }
        let mut out = p;
        let mut ov = vec![];
        if let Some((x, from, _)) =
            self.xs.iter().filter(|(x, _, _)| (x - p.x).abs() <= tol).min_by(|a, b| (a.0 - p.x).abs().total_cmp(&(b.0 - p.x).abs()))
        {
            out.x = *x;
            ov.push(Overlay::Line { a: *from, b: Point::new(*x, p.y), color: MAGENTA, dashed: false });
        }
        if let Some((y, from, _)) =
            self.ys.iter().filter(|(y, _, _)| (y - p.y).abs() <= tol).min_by(|a, b| (a.0 - p.y).abs().total_cmp(&(b.0 - p.y).abs()))
        {
            out.y = *y;
            ov.push(Overlay::Line { a: *from, b: Point::new(p.x, *y), color: MAGENTA, dashed: false });
        }
        if !ov.is_empty() {
            ov.push(Overlay::Label { p: out, text: "align".into(), color: MAGENTA });
        }
        (out, ov)
    }

    /// Snap a moving rectangle (selection bounds after a move by `d`): tries its corners/edges/centre.
    pub fn snap_rect(&self, r: Rect, tol: f64) -> (Vec2, Vec<Overlay>) {
        let c = r.center();
        let xs = [r.x0, c.x, r.x1];
        let ys = [r.y0, c.y, r.y1];
        let best_x = xs
            .iter()
            .flat_map(|x| self.xs.iter().map(move |(t, from, _)| (t - x, *from, *x)))
            .filter(|(d, _, _)| d.abs() <= tol)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
        let best_y = ys
            .iter()
            .flat_map(|y| self.ys.iter().map(move |(t, from, _)| (t - y, *from, *y)))
            .filter(|(d, _, _)| d.abs() <= tol)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
        let mut d = Vec2::ZERO;
        let mut ov = vec![];
        if let Some((dx, from, x)) = best_x {
            d.x = dx;
            let x = x + dx;
            let (y0, y1) = (from.y.min(r.y0), from.y.max(r.y1));
            ov.push(Overlay::Line { a: Point::new(x, y0), b: Point::new(x, y1), color: MAGENTA, dashed: false });
        }
        if let Some((dy, from, y)) = best_y {
            d.y = dy;
            let y = y + dy;
            let (x0, x1) = (from.x.min(r.x0), from.x.max(r.x1));
            ov.push(Overlay::Line { a: Point::new(x0, y), b: Point::new(x1, y), color: MAGENTA, dashed: false });
        }
        (d, ov)
    }

    /// Snap a bounding-box resize. `a` is the scale [`crate::bbox::scale_for_drag`] gave for
    /// dragging `handle` of `r` (about the centre when `from_center`): the edges that handle moves
    /// land on the nearest target within `tol`. A proportional scale stays proportional, so only
    /// the closer of its two axes snaps.
    pub fn snap_scale(&self, r: Rect, handle: Handle, a: Affine, proportional: bool, from_center: bool, tol: f64) -> (Affine, Vec<Overlay>) {
        let origin = if from_center { r.center() } else { handle.opposite().pos(r) };
        let (start, [sx, _, _, sy, _, _]) = (handle.pos(r), a.as_coeffs());
        let (ax, ay) = handle.axes();
        // The nearest target to the edge a scale `s` takes from `v0` (about `o`), as (the scale
        // landing on it, distance, target, the point it comes from). A target on the fixed side
        // would collapse or flip the box: it is no candidate.
        let near = |on: bool, s: f64, v0: f64, o: f64, ts: &[(f64, Point, Kind)]| {
            if !on || (v0 - o).abs() <= 1e-9 {
                return None;
            }
            let v = o + (v0 - o) * s;
            ts.iter()
                .map(|(t, from, _)| ((t - o) / (v0 - o), (t - v).abs(), *t, *from))
                .filter(|(k, d, _, _)| *d <= tol && k.is_finite() && k * s > 0.0)
                .min_by(|p, q| p.1.total_cmp(&q.1))
        };
        let mut hx = near(ax, sx, start.x, origin.x, &self.xs);
        let mut hy = near(ay, sy, start.y, origin.y, &self.ys);
        if proportional && let (Some(x), Some(y)) = (hx, hy) {
            if x.1 <= y.1 {
                hy = None;
            } else {
                hx = None;
            }
        }
        let (mut nx, mut ny) = (hx.map_or(sx, |h| h.0), hy.map_or(sy, |h| h.0));
        if proportional {
            // The two scales are equal but for their signs.
            if hx.is_some() {
                ny = nx.abs() * sy.signum();
            } else if hy.is_some() {
                nx = ny.abs() * sx.signum();
            }
        }
        let out = Affine::translate(origin.to_vec2()) * Affine::scale_non_uniform(nx, ny) * Affine::translate(-origin.to_vec2());
        let nr = out.transform_rect_bbox(r);
        let mut ov = vec![];
        if let Some((_, _, x, from)) = hx {
            ov.push(Overlay::Line { a: Point::new(x, from.y.min(nr.y0)), b: Point::new(x, from.y.max(nr.y1)), color: MAGENTA, dashed: false });
        }
        if let Some((_, _, y, from)) = hy {
            ov.push(Overlay::Line { a: Point::new(from.x.min(nr.x0), y), b: Point::new(from.x.max(nr.x1), y), color: MAGENTA, dashed: false });
        }
        (out, ov)
    }
}

/// Snap `p` for a drawing tool when smart guides (or grid snapping) are on.
pub fn snap_draw(cx: &ToolContext, p: Point, exclude: &[NodeId]) -> (Point, Vec<Overlay>) {
    if cx.snap_to_pixel {
        return (Point::new(p.x.round(), p.y.round()), vec![]);
    }
    if cx.snap_to_grid {
        let s = cx.doc.grid.spacing / cx.doc.grid.subdivisions.max(1) as f64;
        return (vectorcraft_geom::snap::snap_point_to_grid(p, s), vec![]);
    }
    if !cx.smart_guides {
        return (p, vec![]);
    }
    Targets::collect(cx.doc, exclude, None).snap_point(p, cx.tol(5.0))
}

/// Snap a picked point (a transform tool's reference point) to the nearest anchor or centre of the
/// selection or of the object under the pointer, when Snap to Point or Smart Guides is on.
pub fn snap_pick(cx: &ToolContext, p: Point) -> (Point, Vec<Overlay>) {
    if !(cx.snap_to_point || cx.smart_guides) {
        return (p, vec![]);
    }
    let tol = cx.tol(5.0);
    let mut roots = cx.selection.objects.clone();
    roots.extend(hit_test(cx.doc, p, HitOptions { tol, ..cx.hit_options() }).map(|h| h.leaf));
    let near = Rect::new(p.x - tol, p.y - tol, p.x + tol, p.y + tol);
    Targets::points_near(cx.doc, &roots, near).snap_point(p, tol)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;

    #[test]
    fn snaps_to_anchor_and_alignment() {
        let (d, _) = doc_with_rect();
        let t = Targets::collect(&d, &[], None);
        let (p, ov) = t.snap_point(Point::new(102.0, 99.0), 4.0);
        assert_eq!(p, Point::new(100.0, 100.0));
        assert!(matches!(&ov[0], Overlay::Label { text, .. } if text == "anchor"));
        let (p, _) = t.snap_point(Point::new(301.0, 199.0), 4.0);
        assert_eq!(p.y, 200.0);
    }

    #[test]
    fn rect_snap_aligns_edges() {
        let (d, id) = doc_with_rect();
        let t = Targets::collect(&d, &[id], None);
        // Artboard is 0..500; a rect at x0=2 snaps to the artboard's left edge.
        let (dv, ov) = t.snap_rect(Rect::new(2.0, 50.0, 52.0, 90.0), 4.0);
        assert_eq!(dv.x, -2.0);
        assert!(!ov.is_empty());
    }

    #[test]
    fn scale_snap_lands_the_moved_edges_on_targets() {
        use crate::bbox::scale_for_drag;
        let (d, _) = doc_with_rect();
        // The document's rect is 100..200: a box beside it is resized against it.
        let t = Targets::collect(&d, &[], None);
        let r = Rect::new(250.0, 100.0, 300.0, 150.0);
        let snap = |h: Handle, p: Point, shift: bool, alt: bool| {
            let (a, ov) = t.snap_scale(r, h, scale_for_drag(r, h, p, shift, alt), shift, alt, 4.0);
            (a.transform_rect_bbox(r), ov)
        };
        // The bottom edge lands on the neighbour's bottom: same height.
        let (nr, ov) = snap(Handle::Bottom, Point::new(275.0, 197.0), false, false);
        assert_eq!(nr, Rect::new(250.0, 100.0, 300.0, 200.0));
        assert_eq!(ov.len(), 1);
        // A corner snaps each axis on its own: y to the neighbour, x stays free.
        let (nr, ov) = snap(Handle::BottomRight, Point::new(330.0, 202.0), false, false);
        assert_eq!(nr, Rect::new(250.0, 100.0, 330.0, 200.0));
        assert_eq!(ov.len(), 1);
        // Proportional: the snapped axis carries the other one.
        let (nr, _) = snap(Handle::BottomRight, Point::new(340.0, 198.0), true, false);
        assert_eq!(nr, Rect::new(250.0, 100.0, 350.0, 200.0));
        // From the centre the dragged edge still lands on the target.
        let (nr, _) = snap(Handle::Bottom, Point::new(275.0, 198.0), false, true);
        assert_eq!(nr, Rect::new(250.0, 50.0, 300.0, 200.0));
        // Out of reach, and the fixed edge's own target: untouched.
        let (nr, ov) = snap(Handle::Bottom, Point::new(275.0, 180.0), false, false);
        assert_eq!(nr, Rect::new(250.0, 100.0, 300.0, 180.0));
        assert!(ov.is_empty());
        let (nr, ov) = snap(Handle::Bottom, Point::new(275.0, 102.0), false, false);
        assert_eq!(nr, Rect::new(250.0, 100.0, 300.0, 102.0));
        assert!(ov.is_empty());
    }
}
