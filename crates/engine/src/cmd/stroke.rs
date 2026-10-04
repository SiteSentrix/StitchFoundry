//! The Stroke panel: weight, cap, join, alignment, dashes, arrowheads, width profiles and brushes.

use serde_json::Value;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{ArrowAlign, Arrowhead, Dash, LineCap, LineJoin, StrokeAlign, StrokeLayer, WidthProfile};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "stroke.set",
            "Stroke Options",
            ["Window", "Stroke"],
            None,
            "{weight?, cap?: butt|round|square, join?: miter|round|bevel, miterLimit?, align?: center|inside|outside, dash?: [d,g,…]|null (a 0 dash with a round or projecting cap draws dots or squares), dashOffset?, alignDashes?, startArrow?, endArrow?: Arrow|ArrowOpen|Triangle|TriangleOpen|Circle|CircleOpen|Square|SquareOpen|Diamond|Bar|null, arrowAlign?: \"extend\" (tip past the end point, default)|\"tip\" (tip on the end point; the stroke is shortened), profile?: \"uniform\"|\"lens\"|\"taperStart\"|\"taperEnd\", ids?}",
            has_doc,
            stroke_set
        ),
        cmd!(
            "stroke.setAdvanced",
            "Stroke Options",
            [],
            None,
            "{arrowScale?: [start %, end %], swapArrows?: bool, flipProfile?: \"along\"|\"across\", brush?: name|null, ids?} Stroke panel extras",
            has_doc,
            stroke_advanced
        ),
    ]
}

fn stroke_set(s: &mut Session, p: &Value) -> Result<Value> {
    if let Some(w) = p.get("weight").and_then(Value::as_f64) {
        s.paint.stroke_width = w.max(0.0);
    }
    let ids = paint_targets(s, p)?;
    if ids.is_empty() {
        return ok();
    }
    let arrow = |k: &str| -> Result<Option<Option<Arrowhead>>> {
        match p.get(k) {
            None => Ok(None),
            Some(Value::Null) => Ok(Some(None)),
            Some(Value::String(n)) if n == "none" => Ok(Some(None)),
            Some(v) => {
                serde_json::from_value::<Arrowhead>(v.clone()).map(|a| Some(Some(a))).map_err(|_| bad("stroke.set", format!("unknown arrowhead {v}")))
            }
        }
    };
    let (sa, ea) = (arrow("startArrow")?, arrow("endArrow")?);
    let arrow_align = match str_param(p, "arrowAlign") {
        None => None,
        Some("extend") => Some(ArrowAlign::Extend),
        Some("tip") => Some(ArrowAlign::Tip),
        Some(o) => return Err(bad("stroke.set", format!("arrowAlign must be extend|tip, got {o}"))),
    };
    let profile = match str_param(p, "profile") {
        None => None,
        Some("uniform") => Some(None),
        Some(id) => Some(Some(WidthProfile::preset(id).ok_or_else(|| bad("stroke.set", format!("unknown profile {id}")))?)),
    };
    s.edit("Stroke", |d, _| {
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            if n.appearance.stroke().is_none() {
                n.appearance.set_stroke(Paint::solid(Color::BLACK));
            }
            let st: &mut StrokeLayer = n.appearance.stroke_mut().unwrap();
            if let Some(w) = p.get("weight").and_then(Value::as_f64) {
                st.width = w.max(0.0);
            }
            match str_param(p, "cap") {
                Some("round") => st.cap = LineCap::Round,
                Some("square") | Some("projecting") => st.cap = LineCap::Square,
                Some("butt") => st.cap = LineCap::Butt,
                _ => {}
            }
            match str_param(p, "join") {
                Some("round") => st.join = LineJoin::Round,
                Some("bevel") => st.join = LineJoin::Bevel,
                Some("miter") => st.join = LineJoin::Miter,
                _ => {}
            }
            if let Some(m) = p.get("miterLimit").and_then(Value::as_f64) {
                st.miter_limit = m.clamp(1.0, 500.0);
            }
            match str_param(p, "align") {
                Some("inside") => st.align = StrokeAlign::Inside,
                Some("outside") => st.align = StrokeAlign::Outside,
                Some("center") => st.align = StrokeAlign::Center,
                _ => {}
            }
            match p.get("dash") {
                Some(Value::Null) => st.dash = None,
                Some(Value::Array(a)) => {
                    let pattern: Vec<f64> = a.iter().filter_map(Value::as_f64).collect();
                    st.dash = if pattern.is_empty() {
                        None
                    } else {
                        Some(Dash { pattern, offset: f64_or(p, "dashOffset", 0.0), align_corners: bool_or(p, "alignDashes", false) })
                    };
                }
                _ => {}
            }
            if let Some(a) = sa {
                st.start_arrow = a;
            }
            if let Some(a) = ea {
                st.end_arrow = a;
            }
            if let Some(a) = arrow_align {
                st.arrow_align = a;
            }
            if let Some(pr) = &profile {
                st.profile = pr.clone();
            }
        }
        Ok(())
    })?;
    ok()
}

/// Mirror a width profile along the path (t → 1 − t) or across it (swap left/right widths).
pub(crate) fn flip_profile(p: &WidthProfile, along: bool) -> WidthProfile {
    let mut pts: Vec<(f64, f64, f64)> =
        if along { p.points.iter().map(|&(t, l, r)| (1.0 - t, l, r)).collect() } else { p.points.iter().map(|&(t, l, r)| (t, r, l)).collect() };
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    WidthProfile { points: pts }
}

fn stroke_advanced(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "stroke.setAdvanced";
    let scale = match p.get("arrowScale") {
        None => None,
        Some(v) => {
            let a = v.as_array().filter(|a| a.len() == 2).ok_or_else(|| bad(C, "arrowScale must be [start, end]"))?;
            let f = |i: usize| a[i].as_f64().map(|x| x.clamp(1.0, 1000.0));
            Some((f(0).ok_or_else(|| bad(C, "bad arrowScale"))?, f(1).ok_or_else(|| bad(C, "bad arrowScale"))?))
        }
    };
    let flip = match str_param(p, "flipProfile") {
        None => None,
        Some("along") => Some(true),
        Some("across") => Some(false),
        Some(o) => return Err(bad(C, format!("flipProfile must be along|across, got {o}"))),
    };
    let swap = bool_or(p, "swapArrows", false);
    let brush = match p.get("brush") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(Value::String(b)) => Some(Some(b.clone())),
        Some(_) => return Err(bad(C, "brush must be a name or null")),
    };
    if scale.is_none() && flip.is_none() && !swap && brush.is_none() {
        return Err(bad(C, "nothing to change"));
    }
    let ids = paint_targets(s, p)?;
    if ids.is_empty() {
        return ok();
    }
    s.edit("Stroke", |d, _| {
        for id in &ids {
            let Some(n) = d.node_mut(*id) else { continue };
            let Some(st) = n.appearance.stroke_mut() else { continue };
            if let Some(sc) = scale {
                st.arrow_scale = sc;
            }
            if swap {
                std::mem::swap(&mut st.start_arrow, &mut st.end_arrow);
                st.arrow_scale = (st.arrow_scale.1, st.arrow_scale.0);
            }
            if let (Some(along), Some(pr)) = (flip, &st.profile) {
                st.profile = Some(flip_profile(pr, along));
            }
            if let Some(b) = &brush {
                st.brush = b.clone();
            }
        }
        Ok(())
    })?;
    ok()
}
