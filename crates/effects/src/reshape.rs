//! Object-level effects on objects that aren't paths: type, placed images, symbol instances and
//! live objects (blends, envelopes, meshes, repeats).
//!
//! Geometry effects reach them through outlines: [`outline_art`] turns the object into plain paths
//! (glyph outlines, the evaluated live object, the symbol's art) and [`reshape`] runs each effect
//! over all of them as one piece, with the whole art's bounds as the effect's reference box, so a
//! warp bends a line of type as a whole. An image is clipped by its reshaped frame. Raster effects
//! (shadows, glows, blur, feather) stay on the result for the renderer and exporters.

use std::sync::Arc;

use vectorcraft_doc::{Appearance, AppearanceItem, FillLayer, Node, NodeKind, StrokeLayer};
use vectorcraft_geom::{FillRule, PathData, Rect, shapes};

use crate::{GeomContext, apply_one, is_geometry, merged_params};

/// Can object-level effects apply to `n` only through [`outline_art`] / an offscreen layer (it is
/// neither a path nor a container)?
pub fn needs_outline(n: &Node) -> bool {
    matches!(
        n.kind,
        NodeKind::Text(_)
            | NodeKind::Image(_)
            | NodeKind::SymbolInstance { .. }
            | NodeKind::Blend { .. }
            | NodeKind::Envelope { .. }
            | NodeKind::Mesh(_)
            | NodeKind::Repeat(_)
    )
}

/// Type as glyph outlines in document space: one path per run with the run's fill and stroke,
/// then (when the object has its own fills/strokes) the whole outline with the object's stack,
/// painted over the characters as the renderer does.
pub fn outline_text(n: &Node) -> Option<Node> {
    let NodeKind::Text(t) = &n.kind else { return None };
    let layout = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t);
    let mut runs = vec![kurbo::BezPath::new(); t.runs.len()];
    let mut all = kurbo::BezPath::new();
    for g in &layout.glyphs {
        if let Some(r) = runs.get_mut(g.run) {
            r.extend(g.outline.iter());
        }
        all.extend(g.outline.iter());
    }
    let path = |bp: &kurbo::BezPath| PathData::from_bezpath(bp).transformed(t.xf);
    let mut children: Vec<Arc<Node>> = runs
        .iter()
        .zip(&t.runs)
        .filter(|(bp, _)| !bp.elements().is_empty())
        .map(|(bp, run)| {
            let mut ap = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(run.style.fill.clone()))], effects: vec![] };
            if !run.style.stroke.is_none() && run.style.stroke_width > 0.0 {
                ap.items.push(AppearanceItem::Stroke(StrokeLayer::new(run.style.stroke.clone(), run.style.stroke_width)));
            }
            Arc::new(Node::path(n.id, path(bp), ap))
        })
        .collect();
    if !n.appearance.items.is_empty() && !all.elements().is_empty() {
        let ap = Appearance { items: n.appearance.items.clone(), effects: vec![] };
        children.push(Arc::new(Node::path(n.id, path(&all), ap)));
    }
    Some(Node::group(n.id, children))
}

/// `n` as plain art that geometry effects can reshape, without its transparency and object
/// effects: type outlined ([`outline_text`]), live objects evaluated (type inside them outlined),
/// a symbol instance as `symbol_art` (the symbol's art, stained as the caller draws it) placed by
/// the instance's transform. `None` for other kinds (images, paths, containers).
pub fn outline_art(n: &Node, symbol_art: Option<&Node>) -> Option<Node> {
    let hook: &dyn Fn(&Node) -> Option<Node> = &outline_text;
    let mut art = match &n.kind {
        NodeKind::Text(_) => outline_text(n)?,
        NodeKind::SymbolInstance { xf, .. } => {
            let mut a = symbol_art?.clone();
            a.transform(*xf, false);
            Node::group(n.id, vec![Arc::new(a)])
        }
        _ if vectorcraft_doc::live::is_live(n) => vectorcraft_doc::live::expand_deep(n, Some(hook)),
        _ => return None,
    };
    outline_nested_text(&mut art);
    art.opacity = 1.0;
    art.blend = Default::default();
    art.isolate = false;
    // One element: its pieces never knock each other out.
    art.knockout = vectorcraft_doc::Knockout::Off;
    art.mask = None;
    art.appearance.effects.clear();
    Some(art)
}

