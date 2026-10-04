//! The Appearance panel: fill and stroke items, live effects, Clear / Reduce to Basic, and the
//! Eyedropper's appearance copy.

use serde_json::{Value, json};
use vectorcraft_color::{BlendMode, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, FillLayer, NodeKind, StrokeLayer};

use super::edit::selected_roots;
use super::paint::paint_from;
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("appearance.addFill", "Add New Fill", ["Window", "Appearance"], None, "{}", has_selection, |s, _| add_item(s, true)),
        cmd!("appearance.addStroke", "Add New Stroke", ["Window", "Appearance"], None, "{}", has_selection, |s, _| add_item(s, false)),
        cmd!("appearance.clear", "Clear Appearance", ["Window", "Appearance"], None, "{}", has_selection, clear_appearance),
        cmd!("appearance.reduceToBasic", "Reduce to Basic Appearance", ["Window", "Appearance"], None, "{}", has_selection, reduce_basic),
        cmd!(
            "appearance.setItem",
            "Appearance Item",
            [],
            None,
            "{index, opacity?, blend?, visible?, weight?, color?|none?|swatch?|gradient? (as paint.setFill)} edit one fill/stroke of the selection's appearance stack",
            has_selection,
            set_item
        ),
        cmd!("appearance.removeItem", "Remove Item", [], None, "{index}", has_selection, remove_item),
        cmd!("appearance.addEffect", "Add Effect", ["Effect"], None, "{effect: id, params?: {…}} append a live effect", has_selection, add_effect),
        cmd!("appearance.duplicateItem", "Duplicate Item", ["Window", "Appearance"], None, "{index}", has_selection, duplicate_item),
        cmd!("appearance.moveItem", "Reorder Appearance Item", [], None, "{from, to} (paint-order indices)", has_selection, move_item),
        cmd!(
            "appearance.copyFrom",
            "Eyedropper",
            [],
            None,
            "{source: id, ids?} copy fill, stroke, weight, opacity and blend from `source` to ids (default: selection) and to the paint defaults",
            has_doc,
            copy_from
        ),
    ]
}

fn add_item(s: &mut Session, fill: bool) -> Result<Value> {
    let ids = paint_targets(s, &json!({}))?;
    s.edit(if fill { "Add New Fill" } else { "Add New Stroke" }, |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                let item = if fill {
                    AppearanceItem::Fill(FillLayer::new(n.appearance.fill_paint()))
                } else {
                    AppearanceItem::Stroke(StrokeLayer::new(n.appearance.stroke_paint(), n.appearance.stroke_width().max(1.0)))
                };
                n.appearance.items.push(item);
            }
        }
        Ok(())
    })?;
    ok()
}

fn clear_appearance(s: &mut Session, _: &Value) -> Result<Value> {
    let ids = paint_targets(s, &json!({}))?;
    s.edit("Clear Appearance", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                n.appearance = Appearance::basic(Paint::None, Paint::None, 1.0);
            }
        }
        Ok(())
    })?;
    ok()
}

fn reduce_basic(s: &mut Session, _: &Value) -> Result<Value> {
    let ids = paint_targets(s, &json!({}))?;
    s.edit("Reduce to Basic Appearance", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                let f = n.appearance.fill_paint();
                let st = n.appearance.stroke().cloned();
                n.appearance = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(f))], effects: vec![] };
                if let Some(mut st) = st {
                    st.effects.clear();
                    st.opacity = 1.0;
                    st.blend = BlendMode::Normal;
                    n.appearance.items.push(AppearanceItem::Stroke(st));
                }
            }
        }
        Ok(())
    })?;
    ok()
}

fn set_item(s: &mut Session, p: &Value) -> Result<Value> {
    let idx = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("appearance.setItem", "missing index"))? as usize;
    let paint = paint_from(s, p)?;
    let ids = paint_targets(s, p)?;
    let blend = str_param(p, "blend").and_then(BlendMode::parse);
    s.edit("Appearance", |d, _| {
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            let bounds = n.geometric_bounds();
            let Some(item) = n.appearance.items.get_mut(idx) else { continue };
            let (pp, op, bl, vis, bounds) = match item {
                AppearanceItem::Fill(f) => (&mut f.paint, &mut f.opacity, &mut f.blend, &mut f.visible, bounds),
                AppearanceItem::Stroke(st) => {
                    if let Some(w) = p.get("weight").and_then(Value::as_f64) {
                        st.width = w.max(0.0);
                    }
                    let bounds = bounds.map(|b| st.paint_bounds(b));
                    (&mut st.paint, &mut st.opacity, &mut st.blend, &mut st.visible, bounds)
                }
            };
            if let Some(pa) = &paint {
                *pp = super::gradient::place_paint(pa, p, bounds);
            }
            if let Some(o) = p.get("opacity").and_then(Value::as_f64) {
                *op = if o > 1.0 { o / 100.0 } else { o }.clamp(0.0, 1.0) as f32;
            }
            if let Some(b) = blend {
                *bl = b;
            }
            if let Some(v) = p.get("visible").and_then(Value::as_bool) {
                *vis = v;
            }
        }
        Ok(())
    })?;
    ok()
}

