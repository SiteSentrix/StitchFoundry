//! Select menu long tail (Same → text/symbol attributes, Object → Direction Handles / text kinds /
//! brush strokes, Save/recall selections), View → Guides, and File → Close All / Document Color
//! Mode / File Info.

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{CharStyle, ColorMode, Guide, Node, NodeId, NodeKind, TextKind};

use super::colorcmds::{Recolor, Scope, to_cmyk};
use super::edit::selected_roots;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("select.same.symbolInstance", "Symbol Instance", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same(
            s,
            "select.same.symbolInstance",
            |a, b| match (&a.kind, &b.kind) {
                (NodeKind::SymbolInstance { symbol: x, .. }, NodeKind::SymbolInstance { symbol: y, .. }) => x == y,
                _ => false,
            }
        )),
        cmd!("select.same.fontFamily", "Font Family", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.fontFamily",
            |a, b| a.font_family == b.font_family
        )),
        cmd!("select.same.fontFamilyStyle", "Font Family & Style", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.fontFamilyStyle",
            |a, b| a.font_family == b.font_family && a.font_style == b.font_style
        )),
        cmd!("select.same.fontFamilyStyleSize", "Font Family, Style & Size", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| {
            same_text(s, "select.same.fontFamilyStyleSize", |a, b| {
                a.font_family == b.font_family && a.font_style == b.font_style && (a.size - b.size).abs() < 1e-6
            })
        }),
        cmd!("select.same.fontSize", "Font Size", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.fontSize",
            |a, b| (a.size - b.size).abs() < 1e-6
        )),
        cmd!("select.same.textFillColor", "Text Fill Color", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.textFillColor",
            |a, b| a.fill == b.fill
        )),
        cmd!("select.same.textStrokeColor", "Text Stroke Color", ["Select", "Same"], None, "{} → {count}", has_selection, |s, _| same_text(
            s,
            "select.same.textStrokeColor",
            |a, b| a.stroke == b.stroke
        )),
        cmd!(
            "select.object.directionHandles",
            "Direction Handles",
            ["Select", "Object"],
            None,
            "{} direct-select every anchor (showing all handles) of the selected paths → {anchors}",
            has_selection,
            direction_handles
        ),
        cmd!("select.object.pointText", "Point Text Objects", ["Select", "Object"], None, "{} → {count}", has_doc, |s, _| by_kind(
            s,
            |n| matches!(
                &n.kind,
                NodeKind::Text(t) if matches!(t.kind, TextKind::Point)
            )
        )),
        cmd!("select.object.areaText", "Area Text Objects", ["Select", "Object"], None, "{} → {count}", has_doc, |s, _| by_kind(s, |n| matches!(
            &n.kind,
            NodeKind::Text(t) if matches!(t.kind, TextKind::Area { .. })
        ))),
        cmd!("select.object.brushStrokes", "Brush Strokes", ["Select", "Object"], None, "{} → {count}", has_doc, |s, _| by_kind(s, |n| n
            .appearance
            .stroke()
            .is_some_and(|st| st.brush.is_some()))),
        cmd!(
            "select.object.bristleBrushStrokes",
            "Bristle Brush Strokes",
            ["Select", "Object"],
            None,
            "{} objects whose brush name contains \"bristle\" → {count}",
            has_doc,
            |s, _| by_kind(s, |n| n
                .appearance
                .stroke()
                .and_then(|st| st.brush.as_deref())
                .is_some_and(|b| b.to_ascii_lowercase().contains("bristle")))
        ),
        cmd!(
            "select.save",
            "Save Selection…",
            ["Select"],
            None,
            "{name?} remember the current selection for this session (default name \"Selection N\") → {name}",
            has_selection,
            save_selection
        ),
        cmd!("select.recall", "Recall Selection", ["Select"], None, "{name} → {count}", has_doc, recall_selection),
        cmd!("select.editSaved", "Edit Selection…", ["Select"], None, "{name, newName?: rename, delete?: bool}", has_doc, edit_saved),
        cmd!(query "select.savedList", "Saved Selections", [], None, "{} → [name…] for the active document", has_doc, |s, _| {
            let t = s.doc()?.title();
            Ok(json!(s.menu.saved_selections.iter().filter(|x| x.0 == t).map(|x| x.1.clone()).collect::<Vec<_>>()))
        }),
        cmd!(
            "view.guides.make",
            "Make Guides",
            ["View", "Guides"],
            Some("Cmd+5"),
            "{} turn the selected paths into guides → {count}",
            has_selection,
            make_guides
        ),
        cmd!(
            "view.guides.release",
            "Release Guides",
            ["View", "Guides"],
            Some("Cmd+Alt+5"),
            "{} turn the selected guides (or, with none selected, all guide paths) back into paths → {count}",
            has_doc,
            release_guides
        ),
        cmd!(
            query "view.guides.lock",
            "Lock Guides",
            ["View", "Guides"],
            Some("Cmd+Alt+;"),
            "{locked?: bool} toggle (or set) the session's guide lock → {locked}",
            has_doc,
            lock_guides
        ),
        cmd!(
            "view.guides.clear",
            "Clear Guides",
            ["View", "Guides"],
            None,
            "{} delete all ruler guides and guide paths → {count}",
            has_doc,
            clear_guides
        ),
        cmd!("guide.add", "Add Guide", [], None, "{vertical: bool, pos: pt (x for vertical, y for horizontal)} → {index}", has_doc, guide_add),
        cmd!("guide.remove", "Remove Guide", [], None, "{index}", guides_unlocked, guide_remove),
        cmd!("guide.move", "Move Guide", [], None, "{index, pos: pt}", guides_unlocked, guide_move),
        cmd!("file.closeAll", "Close All", ["File"], Some("Cmd+Alt+W"), "{} → {closed}", has_doc, close_all),
        cmd!(
            "file.documentColorMode",
            "Document Color Mode",
            ["File", "Document Color Mode"],
            None,
            "{mode: \"cmyk\"|\"rgb\", convert?: true (convert every colour and swatch to the mode)}",
            has_doc,
            color_mode
        ),
        cmd!(
            "file.info",
            "File Info…",
            ["File"],
            Some("Cmd+Alt+Shift+I"),
            "{title?} set the document title; always → {title, colorMode, units, artboards, objects}",
            has_doc,
            file_info
        ),
    ]
}

