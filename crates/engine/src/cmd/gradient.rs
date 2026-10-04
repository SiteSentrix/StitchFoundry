//! The Gradient panel and the Gradient tool: in-place gradient edits and the gradient vector.

use serde_json::{Value, json};
use vectorcraft_color::{Gradient, GradientGeom, GradientKind, GradientPaint, GradientStop, Paint};
use vectorcraft_doc::appearance::stroke_paint_bounds;
use vectorcraft_doc::{Node, NodeKind};
use vectorcraft_geom::{Affine, Point, Rect};

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
            "{stroke?: bool (default: the targeted item's kind, else the active proxy), kind?: linear|radial|freeform, stops?: [{offset 0..1, color, opacity? 0..1 (or 0..100), midpoint? 0.13..0.87}] (at least 2), angle?: deg, aspect?: %, reverse?: bool, item?: fill/stroke item index|null (omitted: the Appearance panel's active item when it is of the edited kind), ids?} edit the gradient in place (keeps its placement); solid/none paints become the default gradient",
            has_doc,
            edit_gradient
        ),
        cmd!(
            "paint.setGradientGeom",
            "Gradient Vector",
            [],
            None,
            "{start: [x,y], end: [x,y] (document coordinates), ids?, stroke?: bool (default: the targeted item's kind, else the active proxy), item?: fill/stroke item index|null (alias: index; omitted: the Appearance panel's active item when it is of the edited kind)} set the gradient vector (solid paints become the default gradient; type objects set it on their runs, in text space; the aspect ratio is kept)",
            has_doc,
            set_gradient_geom
        ),
        cmd!(
            "gradient.selectStop",
            "Select Gradient Stop",
            [],
            None,
            "{index: stop index (0 = the start) | null to clear} select a stop of the gradient behind the active proxy (the first selected object's, else the default paint): the stop the Gradient tool's annotator, the Gradient and Color panels and Delete/arrow keys act on → {index}",
            always,
            select_stop
        ),
    ]
}

/// The gradient behind the active proxy: the first selected object's (see
/// `Node::proxy_paint`; the Appearance panel's active item when it is of the proxy's kind), else
/// the default paint for new art.
pub(crate) fn active_gradient(s: &Session) -> Option<GradientPaint> {
    let stroke = !s.fill_active;
    let first = s.active().and_then(|d| d.selection.objects.first().and_then(|id| d.doc.node(*id)));
    let paint = match first {
        Some(n) => n.proxy_paint(stroke, s.appearance_item()).map(|(p, ..)| p.clone()),
        None => Some(if stroke { s.paint.stroke.clone() } else { s.paint.fill.clone() }),
    };
    match paint {
        Some(Paint::Gradient(g)) => Some(*g),
        _ => None,
    }
}

fn select_stop(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "gradient.selectStop";
    let index = match p.get("index") {
        None => return Err(bad(C, "missing `index` (a stop index, or null to clear)")),
        Some(Value::Null) => None,
        Some(v) => {
            let i = v.as_u64().ok_or_else(|| bad(C, "`index` must be a whole number or null"))? as usize;
            let n = active_gradient(s).ok_or_else(|| bad(C, "the active paint is not a gradient"))?.gradient.stops.len();
            if i >= n {
                return Err(bad(C, format!("no stop {i} (the gradient has {n})")));
            }
            Some(i)
        }
    };
    s.gradient_stop = index.map(|i| (i, StopOwner::of(s)));
    Ok(json!({ "index": index }))
}

/// Whose gradient a selected stop belongs to: the active document, its first selected object
/// (None: the default paint), the proxy in front and the Appearance panel's active item.
/// Selecting other art, toggling the proxy or picking another item leaves no stop selected.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StopOwner {
    doc: Option<usize>,
    object: Option<NodeId>,
    fill: bool,
    item: Option<usize>,
}

impl StopOwner {
    fn of(s: &Session) -> Self {
        Self { doc: s.active, object: s.active().and_then(|d| d.selection.objects.first().copied()), fill: s.fill_active, item: s.appearance_item() }
    }
}

