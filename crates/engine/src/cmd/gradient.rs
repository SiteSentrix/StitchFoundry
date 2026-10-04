//! The Gradient panel and the Gradient tool: in-place gradient edits and the gradient vector.

use serde_json::{Value, json};
use vectorcraft_color::{Gradient, GradientGeom, GradientKind, GradientPaint, GradientStop, Paint};
use vectorcraft_doc::NodeKind;

use super::appearance::{ItemTarget, edit_items, edits_stroke, item_target};
use super::edit::selected_roots;
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "paint.editGradient",
            "Gradient",
            ["Window", "Gradient"],
            None,
            "{stroke?: bool (default: the targeted item's kind, else the active proxy), kind?: linear|radial|freeform, stops?: [{offset 0..1, color, opacity? 0..1, midpoint? 0..1}], angle?: deg, aspect?: %, reverse?: bool, item?: fill/stroke item index|null (omitted: the Appearance panel's active item when it is of the edited kind), ids?} edit the gradient in place (keeps its placement); solid/none paints become the default gradient",
            has_doc,
            edit_gradient
        ),
        cmd!(
            "paint.setGradientGeom",
            "Gradient Vector",
            [],
            None,
            "{start: [x,y], end: [x,y], ids?, stroke?: bool (default: the targeted item's kind, else the fill), item?: fill/stroke item index|null} set the gradient vector (solid paints become the default gradient)",
            has_doc,
            set_gradient_geom
        ),
    ]
}

/// Apply the gradient edits in `p` to `paint` (pure; unit-tested).
pub(crate) fn apply_gradient_edit(paint: &Paint, p: &Value, bounds: Option<vectorcraft_geom::Rect>) -> std::result::Result<Paint, String> {
    let mut gp = match paint {
        Paint::Gradient(g) => (**g).clone(),
        _ => GradientPaint::new(Gradient::default()),
    };
    if let Some(k) = str_param(p, "kind") {
        let kind = match k.to_ascii_lowercase().as_str() {
            "linear" => GradientKind::Linear,
            "radial" => GradientKind::Radial,
            "freeform" => GradientKind::Freeform,
            _ => return Err(format!("unknown gradient kind `{k}`")),
        };
        if kind != gp.gradient.kind {
            gp.gradient.kind = kind;
            gp.geom = None;
        }
    }
    if let Some(stops) = p.get("stops") {
        let arr = stops.as_array().ok_or("`stops` must be an array")?;
        let mut out = Vec::with_capacity(arr.len());
        for st in arr {
            let offset = st.get("offset").and_then(Value::as_f64).ok_or("stop needs `offset`")?;
            let color = st.get("color").and_then(color_value).ok_or("stop needs a valid `color`")?;
            let opacity = st.get("opacity").and_then(Value::as_f64).map(|o| if o > 1.0 { o / 100.0 } else { o }).unwrap_or(1.0);
            let midpoint = st.get("midpoint").and_then(Value::as_f64).unwrap_or(0.5);
            out.push(GradientStop {
                offset: offset.clamp(0.0, 1.0) as f32,
                color,
                opacity: opacity.clamp(0.0, 1.0) as f32,
                midpoint: midpoint.clamp(0.13, 0.87) as f32,
            });
        }
        if out.len() < 2 {
            return Err("a gradient needs at least two stops".into());
        }
        gp.gradient.stops = out;
        gp.gradient.sort();
        gp.swatch = None;
    }
    if bool_or(p, "reverse", false) {
        gp.gradient.reverse();
        // Midpoints belong to the segment to the right; mirror them.
        let n = gp.gradient.stops.len();
        let mids: Vec<f32> = gp.gradient.stops.iter().map(|s| s.midpoint).collect();
        for i in 0..n {
            gp.gradient.stops[i].midpoint = if i + 1 < n { 1.0 - mids[n - 2 - i] } else { 0.5 };
        }
    }
    if let Some(a) = p.get("angle").and_then(Value::as_f64) {
        let a = ((a + 180.0).rem_euclid(360.0)) - 180.0;
        gp.angle = a;
        if let Some(g) = &mut gp.geom {
            let r = a.to_radians();
            let dir = vectorcraft_geom::Vec2::new(r.cos(), -r.sin());
            if gp.gradient.kind == GradientKind::Radial {
                g.end = g.start + dir * g.length();
            } else {
                let c = g.start.midpoint(g.end);
                let half = g.length() / 2.0;
                g.start = c - dir * half;
                g.end = c + dir * half;
            }
        }
    }
    if let Some(asp) = p.get("aspect").and_then(Value::as_f64) {
        let asp = (asp / 100.0).clamp(0.005, 327.67);
        if gp.geom.is_none()
            && let Some(b) = bounds
        {
            gp.geom = Some(gp.resolve(b));
        }
        if let Some(g) = &mut gp.geom {
            g.aspect = asp;
        }
    }
    Ok(Paint::Gradient(Box::new(gp)))
}