impl Session {
    /// View → Guides → Lock Guides state (canvas guide dragging should honour it).
    pub fn guides_locked(&self) -> bool {
        self.menu.guides_locked
    }
}

// ---------- Select ----------

fn candidates(s: &Session, f: impl Fn(&Node) -> bool) -> Result<Vec<NodeId>> {
    let st = s.doc()?;
    let mut ids = vec![];
    for l in st.doc.layers.iter().filter(|l| l.visible && !l.locked) {
        l.walk(&mut |n| {
            if !n.is_container() && n.visible && !n.locked && f(n) {
                ids.push(n.id);
            }
        });
    }
    Ok(ids)
}

fn finish_same(s: &mut Session, cmd: &str, ids: Vec<NodeId>) -> Result<Value> {
    let n = ids.len();
    s.select(|_, sel| sel.set(ids))?;
    s.doc_mut()?.last_selection_cmd = Some((cmd.to_string(), json!({})));
    Ok(json!({ "count": n }))
}

fn same(s: &mut Session, cmd: &str, eq: fn(&Node, &Node) -> bool) -> Result<Value> {
    let st = s.doc()?;
    let r = st.selection.objects.first().and_then(|id| st.doc.node(*id)).cloned().ok_or_else(|| bad(cmd, "nothing selected"))?;
    let ids = candidates(s, |n| eq(n, &r))?;
    finish_same(s, cmd, ids)
}

fn first_text_style(s: &Session) -> Option<CharStyle> {
    let st = s.active()?;
    let mut out = None;
    for id in &st.selection.objects {
        st.doc.node(*id)?.walk(&mut |n| {
            if out.is_none()
                && let NodeKind::Text(t) = &n.kind
            {
                out = Some(t.first_style());
            }
        });
        if out.is_some() {
            break;
        }
    }
    out
}

fn same_text(s: &mut Session, cmd: &str, eq: fn(&CharStyle, &CharStyle) -> bool) -> Result<Value> {
    let r = first_text_style(s).ok_or_else(|| bad(cmd, "select a text object"))?;
    let ids = candidates(s, |n| match &n.kind {
        NodeKind::Text(t) => t.runs.iter().any(|run| eq(&run.style, &r)),
        _ => false,
    })?;
    finish_same(s, cmd, ids)
}

fn by_kind(s: &mut Session, f: fn(&Node) -> bool) -> Result<Value> {
    let ids = candidates(s, f)?;
    let n = ids.len();
    s.select(|_, sel| sel.set(ids))?;
    Ok(json!({ "count": n }))
}

