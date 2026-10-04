//! Edit → Edit Colors: invert, convert to CMYK/Grayscale/RGB, saturate, adjust colour balance and the
//! three Blend commands. They recolour fills and strokes (solid colours, gradient stops and text
//! runs) of the selected objects and everything inside them. Colours linked to a global swatch are
//! unlinked when they change.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::cms::Model;
use vectorcraft_color::{Color, Paint, keep_model};
use vectorcraft_doc::{AppearanceItem, Document, Node, NodeId, NodeKind};

use super::edit::selected_roots;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "edit.colors.invert",
            "Invert Colors",
            ["Edit", "Edit Colors"],
            None,
            "{fill?: true, stroke?: true, ids?} replace each colour by its RGB inverse (keeps the colour model) → {changed}",
            has_selection,
            |s, p| recolor(s, p, "Invert Colors", &invert)
        ),
        cmd!(
            "edit.colors.toCMYK",
            "Convert to CMYK",
            ["Edit", "Edit Colors"],
            None,
            "{fill?, stroke?, ids?} → {changed}",
            has_selection,
            |s, p| recolor(s, p, "Convert to CMYK", &to_cmyk)
        ),
        cmd!(
            "edit.colors.toGrayscale",
            "Convert to Grayscale",
            ["Edit", "Edit Colors"],
            None,
            "{fill?, stroke?, ids?} → {changed}",
            has_selection,
            |s, p| recolor(s, p, "Convert to Grayscale", &to_gray)
        ),
        cmd!("edit.colors.toRGB", "Convert to RGB", ["Edit", "Edit Colors"], None, "{fill?, stroke?, ids?} → {changed}", has_selection, |s, p| {
            recolor(s, p, "Convert to RGB", &|c| {
                let [r, g, b] = c.to_rgb();
                Color::rgb(r, g, b)
            })
        }),
        cmd!(
            "edit.colors.saturate",
            "Saturate…",
            ["Edit", "Edit Colors"],
            None,
            "{intensity: -100..100 (%), fill?, stroke?, ids?} scale saturation by (1 + intensity/100) → {changed}",
            has_selection,
            saturate
        ),
        cmd!(
            "edit.colors.adjustBalance",
            "Adjust Color Balance…",
            ["Edit", "Edit Colors"],
            None,
            "{r?, g?, b? | c?, m?, y?, k? | gray?: -100..100 (% added per channel), convert?: false (keep the result in the adjusted model), fill?, stroke?, ids?} → {changed}",
            has_selection,
            adjust_balance
        ),
        cmd!(
            "edit.colors.blendFrontToBack",
            "Blend Front to Back",
            ["Edit", "Edit Colors"],
            None,
            "{} ≥3 filled objects: intermediate fills graded between the frontmost and backmost fill → {changed}",
            has_selection,
            |s, _| blend(s, BlendOrder::Stack)
        ),
        cmd!(
            "edit.colors.blendHorizontally",
            "Blend Horizontally",
            ["Edit", "Edit Colors"],
            None,
            "{} ≥3 filled objects: fills graded between the leftmost and rightmost → {changed}",
            has_selection,
            |s, _| blend(s, BlendOrder::Horizontal)
        ),
        cmd!(
            "edit.colors.blendVertically",
            "Blend Vertically",
            ["Edit", "Edit Colors"],
            None,
            "{} ≥3 filled objects: fills graded between the topmost and bottommost → {changed}",
            has_selection,
            |s, _| blend(s, BlendOrder::Vertical)
        ),
    ]
}

/// The RGB inverse of `c`, expressed in `c`'s colour model.
pub(crate) fn invert(c: Color) -> Color {
    c.invert_keep_model()
}

pub(crate) fn to_cmyk(c: Color) -> Color {
    c.in_model(Model::Cmyk)
}

pub(crate) fn to_gray(c: Color) -> Color {
    c.in_model(Model::Gray)
}

