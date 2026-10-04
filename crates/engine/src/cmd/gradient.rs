//! The Gradient panel and the Gradient tool: in-place gradient edits and the gradient vector.

use serde_json::{Value, json};
use vectorcraft_color::{Gradient, GradientGeom, GradientKind, GradientPaint, GradientStop, Paint};
use vectorcraft_doc::appearance::stroke_paint_bounds;
use vectorcraft_doc::{AppearanceItem, NodeKind};
use vectorcraft_geom::{Affine, Point, Rect};

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
            "{stroke?: bool (default: the active proxy), kind?: linear|radial|freeform, stops?: [{offset 0..1, color, opacity? 0..1 (or 0..100), midpoint? 0.13..0.87}] (at least 2), angle?: deg, aspect?: %, reverse?: bool, ids?} edit the gradient in place (keeps its placement); solid/none paints become the default gradient",
            has_doc,
            edit_gradient
        ),
        cmd!(
            "paint.setGradientGeom",
            "Gradient Vector",
            [],
            None,
            "{start: [x,y], end: [x,y] (document coordinates), ids?, stroke?: bool (default: the active proxy), index?: appearance item index (as appearance.setItem; targets that fill or stroke instead of the top one)} set the gradient vector (solid paints become the default gradient; type objects set it on their runs, in text space; the aspect ratio is kept)",
            has_doc,
            set_gradient_geom
        ),
    ]
}

type Parsed<T> = std::result::Result<T, String>;

/// Parse a kind name.
fn parse_kind(k: &str) -> Parsed<GradientKind> {
    GradientKind::parse(k).ok_or_else(|| format!("unknown gradient kind `{k}` (linear, radial, freeform)"))
}

/// Parse `stops` params: at least two, each with a numeric `offset` (clamped to 0..1) and a valid
/// `color`; `opacity` is 0..1 (values above 1 are percentages) and `midpoint` is clamped to the
/// diamond's range. The result is sorted by offset.
pub(crate) fn parse_stops(v: &Value) -> Parsed<Vec<GradientStop>> {
    let arr = v.as_array().ok_or("`stops` must be an array")?;
    if arr.len() < 2 {
        return Err("a gradient needs at least two stops".into());
    }
    let mut out = arr
        .iter()
        .enumerate()
        .map(|(i, st)| {
            let num = |k: &str| st.get(k).map(|v| v.as_f64().ok_or_else(|| format!("stop {i}: `{k}` must be a number"))).transpose();
            let offset = num("offset")?.ok_or_else(|| format!("stop {i} needs `offset`"))?;
            let color = st.get("color").and_then(color_value).ok_or_else(|| format!("stop {i} needs a valid `color`"))?;
            let opacity = num("opacity")?.map(|o| if o > 1.0 { o / 100.0 } else { o }).unwrap_or(1.0);
            Ok(GradientStop {
                offset: offset.clamp(0.0, 1.0) as f32,
                color,
                opacity: opacity.clamp(0.0, 1.0) as f32,
                midpoint: num("midpoint")?.unwrap_or(0.5).clamp(0.13, 0.87) as f32,
            })
        })
        .collect::<Parsed<Vec<_>>>()?;
    out.sort_by(|a, b| a.offset.total_cmp(&b.offset));
    Ok(out)
}

/// Parse a gradient paint object (the `gradient` param of `paint.setFill`). Lossless for everything
/// `vectorcraft_tools::params::gradient_params` writes.
pub(crate) fn parse_gradient(g: &Value) -> Parsed<GradientPaint> {
    if !g.is_object() {
        return Err("`gradient` must be an object".into());
    }
    let kind = str_param(g, "kind").map(parse_kind).transpose()?.unwrap_or_default();
    let stops = g.get("stops").map(parse_stops).transpose()?.unwrap_or_else(|| Gradient::default().stops);
    let mut gp = GradientPaint::new(Gradient { kind, stops });
    gp.angle = f64_or(g, "angle", 0.0);
    gp.swatch = str_param(g, "swatch").map(str::to_string);
    let aspect = aspect_param(g)?;
    let point = |k: &str| g.get(k).map(|_| point_param(g, k).ok_or_else(|| format!("`{k}` must be [x, y]"))).transpose();
    match (point("start")?, point("end")?) {
        (Some(start), Some(end)) => {
            let geom = GradientGeom { start, end, aspect: aspect.unwrap_or(1.0) };
            gp.angle = geom.angle_deg();
            gp.geom = Some(geom);
        }
        (None, None) => {}
        _ => return Err("give both `start` and `end` (or neither)".into()),
    }
    Ok(gp)
}

