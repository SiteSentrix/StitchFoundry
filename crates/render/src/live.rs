//! Live objects in the renderer.
//!
//! Blends and envelopes are evaluated by `vectorcraft_doc::live` and the resulting objects cached
//! by `Arc` identity of the live node (so the steps keep stable `Arc`s and hit the geometry cache
//! frame after frame). Gradient meshes are tessellated into many small solid-colour quads (vello
//! has no mesh shading), with the density chosen from the on-screen patch size; each quad is
//! grown by half a device pixel so neighbours overlap and no antialiasing seams show.

use std::collections::HashMap;
use std::sync::Arc;

use vectorcraft_doc::live::{self, GradientMesh, MeshQuad};
use vectorcraft_doc::{Node, NodeKind};
use vectorcraft_geom::{BezPath, PathData, Point};
use vello_cpu::RenderContext;
use vello_cpu::peniko;

use crate::{Frame, Renderer, text_geom};

type Expanded = Arc<Vec<Arc<Node>>>;
type MeshEntry = (Arc<Node>, Arc<Vec<MeshQuad>>, u64);

/// Per-renderer cache of evaluated live objects.
#[derive(Default)]
pub(crate) struct LiveCache {
    expanded: HashMap<usize, (Arc<Node>, Expanded, u64)>,
    meshes: HashMap<(usize, usize), MeshEntry>,
    stamp: u64,
}

impl LiveCache {
    fn tick(&mut self, stamp: u64) {
        self.stamp = stamp;
        if self.expanded.len() > 256 {
            self.expanded.retain(|_, e| stamp - e.2 <= 3);
        }
        if self.meshes.len() > 256 {
            self.meshes.retain(|_, e| stamp - e.2 <= 3);
        }
    }
}

/// Text converted to one outline path (for envelopes): glyphs in document space, painted with the
/// first run's fill/stroke.
pub(crate) fn outline_text(n: &Node) -> Option<Node> {
    let NodeKind::Text(t) = &n.kind else { return None };
    let g = text_geom(t);
    let mut bp = g.all.clone();
    bp.apply_affine(t.xf);
    let mut out = Node::path(n.id, PathData::from_bezpath(&bp), n.appearance.clone());
    if out.appearance.items.is_empty()
        && let Some(r) = t.runs.first()
    {
        out.appearance = r.style.appearance();
    }
    out.opacity = n.opacity;
    out.blend = n.blend;
    Some(out)
}

/// Evaluate a live node one level (blend steps / envelope result, text outlined; mesh → flat
/// pieces). Non-live nodes return themselves.
pub fn expand_live(n: &Node) -> Vec<Node> {
    let hook: &dyn Fn(&Node) -> Option<Node> = &outline_text;
    live::expand_live_with(n, Some(hook))
}

/// Subdivisions per patch for a patch about `size_px` device pixels across.
fn mesh_level(m: &GradientMesh, bounds_px: f64) -> usize {
    let per = bounds_px / (m.rows.max(m.cols).max(1) as f64);
    ((per / 5.0).ceil() as usize).clamp(2, 32)
}

impl Renderer {
    fn live_expanded(&mut self, a: &Arc<Node>, cache: bool) -> Expanded {
        let key = Arc::as_ptr(a) as usize;
        if cache
            && let Some(e) = self.live.expanded.get_mut(&key)
            && Arc::ptr_eq(&e.0, a)
        {
            e.2 = self.live.stamp;
            return e.1.clone();
        }
        let hook: &dyn Fn(&Node) -> Option<Node> = &outline_text;
        let v: Expanded = match crate::effects::pathfinder_children(a, Some(hook)) {
            Some(children) => Arc::new(children),
            None => Arc::new(expand_live(a).into_iter().map(Arc::new).collect()),
        };
        if cache {
            self.live.expanded.insert(key, (a.clone(), v.clone(), self.live.stamp));
        }
        v
    }

