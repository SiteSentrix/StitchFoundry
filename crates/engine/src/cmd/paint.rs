//! Fill and stroke paint (the toolbar proxies and their defaults) and the Transparency panel.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Gradient, GradientKind, GradientPaint, Paint};
use vectorcraft_doc::{Appearance, CharStyle, Node, NodeId, NodeKind};

use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "paint.setFill",
            "Fill",
            [],
            None,
            "{color?: \"#rrggbb\"|[r,g,b]|{c,m,y,k}|{gray}, none?: true, swatch?: name, gradient?: {kind, stops:[{offset,color}], angle?}, ids?, focus?: true (false keeps the active proxy)} sets selection fill and the default",
            has_doc,
            |s, p| set_paint(s, p, true)
        ),
        cmd!("paint.setStroke", "Stroke", [], None, "same as paint.setFill, for the stroke", has_doc, |s, p| set_paint(s, p, false)),
        cmd!(
            "paint.swap",
            "Swap Fill and Stroke",
            [],
            Some("Shift+X"),
            "{ids?} swap the fill and stroke of the selection (type too) and of the defaults",
            has_doc,
            swap
        ),
        cmd!(
            "paint.default",
            "Default Fill and Stroke",
            [],
            Some("D"),
            "{ids?} white fill and 1 pt black stroke for the selection and the defaults; type gets black fill and no stroke",
            has_doc,
            default_paint
        ),
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
            "{opacity?: 0..100, blend?: name, isolate?, knockout?}",
            has_selection,
            transparency
        ),
    ]
}

/// Commands of the Fill/Stroke proxies, registered at the end of the command list.
pub fn proxy_specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "paint.invert",
            "Invert",
            [],
            None,
            "{stroke?: bool (default: the active proxy), ids?} invert the active proxy's colours of the selection (or ids; groups recolour their contents) keeping each colour's model, as edit.colors.invert does; with nothing selected, invert the default → {changed}",
            has_doc,
            |s, p| proxy_recolor(s, p, "Invert", &super::colorcmds::invert)
        ),
        cmd!(
            "paint.complement",
            "Complement",
            [],
            None,
            "{stroke?: bool (default: the active proxy), ids?} replace the active proxy's colours by their complements ((highest + lowest) − each component, over RGB or CMY; grey unchanged) keeping each colour's model; with nothing selected, the default → {changed}",
            has_doc,
            |s, p| proxy_recolor(s, p, "Complement", &|c: Color| c.complement_keep_model())
        ),
        cmd!(
            "paint.lastColor",
            "Color",
            [],
            Some(","),
            "{stroke?: bool (default: the active proxy), ids?} apply the last solid colour used (paint.recent lastColor) to the active proxy of the selection and the default",
            has_doc,
            |s, p| {
                let paint = Paint::solid(s.last_solid);
                apply_to_proxy(s, p, paint)
            }
        ),
        cmd!(
            "paint.lastGradient",
            "Gradient",
            [],
            Some("."),
            "{stroke?: bool (default: the active proxy), ids?} apply the last gradient used (paint.recent lastGradient), fitted to each object, to the active proxy of the selection and the default",
            has_doc,
            |s, p| {
                let paint = Paint::Gradient(Box::new(GradientPaint { geom: None, ..s.last_gradient.clone() }));
                apply_to_proxy(s, p, paint)
            }
        ),
        cmd!(
            query "paint.recent",
            "Recent Colors",
            [],
            None,
            "{} → {colors: [{hex, color}] newest first (fed by every paint command and the eyedropper), lastColor: {hex, color}, lastGradient: gradient paint}",
            always,
            |s, _| {
                let c = |c: &Color| json!({"hex": c.to_hex(), "color": c});
                Ok(json!({
                    "colors": s.recent_colors.iter().map(c).collect::<Vec<_>>(),
                    "lastColor": c(&s.last_solid),
                    "lastGradient": s.last_gradient,
                }))
            }
        ),
    ]
}

impl Session {
    /// How many colours the Recent Colors rows keep.
    pub const RECENT_MAX: usize = 10;

    /// Remember an applied paint: a solid colour becomes the last colour and the newest recent
    /// colour, a gradient the last gradient. During a live preview this waits for the commit.
    pub(crate) fn remember_paint(&mut self, p: &Paint) {
        if self.in_interaction() {
            self.pending_paint = Some(p.clone());
        } else {
            self.remember_paint_now(p);
        }
    }

    pub(crate) fn remember_paint_now(&mut self, p: &Paint) {
        match p {
            Paint::Solid { color, .. } => {
                self.last_solid = *color;
                self.recent_colors.retain(|c| c != color);
                self.recent_colors.insert(0, *color);
                self.recent_colors.truncate(Self::RECENT_MAX);
            }
            Paint::Gradient(g) => self.last_gradient = (**g).clone(),
            Paint::None | Paint::Pattern { .. } => {}
        }
    }
}

/// The fill or stroke a proxy shows for `n` (type shows its first run's style).
pub(crate) fn proxy_paint(n: &Node, stroke: bool) -> Paint {
    match &n.kind {
        NodeKind::Text(t) => {
            let st = t.runs.first().map(|r| &r.style);
            st.map(|st| if stroke { st.stroke.clone() } else { st.fill.clone() }).unwrap_or_default()
        }
        _ if stroke => n.appearance.stroke_paint(),
        _ => n.appearance.fill_paint(),
    }
}

