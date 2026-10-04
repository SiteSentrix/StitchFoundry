//! Freeform gradients: parsing their points (the `freeform` param of a gradient paint) and the
//! shape test their first points are placed with.

use serde_json::Value;
use vectorcraft_color::{Freeform, FreeformMode, FreeformPoint};
use vectorcraft_doc::Node;
use vectorcraft_geom::Affine;

use super::*;

/// The `opacity` / `spread` convention: 0..1, or a percentage above 1.
fn fraction(v: &Value, key: &str) -> std::result::Result<f32, String> {
    let x = v.as_f64().ok_or_else(|| format!("`{key}` must be a number"))?;
    Ok((if x > 1.0 { x / 100.0 } else { x }).clamp(0.0, 1.0) as f32)
}

/// Apply the `at` (mapped by `to_paint`), `color`, `opacity` and `spread` fields of `v` to `pt`.
fn point_fields(v: &Value, pt: &mut FreeformPoint, to_paint: Affine) -> std::result::Result<(), String> {
    if v.get("at").is_some() {
        pt.at = to_paint * point_param(v, "at").ok_or("`at` must be [x, y]")?;
    }
    if let Some(c) = v.get("color") {
        pt.color = color_value(c).ok_or("`color` must be a colour")?;
    }
    if let Some(o) = v.get("opacity") {
        pt.opacity = fraction(o, "opacity")?;
    }
    if let Some(s) = v.get("spread") {
        pt.spread = fraction(s, "spread")?;
    }
    Ok(())
}

/// Parse the `freeform` param of a gradient paint (`{points: [{at, color, opacity?, spread?}],
/// lines?: [[index, …]], mode?}`; points in the paint's space). Lossless for
/// `vectorcraft_tools::params::freeform_json`.
pub(crate) fn parse_freeform(v: &Value) -> std::result::Result<Freeform, String> {
    let points = v.get("points").and_then(Value::as_array).ok_or("`freeform.points` must be an array")?;
    let mut f = Freeform::default();
    for (i, pv) in points.iter().enumerate() {
        let (Some(at), Some(color)) = (point_param(pv, "at"), pv.get("color").and_then(color_value)) else {
            return Err(format!("freeform point {i} needs `at` [x, y] and a valid `color`"));
        };
        let mut pt = FreeformPoint::new(at, color);
        point_fields(pv, &mut pt, Affine::IDENTITY).map_err(|e| format!("freeform point {i}: {e}"))?;
        f.points.push(pt);
    }
    for l in v.get("lines").and_then(Value::as_array).into_iter().flatten() {
        let ix = l.as_array().map(|a| a.iter().filter_map(Value::as_u64).map(|i| i as usize).collect()).unwrap_or_default();
        f.add_line(ix)?;
    }
    if let Some(m) = str_param(v, "mode") {
        f.mode = parse_mode(m)?;
    }
    Ok(f)
}

pub(crate) fn parse_mode(m: &str) -> std::result::Result<FreeformMode, String> {
    FreeformMode::parse(m).ok_or_else(|| format!("unknown freeform mode `{m}` (points, lines)"))
}

/// Whether `n`'s shape contains `p` (its filled regions; true for objects without them, such as
/// type, whose boxes stand in).
pub(crate) fn inside_fn(n: &Node) -> impl Fn(vectorcraft_geom::Point) -> bool + use<> {
    let shapes = n.clip_shapes(None);
    move |p| shapes.is_empty() || shapes.iter().any(|(bp, rule)| vectorcraft_geom::hit::fill_contains(bp, *rule, p))
}
