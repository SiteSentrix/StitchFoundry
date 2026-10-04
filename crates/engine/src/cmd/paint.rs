//! Fill and stroke paint (the toolbar proxies and their defaults) and the Transparency panel.

use serde_json::{Value, json};
use vectorcraft_color::{BlendMode, Color, Paint};
use vectorcraft_doc::{Appearance, NodeKind};

use super::appearance::{ItemTarget, appearance_targets, edit_items, item_target};
use super::gradient::{item_paint_bounds, place_paint, place_run_paint, run_paint_mut, unplaced};
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "paint.setFill",
            "Fill",
            [],
            None,
            "{color?: \"#rrggbb\"|[r,g,b]|{c,m,y,k}|{gray}, none?: true, swatch?: name (a gradient swatch fits each object, keeping its aspect), gradient?: {kind?: linear|radial|freeform, stops?: [{offset 0..1, color, opacity? 0..1 (or 0..100), midpoint? 0.13..0.87}] (at least 2; default white→black), angle?: deg, start?: [x,y], end?: [x,y] (the vector in document coordinates, both or neither; type objects keep it in text space), aspect?: % (radial; without start/end the gradient is placed on each object's bounds), swatch?: linked gradient swatch name}, item?: appearance item index|null (omitted: the Appearance panel's active item if it is a fill, else the top fill), ids?} sets the selection's fill and the default (new art fits a gradient to itself)",
            has_doc,
            |s, p| set_paint(s, p, true)
        ),
        cmd!("paint.setStroke", "Stroke", [], None, "same as paint.setFill, for the stroke (item?: the stroke item to set)", has_doc, |s, p| {
            set_paint(s, p, false)
        }),
        cmd!("paint.swap", "Swap Fill and Stroke", [], Some("Shift+X"), "{}", has_doc, swap),
        cmd!("paint.default", "Default Fill and Stroke", [], Some("D"), "{}", has_doc, default_paint),
        cmd!("paint.toggleActive", "Toggle Fill/Stroke Focus", [], Some("X"), "{}", always, |s, _| {
            s.fill_active = !s.fill_active;
            Ok(json!({ "fillActive": s.fill_active }))
        }),
        cmd!("paint.none", "None", [], Some("/"), "{} set the active proxy (fill or stroke) to None", has_doc, |s, _| {
            let f = s.fill_active;
            set_paint(s, &json!({"none": true}), f)
        }),
        cmd!(
            "transparency.set",
            "Transparency",
            ["Window", "Transparency"],
            None,
            "{ids?|id?, opacity?: 0..100, blend?: name, isolate?, knockout?, item?: appearance item index|null} for `ids`, the selection, or the object whose opacity mask is being edited; opacity and blend go to the targeted fill/stroke item (omitted: the Appearance panel's active item, else the objects)",
            has_doc,
            transparency
        ),
    ]
}

/// Parse a paint from params (color / none / swatch / gradient). None = no paint keys given.
pub(crate) fn paint_from(s: &Session, p: &Value) -> Result<Option<Paint>> {
    if bool_or(p, "none", false) {
        return Ok(Some(Paint::None));
    }
    if let Some(name) = str_param(p, "swatch") {
        let st = s.doc()?;
        // A pattern definition works as its swatch even without a swatch entry.
        if st.doc.swatch(name).is_none() && st.doc.pattern(name).is_some() {
            return Ok(Some(vectorcraft_doc::pattern::pattern_paint(name)));
        }
        let sw = st.doc.swatch(name).ok_or_else(|| EngineError::Other(format!("no swatch `{name}`")))?;
        let mut paint = sw.paint.clone();
        if sw.global
            && let Paint::Solid { swatch, .. } = &mut paint
        {
            *swatch = Some(name.to_string());
        }
        return Ok(Some(paint));
    }
    if let Some(g) = p.get("gradient") {
        let gp = super::gradient::parse_gradient(g).map_err(|e| bad("paint", e))?;
        return Ok(Some(Paint::Gradient(Box::new(gp))));
    }
    if let Some(c) = p.get("color") {
        let c = color_value(c).ok_or_else(|| bad("paint", format!("bad color {c}")))?;
        return Ok(Some(Paint::solid(c)));
    }
    Ok(None)
}

