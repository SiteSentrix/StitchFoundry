//! Variables (data merge): definitions with kinds, bindings to objects, datasets
//! applied in one undo step, and validation that rejects instead of reinterpreting.

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    s
}

fn text(s: &mut Session, content: &str) -> NodeId {
    let r = s.execute("text.create", &json!({"x": 10, "y": 10, "text": content})).unwrap();
    NodeId(r["id"].as_u64().unwrap())
}

fn rect(s: &mut Session) -> NodeId {
    let r = s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
    NodeId(r["id"].as_u64().unwrap())
}

fn plain(s: &Session, id: NodeId) -> String {
    match &s.doc().unwrap().doc.node(id).unwrap().kind {
        vectorcraft_doc::NodeKind::Text(t) => t.plain_text(),
        _ => panic!("not text"),
    }
}

#[test]
fn text_and_visibility_datasets_apply_in_one_undo_step() {
    let mut s = session();
    let t = text(&mut s, "Alice");
    let r = rect(&mut s);
    s.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
    s.execute("variable.define", &json!({"name": "Show", "kind": "visibility"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Show", "ids": [r.0]})).unwrap();
    s.execute("dataset.new", &json!({"name": "B", "values": {"Name": "Bob", "Show": false}})).unwrap();

    let v = s.execute("dataset.select", &json!({"name": "B"})).unwrap();
    assert_eq!((v["applied"].as_u64(), v["skipped"].as_u64()), (Some(2), Some(0)));
    assert_eq!(plain(&s, t), "Bob");
    assert!(!s.doc().unwrap().doc.node(r).unwrap().visible);

    // One undo step covers the whole dataset application.
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(plain(&s, t), "Alice");
    assert!(s.doc().unwrap().doc.node(r).unwrap().visible);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(plain(&s, t), "Bob");
}

#[test]
fn dataset_next_and_prev_wrap_around() {
    let mut s = session();
    let t = text(&mut s, "x");
    s.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).unwrap();
    for name in ["A", "B"] {
        s.execute("dataset.new", &json!({"name": name, "values": {"Name": name}})).unwrap();
    }
    s.execute("dataset.next", &json!({})).unwrap();
    assert_eq!(plain(&s, t), "A");
    s.execute("dataset.next", &json!({})).unwrap();
    assert_eq!(plain(&s, t), "B");
    s.execute("dataset.next", &json!({})).unwrap();
    assert_eq!(plain(&s, t), "A");
    s.execute("dataset.prev", &json!({})).unwrap();
    assert_eq!(plain(&s, t), "B");
    let v = s.execute("dataset.list", &json!({})).unwrap();
    assert_eq!(v["active"], "B");
}

#[test]
fn mismatched_and_missing_bindings_are_skipped_and_counted() {
    let mut s = session();
    let t = text(&mut s, "x");
    let r = rect(&mut s);
    s.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
    // A text variable bound to a rectangle applies to nothing; a dataset value for an
    // unbound variable is ignored.
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).unwrap();
    s.execute("dataset.new", &json!({"name": "D", "values": {"Name": "y", "Ghost": "z"}})).unwrap_err();
    s.execute("dataset.new", &json!({"name": "D", "values": {"Name": "y"}})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [r.0]})).unwrap_err();
    let v = s.execute("dataset.select", &json!({"name": "D"})).unwrap();
    assert_eq!((v["applied"].as_u64(), v["skipped"].as_u64()), (Some(1), Some(0)));
    assert_eq!(plain(&s, t), "y");
}

#[test]
fn deleting_a_variable_cleans_bindings_values_and_active_dataset() {
    let mut s = session();
    let t = text(&mut s, "x");
    s.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).unwrap();
    s.execute("dataset.new", &json!({"name": "D", "values": {"Name": "y"}})).unwrap();
    s.execute("dataset.select", &json!({"name": "D"})).unwrap();
    s.execute("variable.delete", &json!({"names": ["Name"]})).unwrap();
    let vars = s.execute("variable.list", &json!({})).unwrap();
    assert_eq!(vars["variables"].as_array().map(Vec::len), Some(0));
    let sets = s.execute("dataset.list", &json!({})).unwrap();
    assert!(sets["datasets"][0].get("values").is_none_or(|v| v == &json!({})));
    assert_eq!(sets["active"], "D");
    s.execute("dataset.delete", &json!({"names": ["D"]})).unwrap();
    let sets = s.execute("dataset.list", &json!({})).unwrap();
    assert_eq!(sets["datasets"].as_array().map(Vec::len), Some(0));
    assert!(sets["active"].is_null());
    let v: Value = serde_json::from_str(&serde_json::to_string(&s.doc().unwrap().doc).unwrap()).unwrap();
    assert!(v["variables"].get("bindings").is_none_or(|v| v == &json!({})));
}

#[test]
fn bad_variables_requests_are_errors_not_panics() {
    let mut s = session();
    s.execute("variable.define", &json!({"name": "N", "kind": "text"})).unwrap();
    for (cmd, params) in [
        ("variable.define", json!({})),
        ("variable.define", json!({"name": "", "kind": "text"})),
        ("variable.define", json!({"name": "N", "kind": "colour"})),
        ("variable.define", json!({"name": "N", "kind": "text"})),
        ("variable.delete", json!({})),
        ("variable.bind", json!({})),
        ("variable.bind", json!({"variable": "N"})),
        ("variable.bind", json!({"variable": "Missing", "ids": [1]})),
        ("dataset.new", json!({})),
        ("dataset.new", json!({"name": "D", "values": []})),
        ("dataset.new", json!({"name": "D", "values": {"N": 5}})),
        ("dataset.select", json!({"name": "Missing"})),
        ("dataset.next", json!({})),
    ] {
        assert!(s.execute(cmd, &params).is_err(), "{cmd} {params}");
    }
}