/// Replace type anywhere inside `n` by its outlines.
fn outline_nested_text(n: &mut Node) {
    let Some(ch) = n.children_mut() else { return };
    for c in ch.iter_mut() {
        if let Some(o) = outline_text(c) {
            *c = Arc::new(o);
        } else if c.children().is_some() {
            outline_nested_text(Arc::make_mut(c));
        }
    }
}

/// `n` (see [`needs_outline`]) with its object-level geometry effects applied: its [`outline_art`]
/// reshaped as one piece, or for an image the image clipped by its reshaped frame. The result is
/// a group with `n`'s id, name, transparency, opacity mask and remaining (raster) effects; `None`
/// when `n` has no visible geometry effect or no art (a symbol instance without `symbol_art`).
pub fn reshape(n: &Node, symbol_art: Option<&Node>) -> Option<Node> {
    let fx: Vec<_> = n.appearance.effects.iter().filter(|e| e.visible && is_geometry(&e.id)).collect();
    if fx.is_empty() || !needs_outline(n) {
        return None;
    }
    let mut art = match &n.kind {
        NodeKind::Image(im) => {
            let frame = shapes::rectangle(Rect::new(0.0, 0.0, im.width as f64, im.height as f64)).transformed(im.xf);
            let mut clip = Node::path(n.id, frame, Appearance::default());
            if let NodeKind::Path { clipping, .. } = &mut clip.kind {
                *clipping = true;
            }
            let image = Node { appearance: Appearance::default(), ..Node::new(n.id, n.kind.clone()) };
            Node::new(n.id, NodeKind::Group { children: vec![Arc::new(clip), Arc::new(image)], clip: true })
        }
        _ => outline_art(n, symbol_art)?,
    };
    for e in fx {
        // Every leaf takes the effect with the whole art's bounds as its reference box.
        let Some(bounds) = art.geometric_bounds() else { break };
        let params = merged_params(&e.id, &e.params);
        for_each_leaf(&mut art, &mut |leaf| {
            let Some((path, rule)) = leaf_geometry(leaf) else { return };
            let out = apply_one(&e.id, &params, &path, bounds, &GeomContext::of(leaf));
            set_geometry(leaf, out, rule);
        });
    }
    // The art is a group with `n`'s id (a clip group for an image).
    let mut out = art;
    out.name = n.name.clone();
    out.opacity = n.opacity;
    out.blend = n.blend;
    out.isolate = n.isolate;
    // One element: its pieces never knock each other out.
    out.knockout = vectorcraft_doc::Knockout::Off;
    out.mask = n.mask.clone();
    out.appearance.effects = n.appearance.effects.iter().filter(|e| !is_geometry(&e.id)).cloned().collect();
    Some(out)
}

/// Run `f` on every path and compound path under `n` (guides skipped).
fn for_each_leaf(n: &mut Node, f: &mut dyn FnMut(&mut Node)) {
    match &mut n.kind {
        NodeKind::Path { guide: false, .. } | NodeKind::Compound { .. } => f(n),
        NodeKind::Group { children, .. } | NodeKind::Layer { children, .. } => {
            for c in children.iter_mut() {
                for_each_leaf(Arc::make_mut(c), f);
            }
        }
        _ => {}
    }
}

fn leaf_geometry(n: &Node) -> Option<(PathData, FillRule)> {
    match &n.kind {
        NodeKind::Path { path, rule, .. } => Some((path.clone(), *rule)),
        NodeKind::Compound { children, rule } => {
            Some((PathData::new(children.iter().filter_map(|c| c.path_data()).flat_map(|p| p.subpaths.iter().cloned()).collect()), *rule))
        }
        _ => None,
    }
}

/// Replace the geometry of path or compound `n` (a compound when `path` has several subpaths).
fn set_geometry(n: &mut Node, path: PathData, rule: FillRule) {
    let clipping = matches!(n.kind, NodeKind::Path { clipping: true, .. });
    n.kind = if path.subpaths.len() > 1 {
        let id = n.id;
        let children = path.subpaths.into_iter().map(|sp| Arc::new(Node::path(id, PathData::single(sp), Appearance::default()))).collect();
        NodeKind::Compound { children, rule }
    } else {
        NodeKind::Path { path, rule, live: None, clipping, guide: false }
    };
}
