//! Swatch lookups across colour groups and the walks over the paints that link to swatches.
//!
//! A swatch lives either in `Document::swatches` or in one of `Document::swatch_groups`; every
//! lookup here covers both. Solid paints link to a global swatch by name (`Paint::Solid.swatch`);
//! [`Document::map_solid_paints`] rewrites those links (and colours) wherever art can hold them,
//! copying only the nodes that change.

use std::sync::Arc;

use vectorcraft_color::{Color, Paint, Swatch};

use crate::{AppearanceItem, Document, Node, NodeId, NodeKind};

impl Document {
    /// Every swatch: the ungrouped ones first, then each colour group's in order.
    pub fn swatches_iter(&self) -> impl Iterator<Item = &Swatch> {
        self.swatches.iter().chain(self.swatch_groups.iter().flat_map(|g| g.swatches.iter()))
    }
    pub fn swatches_iter_mut(&mut self) -> impl Iterator<Item = &mut Swatch> {
        self.swatches.iter_mut().chain(self.swatch_groups.iter_mut().flat_map(|g| g.swatches.iter_mut()))
    }
    pub fn swatch(&self, name: &str) -> Option<&Swatch> {
        self.swatches_iter().find(|s| s.name == name)
    }
    pub fn swatch_mut(&mut self, name: &str) -> Option<&mut Swatch> {
        self.swatches_iter_mut().find(|s| s.name == name)
    }
    /// Index of the colour group holding swatch `name` (`None` when ungrouped or missing).
    pub fn swatch_group_of(&self, name: &str) -> Option<usize> {
        self.swatch_groups.iter().position(|g| g.swatches.iter().any(|s| s.name == name))
    }
    /// Is `name` used by a swatch or a colour group (they share one namespace)?
    pub fn swatch_name_taken(&self, name: &str) -> bool {
        self.swatch(name).is_some() || self.swatch_groups.iter().any(|g| g.name == name)
    }
    /// Remove swatch `name` from wherever it lives.
    pub fn remove_swatch(&mut self, name: &str) -> Option<Swatch> {
        if let Some(i) = self.swatches.iter().position(|s| s.name == name) {
            return Some(self.swatches.remove(i));
        }
        let g = self.swatch_group_of(name)?;
        let list = &mut self.swatch_groups[g].swatches;
        let i = list.iter().position(|s| s.name == name)?;
        Some(list.remove(i))
    }

    /// Visit every solid paint (fills, strokes and text runs) in the art, symbol definitions,
    /// pattern tiles and graphic styles. `f(colour, link)` may change both and returns true when it
    /// did. Only nodes holding a changed paint (and the paths to them) are copied. Returns the
    /// number of changed paints.
    pub fn map_solid_paints(&mut self, f: &mut dyn FnMut(&mut Color, &mut Option<String>) -> bool) -> usize {
        let mut n = map_trees(&mut self.layers, f);
        for s in &mut self.symbols {
            n += map_tree(&mut s.art, f);
        }
        for p in &mut self.patterns {
            n += map_trees(&mut p.art, f);
        }
        for gs in &mut self.graphic_styles {
            for it in &mut gs.appearance.items {
                n += usize::from(map_solid(item_paint_mut(it), f));
            }
        }
        n
    }

    /// [`Document::map_solid_paints`] limited to the subtrees of `ids` (each paint visited once even
    /// when an id is inside another).
    pub fn map_solid_paints_in(&mut self, ids: &[NodeId], f: &mut dyn FnMut(&mut Color, &mut Option<String>) -> bool) -> usize {
        let roots: Vec<NodeId> =
            ids.iter().copied().filter(|id| !self.ancestry(*id).is_some_and(|a| a[..a.len() - 1].iter().any(|p| ids.contains(p)))).collect();
        let mut n = 0;
        for id in roots {
            let Some(node) = self.node(id) else { continue };
            if let Some((new, count)) = map_node(node, f)
                && let Some(slot) = self.node_mut(id)
            {
                *slot = new;
                n += count;
            }
        }
        n
    }
}

