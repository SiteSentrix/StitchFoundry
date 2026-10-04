//! The Graphic Styles panel: new, apply (replace or add), link, redefine, break link, rename,
//! delete, duplicate, sort and list.
//!
//! Applying a style links the targeted objects to it ([`Node::graphic_style`]). An object stays
//! linked while it keeps the style's look; editing its appearance or transparency breaks the link
//! ([`in_sync`]), and Redefine Graphic Style updates only the objects still linked.

use std::collections::HashMap;

use serde_json::{Value, json};
use vectorcraft_doc::{Appearance, DEFAULT_GRAPHIC_STYLE, Document, GraphicStyle, Node, NodeId, NodeKind};

use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graphicStyle.apply",
            "Apply Graphic Style",
            ["Window", "Graphic Styles"],
            None,
            "{name, add?: bool, ids?} give the objects (default: the selection; a group's contents) the style's appearance, and themselves its opacity, blend mode, isolate and knockout, and link them to it; add: true (Alt-click) adds its fills, strokes and effects on top of the existing appearance instead and unlinks",
            has_doc,
            style_apply
        ),
        cmd!(
            "graphicStyle.new",
            "New Graphic Style",
            ["Window", "Graphic Styles"],
            None,
            "{name?, id?} a style from object `id` or the first selected object: its appearance, opacity, blend mode, isolate and knockout (a group without fills or strokes of its own lends its topmost object's, type its characters'); links the object. Names are made unique → {name}",
            has_doc,
            style_new
        ),
        cmd!(
            "graphicStyle.delete",
            "Delete Graphic Style",
            ["Window", "Graphic Styles"],
            None,
            "{name} | {names: [name…]} delete styles; their objects keep their look but are unlinked",
            has_doc,
            style_delete
        ),
        cmd!("graphicStyle.duplicate", "Duplicate Graphic Style", ["Window", "Graphic Styles"], None, "{name} → {name}", has_doc, style_duplicate),
        cmd!(
            query "graphicStyle.list",
            "List Graphic Styles",
            [],
            None,
            "{} → {styles: [{id, name, fill, stroke, strokeWidth, fills, strokes, effects: [effect id…], opacity: 0..100, blend, isolate, knockout: \"on\"|\"off\"|\"neutral\", linked: [object ids]}], selected: the style the first selected object is linked to, or null}",
            has_doc,
            style_list
        ),
        cmd!(
            "graphicStyle.redefine",
            "Redefine Graphic Style",
            ["Window", "Appearance"],
            None,
            "{name?, id?} replace the style (default: the one the object was last linked to) with the look of object `id` or the first selected object; the objects still linked to it update, and that object becomes linked → {name}",
            has_doc,
            style_redefine
        ),
        cmd!(
            "graphicStyle.breakLink",
            "Break Link to Graphic Style",
            ["Window", "Graphic Styles"],
            None,
            "{ids?} unlink the objects (default: the selection) from their graphic style; they keep their look",
            has_doc,
            style_break_link
        ),
        cmd!(
            "graphicStyle.rename",
            "Rename Graphic Style",
            ["Window", "Graphic Styles"],
            None,
            "{name, to} (Graphic Style Options; names are unique and linked objects stay linked) → {name}",
            has_doc,
            style_rename
        ),
        cmd!(
            query "graphicStyle.unused",
            "Select All Unused",
            ["Window", "Graphic Styles"],
            None,
            "{} → {names: [the styles no object is linked to]} (the panel selects them)",
            has_doc,
            style_unused
        ),
        cmd!(
            "graphicStyle.sortByName",
            "Sort by Name",
            ["Window", "Graphic Styles"],
            None,
            "{} sort the styles by name (the Default Graphic Style stays first)",
            has_doc,
            style_sort
        ),
    ]
}

// ---------- links ----------

/// Does a style applied to `n` (`top`: `n` is the target itself) descend into its children? Groups
/// and layers style their contents, and so do other containers (blends…) inside them, while a
/// compound path takes it as a whole, as with paint commands ([`leaf_targets`]).
fn descends(n: &Node, top: bool) -> bool {
    match n.kind {
        NodeKind::Group { .. } | NodeKind::Layer { .. } => true,
        NodeKind::Compound { .. } => false,
        _ => !top && n.is_container(),
    }
}

