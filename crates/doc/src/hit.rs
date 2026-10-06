//! Hit testing over the document tree.

use vectorcraft_geom::hit::{fill_contains, stroke_contains};
use vectorcraft_geom::{Point, Rect};

use crate::{Document, Node, NodeId, NodeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitKind {
    Fill,
    Stroke,
    /// Hit a path outline in outline mode, or an unfilled path's edge.
    Outline,
    Bounds,
}

/// The result of a hit test: the leaf that was hit and its ancestry (layer first).
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub leaf: NodeId,
    pub ancestry: Vec<NodeId>,
    pub kind: HitKind,
    /// The innermost envelope in the ancestry whose contents are being edited (Edit Contents):
    /// its content, not the envelope, is the object clicked.
    pub contents_of: Option<NodeId>,
}

impl Hit {
    /// The object the Selection tool selects: the child of the layer (the outermost group).
    /// Inside isolation mode (`scope`), the child of the isolated container instead.
    pub fn top_object(&self, scope: Option<NodeId>) -> NodeId {
        // The innermost of the isolated container and an envelope whose contents are edited.
        let at = |s: Option<NodeId>| s.and_then(|s| self.ancestry.iter().position(|x| *x == s));
        if let Some(i) = at(scope).max(at(self.contents_of)) {
            return self.ancestry.get(i + 1).copied().unwrap_or(self.leaf);
        }
        // Skip layers and sublayers.
        self.ancestry.get(1).copied().unwrap_or(self.leaf)
    }
}

/// Options for hit testing.
#[derive(Clone, Copy, Debug)]
pub struct HitOptions {
    /// Tolerance in document units (screen tolerance / zoom).
    pub tol: f64,
    /// Outline mode: only outlines are hittable.
    pub outline: bool,
    /// "Object Selection by Path Only" preference.
    pub path_only: bool,
}

impl Default for HitOptions {
    fn default() -> Self {
        Self { tol: 3.0, outline: false, path_only: false }
    }
}

/// Topmost editable object under `p`.
pub fn hit_test(doc: &Document, p: Point, opt: HitOptions) -> Option<Hit> {
    let mut chain = Vec::new();
    for layer in doc.layers.iter().rev() {
        if !layer.visible || layer.locked {
            continue;
        }
        if let NodeKind::Layer { template: true, .. } = layer.kind {
            continue;
        }
        // Pattern editing mode: only the tile art is editable.
        if doc.pattern_edit.as_ref().is_some_and(|e| e.layer != layer.id) {
            continue;
        }
        chain.push(layer.id);
        if let Some(mut h) = hit_children(layer, p, opt, &mut chain) {
            h.contents_of = h.ancestry.iter().rev().copied().find(|a| doc.node(*a).is_some_and(edits_contents));
            return Some(h);
        }
        chain.pop();
    }
    None
}

/// Is `n` an envelope whose contents are being edited (they hit, not the envelope)?
fn edits_contents(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Envelope { editing: true, .. })
}

/// Is `p` inside the region `clip` clips to ([`Node::clip_shapes`])? Text counts by its frame:
/// glyph outlines need the font engine, above this crate.
fn clip_contains(clip: &Node, p: Point) -> bool {
    let frame = |n: &Node| {
        let b = n.geometric_bounds()?;
        Some(Node::path(n.id, vectorcraft_geom::shapes::rectangle(b), Default::default()))
    };
    clip.clip_shapes(Some(&frame)).iter().any(|(bp, rule)| fill_contains(bp, *rule, p))
}

fn hit_children(parent: &Node, p: Point, opt: HitOptions, chain: &mut Vec<NodeId>) -> Option<Hit> {
    let children = parent.children()?;
    // A clip group (or a layer with a clipping mask) only hits inside its clipping path.
    if parent.clips()
        && let Some(clip) = children.first()
        && !clip_contains(clip, p)
    {
        return None;
    }
    for c in children.iter().rev() {
        if !c.visible || c.locked {
            continue;
        }
        // (An envelope's content sits where it was, not where the envelope draws it.)
        if !edits_contents(c)
            && let Some(b) = c.reach_bounds()
            && !b.inflate(opt.tol, opt.tol).contains(p)
        {
            continue;
        }
        chain.push(c.id);
        let hit = match &c.kind {
            NodeKind::Layer { .. } | NodeKind::Group { .. } => hit_children(c, p, opt, chain),
            // Edit Contents: the envelope's content hits, undistorted.
            NodeKind::Envelope { editing: true, .. } => hit_children(c, p, opt, chain),
            // A blend's key objects first (Direct and Group Selection pick them); its steps hit
            // as the blend.
            NodeKind::Blend { .. } => hit_children(c, p, opt, chain)
                .or_else(|| hit_leaf(c, p, opt).map(|kind| Hit { leaf: c.id, ancestry: chain.clone(), kind, contents_of: None })),
            _ => hit_leaf(c, p, opt).map(|kind| Hit { leaf: c.id, ancestry: chain.clone(), kind, contents_of: None }),
        };
        if hit.is_some() {
            return hit;
        }
        chain.pop();
    }
    None
}

