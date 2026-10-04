//! Baking live geometry effects for export.
//!
//! SVG, PDF and the clipboard have no notion of Illustrator's live effects, so exporters receive a
//! copy of the document in which every geometry effect — object level, per fill/stroke and
//! Effect → Pathfinder, and those on type, images, symbol instances and live objects (through their
//! outlines, [`reshape`]) — is evaluated exactly like the renderer does. Raster effects (shadows,
//! glows, blur) stay on the objects for the exporter to translate (e.g. SVG filters).

use std::sync::Arc;

use vectorcraft_doc::{Appearance, AppearanceItem, Document, Node, NodeKind};
use vectorcraft_geom::{FillRule, PathData};

use crate::{
    GeomContext, apply_geometry_with, has_geometry, has_pathfinder, is_geometry, is_pathfinder, needs_outline, pathfinder_children, reshape,
};

fn item_effects(item: &AppearanceItem) -> &[vectorcraft_doc::Effect] {
    match item {
        AppearanceItem::Fill(f) => &f.effects,
        AppearanceItem::Stroke(s) => &s.effects,
    }
}

fn clear_item_effects(item: &mut AppearanceItem) {
    match item {
        AppearanceItem::Fill(f) => f.effects.retain(|e| !is_geometry(&e.id)),
        AppearanceItem::Stroke(s) => s.effects.retain(|e| !is_geometry(&e.id)),
    }
}

/// Does anything in `n`'s subtree need baking?
pub fn needs_bake(n: &Node) -> bool {
    has_pathfinder(n)
        || has_geometry(&n.appearance.effects)
        || n.appearance.items.iter().any(|i| has_geometry(item_effects(i)))
        || n.children().is_some_and(|ch| ch.iter().any(|c| needs_bake(c)))
}

fn geometry(n: &Node) -> Option<(PathData, FillRule)> {
    match &n.kind {
        NodeKind::Path { path, rule, guide: false, .. } => Some((path.clone(), *rule)),
        NodeKind::Compound { children, rule } => {
            Some((PathData::new(children.iter().filter_map(|c| c.path_data()).flat_map(|p| p.subpaths.iter().cloned()).collect()), *rule))
        }
        _ => None,
    }
}

fn apply(effects: &[vectorcraft_doc::Effect], path: &PathData, ctx: &GeomContext) -> PathData {
    if !has_geometry(effects) || path.is_empty() {
        return path.clone();
    }
    let b = vectorcraft_geom::Shape::bounding_box(&path.to_bezpath());
    apply_geometry_with(effects, path, b, ctx)
}

/// A path node of `kind`'s flavour (compound when the result has several subpaths).
fn path_kind(d: &mut Document, path: PathData, rule: FillRule) -> NodeKind {
    if path.subpaths.len() > 1 {
        let children = path.subpaths.into_iter().map(|sp| Arc::new(Node::path(d.alloc_id(), PathData::single(sp), Appearance::default()))).collect();
        NodeKind::Compound { children, rule }
    } else {
        NodeKind::Path { path, rule, live: None, clipping: false, guide: false }
    }
}

/// `n` with its geometry effects evaluated (`None` = unchanged).
fn bake_node(d: &mut Document, n: &Node) -> Option<Node> {
    if !needs_bake(n) {
        return None;
    }
    // Type, images, symbol instances and live objects: reshaped through their outlines.
    if needs_outline(n) && has_geometry(&n.appearance.effects) {
        let symbol = match &n.kind {
            NodeKind::SymbolInstance { symbol, .. } => d.symbols.iter().find(|s| s.name == *symbol).map(|s| s.art.clone()),
            _ => None,
        };
        let m = reshape(n, symbol.as_deref())?;
        return Some(bake_node(d, &m).unwrap_or(m));
    }
    if let Some(result) = pathfinder_children(n, None) {
        let mut m = n.clone();
        m.appearance.effects.retain(|e| !is_pathfinder(&e.id));
        let children = result
            .into_iter()
            .map(|c| {
                let mut c = Arc::unwrap_or_clone(c);
                c.id = d.alloc_id();
                Arc::new(bake_node(d, &c).unwrap_or(c))
            })
            .collect();
        if let Some(ch) = m.children_mut() {
            *ch = children;
        }
        return Some(m);
    }
    if let Some(ch) = n.children() {
        if matches!(n.kind, NodeKind::Compound { .. })
            && (has_geometry(&n.appearance.effects) || n.appearance.items.iter().any(|i| has_geometry(item_effects(i))))
        {
            return bake_leaf(d, n);
        }
        let mut m = n.clone();
        let baked: Vec<Arc<Node>> = ch.iter().map(|c| bake_node(d, c).map(Arc::new).unwrap_or_else(|| c.clone())).collect();
        if let Some(ch) = m.children_mut() {
            *ch = baked;
        }
        return Some(m);
    }
    bake_leaf(d, n)
}