/// `stroke` param, defaulting to the proxy that is in front.
fn stroke_param(s: &Session, p: &Value) -> bool {
    p.get("stroke").and_then(Value::as_bool).unwrap_or(!s.fill_active)
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
    let paint = paint_from(s, p)?.ok_or_else(|| bad("paint.setFill", "give color, none, swatch or gradient"))?;
    if bool_or(p, "focus", true) {
        s.fill_active = fill;
    }
    let ids = paint_targets(s, p)?;
    apply_paint(s, &ids, paint, fill)
}

/// [`apply_paint`] on the proxy named by `stroke` (default: the active one), focusing it.
fn apply_to_proxy(s: &mut Session, p: &Value, paint: Paint) -> Result<Value> {
    let fill = !stroke_param(s, p);
    s.fill_active = fill;
    let ids = paint_targets(s, p)?;
    apply_paint(s, &ids, paint, fill)
}

/// Set the fill (or stroke) of `ids` and of the defaults for new art, and remember it.
fn apply_paint(s: &mut Session, ids: &[NodeId], paint: Paint, fill: bool) -> Result<Value> {
    if fill {
        s.paint.fill = paint.clone();
    } else {
        s.paint.stroke = paint.clone();
    }
    if !ids.is_empty() {
        apply_to_nodes(s, ids, &paint, fill)?;
    }
    s.remember_paint(&paint);
    ok()
}

fn apply_to_nodes(s: &mut Session, ids: &[NodeId], paint: &Paint, fill: bool) -> Result<()> {
    s.edit(if fill { "Fill Color" } else { "Stroke Color" }, |d, _| {
        for id in ids {
            let Some(n) = d.node_mut(*id) else { continue };
            if let NodeKind::Text(t) = &mut n.kind {
                for r in &mut t.runs {
                    if fill {
                        r.style.fill = paint.clone();
                    } else {
                        set_run_stroke(&mut r.style, paint.clone());
                    }
                }
                continue;
            }
            if fill {
                n.appearance.set_fill(paint.clone());
            } else {
                n.appearance.set_stroke(paint.clone());
            }
        }
        Ok(())
    })
}

/// A type run's stroke; a painted stroke on a run without one gets 1 pt.
fn set_run_stroke(style: &mut CharStyle, paint: Paint) {
    if !paint.is_none() && style.stroke_width == 0.0 {
        style.stroke_width = 1.0;
    }
    style.stroke = paint;
}

fn swap(s: &mut Session, p: &Value) -> Result<Value> {
    std::mem::swap(&mut s.paint.fill, &mut s.paint.stroke);
    let ids = paint_targets(s, p)?;
    if !ids.is_empty() {
        s.edit("Swap Fill and Stroke", |d, _| {
            for id in &ids {
                let Some(n) = d.node_mut(*id) else { continue };
                if let NodeKind::Text(t) = &mut n.kind {
                    for r in &mut t.runs {
                        let fill = std::mem::take(&mut r.style.fill);
                        r.style.fill = std::mem::take(&mut r.style.stroke);
                        set_run_stroke(&mut r.style, fill);
                    }
                    continue;
                }
                let f = n.appearance.fill_paint();
                let st = n.appearance.stroke_paint();
                n.appearance.set_fill(st);
                n.appearance.set_stroke(f);
            }
            Ok(())
        })?;
    }
    ok()
}

fn default_paint(s: &mut Session, p: &Value) -> Result<Value> {
    s.paint.fill = Paint::solid(Color::WHITE);
    s.paint.stroke = Paint::solid(Color::BLACK);
    s.paint.stroke_width = 1.0;
    let ids = paint_targets(s, p)?;
    if !ids.is_empty() {
        s.edit("Default Fill and Stroke", |d, _| {
            for id in &ids {
                let Some(n) = d.node_mut(*id) else { continue };
                match &mut n.kind {
                    NodeKind::Text(t) => {
                        for r in &mut t.runs {
                            r.style.fill = Paint::solid(Color::BLACK);
                            r.style.stroke = Paint::None;
                        }
                    }
                    _ => n.appearance = Appearance::default_art(),
                }
            }
            Ok(())
        })?;
    }
    ok()
}

/// Invert / Complement: recolour the active proxy of the targets (or the default when there are
/// none) and remember the first target's new paint.
fn proxy_recolor(s: &mut Session, p: &Value, label: &str, f: &dyn Fn(Color) -> Color) -> Result<Value> {
    let stroke = stroke_param(s, p);
    let ids = match ids_param(p, "ids") {
        Some(ids) => ids,
        None => super::edit::selected_roots(s)?,
    };
    if ids.is_empty() {
        let def = if stroke { &mut s.paint.stroke } else { &mut s.paint.fill };
        let changed = super::colorcmds::map_paint(def, f);
        let shown = def.clone();
        s.remember_paint(&shown);
        return Ok(json!({ "changed": changed as usize }));
    }
    let q = json!({"fill": !stroke, "stroke": stroke, "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()});
    let r = super::colorcmds::recolor(s, &q, label, f)?;
    let first = leaf_targets(s, &ids)?.first().and_then(|id| s.doc().ok()?.doc.node(*id).map(|n| proxy_paint(n, stroke)));
    if let Some(shown) = first {
        s.remember_paint(&shown);
    }
    Ok(r)
}

fn transparency(s: &mut Session, p: &Value) -> Result<Value> {
    let mut q = p.clone();
    if let Some(o) = p.get("opacity").and_then(Value::as_f64) {
        q["opacity"] = json!(o / 100.0);
    }
    s.execute("object.setProps", &q)
}