/// A click on the strokes of a path or compound path `n` along `bp`: what any visible stroke
/// paints there (arrowheads, alignment, width profile, projecting caps, miter spikes), else its
/// outline within the tolerance. Outline mode hits the outline only.
fn hit_stroke(n: &Node, bp: &vectorcraft_geom::BezPath, rule: vectorcraft_geom::FillRule, p: Point, opt: HitOptions) -> Option<HitKind> {
    if !opt.outline && n.appearance.stroke_hit(bp, rule, p, opt.tol) {
        return Some(HitKind::Stroke);
    }
    stroke_contains(bp, 0.0, opt.tol, p).then_some(HitKind::Outline)
}

fn hit_leaf(n: &Node, p: Point, opt: HitOptions) -> Option<HitKind> {
    match &n.kind {
        NodeKind::Path { path, rule, .. } => {
            let bp = path.to_bezpath();
            if let Some(k) = hit_stroke(n, &bp, *rule, p, opt) {
                return Some(k);
            }
            let filled = !n.appearance.fill_paint().is_none();
            if !opt.outline && !opt.path_only && filled && fill_contains(&bp, *rule, p) {
                return Some(HitKind::Fill);
            }
            None
        }
        NodeKind::Compound { rule, .. } => {
            let bp = n.stroke_path()?;
            if let Some(k) = hit_stroke(n, &bp, *rule, p, opt) {
                return Some(k);
            }
            (!opt.outline && fill_contains(&bp, *rule, p)).then_some(HitKind::Fill)
        }
        NodeKind::Text(_) | NodeKind::Image(_) | NodeKind::SymbolInstance { .. } | NodeKind::Blend { .. } | NodeKind::Envelope { .. } => {
            n.geometric_bounds().filter(|b| b.inflate(opt.tol, opt.tol).contains(p)).map(|_| HitKind::Bounds)
        }
        NodeKind::Repeat(r) => r.expand().iter().find_map(|g| hit_any(g, p, opt)),
        NodeKind::Mesh(m) => {
            let bp = m.outline().to_bezpath();
            if stroke_contains(&bp, 0.0, opt.tol, p) {
                return Some(HitKind::Outline);
            }
            (!opt.path_only && fill_contains(&bp, vectorcraft_geom::FillRule::NonZero, p)).then_some(HitKind::Fill)
        }
        _ => None,
    }
}

/// Hit anywhere in an evaluated subtree (groups recurse; leaves use [`hit_leaf`]).
fn hit_any(n: &Node, p: Point, opt: HitOptions) -> Option<HitKind> {
    match &n.kind {
        NodeKind::Group { children, .. } => children.iter().rev().find_map(|c| hit_any(c, p, opt)),
        _ => hit_leaf(n, p, opt),
    }
}

