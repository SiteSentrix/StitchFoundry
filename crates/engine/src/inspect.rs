//! Agent-friendly document summaries and view-models for panels.

use serde_json::{Value, json};
use vectorcraft_doc::{ArrowAlign, LineCap, LineJoin, Node, NodeKind, StrokeAlign, StrokeLayer, WidthProfile};

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
    // The angle of its rotated bounding box (counter-clockwise degrees).
    if n.bbox_angle != 0.0 {
        v["rotation"] = json!(n.bbox_angle);
    }
    if let NodeKind::Text(t) = &n.kind {
        // Type shows its characters' paint: the first run's, as paint.proxies and the Stroke
        // options read it. Fills and strokes of the type object itself, above or below its
        // characters, come separately so the two stay apart.
        let default = vectorcraft_doc::CharStyle::default();
        let style = t.runs.first().map_or(&default, |r| &r.style);
        v["fill"] = json!(style.fill.label());
        v["stroke"] = json!(style.stroke.label());
        v["strokeWidth"] = json!(if style.stroke.is_none() { 0.0 } else { style.stroke_width });
        if n.appearance.fill().is_some() || n.appearance.stroke().is_some() {
            v["objectFill"] = json!(n.appearance.fill_paint().label());
            v["objectStroke"] = json!(n.appearance.stroke_paint().label());
            v["objectStrokeWidth"] = json!(n.appearance.stroke_width());
        }
    } else if !n.is_container() {
        v["fill"] = json!(n.appearance.fill_paint().label());
        v["stroke"] = json!(n.appearance.stroke_paint().label());
        v["strokeWidth"] = json!(n.appearance.stroke_width());
    }
    if let Some(st) = node_stroke(n) {
        v["strokeOptions"] = stroke_options(&st);
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
        // The angle the selection's bounding box stands at (counter-clockwise degrees).
        "selectionRotation": d.bbox_angle(&st.selection.objects),
        // The layer, group or object targeted through the Layers panel (`layer.target`).
        "target": st.selection.target.map(|t| t.0),
        "history": st.history.undo.iter().map(|h| h.label.clone()).collect::<Vec<_>>(),
        "redo": st.history.redo.iter().rev().map(|h| h.label.clone()).collect::<Vec<_>>(),
        "objects": d.node_count(),
        "tool": s.tool_id(),
        "paint": {"fill": s.paint.fill.label(), "stroke": s.paint.stroke.label(), "strokeWidth": s.paint.stroke_width, "fillActive": s.fill_active, "appearanceItem": s.appearance_item()},
        "pasteRemembersLayers": d.paste_remembers_layers,
    })
}

/// Cap names as `stroke.set` takes them.
pub const CAPS: [(LineCap, &str); 3] = [(LineCap::Butt, "butt"), (LineCap::Round, "round"), (LineCap::Square, "square")];
/// Join names as `stroke.set` takes them.
pub const JOINS: [(LineJoin, &str); 3] = [(LineJoin::Miter, "miter"), (LineJoin::Round, "round"), (LineJoin::Bevel, "bevel")];
/// Stroke alignment names as `stroke.set` takes them.
pub const ALIGNS: [(StrokeAlign, &str); 3] = [(StrokeAlign::Center, "center"), (StrokeAlign::Inside, "inside"), (StrokeAlign::Outside, "outside")];

fn name_of<T: PartialEq>(table: &[(T, &'static str)], v: T) -> &'static str {
    table.iter().find(|(k, _)| *k == v).map_or("", |(_, n)| n)
}

/// The stroke an object's Stroke panel options describe: type's first run's character stroke,
/// else the topmost stroke of its own (groups and layers too). None without a stroke.
pub fn node_stroke(n: &Node) -> Option<StrokeLayer> {
    match &n.kind {
        NodeKind::Text(t) => Some(t.runs.first().map_or_else(|| vectorcraft_doc::CharStyle::default().stroke_layer(), |r| r.style.stroke_layer())),
        _ => n.appearance.stroke().cloned(),
    }
}

/// A stroke's options in `stroke.set` terms: cap, join, miterLimit, align, dash (null when
/// solid), dashOffset, alignDashes, startArrow/endArrow (null for none), arrowAlign and profile,
/// plus widthPoints: the profile's `[t, left, right]` points (width factors; null when uniform)
/// and gradientMode: how a gradient lies on it (`paint.editGradient` strokeMode).
pub fn stroke_options(st: &StrokeLayer) -> Value {
    json!({
        "cap": name_of(&CAPS, st.cap),
        "join": name_of(&JOINS, st.join),
        "miterLimit": st.miter_limit,
        "align": name_of(&ALIGNS, st.align),
        "dash": st.dash.as_ref().map(|d| &d.pattern),
        "dashOffset": st.dash.as_ref().map_or(0.0, |d| d.offset),
        "alignDashes": st.dash.as_ref().is_some_and(|d| d.align_corners),
        "startArrow": st.start_arrow,
        "endArrow": st.end_arrow,
        "arrowAlign": if st.arrow_align == ArrowAlign::Tip { "tip" } else { "extend" },
        "profile": WidthProfile::id_of(st.profile.as_ref()),
        "widthPoints": st.profile.as_ref().map(|p| &p.points),
        "gradientMode": st.gradient_mode.name(),
    })
}

/// Which Stroke panel values the selected objects don't share (their fields show blank), and
/// whether Align Stroke applies to them (no open path or type among them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StrokeMixed {
    pub weight: bool,
    pub cap: bool,
    pub join: bool,
    pub miter_limit: bool,
    pub align: bool,
    pub dash: bool,
    pub can_align: bool,
}
