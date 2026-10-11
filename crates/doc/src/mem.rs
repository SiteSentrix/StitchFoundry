//! How much memory a document keeps that another doesn't share: what an undo step costs.
//!
//! Documents share structure (`Arc<Node>` children, [`crate::Shared`] resources), so an old
//! version kept in the undo history holds only what the edits after it replaced. The estimate
//! walks the old version beside the newer one and visits only what isn't shared: a subtree whose
//! `Arc` the newer document also holds costs nothing and isn't entered, so the walk is about as
//! long as the edit was (plus a pass over each changed child list). It counts the heap the values
//! own (struct sizes, vector capacities, strings, JSON) plus a typical allocator overhead per
//! allocation, without following every last field: an estimate in the right ballpark, for a
//! memory budget, not an exact figure.

use std::collections::HashSet;
use std::mem::size_of;
use std::sync::Arc;

use serde_json::Value;
use vectorcraft_color::{Paint, Swatch, SwatchGroup};
use vectorcraft_geom::{Anchor, PathData, SubPath};

use crate::text::{ParaStyle, TextObject, TextRun, TextStyleDef};
use crate::{Appearance, AppearanceItem, Document, Effect, GraphicStyle, Node, NodeKind, SavedSelection, Shared};

/// Bookkeeping and rounding a typical allocator adds to each allocation.
const ALLOC: usize = 16;
/// An `Arc`'s two reference counts.
const ARC: usize = 16;
/// The kinds of node this doesn't break down (meshes, envelopes' settings, placed documents…):
/// a typical size of what they own besides the node.
const OTHER_KIND: usize = 256;

impl Document {
    /// Roughly how many bytes of heap this document keeps that `newer` doesn't share with it: what
    /// keeping this version in the undo history costs while `newer` (a later version) is kept too.
    /// Visits only the parts that aren't shared (see the [module docs](self)).
    pub fn retained_bytes(&self, newer: &Document) -> usize {
        if std::ptr::eq(self, newer) {
            return 0;
        }
        let mut w = Walk { newer, seen: None, bytes: size_of::<Document>() + ALLOC + ARC };
        w.list(&self.layers, Some(&newer.layers));
        for (i, s) in self.symbols.iter().enumerate() {
            w.bytes += s.name.capacity();
            w.tree(&s.art, newer.symbols.get(i).map(|n| &n.art));
        }
        w.bytes += vec_bytes(&self.symbols);
        for (i, p) in self.patterns.iter().enumerate() {
            w.bytes += p.name.capacity();
            w.list(&p.art, newer.patterns.get(i).map(|n| &n.art));
        }
        w.bytes += vec_bytes(&self.patterns);
        w.bytes += self.resources_bytes(newer);
        w.bytes
    }

