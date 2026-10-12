//! Flow lines in tatami fills: the rows follow drawn lines instead of running straight. One line
//! makes rows that copy it, moved across the fill (a Florentine effect); two make rows that
//! change shape evenly from the first line to the second (a liquid effect). Taken from other
//! selected art (`embroidery.fillAlongLines`), set by an agent (`embroidery.setFlowLines`) and
//! cleared (`embroidery.clearFlowLines`).

use serde_json::{Value, json};
use sf_core::attr::{Embroidery, StitchType};
use sf_core::model::FillKind;
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_embroidery::outlines_mm;

use super::embroidery::{ids_json, stitched_targets};
use super::*;
use crate::EngineError;

/// Flow lines one fill keeps, and points in each.
const MAX_LINES: usize = 2;
const MAX_POINTS: usize = 20_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.setFlowLines",
            "Set Flow Lines",
            [],
            None,
            "{id, lines: [[[x, y], [x, y], …], …]} (points; none, one or two lines) replace a tatami fill's flow lines: its rows follow them, copying one line or changing shape from the first to the second → {lines}",
            has_doc,
            set_flow_lines
        ),
        cmd!(
            "embroidery.fillAlongLines",
            "Fill Along Lines",
            ["Embroidery"],
            None,
            "{ids?, keep?: bool (keep the lines' art; default false)} the selected tatami fills sew their rows along the other selected lines (the first two): one line, rows that copy it, moved across the fill (a Florentine effect); two, rows that change shape evenly from one to the other (a liquid effect); draw them across or beside the fill, the same way round → {ids, lines}",
            has_selection,
            fill_along_lines
        ),
        cmd!(
            "embroidery.clearFlowLines",
            "Clear Flow Lines",
            ["Embroidery"],
            None,
            "{ids?} the selected fills sew straight rows again → {ids}",
            has_selection,
            clear_flow_lines
        ),
    ]
}

/// Whether an object is stitched as a tatami fill (the fill whose rows follow flow lines).
fn tatami(d: &Document, id: NodeId) -> bool {
    d.node(id).and_then(|n| n.embroidery.as_deref()).is_some_and(|e| matches!(&e.stitch, StitchType::Fill(f) if f.kind == FillKind::Tatami))
}

fn set_flow_lines(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.setFlowLines";
    let id = id_param(p, "id").ok_or_else(|| bad(CMD, "missing object `id`"))?;
    let d = &s.doc()?.doc;
    d.node(id).ok_or_else(|| bad(CMD, format!("no such object id {}", id.0)))?;
    if !tatami(d, id) {
        return Err(bad(CMD, "flow lines go in tatami fills"));
    }
    let list = p.get("lines").and_then(Value::as_array).ok_or_else(|| bad(CMD, "missing `lines` array"))?;
    if list.len() > MAX_LINES {
        return Err(bad(CMD, "a fill follows one or two lines"));
    }
    let mut lines = Vec::with_capacity(list.len());
    for line in list {
        let pts = line.as_array().ok_or_else(|| bad(CMD, format!("each line is [[x, y], …], not {line}")))?;
        if pts.len() > MAX_POINTS {
            return Err(bad(CMD, format!("a flow line has at most {MAX_POINTS} points")));
        }
        let mut out = Vec::with_capacity(pts.len());
        for q in pts {
            let c: Vec<f64> = q.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
            let [x, y] = c.as_slice() else {
                return Err(bad(CMD, format!("each point is [x, y], not {q}")));
            };
            if !(x.is_finite() && y.is_finite()) {
                return Err(bad(CMD, "flow line points must be finite"));
            }
            out.push(sf_core::Point::new(*x, *y));
        }
        if out.len() < 2 {
            return Err(bad(CMD, "a flow line needs two points or more"));
        }
        lines.push(out);
    }
    s.edit("Set Flow Lines", |d, _| {
        let e = d.node_mut(id).ok_or(EngineError::NoNode(id))?.embroidery.as_deref_mut().ok_or(EngineError::NoNode(id))?;
        e.flow = lines;
        Ok(json!({ "lines": e.flow.len() }))
    })
}

fn fill_along_lines(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.fillAlongLines";
    let keep = bool_or(p, "keep", false);
    let d = &s.doc()?.doc;
    let picked = d.paint_order(leaf_targets(s, &targets(s, p)?)?);
    let (fills, art): (Vec<NodeId>, Vec<NodeId>) = picked.iter().partition(|id| tatami(d, **id));
    if fills.is_empty() || art.is_empty() {
        return Err(bad(CMD, "select tatami fills and one or two lines for their rows to follow"));
    }
    // the first two lines of the art, in points
    let lines: Vec<Vec<sf_core::Point>> = art
        .iter()
        .filter_map(|id| d.node(*id))
        .flat_map(outlines_mm)
        .filter(|l| l.len() >= 2 && l.len() <= MAX_POINTS)
        .take(MAX_LINES)
        .map(|l| l.into_iter().map(|q| q * vectorcraft_embroidery::PT_PER_MM).collect())
        .collect();
    if lines.is_empty() {
        return Err(bad(CMD, "the lines to follow have no paths"));
    }
    let n = lines.len();
    s.edit("Fill Along Lines", |d, sel| {
        for id in &fills {
            let e = d.node_mut(*id).ok_or(EngineError::NoNode(*id))?.embroidery.as_deref_mut().ok_or(EngineError::NoNode(*id))?;
            e.flow = lines.clone();
        }
        if !keep {
            for id in &art {
                if d.node(*id).is_some() {
                    d.remove(*id)?;
                }
            }
            sel.objects.retain(|id| !art.contains(id));
        }
        Ok(())
    })?;
    Ok(json!({ "ids": fills.iter().map(|i| i.0).collect::<Vec<_>>(), "lines": n }))
}

fn clear_flow_lines(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = stitched_targets(s, p)?;
    let d = &s.doc()?.doc;
    let ids: Vec<NodeId> =
        ids.into_iter().filter(|id| d.node(*id).and_then(|n| n.embroidery.as_deref()).is_some_and(|e: &Embroidery| !e.flow.is_empty())).collect();
    if ids.is_empty() {
        return Err(bad("embroidery.clearFlowLines", "the selection has no flow lines"));
    }
    s.edit("Clear Flow Lines", |d, _| {
        for id in &ids {
            if let Some(e) = d.node_mut(*id).ok_or(EngineError::NoNode(*id))?.embroidery.as_deref_mut() {
                e.flow.clear();
            }
        }
        Ok(())
    })?;
    Ok(ids_json(&ids))
}
