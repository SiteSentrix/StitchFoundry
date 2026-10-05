//! `command.batch`: several commands as one undo step.

use serde_json::{Value, json};

use super::super::*;
use crate::EngineError;

pub(super) fn batch(s: &mut Session, p: &Value) -> Result<Value> {
    let mut cmds = p.get("commands").and_then(Value::as_array).ok_or_else(|| bad("command.batch", "missing commands"))?.clone();
    let label = str_param(p, "label").unwrap_or("Batch").to_string();
    s.begin_interaction(&label)?;
    // Session-level state a step may change (paint defaults, drawing mode) rolls back too.
    let saved = (s.paint.clone(), s.fill_active, s.draw_mode, s.draw_inside, s.clipboard.clone());
    let mut results = vec![];
    for c in &mut cmds {
        let id = c.get("command").and_then(Value::as_str).unwrap_or("").to_string();
        if id == "command.batch" {
            s.cancel_interaction()?;
            return Err(bad("command.batch", "batches cannot nest"));
        }
        let params = c.get("params").cloned().unwrap_or(json!({}));
        match s.execute_step(&id, &params) {
            Ok((v, noted)) => {
                // The step's journal entry records what it took from the preferences or the
                // clock (a save's date), as a command of its own would.
                if noted != params
                    && let Some(c) = c.as_object_mut()
                {
                    c.insert("params".into(), noted);
                }
                results.push(v);
            }
            Err(e) => {
                s.cancel_interaction()?;
                (s.paint, s.fill_active, s.draw_mode, s.draw_inside, s.clipboard) = saved;
                return Err(EngineError::Other(format!("batch step {} (`{id}`) failed: {e}", results.len())));
            }
        }
    }
    // Commit as one undo step (the interaction's "preview" is the batch itself).
    let mut entry = p.clone();
    if let Some(o) = entry.as_object_mut() {
        o.insert("commands".into(), Value::Array(cmds));
    }
    if let Some(it) = s.doc_mut()?.interaction.as_mut() {
        it.preview = Some(("command.batch".into(), entry));
    }
    s.commit_interaction()?;
    Ok(json!({ "results": results }))
}