    /// The document-wide lists and settings: a copy of the document copies those that aren't
    /// [`Shared`]; the shared ones count only when `newer` has its own.
    fn resources_bytes(&self, newer: &Document) -> usize {
        fn shared<T>(a: &Shared<T>, b: &Shared<T>, f: impl FnOnce(&T) -> usize) -> usize {
            if Shared::ptr_eq(a, b) { 0 } else { ALLOC + ARC + f(a) }
        }
        let mut n = 0;
        n += vec_bytes(&self.artboards) + self.artboards.iter().map(|a| a.name.capacity()).sum::<usize>();
        n += vec_bytes(&self.guides) + vec_bytes(&self.views) + vec_bytes(&self.slices) + vec_bytes(&self.assets);
        n += vec_bytes(&self.text_threads) + self.text_threads.iter().map(vec_bytes).sum::<usize>();
        n += self.images.iter().map(|(k, b)| 4 * size_of::<usize>() + k.capacity() + b.mime.capacity() + size_of_val(b)).sum::<usize>();
        n += map_bytes(&self.export_settings) + map_bytes(&self.extra) + self.print_setup.as_ref().map_or(0, json_bytes);
        n += self.title.capacity();
        n += shared(&self.swatches, &newer.swatches, |s| swatches_bytes(s));
        n += shared(&self.swatch_groups, &newer.swatch_groups, |g| {
            vec_bytes(g) + g.iter().map(|g: &SwatchGroup| g.name.capacity() + swatches_bytes(&g.swatches)).sum::<usize>()
        });
        n += shared(&self.graphic_styles, &newer.graphic_styles, |g| {
            vec_bytes(g) + g.iter().map(|g: &GraphicStyle| g.name.capacity() + appearance_heap(&g.appearance)).sum::<usize>()
        });
        let styles = |s: &Vec<TextStyleDef>| vec_bytes(s) + s.iter().map(|s| s.name.capacity() + map_bytes(&s.attrs)).sum::<usize>();
        n += shared(&self.char_styles, &newer.char_styles, styles);
        n += shared(&self.para_styles, &newer.para_styles, styles);
        n += shared(&self.saved_selections, &newer.saved_selections, |s| {
            vec_bytes(s) + s.iter().map(|s: &SavedSelection| s.name.capacity() + vec_bytes(&s.objects)).sum::<usize>()
        });
        n += shared(&self.variables, &newer.variables, |v| {
            // Map entries: the key and value plus a share of the tree's nodes.
            let entry = 2 * size_of::<usize>();
            vec_bytes(&v.variables)
                + v.variables.iter().map(|v| v.name.capacity()).sum::<usize>()
                + vec_bytes(&v.datasets)
                + v.datasets.iter().map(|d| d.name.capacity() + d.values.keys().map(|k| entry + k.capacity() + 32).sum::<usize>()).sum::<usize>()
                + v.bindings.values().map(|b| entry + 8 + b.iter().map(|s| entry + s.capacity()).sum::<usize>()).sum::<usize>()
        });
        n += shared(&self.unknown, &newer.unknown, map_bytes);
        n
    }
}

/// One estimate in progress.
struct Walk<'a> {
    newer: &'a Document,
    /// Every node the newer document holds (by address), built the first time a node has no
    /// counterpart in place: then a node moved elsewhere (grouped, say) is still found shared.
    seen: Option<HashSet<usize>>,
    bytes: usize,
}

impl Walk<'_> {
    /// The nodes of `list` that `newer` (the counterpart list, if any) doesn't share.
    fn list(&mut self, list: &[Arc<Node>], newer: Option<&Vec<Arc<Node>>>) {
        for (i, n) in list.iter().enumerate() {
            self.tree(n, newer.and_then(|l| l.get(i)));
        }
    }

    /// Node `n` and what under it isn't shared; `newer` is the node in its place in the newer
    /// document, if any.
    fn tree(&mut self, n: &Arc<Node>, newer: Option<&Arc<Node>>) {
        if newer.is_some_and(|m| Arc::ptr_eq(n, m)) {
            return;
        }
        // In its place is another version of the same object (edited, moved): compare their
        // children in place. Else it may sit elsewhere in the newer document.
        let counterpart = newer.filter(|m| m.id == n.id);
        if counterpart.is_none() && self.newer_holds(n) {
            return;
        }
        self.bytes += node_bytes(n, counterpart.map(|m| &**m));
        if let Some(ch) = n.children() {
            self.list(ch, counterpart.and_then(|m| m.children()));
        }
        if let Some(mask) = &n.mask {
            self.tree(&mask.art, counterpart.and_then(|m| m.mask.as_ref()).map(|m| &m.art));
        }
    }

    fn newer_holds(&mut self, n: &Arc<Node>) -> bool {
        let newer = self.newer;
        let seen = self.seen.get_or_insert_with(|| {
            fn add(n: &Arc<Node>, out: &mut HashSet<usize>) {
                // A node already seen was entered already (a node shared within the document).
                if !out.insert(Arc::as_ptr(n) as usize) {
                    return;
                }
                for c in n.children().into_iter().flatten() {
                    add(c, out);
                }
                if let Some(m) = &n.mask {
                    add(&m.art, out);
                }
            }
            let mut out = HashSet::new();
            let roots = newer.layers.iter().chain(newer.symbols.iter().map(|s| &s.art)).chain(newer.patterns.iter().flat_map(|p| &p.art));
            for n in roots {
                add(n, &mut out);
            }
            out
        });
        seen.contains(&(Arc::as_ptr(n) as usize))
    }
}

