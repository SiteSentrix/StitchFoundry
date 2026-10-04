//! Swatches panel commands: moving swatches between colour groups, the panel menu's Add Used
//! Colors, Select All Unused, Merge, Ungroup and Sort by Kind, and colours following the
//! document's colour mode.

use serde_json::{Value, json};
use vectorcraft_color::Paint;

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    s
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn doc(s: &Session) -> &vectorcraft_doc::Document {
    &s.doc().unwrap().doc
}

fn undo_len(s: &Session) -> usize {
    s.doc().unwrap().history.undo.len()
}

/// The swatch names of colour group `group`, or of the ungrouped swatches.
fn names(s: &Session, group: Option<&str>) -> Vec<String> {
    let d = doc(s);
    let list = match group {
        Some(g) => &d.swatch_groups.iter().find(|x| x.name == g).unwrap().swatches,
        None => &d.swatches,
    };
    list.iter().map(|w| w.name.clone()).collect()
}

#[test]
fn move_between_groups_is_one_undo_step() {
    let mut s = session();
    let before = doc(&s).clone();
    let undo = undo_len(&s);
    // Two ungrouped colours into Brights, before its second swatch, in the order given.
    run(&mut s, "swatch.move", json!({"names": ["Orange", "Red"], "group": "Brights", "to": 1}));
    assert_eq!(names(&s, Some("Brights"))[..4], ["Bright Red", "Orange", "Red", "Bright Yellow"]);
    assert!(!names(&s, None).contains(&"Red".to_string()));
    assert_eq!(undo_len(&s), undo + 1);
    // Out of a group to the front of the ungrouped colours (`to` counts the current entries).
    run(&mut s, "swatch.move", json!({"name": "Bright Green", "to": 1}));
    assert_eq!(names(&s, None)[..3], ["[None]", "Bright Green", "White"]);
    run(&mut s, "edit.undo", json!({}));
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(*doc(&s), before);
    // Reordering within a list; past the end goes last.
    run(&mut s, "swatch.move", json!({"name": "White", "to": 999}));
    assert_eq!(names(&s, None).last().map(String::as_str), Some("White"));
}

#[test]
fn move_reorders_groups_and_refuses_bad_moves() {
    let mut s = session();
    run(&mut s, "swatch.move", json!({"name": "Brights", "to": 0}));
    let groups: Vec<&str> = doc(&s).swatch_groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(groups, ["Brights", "Grays"]);
    let undo = undo_len(&s);
    for p in [
        json!({"name": "Sunset", "group": "Grays"}),
        json!({"name": "[None]"}),
        json!({"name": "Grays", "group": "Brights"}),
        json!({"names": ["Red", "Grays"]}),
        json!({"name": "Red", "group": "Nope"}),
        json!({}),
    ] {
        assert!(s.execute("swatch.move", &p).is_err(), "{p}");
    }
    assert_eq!(undo_len(&s), undo, "failed moves leave no undo step");
    assert!(doc(&s).swatch("Sunset").is_some_and(|w| matches!(w.paint, Paint::Gradient(_))));
}
