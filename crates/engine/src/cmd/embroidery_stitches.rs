//! Stitch editing: an object's stitches kept as they are ("frozen") to edit them
//! by hand, and machine functions (trims, stops, jumps) at single stitches.
//!
//! An object's stitches are its own records in sewing order, in document points,
//! with the trims, stops and jumps between them: what its shape generates, or
//! what was frozen and edited. Indices in the commands count those records. The
//! lock stitches and connections the design adds round each object are not part
//! of them.

use serde_json::{Value, json};
use sf_core::{Cmd, Stitch};
use vectorcraft_doc::NodeId;
use vectorcraft_embroidery::PT_PER_MM;

use super::embroidery::{ids_json, stitched_targets};
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.stitches",
            "Object Stitches",
            [],
            None,
            "{id} an object's own stitches in sewing order (document points; its edited ones when frozen) → {id, frozen, stitches: [[x, y, stitch|jump|trim|stop]…]}",
            has_doc,
            stitches
        ),
        cmd!(
            "embroidery.freeze",
            "Freeze Stitches",
            [],
            None,
            "{ids?} keep the selected objects' stitches as they are now, to edit them by hand (the shape no longer regenerates them) → {ids}",
            has_selection,
            freeze
        ),
        cmd!(
            "embroidery.stitches.split",
            "Split Object",
            [],
            None,
            "{id, at} split an object's stitches in two at its record `at`: the object keeps the ones before, a copy sewn right after it takes the rest, both frozen (their stitches kept as they are) → {ids}",
            has_doc,
            split
        ),
        cmd!(
            "embroidery.regenerate",
            "Regenerate Stitches",
            [],
            None,
            "{ids?} drop hand edits: the selected objects' stitches come from their shapes again → {ids}",
            has_selection,
            regenerate
        ),
        cmd!(
            "embroidery.stitches.edit",
            "Edit Stitches",
            [],
            None,
            "{id, ops: [{op: move, indices, by: [dx, dy]} | {op: delete, indices} | {op: insert, after (−1 = first), at: [x, y], cmd?: stitch|jump|trim|stop}]} edit an object's stitches (document points), freezing them first → {id, stitches}",
            has_doc,
            edit
        ),
        cmd!(
            "embroidery.function.insert",
            "Insert Machine Function",
            [],
            None,
            "{id, at, kind: trim|stop|jump} a trim or stop after stitch `at`, or make stitch `at` a jump (the frame moves there without sewing); freezes the object → {id, stitches}",
            has_doc,
            function_insert
        ),
        cmd!(
            "embroidery.function.clear",
            "Clear Machine Function",
            [],
            None,
            "{id, at} remove the trim or stop at record `at`, or sew jump `at` as a stitch → {id, stitches}",
            has_doc,
            function_clear
        ),
    ]
}

fn cmd_name(c: Cmd) -> &'static str {
    match c {
        Cmd::Stitch => "stitch",
        Cmd::Jump => "jump",
        Cmd::Trim => "trim",
        Cmd::Stop => "stop",
        Cmd::ColorChange => "colorChange",
        Cmd::End => "end",
    }
}

fn parse_cmd(s: &str) -> Option<Cmd> {
    match s {
        "stitch" => Some(Cmd::Stitch),
        "jump" => Some(Cmd::Jump),
        "trim" => Some(Cmd::Trim),
        "stop" => Some(Cmd::Stop),
        _ => None,
    }
}

/// The stitches object `id` sews (document points), and whether they are frozen.
fn current(s: &mut Session, id: NodeId, cmd: &str) -> Result<(Vec<Stitch>, bool)> {
    let frozen = {
        let n = s.doc()?.doc.node(id).ok_or_else(|| bad(cmd, format!("no such object id {}", id.0)))?;
        let e = n.embroidery.as_deref().ok_or_else(|| bad(cmd, "the object has no stitch type (Embroidery › Stitch Type)"))?;
        e.frozen.clone()
    };
    if let Some(f) = frozen {
        return Ok((f, true));
    }
    let plan = s.embroidery_plan().ok_or_else(|| bad(cmd, "no document"))?;
    let own = plan.own_stitches(id).into_iter().map(|s| Stitch::new(s.x * PT_PER_MM, s.y * PT_PER_MM, s.cmd)).collect();
    Ok((own, false))
}