fn direction_handles(s: &mut Session, _: &Value) -> Result<Value> {
    let roots = selected_roots(s)?;
    let d = &s.doc()?.doc;
    let mut items: Vec<(NodeId, BTreeSet<(usize, usize)>)> = vec![];
    for r in &roots {
        if let Some(n) = d.node(*r) {
            n.walk(&mut |c| {
                if let Some(p) = c.path_data() {
                    items.push((c.id, p.anchors().map(|(si, ai, _)| (si, ai)).collect()));
                }
            });
        }
    }
    let total: usize = items.iter().map(|x| x.1.len()).sum();
    s.select(|_, sel| {
        sel.clear();
        for (id, refs) in items {
            sel.add(id);
            sel.anchors.insert(id, refs);
        }
    })?;
    Ok(json!({ "anchors": total }))
}

fn save_selection(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let title = st.title();
    let ids = st.selection.objects.clone();
    let name = match str_param(p, "name").filter(|n| !n.trim().is_empty()) {
        Some(n) => n.trim().to_string(),
        None => {
            let mut i = 1;
            while s.menu.saved_selections.iter().any(|x| x.0 == title && x.1 == format!("Selection {i}")) {
                i += 1;
            }
            format!("Selection {i}")
        }
    };
    s.menu.saved_selections.retain(|x| !(x.0 == title && x.1 == name));
    s.menu.saved_selections.push((title, name.clone(), ids));
    Ok(json!({ "name": name }))
}

fn saved_index(s: &Session, p: &Value, cmd: &str) -> Result<usize> {
    let name = str_param(p, "name").ok_or_else(|| bad(cmd, "missing name"))?;
    let t = s.doc()?.title();
    s.menu.saved_selections.iter().position(|x| x.0 == t && x.1 == name).ok_or_else(|| bad(cmd, format!("no saved selection `{name}`")))
}

fn recall_selection(s: &mut Session, p: &Value) -> Result<Value> {
    let i = saved_index(s, p, "select.recall")?;
    let ids = s.menu.saved_selections[i].2.clone();
    s.select(|d, sel| sel.set(ids.into_iter().filter(|id| d.node(*id).is_some())))?;
    Ok(json!({ "count": s.doc()?.selection.len() }))
}

fn edit_saved(s: &mut Session, p: &Value) -> Result<Value> {
    let i = saved_index(s, p, "select.editSaved")?;
    if bool_or(p, "delete", false) {
        s.menu.saved_selections.remove(i);
    } else if let Some(n) = str_param(p, "newName").filter(|n| !n.trim().is_empty()) {
        s.menu.saved_selections[i].1 = n.trim().to_string();
    }
    ok()
}

// ---------- Guides ----------

fn guides_unlocked(s: &Session) -> std::result::Result<(), String> {
    has_doc(s)?;
    if s.menu.guides_locked { Err("guides are locked".into()) } else { Ok(()) }
}

fn set_guide_flag(n: &mut Node, on: bool, count: &mut usize) {
    match &mut n.kind {
        NodeKind::Path { guide, clipping: false, .. } => {
            if *guide != on {
                *guide = on;
                *count += 1;
            }
        }
        _ => {
            if let Some(ch) = n.children_mut() {
                for c in ch.iter_mut() {
                    set_guide_flag(Arc::make_mut(c), on, count);
                }
            }
        }
    }
}

fn make_guides(s: &mut Session, _: &Value) -> Result<Value> {
    let roots = selected_roots(s)?;
    let n = s.edit("Make Guides", |d, sel| {
        let mut n = 0;
        for r in &roots {
            if let Some(node) = d.node_mut(*r) {
                set_guide_flag(node, true, &mut n);
            }
        }
        if n == 0 {
            return Err(EngineError::Other("Make Guides: select paths".into()));
        }
        sel.clear();
        Ok(n)
    })?;
    Ok(json!({ "count": n }))
}

fn guide_paths(d: &vectorcraft_doc::Document) -> Vec<NodeId> {
    let mut v = vec![];
    d.walk(|n| {
        if matches!(n.kind, NodeKind::Path { guide: true, .. }) {
            v.push(n.id);
        }
    });
    v
}

fn release_guides(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let all = guide_paths(&st.doc);
    let sel: Vec<NodeId> = all.iter().copied().filter(|id| st.selection.contains(*id)).collect();
    let targets: Vec<NodeId> = if sel.is_empty() { all.into_iter().filter(|id| st.doc.is_editable(*id)).collect() } else { sel };
    if targets.is_empty() {
        return Err(EngineError::Other("Release Guides: no guides".into()));
    }
    let n = targets.len();
    s.edit("Release Guides", |d, sel| {
        for id in &targets {
            if let Some(NodeKind::Path { guide, .. }) = d.node_mut(*id).map(|n| &mut n.kind) {
                *guide = false;
            }
        }
        sel.set(targets.iter().copied());
        Ok(())
    })?;
    Ok(json!({ "count": n }))
}

