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
fn deleting_a_variable_cleans_bindings_values_and_leaves_datasets_alone() {
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
    // The dataset named "D" is still the active one: `variable.delete` prunes variables, and
    // the dataset's own name is its own namespace.
    assert_eq!(sets["active"], "D");
    s.execute("dataset.delete", &json!({"names": ["D"]})).unwrap();
    let sets = s.execute("dataset.list", &json!({})).unwrap();
    assert_eq!(sets["datasets"].as_array().map(Vec::len), Some(0));
    assert!(sets["active"].is_null());
    let v: Value = serde_json::from_str(&serde_json::to_string(&s.doc().unwrap().doc).unwrap()).unwrap();
    assert!(v["variables"].get("bindings").is_none_or(|v| v == &json!({})));
}

/// A dataset may share a variable's name: they are separate namespaces, and deleting the
/// variable must not silently unselect the dataset that happens to be called the same.
#[test]
fn a_dataset_survives_a_variable_of_the_same_name_being_deleted() {
    let mut s = session();
    s.execute("variable.define", &json!({"name": "Row", "kind": "text"})).unwrap();
    s.execute("dataset.new", &json!({"name": "Row", "values": {"Row": "y"}})).unwrap();
    s.execute("dataset.select", &json!({"name": "Row"})).unwrap();
    s.execute("variable.delete", &json!({"names": ["Row"]})).unwrap();
    let sets = s.execute("dataset.list", &json!({})).unwrap();
    assert_eq!(sets["datasets"][0]["name"], "Row", "the dataset is not a variable");
    assert_eq!(sets["active"], "Row", "the active dataset keeps its name");
}

