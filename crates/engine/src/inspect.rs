//! Agent-friendly document summaries and view-models for panels.

use serde_json::{Value, json};
use vectorcraft_doc::{Node, NodeKind};

use crate::Session;

fn rect_json(r: Option<vectorcraft_geom::Rect>) -> Value {
    match r {
        Some(r) => json!({ "x": r.x0, "y": r.y0, "width": r.width(), "height": r.height() }),
        None => Value::Null,
    }
}

/// Compact tree summary of a node (for `document.inspect`).
pub fn node_summary(n: &Node) -> Value {
    let mut v = json!({
        "id": n.id.0,
        "name": n.display_name(),
        "kind": n.kind_label(),
        "visible": n.visible,
        "locked": n.locked,
        "bounds": rect_json(n.geometric_bounds()),
    });
    if n.opacity < 1.0 {
        v["opacity"] = json!(n.opacity);
    }
    if !n.is_container() {
        v["fill"] = json!(n.appearance.fill_paint().label());
        v["stroke"] = json!(n.appearance.stroke_paint().label());
        v["strokeWidth"] = json!(n.appearance.stroke_width());
    }
    match &n.kind {
        NodeKind::Path { path, .. } => {
            v["anchors"] = json!(path.anchor_count());
            v["closed"] = json!(path.is_closed());
        }
        NodeKind::Text(t) => v["text"] = json!(t.plain_text()),
        _ => {}
    }
    if let Some(ch) = n.children() {
        v["children"] = Value::Array(ch.iter().rev().map(|c| node_summary(c)).collect());
    }
    v
}

pub fn document(s: &Session) -> Value {
    let Some(st) = s.active() else { return Value::Null };
    let d = &st.doc;
    json!({
        "title": st.title(),
        "path": st.path,
        "dirty": st.is_dirty(),
        "revision": st.revision,
        "units": d.units.label(),
        "colorMode": format!("{:?}", d.color_mode),
        "artboards": d.artboards.iter().map(|a| json!({"name": a.name, "x": a.rect.x0, "y": a.rect.y0, "width": a.rect.width(), "height": a.rect.height()})).collect::<Vec<_>>(),
        // Top of the stack first, like the Layers panel.
        "layers": d.layers.iter().rev().map(|l| node_summary(l)).collect::<Vec<_>>(),
        "currentLayer": st.active_layer.map(|l| l.0),
        "isolation": st.isolation.map(|l| l.0),
        "selection": st.selection.objects.iter().map(|i| i.0).collect::<Vec<_>>(),
        "selectionBounds": rect_json(d.bounds_of(&st.selection.objects, false)),
        "history": st.history.undo.iter().map(|h| h.label.clone()).collect::<Vec<_>>(),
        "redo": st.history.redo.iter().rev().map(|h| h.label.clone()).collect::<Vec<_>>(),
        "objects": d.node_count(),
        "tool": s.tool_id(),
        "paint": {"fill": s.paint.fill.label(), "stroke": s.paint.stroke.label(), "strokeWidth": s.paint.stroke_width, "fillActive": s.fill_active, "appearanceItem": s.appearance_item()},
    })
}
