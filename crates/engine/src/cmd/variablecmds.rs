//! Variables (data merge): named values bound to objects, applied per dataset.
//!
//! One template document produces many variants: bind a type object to a `text`
//! variable and anything to a `visibility` variable, put a row of values in each
//! dataset, and `dataset.select` swaps the whole document to that row in one undo
//! step. Agents reach every command through `run_command`, like all engine commands.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use vectorcraft_doc::{DataSet, DataValue, NodeId, NodeKind, Variable, VariableKind, Variables};

use super::edit::selected_roots;
use super::textedit::set_plain_text;
use super::*;

fn var_name(p: &Value, cmd: &str) -> Result<String> {
    named(str_param(p, "name"), "name", cmd)
}

/// `key`'s value, as a non-empty name.
fn named(v: Option<&str>, key: &str, cmd: &str) -> Result<String> {
    match v.filter(|v| !v.is_empty()) {
        Some(v) => Ok(v.to_string()),
        None => Err(bad(cmd, format!("missing `{key}`"))),
    }
}

fn names_param(p: &Value, cmd: &str) -> Result<Vec<String>> {
    match p.get("names").and_then(Value::as_array) {
        Some(names) if !names.is_empty() => {
            let mut out = Vec::with_capacity(names.len());
            for n in names {
                match n.as_str().filter(|v| !v.is_empty()) {
                    Some(v) => out.push(v.to_string()),
                    None => return Err(bad(cmd, "`names` must be non-empty strings")),
                }
            }
            Ok(out)
        }
        _ => Err(bad(cmd, "missing `names`")),
    }
}

fn dataset_value(cmd: &str, var: &str, kind: VariableKind, v: &Value) -> Result<DataValue> {
    match kind {
        VariableKind::Text => v.as_str().map(|t| DataValue::Text(t.to_string())),
        VariableKind::Visibility => v.as_bool().map(DataValue::Visible),
    }
    .ok_or_else(|| bad(cmd, format!("value for `{var}` must match its variable kind")))
}

/// The `values` object of `dataset.new` / `dataset.set`, each entry checked against its
/// variable's kind. Every name must be a variable the document defines: a value nothing
/// reads is a typo, not a value to keep.
fn values_param(vars: &Variables, p: &Value, cmd: &str) -> Result<BTreeMap<String, DataValue>> {
    let mut out = BTreeMap::new();
    let Some(given) = p.get("values") else { return Ok(out) };
    let Some(obj) = given.as_object() else { return Err(bad(cmd, "`values` must be an object")) };
    for (var, v) in obj {
        let kind = vars.variable(var).map(|d| d.kind).ok_or_else(|| bad(cmd, format!("no variable named `{var}`")))?;
        out.insert(var.clone(), dataset_value(cmd, var, kind, v)?);
    }
    Ok(out)
}

fn define(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.define";
    let name = var_name(p, C)?;
    let kind = str_param(p, "kind").and_then(VariableKind::parse).ok_or_else(|| bad(C, "`kind` must be `text` or `visibility`"))?;
    if s.doc()?.doc.variables.variable(&name).is_some() {
        return Err(bad(C, format!("variable `{name}` is already defined")));
    }
    s.edit("New Variable", |d, _| {
        d.variables.variables.push(Variable { name: name.clone(), kind });
        Ok(())
    })?;
    Ok(json!({"name": name, "kind": kind.label()}))
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    let names = names_param(p, "variable.delete")?;
    let n = s.edit("Delete Variables", |d, _| Ok(d.variables.prune(&names)))?;
    Ok(json!({"deleted": n}))
}

fn list(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let variables: Vec<Value> = st
        .doc
        .variables
        .variables
        .iter()
        .map(|v| {
            let bindings = st.doc.variables.bindings.values().filter(|b| *b == &v.name).count();
            json!({"name": v.name, "kind": v.kind.label(), "bindings": bindings})
        })
        .collect();
    Ok(json!({"variables": variables}))
}