fn lock_guides(s: &mut Session, p: &Value) -> Result<Value> {
    let v = p.get("locked").and_then(Value::as_bool).unwrap_or(!s.menu.guides_locked);
    s.menu.guides_locked = v;
    Ok(json!({ "locked": v }))
}

fn clear_guides(s: &mut Session, _: &Value) -> Result<Value> {
    let st = s.doc()?;
    let paths = guide_paths(&st.doc);
    let n = paths.len() + st.doc.guides.len();
    if n == 0 {
        return Ok(json!({ "count": 0 }));
    }
    s.edit("Clear Guides", |d, _| {
        d.guides.clear();
        for id in &paths {
            let _ = d.remove(*id);
        }
        Ok(())
    })?;
    Ok(json!({ "count": n }))
}

fn guide_add(s: &mut Session, p: &Value) -> Result<Value> {
    let pos = f64_req(p, "pos", "guide.add")?;
    let vertical = bool_or(p, "vertical", false);
    let i = s.edit("New Guide", |d, _| {
        d.guides.push(Guide { vertical, pos });
        Ok(d.guides.len() - 1)
    })?;
    Ok(json!({ "index": i }))
}

fn guide_index(s: &Session, p: &Value, cmd: &str) -> Result<usize> {
    let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "missing index"))? as usize;
    if i >= s.doc()?.doc.guides.len() {
        return Err(bad(cmd, "no such guide"));
    }
    Ok(i)
}

fn guide_remove(s: &mut Session, p: &Value) -> Result<Value> {
    let i = guide_index(s, p, "guide.remove")?;
    s.edit("Delete Guide", |d, _| {
        d.guides.remove(i);
        Ok(())
    })?;
    ok()
}

fn guide_move(s: &mut Session, p: &Value) -> Result<Value> {
    let i = guide_index(s, p, "guide.move")?;
    let pos = f64_req(p, "pos", "guide.move")?;
    s.edit("Move Guide", |d, _| {
        d.guides[i].pos = pos;
        Ok(())
    })?;
    ok()
}

// ---------- File ----------

fn close_all(s: &mut Session, _: &Value) -> Result<Value> {
    let n = s.documents().len();
    while !s.documents().is_empty() {
        s.close_document(s.documents().len() - 1);
    }
    Ok(json!({ "closed": n }))
}

fn color_mode(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "file.documentColorMode";
    let mode = match str_param(p, "mode").map(str::to_ascii_lowercase).as_deref() {
        Some("cmyk") => ColorMode::Cmyk,
        Some("rgb") => ColorMode::Rgb,
        _ => return Err(bad(C, "mode must be \"cmyk\" or \"rgb\"")),
    };
    let convert = bool_or(p, "convert", true);
    if s.doc()?.doc.color_mode == mode {
        return ok();
    }
    let f = move |c: vectorcraft_color::Color| match mode {
        ColorMode::Cmyk => to_cmyk(c),
        ColorMode::Rgb => {
            let [r, g, b] = c.to_rgb();
            vectorcraft_color::Color::rgb(r, g, b)
        }
    };
    s.edit("Document Color Mode", |d, _| {
        d.color_mode = mode;
        if convert {
            let layers: Vec<NodeId> = d.layers.iter().map(|l| l.id).collect();
            Recolor::new(Scope { fill: true, stroke: true, images: false, patterns: false }, &f).run(d, &layers);
            for sw in d.swatches_iter_mut() {
                if let vectorcraft_color::Paint::Solid { color, .. } = &mut sw.paint {
                    *color = f(*color);
                }
            }
        }
        Ok(())
    })?;
    ok()
}

fn file_info(s: &mut Session, p: &Value) -> Result<Value> {
    if let Some(t) = str_param(p, "title") {
        let t = t.to_string();
        s.edit("File Info", |d, _| {
            d.title = t;
            Ok(())
        })?;
    }
    let d = &s.doc()?.doc;
    Ok(json!({
        "title": d.title,
        "colorMode": match d.color_mode { ColorMode::Rgb => "rgb", ColorMode::Cmyk => "cmyk" },
        "units": d.units.label(),
        "artboards": d.artboards.len(),
        "objects": d.node_count(),
    }))
}
