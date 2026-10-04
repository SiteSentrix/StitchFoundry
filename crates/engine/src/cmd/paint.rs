//! Fill and stroke paint (the toolbar proxies and their defaults) and the Transparency panel.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Gradient, GradientKind, GradientPaint, Paint};
use vectorcraft_doc::{Appearance, NodeKind};

use super::appearance::{ItemTarget, appearance_targets, edit_items, item_target};
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "paint.setFill",
            "Fill",
            [],
            None,
            "{color?: \"#rrggbb\"|[r,g,b]|{c,m,y,k}|{gray}, none?: true, swatch?: name, gradient?: {kind, stops:[{offset,color}], angle?}, item?: appearance item index|null (omitted: the Appearance panel's active item if it is a fill, else the top fill), ids?} sets the selection's fill and the default",
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
            "{opacity?: 0..100, blend?: name, isolate?, knockout?, item?: appearance item index|null, ids?} opacity and blend go to the targeted fill/stroke item (omitted: the Appearance panel's active item, else the objects)",
            has_selection,
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
        let kind = match str_param(g, "kind") {
            Some("radial") => GradientKind::Radial,
            _ => GradientKind::Linear,
        };
        let mut grad = Gradient { kind, ..Default::default() };
        if let Some(stops) = g.get("stops").and_then(Value::as_array) {
            grad.stops = stops
                .iter()
                .filter_map(|st| {
                    Some(vectorcraft_color::GradientStop {
                        offset: st.get("offset")?.as_f64()? as f32,
                        color: color_value(st.get("color")?)?,
                        opacity: st.get("opacity").and_then(Value::as_f64).unwrap_or(1.0) as f32,
                        midpoint: 0.5,
                    })
                })
                .collect();
            grad.sort();
        }
        let mut gp = GradientPaint::new(grad);
        gp.angle = f64_or(g, "angle", 0.0);
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
    if fill {
        s.paint.fill = paint.clone();
    } else {
        s.paint.stroke = paint.clone();
    }
    s.fill_active = fill;
    let item = item_target(s, p, cmd)?;
    let ids = item.targets(s, p)?;
    edit_items(s, &ids, item, cmd, if fill { "Fill Color" } else { "Stroke Color" }, fill, |n, index| {
        if index.is_none()
            && let NodeKind::Text(t) = &mut n.kind
        {
            for r in &mut t.runs {
                if fill {
                    r.style.fill = paint.clone();
                } else {
                    r.style.stroke = paint.clone();
                    if r.style.stroke_width == 0.0 {
                        r.style.stroke_width = 1.0;
                    }
                }
            }
        } else {
            n.appearance.set_paint_at(index, fill, paint.clone());
        }
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
    let item = item_target(s, p, C)?;
    let mut q = p.as_object().cloned().unwrap_or_default();
    q.remove("item");
    let opacity = q.remove("opacity").and_then(|v| v.as_f64()).map(|o| (o / 100.0).clamp(0.0, 1.0));
    if let ItemTarget::Item { index, explicit } = item {
        // Opacity and blend belong to the targeted fill/stroke; isolate/knockout stay object-level.
        if explicit {
            let d = &s.doc()?.doc;
            if !appearance_targets(s, p)?.iter().any(|id| d.node(*id).is_some_and(|n| index < n.appearance.items.len())) {
                return Err(bad(C, format!("no appearance item {index}")));
            }
        }
        let mut set = json!({ "index": index });
        for key in ["blend", "ids", "id"] {
            if let Some(v) = p.get(key) {
                set[key] = v.clone();
            }
        }
        if let Some(o) = opacity {
            set["opacity"] = json!(o);
        }
        q.remove("blend");
        s.execute("appearance.setItem", &set)?;
        if q.keys().all(|k| k == "ids" || k == "id") {
            return ok();
        }
    } else if let Some(o) = opacity {
        q.insert("opacity".into(), json!(o));
    }
    s.execute("object.setProps", &Value::Object(q))
}