/// The objects whose appearance a style applied to `n` sets: `n`, or its painted contents.
fn painted<'a>(n: &'a Node, top: bool, out: &mut Vec<&'a Node>) {
    match n.children() {
        Some(ch) if descends(n, top) => ch.iter().for_each(|c| painted(c, false, out)),
        _ => out.push(n),
    }
}

/// [`painted`], mutably (copying only the nodes on the way).
fn paint_mut(n: &mut Node, top: bool, f: &mut impl FnMut(&mut Node)) {
    if !descends(n, top) {
        return f(n);
    }
    for c in n.children_mut().into_iter().flatten() {
        paint_mut(std::sync::Arc::make_mut(c), false, f);
    }
}

/// Does `n` still have style `g`'s look (its transparency, and its appearance on every painted
/// object)? Linked objects that don't are no longer linked.
pub(crate) fn in_sync(n: &Node, g: &GraphicStyle) -> bool {
    let mut leaves = vec![];
    painted(n, true, &mut leaves);
    g.transparency_matches(n) && leaves.iter().all(|l| l.appearance == g.appearance)
}

/// The style `n` is linked to (and still looks like).
pub(crate) fn linked_style<'a>(d: &'a Document, n: &Node) -> Option<&'a GraphicStyle> {
    d.graphic_style_by_id(n.graphic_style?).filter(|g| in_sync(n, g))
}

/// Objects anywhere in the art that `f` accepts.
fn objects_where(d: &Document, mut f: impl FnMut(&Node) -> bool) -> Vec<NodeId> {
    let mut ids = vec![];
    for l in &d.layers {
        l.walk(&mut |n| {
            if f(n) {
                ids.push(n.id);
            }
        });
    }
    ids
}

/// The objects linked to each style id.
fn links(d: &Document) -> HashMap<u32, Vec<NodeId>> {
    let mut out: HashMap<u32, Vec<NodeId>> = HashMap::new();
    for l in &d.layers {
        l.walk(&mut |n| {
            if let Some(g) = linked_style(d, n) {
                out.entry(g.id).or_default().push(n.id);
            }
        });
    }
    out
}

/// The appearance a new or redefined style takes from `n`: its own. A group without fills or
/// strokes of its own lends those of its topmost painted object, and type its characters' fill
/// and stroke.
fn captured(n: &Node) -> Appearance {
    let mut ap = n.appearance.clone();
    if !ap.items.is_empty() {
        return ap;
    }
    match &n.kind {
        NodeKind::Text(t) => {
            if let Some(r) = t.runs.first() {
                ap.items = Appearance::basic(r.style.fill.clone(), r.style.stroke.clone(), r.style.stroke_width).items;
            }
        }
        _ => {
            let mut leaves = vec![];
            painted(n, true, &mut leaves);
            if let Some(top) = leaves.last().filter(|l| l.id != n.id) {
                // The topmost object's own effects apply before the group's.
                let top = captured(top);
                ap.items = top.items;
                ap.effects.splice(0..0, top.effects);
            }
        }
    }
    ap
}

/// Give `id` style `g` (its painted objects the appearance, itself the transparency) and link it.
fn apply_style(d: &mut Document, id: NodeId, g: &GraphicStyle) {
    if let Some(n) = d.node_mut(id) {
        paint_mut(n, true, &mut |l| l.appearance = g.appearance.clone());
        g.apply_transparency(n);
        n.graphic_style = Some(g.id);
    }
}

fn set_link(d: &mut Document, ids: &[NodeId], link: Option<u32>) {
    for id in ids {
        if let Some(n) = d.node_mut(*id) {
            n.graphic_style = link;
        }
    }
}

fn style_index(d: &Document, name: &str) -> Result<usize> {
    d.graphic_style_index(name).ok_or_else(|| EngineError::Other(format!("no graphic style `{name}`")))
}