/// Every colour in the subtree of `n` with the swatch it links to: solid fills, strokes and text
/// runs (with their link) and gradient stops (unlinked), in paint order.
pub fn node_colors(n: &Node, f: &mut dyn FnMut(&Color, Option<&str>)) {
    n.walk(&mut |m| {
        for p in node_paints(m) {
            match p {
                Paint::Solid { color, swatch } => f(color, swatch.as_deref()),
                Paint::Gradient(g) => g.gradient.stops.iter().for_each(|s| f(&s.color, None)),
                _ => {}
            }
        }
    });
}

fn item_paint(it: &AppearanceItem) -> &Paint {
    match it {
        AppearanceItem::Fill(l) => &l.paint,
        AppearanceItem::Stroke(l) => &l.paint,
    }
}

fn item_paint_mut(it: &mut AppearanceItem) -> &mut Paint {
    match it {
        AppearanceItem::Fill(l) => &mut l.paint,
        AppearanceItem::Stroke(l) => &mut l.paint,
    }
}

/// The paints a node holds itself (not its children): appearance items, then text runs.
fn node_paints(n: &Node) -> impl Iterator<Item = &Paint> {
    let runs = match &n.kind {
        NodeKind::Text(t) => t.runs.as_slice(),
        _ => &[],
    };
    n.appearance.items.iter().map(item_paint).chain(runs.iter().flat_map(|r| [&r.style.fill, &r.style.stroke]))
}

/// Mutable [`node_paints`], in the same order.
fn node_paints_mut(n: &mut Node) -> impl Iterator<Item = &mut Paint> {
    let runs = match &mut n.kind {
        NodeKind::Text(t) => t.runs.as_mut_slice(),
        _ => &mut [],
    };
    n.appearance.items.iter_mut().map(item_paint_mut).chain(runs.iter_mut().flat_map(|r| [&mut r.style.fill, &mut r.style.stroke]))
}

/// Apply `f` to a solid paint; true when it changed.
fn map_solid(p: &mut Paint, f: &mut dyn FnMut(&mut Color, &mut Option<String>) -> bool) -> bool {
    match p {
        Paint::Solid { color, swatch } => f(color, swatch),
        _ => false,
    }
}

/// `n` with `f` applied to its subtree's solid paints, or `None` when nothing changed (then nothing
/// was copied). Also returns the number of changed paints.
fn map_node(n: &Node, f: &mut dyn FnMut(&mut Color, &mut Option<String>) -> bool) -> Option<(Node, usize)> {
    // Try each own paint on a scratch copy first, so unchanged nodes are never cloned.
    let mut changes: Vec<(usize, Color, Option<String>)> = vec![];
    for (i, p) in node_paints(n).enumerate() {
        if let Paint::Solid { color, swatch } = p {
            let (mut c, mut s) = (*color, swatch.clone());
            if f(&mut c, &mut s) {
                changes.push((i, c, s));
            }
        }
    }
    let mut count = changes.len();
    let mut out: Option<Node> = None;
    if !changes.is_empty() {
        let mut m = n.clone();
        let mut changes = changes.into_iter().peekable();
        for (i, p) in node_paints_mut(&mut m).enumerate() {
            if changes.peek().is_some_and(|c| c.0 == i)
                && let (Some((_, c, s)), Paint::Solid { color, swatch }) = (changes.next(), p)
            {
                *color = c;
                *swatch = s;
            }
        }
        out = Some(m);
    }
    if let Some(ch) = n.children() {
        for (i, c) in ch.iter().enumerate() {
            if let Some((new, k)) = map_node(c, f) {
                let m = out.get_or_insert_with(|| n.clone());
                if let Some(slot) = m.children_mut().and_then(|v| v.get_mut(i)) {
                    *slot = Arc::new(new);
                }
                count += k;
            }
        }
    }
    out.map(|m| (m, count))
}

fn map_tree(n: &mut Arc<Node>, f: &mut dyn FnMut(&mut Color, &mut Option<String>) -> bool) -> usize {
    match map_node(n, f) {
        Some((new, count)) => {
            *n = Arc::new(new);
            count
        }
        None => 0,
    }
}

