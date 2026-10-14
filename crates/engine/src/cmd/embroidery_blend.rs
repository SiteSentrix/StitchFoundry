//! Colour blends (`embroidery.makeBlend`, Multi Blend): a tatami fill sewn as stacked layers,
//! one per colour, each densest across its own band and fading to nothing at the next colour's,
//! so neighbouring colours blend across the rows. The layers live in the one object
//! (`FillParams::blend`), or with `separate` become objects of their own.

use serde_json::{Value, json};
use sf_core::attr::StitchType;
use sf_core::model::{FillKind, FillUnderlay};
use sf_core::threads::{Rgb, Thread};
use vectorcraft_color::Paint;
use vectorcraft_doc::{Document, NodeId};

use super::*;
use crate::EngineError;

const CMD: &str = "embroidery.makeBlend";
/// Most colours one blend takes.
const MAX_COLORS: usize = 6;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "embroidery.makeBlend",
        "Blend Colors",
        [],
        None,
        "{colors: [\"#rrggbb\", …] (2 to 6, first to last across the rows), ids?, spacing?: mm (the densest rows; default the fill's), separate?: bool (false: the layers stay in the one object, its blend: setParams blend [] takes it out; true: one object per layer)} sew each selected tatami fill as stacked layers, one per colour, each densest across its own band and fading to nothing at the next colour's, so the colours blend across the rows (turn the fill's angle to turn the blend) → {ids}",
        has_selection,
        make_blend
    )]
}

/// Whether an object is stitched as a tatami fill.
fn tatami(d: &Document, id: NodeId) -> bool {
    d.node(id).and_then(|n| n.embroidery.as_deref()).is_some_and(|e| matches!(&e.stitch, StitchType::Fill(f) if f.kind == FillKind::Tatami))
}

/// The spacing profile of layer `k` of `n`: densest (`dense` mm) at its own colour's place
/// across the rows, no rows from the next colours' places on.
fn layer_profile(k: usize, n: usize, dense: f64) -> Vec<[f64; 2]> {
    let t = |j: usize| j as f64 / (n - 1).max(1) as f64;
    let mut p = Vec::with_capacity(3);
    if k > 0 {
        p.push([t(k - 1), 0.0]);
    }
    p.push([t(k), dense]);
    if k + 1 < n {
        p.push([t(k + 1), 0.0]);
    }
    p
}

fn make_blend(s: &mut Session, p: &Value) -> Result<Value> {
    let list = p.get("colors").and_then(Value::as_array).ok_or_else(|| bad(CMD, "missing `colors`: two to six \"#rrggbb\" colours"))?;
    if !(2..=MAX_COLORS).contains(&list.len()) {
        return Err(bad(CMD, format!("a blend takes 2 to {MAX_COLORS} colours, not {}", list.len())));
    }
    let colors = list.iter().map(|v| color_value(v).ok_or_else(|| bad(CMD, format!("not a colour: {v}")))).collect::<Result<Vec<_>>>()?;
    let spacing = match p.get("spacing") {
        None => None,
        Some(v) => Some(v.as_f64().filter(|v| (0.2..=5.0).contains(v)).ok_or_else(|| bad(CMD, "`spacing` is 0.2 to 5 mm"))?),
    };
    let separate = match p.get("separate") {
        None => false,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "`separate` is true or false"))?,
    };
    let d = &s.doc()?.doc;
    let ids: Vec<NodeId> = super::embroidery::stitched_targets(s, p)?.into_iter().filter(|id| tatami(d, *id)).collect();
    if ids.is_empty() {
        return Err(bad(CMD, "select tatami fills to blend"));
    }
    let threads: Vec<(Thread, vectorcraft_color::Color)> = colors
        .iter()
        .map(|c| {
            let v = c.to_rgba8(1.0);
            let rgb = Rgb::new(v[0], v[1], v[2]);
            (Thread { color: rgb, name: rgb.to_hex(), code: String::new(), chart: String::new() }, *c)
        })
        .collect();
    let n = threads.len();
    if !separate {
        // one object: the layers are its blend, its thread and colour the first's
        s.edit("Blend Colors", |d, sel| {
            for id in &ids {
                let node = d.node_mut(*id).ok_or(EngineError::NoNode(*id))?;
                let Some(e) = node.embroidery.as_deref_mut() else { continue };
                let StitchType::Fill(f) = &mut e.stitch else { continue };
                let dense = spacing.unwrap_or(f.spacing).clamp(0.2, 5.0);
                f.blend = threads
                    .iter()
                    .enumerate()
                    .map(|(k, (thread, _))| sf_core::model::BlendLayer { thread: thread.clone(), spacing_profile: layer_profile(k, n, dense) })
                    .collect();
                if let Some((thread, color)) = threads.first() {
                    e.thread = Some(thread.clone());
                    node.appearance.set_fill(Paint::solid(*color));
                }
            }
            sel.set(ids.iter().copied());
            Ok(())
        })?;
        return Ok(json!({ "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>() }));
    }
    let made = s.edit("Blend Colors", |d, sel| {
        let mut made = Vec::new();
        for id in &ids {
            let (parent, index, _) = d.position(*id).ok_or(EngineError::NoNode(*id))?;
            let base = d.node(*id).ok_or(EngineError::NoNode(*id))?.clone();
            for (k, (thread, color)) in threads.iter().enumerate() {
                // The fill itself is the first layer; the others are copies stacked above it.
                let mut layer = if k == 0 { base.clone() } else { d.reid(&base) };
                let Some(e) = layer.embroidery.as_deref_mut() else { continue };
                let StitchType::Fill(f) = &mut e.stitch else { continue };
                let dense = spacing.unwrap_or(f.spacing).clamp(0.2, 5.0);
                f.spacing_profile = layer_profile(k, n, dense);
                f.spacing_end = 0.0;
                if k > 0 {
                    // Underlay once, under the first layer.
                    f.underlay = FillUnderlay { edge_run: false, tatami: false, ..f.underlay.clone() };
                    e.entry = None;
                    e.exit = None;
                }
                e.thread = Some(thread.clone());
                layer.appearance.set_fill(Paint::solid(*color));
                let lid = layer.id;
                if k == 0 {
                    *d.node_mut(*id).ok_or(EngineError::NoNode(*id))? = layer;
                } else {
                    d.insert(parent, index + k, layer)?;
                }
                made.push(lid);
            }
        }
        sel.set(made.iter().copied());
        Ok(made)
    })?;
    Ok(json!({ "ids": made.iter().map(|i| i.0).collect::<Vec<_>>() }))
}