/// One node's own heap: the node, and what it owns besides its children and mask art (counted
/// as nodes of their own). Its appearance counts unless `newer`, its newer version, shares it.
fn node_bytes(n: &Node, newer: Option<&Node>) -> usize {
    fn boxed<T>(b: &Option<Box<T>>) -> usize {
        b.as_ref().map_or(0, |_| ALLOC + size_of::<T>())
    }
    let mut b = ALLOC + ARC + size_of::<Node>();
    b += n.name.as_ref().map_or(0, String::capacity);
    if !newer.is_some_and(|m| Shared::ptr_eq(&m.appearance, &n.appearance)) {
        b += ALLOC + ARC + appearance_heap(&n.appearance);
    }
    b += boxed(&n.mask) + boxed(&n.graph) + boxed(&n.shaper) + boxed(&n.attrs) + boxed(&n.slice) + boxed(&n.perspective);
    b += n.trace.as_ref().map_or(0, |t| ALLOC + json_bytes(t));
    b += match &n.kind {
        NodeKind::Path { path, .. } => path_bytes(path),
        NodeKind::Layer { children, .. }
        | NodeKind::Group { children, .. }
        | NodeKind::Compound { children, .. }
        | NodeKind::Blend { children, .. }
        | NodeKind::Envelope { content: children, .. } => vec_bytes(children),
        NodeKind::Repeat(r) => vec_bytes(&r.source) + OTHER_KIND,
        NodeKind::Text(t) => ALLOC + text_bytes(t),
        NodeKind::Image(im) => im.key.capacity(),
        NodeKind::SymbolInstance { symbol, .. } => symbol.capacity(),
        _ => OTHER_KIND,
    };
    b
}

fn path_bytes(p: &PathData) -> usize {
    let anchors: usize =
        p.subpaths.iter().map(|s| if s.anchors.capacity() == 0 { 0 } else { ALLOC + s.anchors.capacity() * size_of::<Anchor>() }).sum();
    anchors + if p.subpaths.capacity() == 0 { 0 } else { ALLOC + p.subpaths.capacity() * size_of::<SubPath>() }
}

fn text_bytes(t: &TextObject) -> usize {
    size_of::<TextObject>()
        + vec_bytes(&t.runs)
        + t.runs.iter().map(|r: &TextRun| r.text.capacity()).sum::<usize>()
        + t.paras.capacity() * size_of::<ParaStyle>()
        + vec_bytes(&t.wrap)
        + vec_bytes(&t.cached_baselines)
}

/// The heap an appearance owns (not the appearance itself).
fn appearance_heap(a: &Appearance) -> usize {
    size_of::<Appearance>()
        + vec_bytes(&a.items)
        + a.items
            .iter()
            .map(|i| match i {
                AppearanceItem::Fill(f) => paint_heap(&f.paint),
                AppearanceItem::Stroke(s) => paint_heap(&s.paint),
            })
            .sum::<usize>()
        + vec_bytes(&a.effects)
        + a.effects.iter().map(|e: &Effect| e.id.capacity() + json_bytes(&e.params)).sum::<usize>()
}

fn paint_heap(p: &Paint) -> usize {
    match p {
        Paint::Solid { swatch, .. } => swatch.as_ref().map_or(0, String::capacity),
        Paint::Gradient(g) => ALLOC + size_of_val(&**g) + vec_bytes(&g.gradient.stops),
        Paint::Pattern { pattern, .. } => pattern.capacity(),
        Paint::None => 0,
    }
}