/// Deleting art lets its bindings go, as it does for Asset Export's assets: a binding that
/// outlived its object would count as a skip in every dataset application and grow the file.
#[test]
fn deleted_art_lets_its_bindings_go() {
    let mut s = session();
    let t = text(&mut s, "x");
    let r = rect(&mut s);
    s.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
    s.execute("variable.define", &json!({"name": "Show", "kind": "visibility"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Show", "ids": [r.0]})).unwrap();
    s.execute("dataset.new", &json!({"name": "D", "values": {"Name": "y", "Show": false}})).unwrap();

    s.execute("select.set", &json!({"ids": [t.0]})).unwrap();
    s.execute("edit.clear", &json!({})).unwrap();
    let vars = s.execute("variable.list", &json!({})).unwrap();
    let of = |name: &str| vars["variables"].as_array().map(|a| a.iter().find(|v| v["name"] == name).unwrap()["bindings"].as_u64().unwrap()).unwrap();
    assert_eq!(of("Name"), 0, "the deleted text object took its binding with it");
    assert_eq!(of("Show"), 1, "the rectangle is still bound");
    // Nothing stale is left for the application to skip.
    let v = s.execute("dataset.select", &json!({"name": "D"})).unwrap();
    assert_eq!((v["applied"].as_u64(), v["skipped"].as_u64()), (Some(1), Some(0)));
}

/// `deleted` counts what was deleted, not what was asked for.
#[test]
fn delete_reports_what_it_actually_removed() {
    let mut s = session();
    s.execute("variable.define", &json!({"name": "A", "kind": "text"})).unwrap();
    s.execute("variable.define", &json!({"name": "B", "kind": "text"})).unwrap();
    s.execute("dataset.new", &json!({"name": "D"})).unwrap();
    assert_eq!(s.execute("variable.delete", &json!({"names": ["A"]})).unwrap()["deleted"], 1);
    assert_eq!(s.execute("variable.delete", &json!({"names": ["A"]})).unwrap()["deleted"], 0, "already gone");
    assert_eq!(s.execute("variable.delete", &json!({"names": ["A", "B"]})).unwrap()["deleted"], 1, "only B was left");
    assert_eq!(s.execute("dataset.delete", &json!({"names": ["D"]})).unwrap()["deleted"], 1);
    assert_eq!(s.execute("dataset.delete", &json!({"names": ["D"]})).unwrap()["deleted"], 0);
}

/// An object holds one binding, so binding it again replaces what it had, and the reply says
/// which objects were rebound rather than letting the old variable silently lose them.
#[test]
fn binding_again_replaces_and_says_so() {
    let mut s = session();
    let t = text(&mut s, "x");
    s.execute("variable.define", &json!({"name": "First", "kind": "text"})).unwrap();
    s.execute("variable.define", &json!({"name": "Second", "kind": "text"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "First", "ids": [t.0]})).unwrap();
    let v = s.execute("variable.bind", &json!({"variable": "Second", "ids": [t.0]})).unwrap();
    assert_eq!(v["replaced"], json!([t.0]));
    let vars = s.execute("variable.list", &json!({})).unwrap();
    let of = |n: &str| vars["variables"].as_array().unwrap().iter().find(|v| v["name"] == n).unwrap()["bindings"].as_u64();
    assert_eq!((of("First"), of("Second")), (Some(0), Some(1)));
    // Rebinding to the same variable changes nothing and reports no replacement.
    assert_eq!(s.execute("variable.bind", &json!({"variable": "Second", "ids": [t.0]})).unwrap()["replaced"], json!([]));
    assert_eq!(s.execute("variable.unbind", &json!({"ids": [t.0]})).unwrap()["unbound"], 1);
    assert_eq!(s.execute("variable.unbind", &json!({"ids": [t.0]})).unwrap()["unbound"], 0);
}

/// Locked art is the document's to rewrite only while it isn't locked.
#[test]
fn locked_art_is_neither_bound_nor_rewritten() {
    let mut s = session();
    let t = text(&mut s, "x");
    let r = rect(&mut s);
    s.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
    s.execute("variable.define", &json!({"name": "Show", "kind": "visibility"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Show", "ids": [r.0]})).unwrap();
    s.execute("dataset.new", &json!({"name": "D", "values": {"Name": "y", "Show": false}})).unwrap();

    s.execute("select.set", &json!({"ids": [t.0]})).unwrap();
    s.execute("object.lock", &json!({})).unwrap();
    assert!(s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).is_err(), "locked art takes no new binding");
    // The bindings made before the lock still count, and the application skips the locked art.
    let v = s.execute("dataset.select", &json!({"name": "D"})).unwrap();
    assert_eq!((v["applied"].as_u64(), v["skipped"].as_u64()), (Some(1), Some(1)));
    assert_eq!(plain(&s, t), "x", "the locked text was left alone");
    assert!(!s.doc().unwrap().doc.node(r).unwrap().visible, "the unlocked rectangle still applied");
}

/// `dataset.set` is how a value changes after the row exists: replacing it must not need the
/// dataset to be deleted and rebuilt (which loses its place in the list and the active one).
#[test]
fn dataset_set_replaces_the_row_and_reports_what_it_dropped() {
    let mut s = session();
    let t = text(&mut s, "x");
    let r = rect(&mut s);
    s.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
    s.execute("variable.define", &json!({"name": "Show", "kind": "visibility"})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Name", "ids": [t.0]})).unwrap();
    s.execute("variable.bind", &json!({"variable": "Show", "ids": [r.0]})).unwrap();
    s.execute("dataset.new", &json!({"name": "D", "values": {"Name": "y", "Show": false}})).unwrap();

    // The dataset keeps its place and stays active while its values change.
    let v = s.execute("dataset.set", &json!({"name": "D", "values": {"Name": "z"}})).unwrap();
    assert_eq!((v["values"].clone(), v["removed"].clone()), (json!(1), json!(["Show"])));
    let sets = s.execute("dataset.list", &json!({})).unwrap();
    assert_eq!(sets["datasets"].as_array().map(Vec::len), Some(1));
    assert_eq!(sets["datasets"][0]["values"], json!({"Name": {"text": "z"}}));

    s.execute("dataset.select", &json!({"name": "D"})).unwrap();
    assert_eq!(plain(&s, t), "z");
    assert!(s.doc().unwrap().doc.node(r).unwrap().visible, "`Show` has no value in this row now");

    // Errors are errors, not a silent rewrite of some other row.
    assert!(s.execute("dataset.set", &json!({"name": "Missing"})).is_err());
    assert!(s.execute("dataset.set", &json!({"name": "D", "values": {"Ghost": "z"}})).is_err());
    assert!(s.execute("dataset.set", &json!({"name": "D", "values": {"Show": "yes"}})).is_err());
    assert!(s.execute("dataset.set", &json!({"name": "D", "values": []})).is_err());
    // One step, so one undo puts the row back as it was.
    s.execute("dataset.set", &json!({"name": "D", "values": {"Name": "w"}})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(plain(&s, t), "z");
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