fn bind(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.bind";
    let name = named(str_param(p, "variable"), "variable", C)?;
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    if ids.is_empty() {
        return Err(bad(C, "nothing to bind"));
    }
    let doc = &s.doc()?.doc;
    let kind = doc.variables.variable(&name).map(|v| v.kind).ok_or_else(|| bad(C, format!("no variable named `{name}`")))?;
    for id in &ids {
        match doc.node(*id) {
            None => return Err(EngineError::NoNode(*id)),
            // Art on a locked or hidden layer is not the document's to rewrite, and a text
            // variable only drives type.
            Some(_) if !doc.is_editable(*id) => return Err(bad(C, format!("node {} is locked", id.0))),
            Some(n) if kind == VariableKind::Text && !matches!(n.kind, NodeKind::Text(_)) => {
                return Err(bad(C, format!("node {} is not type", id.0)));
            }
            Some(_) => {}
        }
    }
    // An object holds one binding, so binding it again replaces what it had; say which.
    let replaced: Vec<u64> = ids.iter().filter(|id| doc.variables.bindings.get(id).is_some_and(|b| *b != name)).map(|id| id.0).collect();
    s.edit("Bind Variable", |d, _| {
        for id in &ids {
            d.variables.bindings.insert(*id, name.clone());
        }
        Ok(())
    })?;
    Ok(json!({"variable": name, "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>(), "replaced": replaced}))
}

fn unbind(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    let n = s.edit("Unbind Variable", |d, _| {
        let mut n = 0;
        for id in &ids {
            if d.variables.bindings.remove(id).is_some() {
                n += 1;
            }
        }
        Ok(n)
    })?;
    Ok(json!({"unbound": n}))
}

fn dataset_new(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dataset.new";
    let name = var_name(p, C)?;
    let st = s.doc()?;
    // A dataset and a variable are separate namespaces, so a dataset may share a variable's
    // name; it is its own row of values either way.
    let values = values_param(&st.doc.variables, p, C)?;
    if st.doc.variables.dataset(&name).is_some() {
        return Err(bad(C, format!("dataset `{name}` already exists")));
    }
    s.edit("New Data Set", |d, _| {
        d.variables.datasets.push(DataSet { name: name.clone(), values: values.clone() });
        Ok(())
    })?;
    Ok(json!({"name": name}))
}

/// Replace a dataset's values. A variable left out of `values` loses its value in this row,
/// so the dataset ends up holding exactly what the call asked for.
fn dataset_set(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dataset.set";
    let name = var_name(p, C)?;
    let values = values_param(&s.doc()?.doc.variables, p, C)?;
    let (set, removed) = s.edit("Edit Data Set", |d, _| {
        let ds = d.variables.datasets.iter_mut().find(|x| x.name == name).ok_or_else(|| bad(C, format!("no dataset named `{name}`")))?;
        let removed: Vec<String> = ds.values.keys().filter(|k| !values.contains_key(*k)).cloned().collect();
        ds.values = values.clone();
        Ok((values.len(), removed))
    })?;
    Ok(json!({"name": name, "values": set, "removed": removed}))
}

fn dataset_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let names = names_param(p, "dataset.delete")?;
    let n = s.edit("Delete Data Sets", |d, _| {
        let before = d.variables.datasets.len();
        d.variables.datasets.retain(|d| !names.contains(&d.name));
        // The active dataset's own name is the one that goes, not a variable's.
        if d.variables.active_dataset.as_ref().is_some_and(|a| names.contains(a)) {
            d.variables.active_dataset = None;
        }
        Ok(before - d.variables.datasets.len())
    })?;
    Ok(json!({"deleted": n}))
}

fn dataset_list(s: &mut Session, _p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let datasets: Vec<Value> = st.doc.variables.datasets.iter().map(|d| json!({"name": d.name, "values": d.values})).collect();
    Ok(json!({"datasets": datasets, "active": st.doc.variables.active_dataset}))
}

fn apply_dataset(s: &mut Session, name: &str) -> Result<Value> {
    const C: &str = "dataset.select";
    let (applied, skipped) = s.edit("Apply Data Set", |d, _| {
        let ds = d.variables.dataset(name).cloned().ok_or_else(|| bad(C, format!("no dataset named `{name}`")))?;
        let bindings: Vec<(NodeId, String)> = d.variables.bindings.iter().map(|(id, v)| (*id, v.clone())).collect();
        let mut applied = 0usize;
        let mut skipped = 0usize;
        for (id, var) in bindings {
            // A binding whose variable is gone, whose value this row doesn't carry, or whose
            // value doesn't fit the variable's kind, is left alone and counted, never
            // silently reinterpreted.
            let (kind, value) = match (d.variables.variable(&var).map(|v| v.kind), ds.values.get(&var)) {
                (Some(kind), Some(value)) if value.fits(kind) => (kind, value.clone()),
                _ => {
                    skipped += 1;
                    continue;
                }
            };
            // Art on a locked or hidden layer, and text whose object is gone, can't be
            // rewritten: skipped and counted, like the rest.
            if !d.is_editable(id) {
                skipped += 1;
                continue;
            }
            let done = match (kind, &value) {
                // The Type tool's own whole-range replace, so a dataset's text is styled and
                // laid out exactly as typing it would be.
                (VariableKind::Text, DataValue::Text(text)) => set_plain_text(d, id, text).is_ok(),
                (VariableKind::Visibility, DataValue::Visible(show)) => d.node_mut(id).is_some_and(|n| {
                    n.visible = *show;
                    true
                }),
                _ => false,
            };
            if done {
                applied += 1;
            } else {
                skipped += 1;
            }
        }
        d.variables.active_dataset = Some(name.to_string());
        Ok((applied, skipped))
    })?;
    Ok(json!({"name": name, "applied": applied, "skipped": skipped}))
}

fn dataset_select(s: &mut Session, p: &Value) -> Result<Value> {
    apply_dataset(s, &var_name(p, "dataset.select")?)
}

fn dataset_step(s: &mut Session, cmd: &str, forward: bool) -> Result<Value> {
    let st = s.doc()?;
    let n = st.doc.variables.datasets.len();
    if n == 0 {
        return Err(bad(cmd, "no datasets"));
    }
    let at = st.doc.variables.active_dataset.as_deref().and_then(|a| st.doc.variables.datasets.iter().position(|d| d.name == a));
    let next = match (at, forward) {
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
        (None, true) => 0,
        (None, false) => n - 1,
    };
    let name = st.doc.variables.datasets.get(next).map(|d| d.name.clone()).ok_or_else(|| bad(cmd, "no datasets"))?;
    apply_dataset(s, &name)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("variable.define", "New Variable…", ["Window", "Variables"], None, "{name, kind: text|visibility} define a variable", has_doc, define),
        cmd!(
            "variable.delete",
            "Delete Variable",
            ["Window", "Variables"],
            None,
            "{names} delete variables with their bindings and values",
            has_doc,
            delete
        ),
        cmd!(query "variable.list", "List Variables", [], None, "{} → variables with kinds and binding counts", has_doc, list),
        cmd!(
            "variable.bind",
            "Bind Variable",
            ["Window", "Variables"],
            None,
            "{variable, ids?} bind objects (default: the selection) to a variable; text needs type, locked art is refused. An object holds one binding, so this replaces what it had → {ids, replaced}",
            has_doc,
            bind
        ),
        cmd!("variable.unbind", "Unbind Variable", ["Window", "Variables"], None, "{ids?} drop bindings (default: the selection)", has_doc, unbind),
        cmd!("dataset.new", "New Data Set…", ["Window", "Variables"], None, "{name, values?: {variable: value}} add a dataset", has_doc, dataset_new),
        cmd!(
            "dataset.set",
            "Edit Data Set…",
            ["Window", "Variables"],
            None,
            "{name, values?: {variable: value}} replace a dataset's values; a variable left out loses its value in this row → {values, removed}",
            has_doc,
            dataset_set
        ),
        cmd!("dataset.delete", "Delete Data Set", ["Window", "Variables"], None, "{names} delete datasets", has_doc, dataset_delete),
        cmd!(query "dataset.list", "List Data Sets", [], None, "{} → datasets with values and the active one", has_doc, dataset_list),
        cmd!(
            "dataset.select",
            "Select Data Set",
            ["Window", "Variables"],
            None,
            "{name} apply a dataset in one undo step → {applied, skipped}",
            has_doc,
            dataset_select
        ),
        cmd!("dataset.next", "Next Data Set", ["Window", "Variables"], None, "{} apply the next dataset, wrapping", has_doc, |s, _| dataset_step(
            s,
            "dataset.next",
            true
        )),
        cmd!("dataset.prev", "Previous Data Set", ["Window", "Variables"], None, "{} apply the previous dataset, wrapping", has_doc, |s, _| {
            dataset_step(s, "dataset.prev", false)
        }),
    ]
}