fn swatches_bytes(s: &[Swatch]) -> usize {
    ALLOC + std::mem::size_of_val(s) + s.iter().map(|w| w.name.capacity() + paint_heap(&w.paint)).sum::<usize>()
}

/// A vector's buffer (not what its elements own).
fn vec_bytes<T>(v: &Vec<T>) -> usize {
    if v.capacity() == 0 { 0 } else { ALLOC + v.capacity() * size_of::<T>() }
}

fn map_bytes(m: &serde_json::Map<String, Value>) -> usize {
    m.iter().map(|(k, v)| ALLOC + 4 * size_of::<usize>() + k.capacity() + size_of::<Value>() + json_bytes(v)).sum()
}

/// The heap a JSON value owns.
fn json_bytes(v: &Value) -> usize {
    match v {
        Value::String(s) => s.capacity(),
        Value::Array(a) => vec_bytes(a) + a.iter().map(json_bytes).sum::<usize>(),
        Value::Object(m) => map_bytes(m),
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::shapes;

    fn doc_with_rects(n: usize) -> Document {
        let mut d = Document::new(1000.0, 1000.0);
        let layer = d.layers[0].id;
        for i in 0..n {
            let id = d.alloc_id();
            let r = shapes::rectangle(kurbo::Rect::new(i as f64, 0.0, i as f64 + 10.0, 10.0));
            d.insert(Some(layer), usize::MAX, Node::path(id, r, Appearance::default_art())).unwrap();
        }
        d
    }

    #[test]
    fn an_unchanged_copy_keeps_nothing_of_its_own() {
        let d = doc_with_rects(10);
        let copy = d.clone();
        // Only the document itself and its own lists: no node, no shared resource.
        assert!(d.retained_bytes(&copy) < 2048, "{}", d.retained_bytes(&copy));
        assert_eq!(d.retained_bytes(&d), 0);
    }

    /// Moving every rectangle: each keeps its old node and anchors (its appearance stays shared).
    /// The process grew by about 775 bytes per moved rectangle when measured (#1244).
    #[test]
    fn moving_rectangles_costs_their_nodes() {
        let n = 1000;
        let before = doc_with_rects(n);
        let mut after = before.clone();
        let ids: Vec<_> = after.layers[0].children().unwrap().iter().map(|c| c.id).collect();
        for id in ids {
            let node = after.node_mut(id).unwrap();
            if let NodeKind::Path { path, .. } = &mut node.kind {
                for a in path.subpaths.iter_mut().flat_map(|s| s.anchors.iter_mut()) {
                    a.p.x += 5.0;
                }
            }
        }
        let per = before.retained_bytes(&after) / n;
        assert!((400..1200).contains(&per), "{per} bytes per moved rectangle");
    }

    #[test]
    fn a_grouped_object_is_found_shared() {
        let before = doc_with_rects(100);
        let mut after = before.clone();
        let ids: Vec<_> = after.layers[0].children().unwrap().iter().map(|c| c.id).collect();
        // Put the rectangles in a group (the same Arcs, elsewhere).
        let gid = after.alloc_id();
        let layer = Arc::make_mut(&mut after.layers[0]);
        let kids = std::mem::take(layer.children_mut().unwrap());
        layer.children_mut().unwrap().push(Arc::new(Node::group(gid, kids)));
        assert_eq!(ids.len(), 100);
        let bytes = before.retained_bytes(&after);
        assert!(bytes < 8192, "only the old layer and its child list: {bytes}");
    }

    #[test]
    fn a_resource_counts_only_when_changed() {
        let before = doc_with_rects(1);
        let mut after = before.clone();
        let same = before.retained_bytes(&after);
        after.swatches.push(before.swatches[1].clone());
        assert!(before.retained_bytes(&after) > same + 1000, "the old swatch list is the old version's own");
    }
}
