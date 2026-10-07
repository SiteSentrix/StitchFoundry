//! Smart Guides: snapping to anchors, object bounds (edges/centres), artboards, with the magenta
//! construction lines and labels Illustrator users expect.

use vectorcraft_doc::hit::{HitOptions, hit_test};
use vectorcraft_doc::{Document, NodeId, NodeKind};
use vectorcraft_geom::{Point, Rect, Vec2};

use crate::{Overlay, ToolContext};

pub const MAGENTA: [u8; 3] = [0xff, 0x3d, 0xfc];

#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Anchor,
    Center,
    Edge,
    Artboard,
    Bleed,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::Anchor => "anchor",
            Kind::Center => "center",
            Kind::Edge => "path",
            Kind::Artboard => "artboard",
            Kind::Bleed => "bleed",
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
        Self::collect_inner(doc, exclude, None, visible)
    }

    /// Targets for dragging artboard `index`: everything except that artboard and `exclude` (the
    /// art that moves along with it).
    pub fn for_artboard(doc: &Document, index: usize, exclude: &[NodeId]) -> Self {
        Self::collect_inner(doc, exclude, Some(index), None)
    }

    fn collect_inner(doc: &Document, exclude: &[NodeId], skip_artboard: Option<usize>, visible: Option<Rect>) -> Self {
        let mut t = Targets::default();
        let excluded = |id: NodeId| exclude.iter().any(|e| doc.ancestry(id).is_some_and(|a| a.contains(e)));
        let add_rect = |t: &mut Targets, r: Rect, kind: Kind| {
            let c = r.center();
            for p in [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)] {
                t.xs.push((p.x, p, kind));
                t.ys.push((p.y, p, kind));
            }
            // A bleed shares its artboard's centre.
            if kind != Kind::Bleed {
                t.points.push((c, Kind::Center));
                t.xs.push((c.x, c, Kind::Center));
                t.ys.push((c.y, c, Kind::Center));
            }
        };
        for (_, ab) in doc.artboards.iter().enumerate().filter(|(i, _)| Some(*i) != skip_artboard) {
            add_rect(&mut t, ab.rect, Kind::Artboard);
            if doc.setup.has_bleed() {
                add_rect(&mut t, doc.setup.bleed_rect(ab.rect), Kind::Bleed);
            }
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

    /// Snap a dragged point that carries others along at `offsets` (a resize handle and the
    /// bleed edge beyond it): onto an anchor or centre near the point itself, otherwise into line
    /// with targets, each axis on whichever of them comes nearest.
    pub fn snap_point_with(&self, p: Point, offsets: &[Vec2], tol: f64) -> (Point, Vec<Overlay>) {
        if self.points.iter().any(|(q, _)| q.distance(p) <= tol) {
            return self.snap_point(p, tol);
        }
        let rects: Vec<Rect> = std::iter::once(Vec2::ZERO).chain(offsets.iter().copied()).map(|o| Rect::from_points(p + o, p + o)).collect();
        let (adj, mut ov) = self.snap_rects(&rects, tol);
        let out = p + adj;
        if !ov.is_empty() {
            ov.push(Overlay::Label { p: out, text: "align".into(), color: MAGENTA });
        }
        (out, ov)
    }

    /// Snap a moving rectangle (selection bounds after a move by `d`): tries its corners/edges/centre.
    pub fn snap_rect(&self, r: Rect, tol: f64) -> (Vec2, Vec<Overlay>) {
        self.snap_rects(&[r], tol)
    }

    /// Snap rectangles that move together (an artboard and its bleed): the edge or centre of any
    /// of them nearest a target, per axis. Returns the shift and the guides.
    pub fn snap_rects(&self, rects: &[Rect], tol: f64) -> (Vec2, Vec<Overlay>) {
        let nearest = |targets: &[(f64, Point, Kind)], along: fn(&Rect) -> [f64; 3]| {
            rects
                .iter()
                .flat_map(|r| along(r).into_iter().map(move |v| (v, *r)))
                .flat_map(|(v, r)| targets.iter().map(move |(t, from, _)| (t - v, *from, v, r)))
                .filter(|(d, ..)| d.abs() <= tol)
                .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
        };
        let best_x = nearest(&self.xs, |r| [r.x0, r.center().x, r.x1]);
        let best_y = nearest(&self.ys, |r| [r.y0, r.center().y, r.y1]);
        let mut d = Vec2::ZERO;
        let mut ov = vec![];
        if let Some((dx, from, x, r)) = best_x {
            d.x = dx;
            let x = x + dx;
            let (y0, y1) = (from.y.min(r.y0), from.y.max(r.y1));
            ov.push(Overlay::Line { a: Point::new(x, y0), b: Point::new(x, y1), color: MAGENTA, dashed: false });
        }
        if let Some((dy, from, y, r)) = best_y {
            d.y = dy;
            let y = y + dy;
            let (x0, x1) = (from.x.min(r.x0), from.x.max(r.x1));
            ov.push(Overlay::Line { a: Point::new(x0, y), b: Point::new(x1, y), color: MAGENTA, dashed: false });
        }
        (d, ov)
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
    fn artboard_bleed_is_a_target() {
        let (mut d, _) = doc_with_rect();
        let t = Targets::collect(&d, &[], None);
        assert_eq!(t.snap_point(Point::new(512.0, 300.0), 4.0).0.x, 512.0, "no bleed, nothing near");
        d.setup.bleed = [10.0; 4];
        let t = Targets::collect(&d, &[], None);
        assert_eq!(t.snap_point(Point::new(512.0, 300.0), 4.0).0.x, 510.0);
    }

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
}
