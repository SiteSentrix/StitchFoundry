//! Variables (data merge): named values bound to objects, applied per dataset.
//!
//! One template document produces many variants: bind a type object to a `text`
//! variable and anything to a `visibility` variable, put a row of values in each
//! dataset, and `dataset.select` swaps the whole document to that row in one undo
//! step. Agents reach every command through `run_command`, like all engine commands.

use serde_json::{Value, json};
use vectorcraft_doc::{DataValue, NodeId, NodeKind, VariableKind};

use super::edit::selected_roots;
use super::textedit::set_plain_text;
use super::*;

fn var_name(p: &Value, cmd: &str) -> Result<String> {
    match str_param(p, "name").filter(|v| !v.is_empty()) {
        Some(v) => Ok(v.to_string()),
        None => Err(bad(cmd, "missing `name`")),
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

fn values_param(s: &Session, p: &Value, cmd: &str) -> Result<std::collections::BTreeMap<String, DataValue>> {
    let mut out = std::collections::BTreeMap::new();
    let Some(given) = p.get("values") else { return Ok(out) };
    let Some(obj) = given.as_object() else { return Err(bad(cmd, "`values` must be an object")) };
    for (var, v) in obj {
        let kind = s.doc()?.doc.variables.variable(var).map(|d| d.kind).ok_or_else(|| bad(cmd, format!("no variable named `{var}`")))?;
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
        d.variables.variables.push(vectorcraft_doc::Variable { name: name.clone(), kind });
        Ok(())
    })?;
    Ok(json!({"name": name, "kind": kind.label()}))
}

fn delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "variable.delete";
    let names = names_param(p, C)?;
    let n = s.edit("Delete Variables", |d, _| {
        d.variables.prune(&names);
        Ok(names.len())
    })?;
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
    let name = str_param(p, "variable").filter(|v| !v.is_empty()).map(str::to_string).ok_or_else(|| bad(C, "missing `variable`"))?;
    let ids = match ids_param(p, "ids") {
        Some(v) => v,
        None => selected_roots(s)?,
    };
    if ids.is_empty() {
        return Err(bad(C, "nothing to bind"));
    }
    let kind = s.doc()?.doc.variables.variable(&name).map(|v| v.kind).ok_or_else(|| bad(C, format!("no variable named `{name}`")))?;
    // A text variable only drives type; visibility drives anything that exists.
    if kind == VariableKind::Text {
        for id in &ids {
            match s.doc()?.doc.node(*id).map(|n| &n.kind) {
                Some(NodeKind::Text(_)) => {}
                Some(_) => return Err(bad(C, format!("node {} is not type", id.0))),
                None => return Err(EngineError::NoNode(*id)),
            }
        }
    } else {
        for id in &ids {
            if s.doc()?.doc.node(*id).is_none() {
                return Err(EngineError::NoNode(*id));
            }
        }
    }
    s.edit("Bind Variable", |d, _| {
        for id in &ids {
            d.variables.bindings.insert(*id, name.clone());
        }
        Ok(())
    })?;
    Ok(json!({"variable": name, "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()}))
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
    let values = values_param(s, p, C)?;
    if s.doc()?.doc.variables.datasets.iter().any(|d| d.name == name) {
        return Err(bad(C, format!("dataset `{name}` already exists")));
    }
    s.edit("New Data Set", |d, _| {
        d.variables.datasets.push(vectorcraft_doc::DataSet { name: name.clone(), values: values.clone() });
        Ok(())
    })?;
    Ok(json!({"name": name}))
}

fn dataset_delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "dataset.delete";
    let names = names_param(p, C)?;
    let n = s.edit("Delete Data Sets", |d, _| {
        let before = d.variables.datasets.len();
        d.variables.datasets.retain(|d| !names.contains(&d.name));
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
    let (applied, skipped) = s.edit("Data Set", |d, _| {
        let ds = d.variables.datasets.iter().find(|x| x.name == name).cloned().ok_or_else(|| bad(C, format!("no dataset named `{name}`")))?;
        let bindings: Vec<(NodeId, String)> = d.variables.bindings.iter().map(|(id, v)| (*id, v.clone())).collect();
        let mut applied = 0usize;
        let mut skipped = 0usize;
        for (id, var) in bindings {
            let (kind, value) = match (d.variables.variable(&var).map(|v| v.kind), ds.values.get(&var)) {
                (Some(kind), Some(value)) if value.fits(kind) => (kind, value.clone()),
                _ => {
                    skipped += 1;
                    continue;
                }
            };
            let done = match (kind, &value) {
                (VariableKind::Text, DataValue::Text(text)) => {
                    if d.node(id).is_some_and(|n| matches!(n.kind, NodeKind::Text(_))) {
                        set_plain_text(d, id, text).is_ok()
                    } else {
                        false
                    }
                }
                (VariableKind::Visibility, DataValue::Visible(show)) => {
                    if let Some(n) = d.node_mut(id) {
                        n.visible = *show;
                        true
                    } else {
                        false
                    }
                }
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
    let name = var_name(p, "dataset.select")?;
    apply_dataset(s, &name)
}

fn dataset_step(s: &mut Session, cmd: &str, forward: bool) -> Result<Value> {
    let st = s.doc()?;
    if st.doc.variables.datasets.is_empty() {
        return Err(bad(cmd, "no datasets"));
    }
    let at = st.doc.variables.active_dataset.as_ref().and_then(|a| st.doc.variables.datasets.iter().position(|d| &d.name == a));
    let next = match at {
        Some(i) if forward => (i + 1) % st.doc.variables.datasets.len(),
        Some(i) => (i + st.doc.variables.datasets.len() - 1) % st.doc.variables.datasets.len(),
        None if forward => 0,
        None => st.doc.variables.datasets.len() - 1,
    };
    let name = st.doc.variables.datasets.get(next).map(|d| d.name.clone()).ok_or_else(|| bad(cmd, "no datasets"))?;
    apply_dataset(s, &name)
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("variable.define", "New Variable", [], None, "{name, kind: text|visibility} define a variable", has_doc, define),
        cmd!("variable.delete", "Delete Variable", [], None, "{names} delete variables with their bindings and values", has_doc, delete),
        cmd!("variable.list", "List Variables", [], None, "{} → variables with kinds and binding counts", has_doc, list),
        cmd!(
            "variable.bind",
            "Bind Variable",
            [],
            None,
            "{variable, ids?} bind objects (default: the selection) to a variable; text needs type",
            has_doc,
            bind
        ),
        cmd!("variable.unbind", "Unbind Variable", [], None, "{ids?} drop bindings (default: the selection)", has_doc, unbind),
        cmd!("dataset.new", "New Data Set", [], None, "{name, values?: {variable: value}} add a dataset", has_doc, dataset_new),
        cmd!("dataset.delete", "Delete Data Set", [], None, "{names} delete datasets", has_doc, dataset_delete),
        cmd!("dataset.list", "List Data Sets", [], None, "{} → datasets with values and the active one", has_doc, dataset_list),
        cmd!("dataset.select", "Select Data Set", [], None, "{name} apply a dataset in one undo step → {applied, skipped}", has_doc, dataset_select),
        cmd!("dataset.next", "Next Data Set", [], None, "{} apply the next dataset, wrapping", has_doc, |s, _| dataset_step(s, "dataset.next", true)),
        cmd!("dataset.prev", "Previous Data Set", [], None, "{} apply the previous dataset, wrapping", has_doc, |s, _| dataset_step(
            s,
            "dataset.prev",
            false
        )),
    ]
}