/// The object a style is made from: `id`, else the first selected object.
fn source<'a>(s: &'a Session, p: &Value, cmd: &str) -> Result<(NodeId, &'a Node)> {
    let id = match id_param(p, "id") {
        Some(id) => id,
        None => super::edit::selected_roots(s)?.first().copied().ok_or_else(|| bad(cmd, "select an object (or pass `id`)"))?,
    };
    Ok((id, s.doc()?.doc.node(id).ok_or_else(|| bad(cmd, format!("no object {}", id.0)))?))
}

impl Session {
    /// The graphic style of the first selected object: the one it was last linked to, and whether
    /// it is still linked (still has that style's look).
    pub fn selection_graphic_style(&self) -> Option<(&GraphicStyle, bool)> {
        let st = self.active()?;
        let n = st.doc.node(*st.selection.objects.first()?)?;
        let g = st.doc.graphic_style_by_id(n.graphic_style?)?;
        Some((g, in_sync(n, g)))
    }
}

// ---------- commands ----------

fn style_apply(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("graphicStyle.apply", "missing name"))?;
    let i = style_index(&s.doc()?.doc, name)?;
    let add = bool_or(p, "add", false);
    let ids = super::appearance::appearance_targets(s, p)?;
    if ids.is_empty() {
        return Err(bad("graphicStyle.apply", "nothing selected (or pass `ids`)"));
    }
    s.edit("Apply Graphic Style", |d, _| {
        if add {
            let ap = d.graphic_styles[i].appearance.clone();
            for id in &ids {
                if let Some(n) = d.node_mut(*id) {
                    paint_mut(n, true, &mut |l| {
                        l.appearance.items.extend(ap.items.iter().cloned());
                        l.appearance.effects.extend(ap.effects.iter().cloned());
                    });
                    n.graphic_style = None;
                }
            }
        } else {
            d.graphic_style_id(i);
            let g = d.graphic_styles[i].clone();
            for id in &ids {
                apply_style(d, *id, &g);
            }
        }
        Ok(())
    })?;
    ok()
}

fn style_new(s: &mut Session, p: &Value) -> Result<Value> {
    let (src, n) = source(s, p, "graphicStyle.new")?;
    let d = &s.doc()?.doc;
    let name = match str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => unique_name(n, |x| d.graphic_style(x).is_some()),
        None => d.new_graphic_style_name(),
    };
    let g = GraphicStyle { id: d.next_graphic_style_id(), ..GraphicStyle::of(name.clone(), captured(n), n) };
    s.edit("New Graphic Style", |d, _| {
        set_link(d, &[src], Some(g.id));
        d.graphic_styles.push(g);
        Ok(())
    })?;
    Ok(json!({ "name": name }))
}

fn style_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let names: Vec<String> = match (p.get("names").and_then(Value::as_array), str_param(p, "name")) {
        (Some(a), _) => a.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        (None, Some(n)) => vec![n.to_string()],
        (None, None) => return Err(bad("graphicStyle.delete", "missing name")),
    };
    s.edit(if names.len() > 1 { "Delete Graphic Styles" } else { "Delete Graphic Style" }, |d, _| {
        for name in &names {
            let i = style_index(d, name)?;
            let id = d.graphic_styles.remove(i).id;
            if id != 0 {
                let ids = objects_where(d, |n| n.graphic_style == Some(id));
                set_link(d, &ids, None);
            }
        }
        Ok(())
    })?;
    ok()
}

fn style_duplicate(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("graphicStyle.duplicate", "missing name"))?.to_string();
    let new = s.edit("Duplicate Graphic Style", |d, _| {
        let pos = style_index(d, &name)?;
        let nm = unique_name(&format!("{name} copy"), |n| d.graphic_style(n).is_some());
        let g = GraphicStyle { name: nm.clone(), id: d.next_graphic_style_id(), ..d.graphic_styles[pos].clone() };
        d.graphic_styles.insert(pos + 1, g);
        Ok(nm)
    })?;
    Ok(json!({"name": new}))
}