/// Objects (children of layers, or of `scope` in isolation mode) touched by a marquee rect.
pub fn marquee(doc: &Document, r: Rect, scope: Option<NodeId>, leaves: bool) -> Vec<NodeId> {
    let mut out = Vec::new();
    let tops: Vec<&std::sync::Arc<Node>> = match scope.and_then(|s| doc.node(s)) {
        Some(s) => s.children().map(|c| c.iter().collect()).unwrap_or_default(),
        None => doc
            .layers
            .iter()
            .filter(|l| l.visible && !l.locked && doc.pattern_edit.as_ref().is_none_or(|e| e.layer == l.id))
            .flat_map(|l| l.children().into_iter().flatten())
            .collect(),
    };
    fn touches(n: &Node, r: Rect) -> bool {
        match &n.kind {
            NodeKind::Path { path, .. } => vectorcraft_geom::hit::intersects_rect(path, r),
            NodeKind::Layer { children, .. } | NodeKind::Group { children, .. } | NodeKind::Compound { children, .. } => {
                children.iter().any(|c| c.visible && touches(c, r))
            }
            _ => n.geometric_bounds().is_some_and(|b| b.intersect(r).area() > 0.0 || r.contains(b.origin())),
        }
    }
    fn collect_leaves(n: &Node, r: Rect, out: &mut Vec<NodeId>) {
        match &n.kind {
            NodeKind::Layer { children, .. } | NodeKind::Group { children, .. } => {
                for c in children {
                    if c.visible && !c.locked {
                        collect_leaves(c, r, out);
                    }
                }
            }
            _ => {
                if touches(n, r) {
                    out.push(n.id);
                }
            }
        }
    }
    for t in tops {
        if !t.visible || t.locked {
            continue;
        }
        if leaves {
            collect_leaves(t, r, &mut out);
        } else if touches(t, r) {
            out.push(t.id);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Appearance, Node};
    use std::sync::Arc;
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_geom::shapes;

    #[test]
    fn hits_topmost() {
        let mut d = Document::new(200.0, 200.0);
        let l = d.layers[0].id;
        let a = d.alloc_id();
        d.insert(Some(l), 9, Node::path(a, shapes::rectangle(Rect::new(0.0, 0.0, 50.0, 50.0)), Appearance::default_art())).unwrap();
        let b = d.alloc_id();
        d.insert(Some(l), 9, Node::path(b, shapes::rectangle(Rect::new(25.0, 25.0, 75.0, 75.0)), Appearance::default_art())).unwrap();
        let h = hit_test(&d, Point::new(30.0, 30.0), HitOptions::default()).unwrap();
        assert_eq!(h.leaf, b);
        assert_eq!(h.kind, HitKind::Fill);
        let h = hit_test(&d, Point::new(10.0, 10.0), HitOptions::default()).unwrap();
        assert_eq!(h.leaf, a);
        assert!(hit_test(&d, Point::new(150.0, 150.0), HitOptions::default()).is_none());
        // Outline mode: interior misses.
        assert!(hit_test(&d, Point::new(10.0, 10.0), HitOptions { outline: true, ..Default::default() }).is_none());
        assert!(hit_test(&d, Point::new(0.5, 10.0), HitOptions { outline: true, ..Default::default() }).is_some());
    }

    #[test]
    fn unfilled_interior_misses() {
        let mut d = Document::new(200.0, 200.0);
        let l = d.layers[0].id;
        let a = d.alloc_id();
        d.insert(
            Some(l),
            9,
            Node::path(a, shapes::rectangle(Rect::new(0.0, 0.0, 50.0, 50.0)), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 1.0)),
        )
        .unwrap();
        assert!(hit_test(&d, Point::new(25.0, 25.0), HitOptions::default()).is_none());
        assert!(hit_test(&d, Point::new(50.5, 25.0), HitOptions::default()).is_some());
    }

    #[test]
    fn group_top_object() {
        let mut d = Document::new(200.0, 200.0);
        let l = d.layers[0].id;
        let a = d.alloc_id();
        let g = d.alloc_id();
        let p = Node::path(a, shapes::rectangle(Rect::new(0.0, 0.0, 50.0, 50.0)), Appearance::default_art());
        d.insert(Some(l), 0, Node::group(g, vec![Arc::new(p)])).unwrap();
        let h = hit_test(&d, Point::new(10.0, 10.0), HitOptions::default()).unwrap();
        assert_eq!(h.leaf, a);
        assert_eq!(h.top_object(None), g);
        assert_eq!(h.top_object(Some(g)), a);
        assert_eq!(marquee(&d, Rect::new(-5.0, -5.0, 5.0, 5.0), None, false), vec![g]);
        assert_eq!(marquee(&d, Rect::new(-5.0, -5.0, 5.0, 5.0), None, true), vec![a]);
        assert!(marquee(&d, Rect::new(100.0, 100.0, 105.0, 105.0), None, false).is_empty());
    }

    #[test]
    fn locked_and_hidden_skip() {
        let mut d = Document::new(200.0, 200.0);
        let l = d.layers[0].id;
        let a = d.alloc_id();
        d.insert(Some(l), 0, Node::path(a, shapes::rectangle(Rect::new(0.0, 0.0, 50.0, 50.0)), Appearance::default_art())).unwrap();
        d.node_mut(a).unwrap().locked = true;
        assert!(hit_test(&d, Point::new(10.0, 10.0), HitOptions::default()).is_none());
        d.node_mut(a).unwrap().locked = false;
        d.node_mut(l).unwrap().visible = false;
        assert!(hit_test(&d, Point::new(10.0, 10.0), HitOptions::default()).is_none());
    }
}