/// Stores `stitches` as object `id`'s frozen stitches.
fn store(s: &mut Session, id: NodeId, label: &str, stitches: Vec<Stitch>) -> Result<Value> {
    let count = stitches.len();
    s.edit(label, |d, _| {
        let n = d.node_mut(id).ok_or(EngineError::NoNode(id))?;
        if let Some(e) = n.embroidery.as_deref_mut() {
            e.frozen = Some(stitches);
        }
        Ok(())
    })?;
    Ok(json!({ "id": id.0, "stitches": count }))
}

fn round(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn stitches(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.stitches";
    let id = id_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`"))?;
    let (st, frozen) = current(s, id, CMD)?;
    let list: Vec<Value> = st.iter().map(|q| json!([round(q.x), round(q.y), cmd_name(q.cmd)])).collect();
    Ok(json!({ "id": id.0, "frozen": frozen, "stitches": list }))
}

fn freeze(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.freeze";
    let ids = stitched_targets(s, p)?;
    let mut lists = Vec::new();
    for id in &ids {
        let (st, frozen) = current(s, *id, CMD)?;
        if !frozen && st.iter().any(|q| q.cmd == Cmd::Stitch) {
            lists.push((*id, st));
        }
    }
    if lists.is_empty() {
        return Err(bad(CMD, "the selected objects make no stitches, or are frozen already"));
    }
    s.edit("Freeze Stitches", |d, _| {
        for (id, st) in &lists {
            if let Some(e) = d.node_mut(*id).ok_or(EngineError::NoNode(*id))?.embroidery.as_deref_mut() {
                e.frozen = Some(st.clone());
            }
        }
        Ok(())
    })?;
    Ok(ids_json(&lists.iter().map(|l| l.0).collect::<Vec<_>>()))
}

fn split(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.stitches.split";
    let id = id_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`"))?;
    let at = p.get("at").and_then(Value::as_u64).ok_or_else(|| bad(CMD, "missing `at`, a record index"))? as usize;
    let (st, _) = current(s, id, CMD)?;
    let stitches = |v: &[Stitch]| v.iter().filter(|q| q.cmd == Cmd::Stitch).count();
    if at >= st.len() || stitches(&st[..at]) < 2 || stitches(&st[at..]) < 2 {
        return Err(bad(CMD, "split where each part keeps two stitches or more"));
    }
    let (first, second) = (st[..at].to_vec(), st[at..].to_vec());
    let copy = s.edit("Split Object", |d, sel| {
        let (parent, index, _) = d.position(id).ok_or(EngineError::NoNode(id))?;
        let n = d.node(id).cloned().ok_or(EngineError::NoNode(id))?;
        let mut c = d.reid(&n);
        if let Some(e) = c.embroidery.as_deref_mut() {
            e.frozen = Some(second.clone());
            e.entry = None;
            e.exit = None;
        }
        let copy = d.insert(parent, index + 1, c)?;
        if let Some(e) = d.node_mut(id).ok_or(EngineError::NoNode(id))?.embroidery.as_deref_mut() {
            e.frozen = Some(first.clone());
        }
        sel.set([id, copy]);
        Ok(copy)
    })?;
    Ok(ids_json(&[id, copy]))
}

fn regenerate(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.regenerate";
    let ids = stitched_targets(s, p)?;
    let frozen: Vec<NodeId> = {
        let d = &s.doc()?.doc;
        ids.iter().copied().filter(|id| d.node(*id).and_then(|n| n.embroidery.as_deref()).is_some_and(|e| e.frozen.is_some())).collect()
    };
    if frozen.is_empty() {
        return Err(bad(CMD, "no selected object has edited stitches"));
    }
    s.edit("Regenerate Stitches", |d, _| {
        for id in &frozen {
            if let Some(e) = d.node_mut(*id).ok_or(EngineError::NoNode(*id))?.embroidery.as_deref_mut() {
                e.frozen = None;
            }
        }
        Ok(())
    })?;
    Ok(ids_json(&frozen))
}

fn point(v: Option<&Value>, what: &str) -> std::result::Result<(f64, f64), String> {
    let a = v.and_then(Value::as_array).ok_or_else(|| format!("`{what}` is [x, y]"))?;
    match (a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_f64)) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok((x, y)),
        _ => Err(format!("`{what}` is [x, y] (numbers)")),
    }
}