fn map_trees(v: &mut [Arc<Node>], f: &mut dyn FnMut(&mut Color, &mut Option<String>) -> bool) -> usize {
    v.iter_mut().map(|n| map_tree(n, f)).sum()
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::SwatchGroup;
    use vectorcraft_geom::{Rect, shapes};

    use super::*;
    use crate::Appearance;

    fn linked(color: Color, name: &str) -> Paint {
        Paint::Solid { color, swatch: Some(name.into()) }
    }

    /// A document with a grouped global swatch "Brand", a rectangle filled with it and an unlinked one.
    fn doc() -> (Document, NodeId, NodeId) {
        let mut d = Document::new(100.0, 100.0);
        let red = Color::rgb(1.0, 0.0, 0.0);
        d.swatch_groups.push(SwatchGroup {
            name: "Mine".into(),
            swatches: vec![Swatch { name: "Brand".into(), paint: Paint::solid(red), global: true, spot: false }],
        });
        let layer = d.layers[0].id;
        let mut ids = vec![];
        for paint in [linked(red, "Brand"), Paint::solid(red)] {
            let id = d.alloc_id();
            let r = shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0));
            d.insert(Some(layer), usize::MAX, Node::path(id, r, Appearance::basic(paint, Paint::None, 0.0))).unwrap();
            ids.push(id);
        }
        (d, ids[0], ids[1])
    }

    #[test]
    fn lookups_cover_colour_groups() {
        let (mut d, _, _) = doc();
        let total = d.swatches.len() + d.swatch_groups.iter().map(|g| g.swatches.len()).sum::<usize>();
        assert_eq!(d.swatches_iter().count(), total);
        assert!(d.swatch("Brand").is_some_and(|s| s.global));
        assert_eq!(d.swatch_group_of("Brand").map(|g| d.swatch_groups[g].name.as_str()), Some("Mine"));
        d.swatch_mut("Brand").unwrap().spot = true;
        assert!(d.swatch("Brand").unwrap().spot);
        assert!(d.swatch_name_taken("Mine") && d.swatch_name_taken("Brand") && !d.swatch_name_taken("Nope"));
        assert_eq!(d.remove_swatch("Brand").map(|s| s.name), Some("Brand".into()));
        assert!(d.swatch("Brand").is_none() && d.remove_swatch("Brand").is_none());
    }

    #[test]
    fn mapping_copies_only_changed_nodes() {
        let (mut d, a, b) = doc();
        let before = d.clone();
        let blue = Color::rgb(0.0, 0.0, 1.0);
        let n = d.map_solid_paints(&mut |c, s| {
            if s.as_deref() != Some("Brand") {
                return false;
            }
            *c = blue;
            true
        });
        assert_eq!(n, 1);
        assert_eq!(d.node(a).unwrap().appearance.fill_paint(), linked(blue, "Brand"));
        assert_eq!(d.node(b).unwrap().appearance.fill_paint(), Paint::solid(Color::rgb(1.0, 0.0, 0.0)));
        let unchanged = |d: &Document| d.layers[0].children().unwrap()[1].clone();
        assert!(Arc::ptr_eq(&unchanged(&d), &unchanged(&before)), "the unlinked rectangle is shared, not copied");
        // Nothing to change: the tree is untouched.
        let again = d.clone();
        assert_eq!(d.map_solid_paints(&mut |_, _| false), 0);
        assert!(Arc::ptr_eq(&d.layers[0], &again.layers[0]));
    }

    #[test]
    fn mapping_within_ids_visits_each_paint_once() {
        let (mut d, a, b) = doc();
        let layer = d.layers[0].id;
        let mut seen = 0;
        let n = d.map_solid_paints_in(&[layer, a], &mut |_, s| {
            seen += 1;
            s.take().is_some()
        });
        assert_eq!((n, seen), (1, 2), "the layer covers `a`: each fill is visited once");
        assert_eq!(d.node(a).unwrap().appearance.fill_paint(), Paint::solid(Color::rgb(1.0, 0.0, 0.0)));
        let mut colors = vec![];
        node_colors(d.node(b).unwrap(), &mut |c, s| colors.push((*c, s.map(str::to_string))));
        assert_eq!(colors, [(Color::rgb(1.0, 0.0, 0.0), None)]);
    }
}