fn set_paint(s: &mut Session, p: &Value, fill: bool) -> Result<Value> {
    let cmd = if fill { "paint.setFill" } else { "paint.setStroke" };
    let paint = paint_from(s, p)?.ok_or_else(|| bad(cmd, "give color, none, swatch or gradient"))?;
    // New art gets the paint fitted to itself, not placed where this one is.
    if fill {
        s.paint.fill = unplaced(&paint);
    } else {
        s.paint.stroke = unplaced(&paint);
    }
    s.fill_active = fill;
    let item = item_target(s, p, cmd)?.of_kind(s, fill);
    let ids = item.targets(s, p)?;
    edit_items(s, &ids, item, cmd, if fill { "Fill Color" } else { "Stroke Color" }, fill, |n, index| {
        if index.is_none()
            && let NodeKind::Text(t) = &mut n.kind
        {
            let (xf, lb) = (t.xf, t.local_bounds());
            for r in &mut t.runs {
                if !fill && r.style.stroke_width == 0.0 {
                    r.style.stroke_width = 1.0;
                }
                let (cur, b) = run_paint_mut(r, !fill, lb);
                *cur = place_run_paint(&paint, p, xf, b);
            }
            return Ok(());
        }
        // A missing top fill or stroke is created first: a new stroke's weight sizes the box its
        // gradient fits.
        if n.appearance.paint_at(index, fill).is_none() {
            n.appearance.set_paint_at(index, fill, Paint::None);
        }
        let placed = place_paint(&paint, p, item_paint_bounds(n, index, fill));
        n.appearance.set_paint_at(index, fill, placed);
        Ok(())
    })?;
    ok()
}

fn swap(s: &mut Session, _: &Value) -> Result<Value> {
    std::mem::swap(&mut s.paint.fill, &mut s.paint.stroke);
    let ids = paint_targets(s, &json!({}))?;
    if !ids.is_empty() {
        s.edit("Swap Fill and Stroke", |d, _| {
            for id in &ids {
                if let Some(n) = d.node_mut(*id) {
                    let f = n.appearance.fill_paint();
                    let st = n.appearance.stroke_paint();
                    n.appearance.set_fill(st);
                    n.appearance.set_stroke(f);
                }
            }
            Ok(())
        })?;
    }
    ok()
}

fn default_paint(s: &mut Session, _: &Value) -> Result<Value> {
    s.paint.fill = Paint::solid(Color::WHITE);
    s.paint.stroke = Paint::solid(Color::BLACK);
    s.paint.stroke_width = 1.0;
    let ids = paint_targets(s, &json!({}))?;
    if !ids.is_empty() {
        s.edit("Default Fill and Stroke", |d, _| {
            for id in &ids {
                if let Some(n) = d.node_mut(*id)
                    && !matches!(n.kind, NodeKind::Text(_))
                {
                    n.appearance = Appearance::default_art();
                }
            }
            Ok(())
        })?;
    }
    ok()
}

fn transparency(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "transparency.set";
    let ids = super::opacitymask::transparency_targets(s, p)?;
    if ids.is_empty() {
        return Err(bad(C, "select objects or give ids"));
    }
    // While an opacity mask is edited the selection is its art, so the Appearance panel's active
    // item (a row of that art) doesn't stand for the masked object.
    let editing_mask = p.get("ids").is_none() && p.get("id").is_none() && s.doc()?.doc.mask_edit.is_some();
    let item = if editing_mask && p.get("item").is_none() { ItemTarget::Top } else { item_target(s, p, C)? };
    let mut q = p.as_object().cloned().unwrap_or_default();
    q.remove("item");
    q.remove("id");
    q.insert("ids".into(), json!(ids.iter().map(|id| id.0).collect::<Vec<_>>()));
    let ItemTarget::Item { index, explicit } = item else {
        return s.execute("object.setProps", &Value::Object(q));
    };
    // Opacity and blend belong to the targeted fill/stroke; isolate/knockout stay object-level.
    let item_ids = if editing_mask { ids } else { appearance_targets(s, p)? };
    if explicit {
        let d = &s.doc()?.doc;
        if !item_ids.iter().any(|id| d.node(*id).is_some_and(|n| index < n.appearance.items.len())) {
            return Err(bad(C, format!("no appearance item {index}")));
        }
    }
    let (opacity, blend) = (q.remove("opacity"), q.remove("blend"));
    if let Some(b) = &blend
        && b.as_str().and_then(BlendMode::parse).is_none()
    {
        return Err(bad(C, format!("unknown blend mode {b}")));
    }
    if opacity.is_some() || blend.is_some() {
        let ids: Vec<u64> = item_ids.iter().map(|id| id.0).collect();
        s.execute("appearance.setItem", &json!({ "index": index, "ids": ids, "opacity": opacity, "blend": blend }))?;
    }
    if q.keys().all(|k| k == "ids") {
        return ok();
    }
    s.execute("object.setProps", &Value::Object(q))
}