/// The `aspect` param (a percentage) as a ratio.
fn aspect_param(p: &Value) -> Parsed<Option<f64>> {
    p.get("aspect")
        .map(|v| v.as_f64().map(|a| (a / 100.0).clamp(0.005, 327.67)).ok_or_else(|| "`aspect` must be a number (%)".to_string()))
        .transpose()
}

/// `paint` as applied to an object with `bounds`: a gradient given an `aspect` but no vector is
/// placed on the bounds so the aspect sticks.
pub(crate) fn place_paint(paint: &Paint, p: &Value, bounds: Option<Rect>) -> Paint {
    match (paint, p.get("gradient").and_then(|g| g.get("aspect"))) {
        (Paint::Gradient(g), Some(a)) if g.geom.is_none() => {
            apply_gradient_edit(paint, &json!({ "aspect": a }), bounds).unwrap_or_else(|_| paint.clone())
        }
        _ => paint.clone(),
    }
}

/// Apply the gradient edits in `p` to `paint` (pure; unit-tested).
pub(crate) fn apply_gradient_edit(paint: &Paint, p: &Value, bounds: Option<Rect>) -> std::result::Result<Paint, String> {
    let mut gp = match paint {
        Paint::Gradient(g) => (**g).clone(),
        _ => GradientPaint::new(Gradient::default()),
    };
    if let Some(k) = str_param(p, "kind") {
        let kind = parse_kind(k)?;
        if kind != gp.gradient.kind {
            gp.gradient.kind = kind;
            gp.geom = None;
        }
    }
    if let Some(stops) = p.get("stops") {
        gp.gradient.stops = parse_stops(stops)?;
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
    if let Some(asp) = aspect_param(p)? {
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
    let stroke = p.get("stroke").and_then(Value::as_bool).unwrap_or(!s.fill_active);
    let ids = paint_targets(s, p)?;
    // Validate against the defaults first so bad params fail without touching the document.
    let default_paint = if stroke { s.paint.stroke.clone() } else { s.paint.fill.clone() };
    let new_default = apply_gradient_edit(&default_paint, p, None).map_err(|e| bad(C, e))?;
    if ids.is_empty() {
        if stroke {
            s.paint.stroke = new_default;
        } else {
            s.paint.fill = new_default;
        }
        return Ok(json!({"ids": []}));
    }
    let mut err = None;
    s.edit("Gradient", |d, _| {
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            if let NodeKind::Text(t) = &mut n.kind {
                let lb = t.local_bounds();
                for r in &mut t.runs {
                    let (cur, b) = run_paint_mut(r, stroke, lb);
                    match apply_gradient_edit(cur, p, Some(b)) {
                        Ok(np) => *cur = np,
                        Err(e) => err = Some(e),
                    }
                }
                continue;
            }
            let b = n.geometric_bounds();
            let ap = &mut n.appearance;
            let (cur, b) = if stroke { (ap.stroke_paint(), b.zip(ap.stroke()).map(|(b, st)| st.paint_bounds(b))) } else { (ap.fill_paint(), b) };
            match apply_gradient_edit(&cur, p, b) {
                Ok(np) => {
                    if stroke {
                        ap.set_stroke(np)
                    } else {
                        ap.set_fill(np)
                    }
                }
                Err(e) => err = Some(e),
            }
        }
        match err.take() {
            Some(e) => Err(bad(C, e)),
            None => Ok(()),
        }
    })?;
    if stroke {
        s.paint.stroke = new_default;
    } else {
        s.paint.fill = new_default;
    }
    Ok(json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
}

/// A type run's fill or stroke paint and the box (text space) an unplaced gradient on it fits:
/// the layout bounds `lb`, grown by half the run's stroke weight for strokes.
pub(crate) fn run_paint_mut(r: &mut vectorcraft_doc::TextRun, stroke: bool, lb: Rect) -> (&mut Paint, Rect) {
    if stroke {
        let b = stroke_paint_bounds(lb, r.style.stroke_width);
        (&mut r.style.stroke, b)
    } else {
        (&mut r.style.fill, lb)
    }
}

/// `paint` as applied to a type run whose text space `xf` maps to the document: a vector given in
/// document coordinates moves into text space, and an aspect without a vector places the gradient
/// on the run's `bounds` (text space).
pub(crate) fn place_run_paint(paint: &Paint, p: &Value, xf: Affine, bounds: Rect) -> Paint {
    let mut out = place_paint(paint, p, Some(bounds));
    if let (Paint::Gradient(src), Paint::Gradient(g)) = (paint, &mut out)
        && src.geom.is_some()
        && let Some(inv) = invert(xf)
    {
        g.transform(inv);
        g.angle = g.geom.map_or(g.angle, |geom| geom.angle_deg());
    }
    out
}

/// The inverse of `a`, if it has one.
fn invert(a: Affine) -> Option<Affine> {
    (a.determinant().abs() > 1e-12).then(|| a.inverse())
}

/// `cur` as a gradient whose vector runs from `start` to `end` in the document. `to_doc` maps the
/// paint's space (text space for type runs) to the document and `bounds` (in that space) fits an
/// unplaced gradient, so the aspect ratio the document shows carries over.
fn vector_paint(cur: &Paint, start: Point, end: Point, to_doc: Affine, bounds: Option<Rect>) -> Option<Paint> {
    let from_doc = invert(to_doc)?;
    let mut gp = match cur {
        Paint::Gradient(g) => (**g).clone(),
        _ => GradientPaint::new(Gradient::default()),
    };
    let kind = gp.gradient.kind;
    let map = |mut g: GradientGeom, a: Affine| {
        if a != Affine::IDENTITY {
            g.transform(a, kind);
        }
        g
    };
    let aspect = gp.geom.or_else(|| bounds.map(|b| gp.resolve(b))).map_or(1.0, |g| map(g, to_doc).aspect);
    let geom = map(GradientGeom { start, end, aspect }, from_doc);
    gp.angle = geom.angle_deg();
    gp.geom = Some(geom);
    Some(Paint::Gradient(Box::new(gp)))
}

fn set_gradient_geom(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "paint.setGradientGeom";
    let start = point_param(p, "start").ok_or_else(|| bad(C, "missing start [x,y]"))?;
    let end = point_param(p, "end").ok_or_else(|| bad(C, "missing end [x,y]"))?;
    let stroke = p.get("stroke").and_then(Value::as_bool).unwrap_or(!s.fill_active);
    let index = p.get("index").map(|v| v.as_u64().map(|i| i as usize).ok_or_else(|| bad(C, "`index` must be a whole number"))).transpose()?;
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    let targets = leaf_targets(s, &ids)?;
    if targets.is_empty() {
        return Err(EngineError::Other("nothing selected".into()));
    }
    s.edit("Gradient", |d, _| {
        let mut hit = false;
        for id in &targets {
            let Some(n) = d.node_mut(*id) else { continue };
            let b = n.geometric_bounds();
            if let Some(i) = index {
                let Some(item) = n.appearance.items.get_mut(i) else { continue };
                let (paint, b) = match item {
                    AppearanceItem::Fill(f) => (&mut f.paint, b),
                    AppearanceItem::Stroke(st) => {
                        let b = b.map(|b| st.paint_bounds(b));
                        (&mut st.paint, b)
                    }
                };
                if let Some(np) = vector_paint(paint, start, end, Affine::IDENTITY, b) {
                    *paint = np;
                    hit = true;
                }
                continue;
            }
            if let NodeKind::Text(t) = &mut n.kind {
                let (xf, lb) = (t.xf, t.local_bounds());
                for r in &mut t.runs {
                    if stroke && r.style.stroke_width == 0.0 {
                        r.style.stroke_width = 1.0;
                    }
                    let (paint, b) = run_paint_mut(r, stroke, lb);
                    if let Some(np) = vector_paint(paint, start, end, xf, Some(b)) {
                        *paint = np;
                        hit = true;
                    }
                }
                continue;
            }
            let ap = &mut n.appearance;
            let (cur, b) = if stroke { (ap.stroke_paint(), b.zip(ap.stroke()).map(|(b, st)| st.paint_bounds(b))) } else { (ap.fill_paint(), b) };
            if let Some(np) = vector_paint(&cur, start, end, Affine::IDENTITY, b) {
                if stroke {
                    ap.set_stroke(np);
                } else {
                    ap.set_fill(np);
                }
                hit = true;
            }
        }
        match (hit, index) {
            (false, Some(i)) => Err(bad(C, format!("no appearance item at index {i}"))),
            _ => Ok(()),
        }
    })?;
    Ok(json!({ "ids": targets.iter().map(|i| i.0).collect::<Vec<_>>() }))
}