    fn mesh_quads(&mut self, a: &Arc<Node>, m: &GradientMesh, level: usize, cache: bool) -> Arc<Vec<MeshQuad>> {
        let key = (Arc::as_ptr(a) as usize, level);
        if cache
            && let Some(e) = self.live.meshes.get_mut(&key)
            && Arc::ptr_eq(&e.0, a)
        {
            e.2 = self.live.stamp;
            return e.1.clone();
        }
        let q = Arc::new(m.quads(level));
        if cache {
            self.live.meshes.insert(key, (a.clone(), q.clone(), self.live.stamp));
        }
        q
    }

    /// Draw a live node reached through the tree walk (handles its transparency group).
    pub(crate) fn draw_live(&mut self, ctx: &mut RenderContext, f: &Frame, a: &Arc<Node>) {
        let stamp = self.stamp;
        self.live.tick(stamp);
        let opacity = self.opacity_of(a);
        if !f.opts.outline && (opacity < 1.0 || a.blend != vectorcraft_color::BlendMode::Normal || a.isolate) {
            let blends = self.blends_through(a);
            let bounds = if blends { self.bounds_of(a) } else { None };
            let comp = crate::group::Composite { blend: a.blend, opacity, isolated: a.isolate, blends, bounds, ..Default::default() };
            self.group(ctx, f, comp, &mut |r, c, fr| r.draw_live_body(c, fr, a, true));
        } else {
            self.draw_live_body(ctx, f, a, true);
        }
        self.stats.drawn += 1;
    }

    /// Draw the evaluated content of a live node (no transparency group).
    pub(crate) fn draw_live_body(&mut self, ctx: &mut RenderContext, f: &Frame, a: &Arc<Node>, cache: bool) {
        match &a.kind {
            NodeKind::Mesh(m) => self.draw_mesh(ctx, f, a, m, cache),
            NodeKind::Blend { .. } | NodeKind::Envelope { .. } | NodeKind::Repeat(_) | NodeKind::Group { .. } | NodeKind::Layer { .. } => {
                let items = self.live_expanded(a, cache);
                for c in items.iter() {
                    self.draw_arc(ctx, f, c);
                }
            }
            _ => {}
        }
    }

    fn draw_mesh(&mut self, ctx: &mut RenderContext, f: &Frame, a: &Arc<Node>, m: &GradientMesh, cache: bool) {
        if !m.is_valid() {
            return;
        }
        if f.opts.outline {
            let bp = m.lines().to_bezpath();
            self.hairline(ctx, f, &bp, [0, 0, 0, 255]);
            return;
        }
        let Some(b) = m.bounds() else { return };
        let level = mesh_level(m, b.width().max(b.height()) / f.px.max(1e-12));
        let quads = self.mesh_quads(a, m, level, cache);
        ctx.set_transform(f.view);
        ctx.set_fill_rule(peniko::Fill::NonZero);
        let grow = f.px * 0.5;
        let alpha = self.alpha;
        for q in quads.iter() {
            if q.opacity <= 0.0 {
                continue;
            }
            let c = Point::new((q.pts[0].x + q.pts[1].x + q.pts[2].x + q.pts[3].x) / 4.0, (q.pts[0].y + q.pts[1].y + q.pts[2].y + q.pts[3].y) / 4.0);
            let mut bp = BezPath::new();
            for (i, p) in q.pts.iter().enumerate() {
                let d = *p - c;
                let len = d.hypot();
                let p2 = if len > 1e-12 { *p + d * (grow / len) } else { *p };
                if i == 0 {
                    bp.move_to(p2);
                } else {
                    bp.line_to(p2);
                }
            }
            bp.close_path();
            ctx.set_paint(f.ink.color(&q.color, q.opacity * alpha));
            ctx.fill_path(&bp);
        }
    }

    /// Draw a live node reached without its `Arc` (thumbnails, clip fallbacks): uncached.
    pub(crate) fn draw_live_node(&mut self, ctx: &mut RenderContext, f: &Frame, n: &Node) {
        let a = Arc::new(n.clone());
        self.draw_live_body(ctx, f, &a, false);
    }
}