fn style_list(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let links = links(d);
    let styles: Vec<Value> = d
        .graphic_styles
        .iter()
        .map(|g| {
            let ap = &g.appearance;
            let count = |fill: bool| ap.items.iter().filter(|i| i.is_fill() == fill).count();
            json!({
                "id": g.id,
                "name": g.name,
                "fill": ap.fill_paint().label(),
                "stroke": ap.stroke_paint().label(),
                "strokeWidth": ap.stroke_width(),
                "fills": count(true),
                "strokes": count(false),
                "effects": ap.effects.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
                "opacity": (g.opacity * 100.0).round(),
                "blend": g.blend.label(),
                "isolate": g.isolate,
                "knockout": g.knockout.label(),
                "linked": links.get(&g.id).map(|v| v.iter().map(|i| i.0).collect::<Vec<_>>()).unwrap_or_default(),
            })
        })
        .collect();
    let selected = s.selection_graphic_style().filter(|(_, linked)| *linked).map(|(g, _)| g.name.clone());
    Ok(json!({ "styles": styles, "selected": selected }))
}

fn style_redefine(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "graphicStyle.redefine";
    let (src, n) = source(s, p, cmd)?;
    let d = &s.doc()?.doc;
    let i = match str_param(p, "name") {
        Some(name) => style_index(d, name)?,
        None => n
            .graphic_style
            .and_then(|id| d.graphic_style_by_id(id))
            .and_then(|g| d.graphic_style_index(&g.name))
            .ok_or_else(|| bad(cmd, "the object has no graphic style (pass `name`)"))?,
    };
    let look = GraphicStyle::of(String::new(), captured(n), n);
    let name = d.graphic_styles[i].name.clone();
    s.edit(&format!("Redefine Graphic Style \u{201c}{name}\u{201d}"), |d, _| {
        let id = d.graphic_style_id(i);
        let old = std::mem::replace(&mut d.graphic_styles[i], GraphicStyle { name: name.clone(), id, ..look });
        // Objects still linked follow the new definition; edited ones are unlinked (they keep
        // their look), and the source keeps its own.
        let linked = objects_where(d, |n| n.graphic_style == Some(id) && n.id != src);
        let (follow, edited): (Vec<NodeId>, Vec<NodeId>) = linked.into_iter().partition(|nid| d.node(*nid).is_some_and(|n| in_sync(n, &old)));
        set_link(d, &edited, None);
        let g = d.graphic_styles[i].clone();
        for nid in follow {
            apply_style(d, nid, &g);
        }
        set_link(d, &[src], Some(id));
        Ok(())
    })?;
    Ok(json!({ "name": name }))
}

fn style_break_link(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = super::appearance::appearance_targets(s, p)?;
    s.edit("Break Link to Graphic Style", |d, _| {
        set_link(d, &ids, None);
        Ok(())
    })?;
    ok()
}

fn style_rename(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "graphicStyle.rename";
    let name = str_param(p, "name").ok_or_else(|| bad(cmd, "missing name"))?.to_string();
    let to = str_param(p, "to").map(str::trim).filter(|t| !t.is_empty()).ok_or_else(|| bad(cmd, "missing `to`"))?.to_string();
    s.edit("Graphic Style Options", |d, _| {
        let i = style_index(d, &name)?;
        if to != name && d.graphic_style(&to).is_some() {
            return Err(EngineError::Other(format!("a graphic style named `{to}` already exists")));
        }
        d.graphic_styles[i].name = to.clone();
        Ok(())
    })?;
    Ok(json!({ "name": to }))
}

fn style_unused(s: &mut Session, _: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let links = links(d);
    let names: Vec<&str> = d.graphic_styles.iter().filter(|g| !links.contains_key(&g.id)).map(|g| g.name.as_str()).collect();
    Ok(json!({ "names": names }))
}

fn style_sort(s: &mut Session, _: &Value) -> Result<Value> {
    s.edit("Sort by Name", |d, _| {
        d.graphic_styles.sort_by_cached_key(|g| (g.name != DEFAULT_GRAPHIC_STYLE, g.name.to_lowercase()));
        Ok(())
    })?;
    ok()
}