fn remove_item(s: &mut Session, p: &Value) -> Result<Value> {
    let idx = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("appearance.removeItem", "missing index"))? as usize;
    let ids = paint_targets(s, p)?;
    s.edit("Remove Item", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id)
                && idx < n.appearance.items.len()
            {
                n.appearance.items.remove(idx);
            }
        }
        Ok(())
    })?;
    ok()
}

fn add_effect(s: &mut Session, p: &Value) -> Result<Value> {
    let id = str_param(p, "effect").ok_or_else(|| bad("appearance.addEffect", "missing effect id"))?.to_string();
    let params = p.get("params").cloned().unwrap_or(json!({}));
    let ids = targets(s, p)?;
    s.edit("Add Effect", |d, _| {
        for nid in &ids {
            if let Some(n) = d.node_mut(*nid) {
                n.appearance.effects.push(vectorcraft_doc::Effect { id: id.clone(), params: params.clone(), visible: true });
            }
        }
        Ok(())
    })?;
    ok()
}

fn duplicate_item(s: &mut Session, p: &Value) -> Result<Value> {
    let idx = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("appearance.duplicateItem", "missing index"))? as usize;
    let ids = paint_targets(s, p)?;
    s.edit("Duplicate Item", |d, _| {
        let mut any = false;
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            if let Some(item) = n.appearance.items.get(idx).cloned() {
                n.appearance.items.insert(idx + 1, item);
                any = true;
            }
        }
        if any { Ok(()) } else { Err(bad("appearance.duplicateItem", format!("no item at index {idx}"))) }
    })?;
    ok()
}

fn move_item(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "appearance.moveItem";
    let from = p.get("from").and_then(Value::as_u64).ok_or_else(|| bad(C, "missing from"))? as usize;
    let to = p.get("to").and_then(Value::as_u64).ok_or_else(|| bad(C, "missing to"))? as usize;
    let ids = paint_targets(s, p)?;
    s.edit("Reorder Appearance", |d, _| {
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            let items = &mut n.appearance.items;
            if from >= items.len() {
                return Err(bad(C, format!("no item at index {from}")));
            }
            let it = items.remove(from);
            let to = to.min(items.len());
            items.insert(to, it);
        }
        Ok(())
    })?;
    ok()
}

fn copy_from(s: &mut Session, p: &Value) -> Result<Value> {
    let src_id = id_param(p, "source").ok_or_else(|| bad("appearance.copyFrom", "missing `source` id"))?;
    let src = s.doc()?.doc.node(src_id).cloned().ok_or(EngineError::NoNode(src_id))?;
    let appearance = match &src.kind {
        NodeKind::Text(t) => match t.runs.first() {
            Some(r) => Appearance::basic(r.style.fill.clone(), r.style.stroke.clone(), r.style.stroke_width),
            None => src.appearance.clone(),
        },
        _ => src.appearance.clone(),
    };
    s.paint.fill = appearance.fill_paint();
    s.paint.stroke = appearance.stroke_paint();
    if appearance.stroke().is_some() {
        s.paint.stroke_width = appearance.stroke_width();
    }
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    let mut targets = leaf_targets(s, &ids)?;
    targets.retain(|id| *id != src_id);
    if targets.is_empty() {
        return Ok(json!({ "ids": [] }));
    }
    let (opacity, blend) = (src.opacity, src.blend);
    s.edit("Eyedropper", |d, _| {
        for id in &targets {
            let Some(n) = d.node_mut(*id) else { continue };
            n.opacity = opacity;
            n.blend = blend;
            if let NodeKind::Text(t) = &mut n.kind {
                for r in &mut t.runs {
                    r.style.fill = appearance.fill_paint();
                    r.style.stroke = appearance.stroke_paint();
                    r.style.stroke_width = appearance.stroke_width();
                }
                continue;
            }
            n.appearance = appearance.clone();
        }
        Ok(())
    })?;
    Ok(json!({ "ids": targets.iter().map(|i| i.0).collect::<Vec<_>>() }))
}