impl Session {
    /// The selected gradient stop (`gradient.selectStop`), while the gradient it was selected on
    /// is still the one behind the active proxy. Callers check it against the stop count (an undo
    /// can remove stops).
    pub fn selected_stop(&self) -> Option<usize> {
        self.gradient_stop.filter(|(_, owner)| *owner == StopOwner::of(self)).map(|(i, _)| i)
    }
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

/// `paint` without the placement it had on the art it came from (a swatch, or the default paint
/// for new art): a placed gradient keeps its angle and fits each object it lands on.
pub(crate) fn unplaced(paint: &Paint) -> Paint {
    match paint {
        Paint::Gradient(g) if g.geom.is_some() => Paint::Gradient(Box::new(GradientPaint { geom: None, ..(**g).clone() })),
        _ => paint.clone(),
    }
}

/// Is `p` applying a swatch (whose gradient placement belongs to the art it was saved from)?
fn applies_swatch(p: &Value) -> bool {
    p.get("swatch").is_some()
}

/// `paint` as applied to an object with `bounds`: a gradient given an `aspect` but no vector is
/// placed on the bounds so the aspect sticks, and a gradient swatch fits the object (keeping its
/// aspect) instead of the art it was saved from.
pub(crate) fn place_paint(paint: &Paint, p: &Value, bounds: Option<Rect>) -> Paint {
    let Paint::Gradient(g) = paint else { return paint.clone() };
    let aspect = match g.geom {
        Some(geom) if applies_swatch(p) => (geom.aspect != 1.0).then(|| json!(geom.aspect * 100.0)),
        None => p.get("gradient").and_then(|g| g.get("aspect")).cloned(),
        Some(_) => return paint.clone(),
    };
    let fitted = unplaced(paint);
    match aspect {
        Some(a) => apply_gradient_edit(&fitted, &json!({ "aspect": a }), bounds).unwrap_or(fitted),
        None => fitted,
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
    let item = item_target(s, p, C)?;
    let stroke = edits_stroke(s, p, item, !s.fill_active)?;
    let item = item.of_kind(s, !stroke);
    let ids = item.targets(s, p)?;
    // Validate against the defaults first so bad params fail without touching the document.
    let default_paint = if stroke { s.paint.stroke.clone() } else { s.paint.fill.clone() };
    let new_default = apply_gradient_edit(&default_paint, p, None).map_err(|e| bad(C, e))?;
    edit_items(s, &ids, item, C, "Gradient", !stroke, |n, index| {
        if index.is_none()
            && let NodeKind::Text(t) = &mut n.kind
        {
            let lb = t.local_bounds();
            for r in &mut t.runs {
                let (cur, b) = run_paint_mut(r, stroke, lb);
                *cur = apply_gradient_edit(cur, p, Some(b)).map_err(|e| bad(C, e))?;
            }
            return Ok(());
        }
        let b = item_paint_bounds(n, index, !stroke);
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

/// The box an unplaced gradient on fill or stroke `index` of `n` fits (`None`: the topmost): the
/// geometric bounds, grown by half the weight for a stroke (`None` without that stroke).
pub(crate) fn item_paint_bounds(n: &Node, index: Option<usize>, fill: bool) -> Option<Rect> {
    let b = n.geometric_bounds()?;
    if fill { Some(b) } else { n.appearance.stroke_at(index).map(|st| st.paint_bounds(b)) }
}

/// `paint` as applied to a type run whose text space `xf` maps to the document: a vector given in
/// document coordinates moves into text space, and an aspect without a vector places the gradient
/// on the run's `bounds` (text space).
pub(crate) fn place_run_paint(paint: &Paint, p: &Value, xf: Affine, bounds: Rect) -> Paint {
    let mut out = place_paint(paint, p, Some(bounds));
    if let (Paint::Gradient(src), Paint::Gradient(g)) = (paint, &mut out)
        && src.geom.is_some()
        && !applies_swatch(p)
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
    // `index` is an alias of `item`.
    let mut p = p.clone();
    if let (None, Some(i)) = (p.get("item"), p.get("index").cloned()) {
        p["item"] = i;
    }
    let p = &p;
    let item = item_target(s, p, C)?;
    let stroke = edits_stroke(s, p, item, !s.fill_active)?;
    let item = item.of_kind(s, !stroke);
    let targets = match item {
        ItemTarget::Top => leaf_targets(s, &ids_param(p, "ids").map_or_else(|| selected_roots(s), Ok)?)?,
        _ => item.targets(s, p)?,
    };
    if targets.is_empty() {
        return Err(EngineError::Other("nothing selected".into()));
    }
    edit_items(s, &targets, item, C, "Gradient", !stroke, |n, index| {
        if index.is_none()
            && let NodeKind::Text(t) = &mut n.kind
        {
            let (xf, lb) = (t.xf, t.local_bounds());
            for r in &mut t.runs {
                if stroke && r.style.stroke_width == 0.0 {
                    r.style.stroke_width = 1.0;
                }
                let (paint, b) = run_paint_mut(r, stroke, lb);
                if let Some(np) = vector_paint(paint, start, end, xf, Some(b)) {
                    *paint = np;
                }
            }
            return Ok(());
        }
        let b = item_paint_bounds(n, index, !stroke);
        if let Some(np) = vector_paint(n.appearance.paint_at(index, !stroke).unwrap_or(&Paint::None), start, end, Affine::IDENTITY, b) {
            n.appearance.set_paint_at(index, !stroke, np);
        }
        Ok(())
    })?;
    Ok(json!({ "ids": targets.iter().map(|i| i.0).collect::<Vec<_>>() }))
}