/// Apply `f` to a paint; returns whether anything changed.
pub(crate) fn map_paint(p: &mut Paint, f: &dyn Fn(Color) -> Color) -> bool {
    match p {
        Paint::Solid { color, swatch } => {
            let n = f(*color);
            if n != *color {
                *color = n;
                *swatch = None;
                return true;
            }
            false
        }
        Paint::Gradient(g) => {
            let mut ch = false;
            for st in &mut g.gradient.stops {
                let n = f(st.color);
                if n != st.color {
                    st.color = n;
                    ch = true;
                }
            }
            if ch {
                g.swatch = None;
            }
            ch
        }
        _ => false,
    }
}

/// Recolour a node and its descendants. Returns the number of paints changed.
pub(crate) fn map_node_colors(n: &mut Node, f: &dyn Fn(Color) -> Color, fill: bool, stroke: bool) -> usize {
    let mut count = 0;
    for it in &mut n.appearance.items {
        match it {
            AppearanceItem::Fill(l) if fill => count += map_paint(&mut l.paint, f) as usize,
            AppearanceItem::Stroke(l) if stroke => count += map_paint(&mut l.paint, f) as usize,
            _ => {}
        }
    }
    match &mut n.kind {
        NodeKind::Text(t) => {
            for r in &mut t.runs {
                if fill {
                    count += map_paint(&mut r.style.fill, f) as usize;
                }
                if stroke {
                    count += map_paint(&mut r.style.stroke, f) as usize;
                }
            }
        }
        _ => {
            if let Some(ch) = n.children_mut() {
                for c in ch.iter_mut() {
                    count += map_node_colors(Arc::make_mut(c), f, fill, stroke);
                }
            }
        }
    }
    count
}

fn recolor_ids(s: &mut Session, label: &str, ids: Vec<NodeId>, fill: bool, stroke: bool, f: &dyn Fn(Color) -> Color) -> Result<Value> {
    let n = s.edit(label, |d, _| {
        let mut n = 0;
        for id in &ids {
            if let Some(node) = d.node_mut(*id) {
                n += map_node_colors(node, f, fill, stroke);
            }
        }
        Ok(n)
    })?;
    Ok(json!({ "changed": n }))
}

pub(crate) fn recolor(s: &mut Session, p: &Value, label: &str, f: &dyn Fn(Color) -> Color) -> Result<Value> {
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    recolor_ids(s, label, ids, bool_or(p, "fill", true), bool_or(p, "stroke", true), f)
}

fn saturate(s: &mut Session, p: &Value) -> Result<Value> {
    let i = (f64_or(p, "intensity", 0.0).clamp(-100.0, 100.0) / 100.0) as f32;
    recolor(s, p, "Saturate", &move |c| {
        let [h, sat, v] = c.to_hsb();
        if sat <= 0.0 {
            return c;
        }
        keep_model(c, Color::from_hsb(h, (sat * (1.0 + i)).clamp(0.0, 1.0), v))
    })
}

fn adjust_balance(s: &mut Session, p: &Value) -> Result<Value> {
    let g = |k: &str| (f64_or(p, k, 0.0).clamp(-100.0, 100.0) / 100.0) as f32;
    let convert = bool_or(p, "convert", false);
    let cmyk = ["c", "m", "y", "k"].iter().any(|k| p.get(*k).is_some());
    let gray = p.get("gray").is_some();
    let rgb = ["r", "g", "b"].iter().any(|k| p.get(*k).is_some());
    if !(cmyk || gray || rgb) {
        return Err(bad("edit.colors.adjustBalance", "give r/g/b, c/m/y/k or gray adjustments"));
    }
    let (dr, dg, db) = (g("r"), g("g"), g("b"));
    let (dc, dm, dy, dk) = (g("c"), g("m"), g("y"), g("k"));
    let dgray = g("gray");
    let cl = |v: f32| v.clamp(0.0, 1.0);
    recolor(s, p, "Adjust Colors", &move |c| {
        let out = if cmyk {
            let [cc, m, y, k] = c.to_cmyk();
            Color::cmyk(cl(cc + dc), cl(m + dm), cl(y + dy), cl(k + dk))
        } else if gray {
            let Color::Gray { k } = to_gray(c) else { unreachable!() };
            Color::gray(cl(k + dgray))
        } else {
            let [r, gg, b] = c.to_rgb();
            Color::rgb(cl(r + dr), cl(gg + dg), cl(b + db))
        };
        if convert { out } else { keep_model(c, out) }
    })
}