fn bake_leaf(d: &mut Document, n: &Node) -> Option<Node> {
    let (base, rule) = geometry(n)?;
    let w = n.appearance.stroke_width();
    let ctx = GeomContext { stroke_width: if w > 0.0 { w } else { 1.0 } };
    let g = apply(&n.appearance.effects, &base, &ctx);
    let mut m = n.clone();
    m.appearance.effects.retain(|e| !is_geometry(&e.id));
    if !n.appearance.items.iter().any(|i| has_geometry(item_effects(i))) {
        m.kind = path_kind(d, g, rule);
        return Some(m);
    }
    // Per-item geometry: one path per fill/stroke, grouped under the object's transparency and
    // raster effects (what Expand Appearance produces).
    let children = n
        .appearance
        .items
        .iter()
        .map(|item| {
            let ig = apply(item_effects(item), &g, &ctx);
            let mut it = item.clone();
            clear_item_effects(&mut it);
            let id = d.alloc_id();
            let kind = path_kind(d, ig, rule);
            Arc::new(Node {
                id,
                name: None,
                appearance: Appearance { items: vec![it], effects: vec![] },
                opacity: 1.0,
                blend: Default::default(),
                isolate: false,
                knockout: false,
                mask: None,
                trace: None,
                wrap: None,
                graph: None,
                kind,
                ..n.clone()
            })
        })
        .collect();
    m.appearance.items.clear();
    m.kind = NodeKind::Group { children, clip: false };
    Some(m)
}

/// A copy of `doc` with every live geometry effect baked into plain paths, or `None` when the
/// document has none (export it as is).
pub fn bake_document(doc: &Document) -> Option<Document> {
    if !doc.layers.iter().any(|l| needs_bake(l)) {
        return None;
    }
    let mut d = doc.clone();
    let layers = doc.layers.clone();
    d.layers = layers.iter().map(|l| bake_node(&mut d, l).map(Arc::new).unwrap_or_else(|| l.clone())).collect();
    Some(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use vectorcraft_doc::{Effect, NodeId};
    use vectorcraft_geom::{Rect, shapes};

    fn fx(id: &str, params: serde_json::Value) -> Effect {
        Effect { id: id.into(), params, visible: true }
    }

    fn doc_with(n: Node) -> Document {
        let mut d = Document::new(400.0, 400.0);
        let l = d.layers[0].id;
        d.insert(Some(l), 0, n).unwrap();
        d
    }

    #[test]
    fn plain_documents_are_not_copied() {
        let mut d = Document::new(100.0, 100.0);
        let id = d.alloc_id();
        let d = doc_with(Node::path(id, shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), Appearance::default_art()));
        assert!(bake_document(&d).is_none());
    }

    #[test]
    fn object_effects_become_geometry_and_raster_effects_stay() {
        let mut n = Node::path(NodeId(50), shapes::rectangle(Rect::new(0.0, 0.0, 100.0, 100.0)), Appearance::default_art());
        n.appearance.effects.push(fx("path.offsetPath", json!({"offset": 10.0})));
        n.appearance.effects.push(fx("stylize.dropShadow", json!({})));
        let d = bake_document(&doc_with(n)).unwrap();
        let m = d.node(NodeId(50)).unwrap();
        let b = m.geometric_bounds().unwrap();
        assert!((b.width() - 120.0).abs() < 0.5, "{b:?}");
        assert_eq!(m.appearance.effects.len(), 1);
        assert_eq!(m.appearance.effects[0].id, "stylize.dropShadow");
    }

    #[test]
    fn item_effects_split_into_one_path_per_item() {
        let mut n = Node::path(NodeId(50), shapes::rectangle(Rect::new(0.0, 0.0, 100.0, 100.0)), Appearance::default_art());
        if let AppearanceItem::Stroke(s) = &mut n.appearance.items[1] {
            s.effects.push(fx("path.offsetPath", json!({"offset": 5.0})));
        }
        let d = bake_document(&doc_with(n)).unwrap();
        let m = d.node(NodeId(50)).unwrap();
        let ch = m.children().unwrap();
        assert_eq!(ch.len(), 2);
        assert!((ch[0].geometric_bounds().unwrap().width() - 100.0).abs() < 1e-6);
        assert!((ch[1].geometric_bounds().unwrap().width() - 110.0).abs() < 0.5);
        assert!(ch[0].id != ch[1].id && ch[0].id != NodeId(50));
    }
}