fn edit_gradient(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "paint.editGradient";
    let item = item_target(s, p, C)?;
    let ids = item.targets(s, p)?;
    let stroke = edits_stroke(s, p, item, &ids, !s.fill_active)?;
    // Validate against the defaults first so bad params fail without touching the document.
    let default_paint = if stroke { s.paint.stroke.clone() } else { s.paint.fill.clone() };
    let new_default = apply_gradient_edit(&default_paint, p, None).map_err(|e| bad(C, e))?;
    edit_items(s, &ids, item, C, "Gradient", !stroke, |n, index| {
        if index.is_none()
            && let NodeKind::Text(t) = &mut n.kind
        {
            for r in &mut t.runs {
                let cur = if stroke { &mut r.style.stroke } else { &mut r.style.fill };
                *cur = apply_gradient_edit(cur, p, None).map_err(|e| bad(C, e))?;
            }
            return Ok(());
        }
        let b = n.geometric_bounds();
        let np = apply_gradient_edit(n.appearance.paint_at(index, !stroke).unwrap_or(&Paint::None), p, b).map_err(|e| bad(C, e))?;
        n.appearance.set_paint_at(index, !stroke, np);
        Ok(())
    })?;
    if stroke {
        s.paint.stroke = new_default;
    } else {
        s.paint.fill = new_default;
    }
    Ok(json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
}

fn set_gradient_geom(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "paint.setGradientGeom";
    let start = point_param(p, "start").ok_or_else(|| bad(C, "missing start [x,y]"))?;
    let end = point_param(p, "end").ok_or_else(|| bad(C, "missing end [x,y]"))?;
    let item = item_target(s, p, C)?;
    let targets = match item {
        ItemTarget::Top => leaf_targets(s, &ids_param(p, "ids").map_or_else(|| selected_roots(s), Ok)?)?,
        _ => item.targets(s, p)?,
    };
    if targets.is_empty() {
        return Err(EngineError::Other("nothing selected".into()));
    }
    let stroke = edits_stroke(s, p, item, &targets, false)?;
    let geom = GradientGeom { start, end, aspect: 1.0 };
    edit_items(s, &targets, item, C, "Gradient", !stroke, |n, index| {
        if matches!(n.kind, NodeKind::Text(_)) {
            return Ok(());
        }
        let mut gp = match n.appearance.paint_at(index, !stroke) {
            Some(Paint::Gradient(g)) => (**g).clone(),
            _ => GradientPaint::new(Gradient::default()),
        };
        let aspect = gp.geom.map(|g| g.aspect).unwrap_or(1.0);
        gp.geom = Some(GradientGeom { aspect, ..geom });
        gp.angle = geom.angle_deg();
        n.appearance.set_paint_at(index, !stroke, Paint::Gradient(Box::new(gp)));
        Ok(())
    })?;
    Ok(json!({ "ids": targets.iter().map(|i| i.0).collect::<Vec<_>>() }))
}