/// The record indices an op names, sorted, each checked against `len`.
fn indices(op: &Value, len: usize) -> std::result::Result<Vec<usize>, String> {
    let a = op.get("indices").and_then(Value::as_array).ok_or("missing `indices`")?;
    let mut out = Vec::with_capacity(a.len());
    for v in a {
        let i = v.as_u64().ok_or("indices are record numbers (0, 1, 2…)")? as usize;
        if i >= len {
            return Err(format!("no record {i}: the object has {len}"));
        }
        out.push(i);
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

fn apply(st: &mut Vec<Stitch>, op: &Value) -> std::result::Result<(), String> {
    match op.get("op").and_then(Value::as_str) {
        Some("move") => {
            let (dx, dy) = point(op.get("by"), "by")?;
            for i in indices(op, st.len())? {
                st[i].x += dx;
                st[i].y += dy;
            }
        }
        Some("delete") => {
            for i in indices(op, st.len())?.into_iter().rev() {
                st.remove(i);
            }
        }
        Some("insert") => {
            let after = op.get("after").and_then(Value::as_i64).ok_or("missing `after` (−1 inserts first)")?;
            if after < -1 || after >= st.len() as i64 {
                return Err(format!("`after` is −1 to {}", st.len() as i64 - 1));
            }
            let (x, y) = point(op.get("at"), "at")?;
            let cmd = match op.get("cmd").and_then(Value::as_str) {
                None => Cmd::Stitch,
                Some(c) => parse_cmd(c).ok_or_else(|| format!("unknown cmd `{c}`: stitch, jump, trim or stop"))?,
            };
            st.insert((after + 1) as usize, Stitch::new(x, y, cmd));
        }
        Some(other) => return Err(format!("unknown op `{other}`: move, delete or insert")),
        None => return Err("each op needs `op`: move, delete or insert".into()),
    }
    Ok(())
}

fn edit(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.stitches.edit";
    let id = id_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`"))?;
    let ops = p.get("ops").and_then(Value::as_array).ok_or_else(|| bad(CMD, "missing `ops` (a list)"))?.clone();
    let (mut st, _) = current(s, id, CMD)?;
    for op in &ops {
        apply(&mut st, op).map_err(|m| bad(CMD, m))?;
    }
    store(s, id, "Edit Stitches", st)
}

/// The record index `at` of object `id`'s stitches.
fn record(p: &Value, len: usize, cmd: &str) -> Result<usize> {
    let at = p.get("at").and_then(Value::as_u64).ok_or_else(|| bad(cmd, "missing `at` (a record number)"))? as usize;
    if at >= len {
        return Err(bad(cmd, format!("no record {at}: the object has {len}")));
    }
    Ok(at)
}

fn function_insert(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.function.insert";
    let id = id_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`"))?;
    let kind = str_param(p, "kind").ok_or_else(|| bad(CMD, "missing `kind`: trim, stop or jump"))?;
    let (mut st, _) = current(s, id, CMD)?;
    let at = record(p, st.len(), CMD)?;
    let here = st[at];
    match kind {
        "trim" | "stop" => {
            let c = if kind == "trim" { Cmd::Trim } else { Cmd::Stop };
            st.insert(at + 1, Stitch::new(here.x, here.y, c));
        }
        "jump" if here.cmd == Cmd::Stitch => st[at].cmd = Cmd::Jump,
        "jump" => return Err(bad(CMD, format!("record {at} is a {}, not a stitch", cmd_name(here.cmd)))),
        other => return Err(bad(CMD, format!("unknown kind `{other}`: trim, stop or jump"))),
    }
    store(s, id, "Insert Machine Function", st)
}

fn function_clear(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.function.clear";
    let id = id_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`"))?;
    let (mut st, _) = current(s, id, CMD)?;
    let at = record(p, st.len(), CMD)?;
    match st[at].cmd {
        Cmd::Trim | Cmd::Stop | Cmd::ColorChange => {
            st.remove(at);
        }
        Cmd::Jump => st[at].cmd = Cmd::Stitch,
        Cmd::Stitch | Cmd::End => return Err(bad(CMD, format!("record {at} is a stitch: no machine function there"))),
    }
    store(s, id, "Clear Machine Function", st)
}