enum BlendOrder {
    Stack,
    Horizontal,
    Vertical,
}

/// Leaf objects with a solid fill among the selection (paint order).
fn filled_leaves(d: &Document, roots: &[NodeId]) -> Vec<NodeId> {
    fn visit(n: &Node, out: &mut Vec<NodeId>) {
        match &n.kind {
            NodeKind::Group { children, .. } | NodeKind::Layer { children, .. } => {
                for c in children {
                    visit(c, out);
                }
            }
            NodeKind::Text(t) => {
                if t.first_style().fill.color().is_some() {
                    out.push(n.id);
                }
            }
            _ => {
                if n.appearance.fill_paint().color().is_some() {
                    out.push(n.id);
                }
            }
        }
    }
    let mut out = vec![];
    for r in roots {
        if let Some(n) = d.node(*r) {
            visit(n, &mut out);
        }
    }
    out
}

fn fill_color(n: &Node) -> Option<Color> {
    match &n.kind {
        NodeKind::Text(t) => t.first_style().fill.color(),
        _ => n.appearance.fill_paint().color(),
    }
}

/// Interpolate within the endpoints' model when they share one.
pub(crate) fn lerp_model(a: Color, b: Color, t: f32) -> Color {
    let l = |x: f32, y: f32| x + (y - x) * t;
    match (a, b) {
        (Color::Cmyk { c, m, y, k }, Color::Cmyk { c: c2, m: m2, y: y2, k: k2 }) => Color::cmyk(l(c, c2), l(m, m2), l(y, y2), l(k, k2)),
        (Color::Gray { k }, Color::Gray { k: k2 }) => Color::gray(l(k, k2)),
        _ => a.lerp(&b, t),
    }
}

fn blend(s: &mut Session, order: BlendOrder) -> Result<Value> {
    let roots = selected_roots(s)?;
    let d = &s.doc()?.doc;
    let mut ids = filled_leaves(d, &roots);
    if ids.len() < 3 {
        return Err(EngineError::Other("Blend colors: select at least three objects with filled colours".into()));
    }
    let center = |id: &NodeId| d.node(*id).and_then(|n| n.geometric_bounds()).map(|b| b.center()).unwrap_or_default();
    match order {
        BlendOrder::Stack => {
            // Front to back: frontmost first.
            ids.reverse();
        }
        BlendOrder::Horizontal => ids.sort_by(|a, b| center(a).x.total_cmp(&center(b).x)),
        BlendOrder::Vertical => ids.sort_by(|a, b| center(a).y.total_cmp(&center(b).y)),
    }
    let first = d.node(ids[0]).and_then(fill_color).unwrap_or_default();
    let last = d.node(*ids.last().unwrap()).and_then(fill_color).unwrap_or_default();
    // Ends in one model blend in it; mixed ones give colours in the document's model.
    let model = if first.model() == last.model() { first.model() } else { d.color_mode.model() };
    let n = ids.len();
    let label = match order {
        BlendOrder::Stack => "Blend Front to Back",
        BlendOrder::Horizontal => "Blend Horizontally",
        BlendOrder::Vertical => "Blend Vertically",
    };
    let changed = s.edit(label, |d, _| {
        let mut changed = 0;
        for (i, id) in ids.iter().enumerate().take(n - 1).skip(1) {
            let c = lerp_model(first, last, i as f32 / (n - 1) as f32).in_model(model);
            if let Some(node) = d.node_mut(*id) {
                changed += map_node_colors(node, &|_| c, true, false);
            }
        }
        Ok(changed)
    })?;
    Ok(json!({ "changed": changed }))
}
