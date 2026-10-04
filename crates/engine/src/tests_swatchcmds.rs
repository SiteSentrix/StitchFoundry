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

fn rect_filled(s: &mut Session, fill: Value) -> NodeId {
    let id = NodeId(run(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10}))["id"].as_u64().unwrap());
    let mut p = fill;
    p["ids"] = json!([id.0]);
    run(s, "paint.setFill", p);
    run(s, "paint.setStroke", json!({"ids": [id.0], "none": true}));
    id
}

fn fill(s: &Session, id: NodeId) -> Paint {
    doc(s).node(id).unwrap().appearance.fill_paint()
}

#[test]
fn add_used_colors_skips_colours_that_have_swatches() {
    let mut s = session();
    rect_filled(&mut s, json!({"color": "#123456"}));
    rect_filled(&mut s, json!({"color": "#ed1c24"}));
    rect_filled(&mut s, json!({"swatch": "Orange"}));
    let undo = undo_len(&s);
    let r = run(&mut s, "swatch.addUsedColors", json!({}));
    assert_eq!(r, json!({"added": ["R=18 G=52 B=86"], "linked": 0}), "Red and Orange are swatches already");
    assert_eq!(undo_len(&s), undo + 1);
    assert_eq!(run(&mut s, "swatch.addUsedColors", json!({}))["added"], json!([]));
    // Only the selection's colours, as global swatches its paints link to.
    let d = rect_filled(&mut s, json!({"color": "#abcdef"}));
    rect_filled(&mut s, json!({"color": "#fedcba"}));
    run(&mut s, "select.set", json!({"ids": [d.0]}));
    let r = run(&mut s, "swatch.addUsedColors", json!({"selection": true, "global": true}));
    assert_eq!(r, json!({"added": ["R=171 G=205 B=239"], "linked": 1}));
    assert_eq!(fill(&s, d), Paint::Solid { color: vectorcraft_color::Color::from_hex("#abcdef").unwrap(), swatch: Some("R=171 G=205 B=239".into()) });
    assert!(doc(&s).swatch("R=171 G=205 B=239").unwrap().global);
}

#[test]
fn unused_leaves_out_linked_colours_gradients_and_patterns_in_use() {
    let mut s = session();
    let all = run(&mut s, "swatch.unused", json!({}));
    assert!(!all["names"].as_array().unwrap().contains(&json!("[None]")));
    assert!(all["names"].as_array().unwrap().contains(&json!("Sunset")));
    run(&mut s, "swatch.edit", json!({"name": "Teal", "global": true}));
    rect_filled(&mut s, json!({"swatch": "Teal"}));
    run(&mut s, "swatch.edit", json!({"name": "Teal", "color": "#010203"}));
    rect_filled(&mut s, json!({"swatch": "Sunset"}));
    rect_filled(&mut s, json!({"color": "#ed1c24"}));
    let r = rect_filled(&mut s, json!({"color": "#ff0000"}));
    run(&mut s, "select.set", json!({"ids": [r.0]}));
    run(&mut s, "object.pattern.make", json!({"name": "Dots", "width": 20, "height": 20}));
    run(&mut s, "object.pattern.done", json!({}));
    rect_filled(&mut s, json!({"swatch": "Dots"}));
    let names = run(&mut s, "swatch.unused", json!({}))["names"].clone();
    let names: Vec<&str> = names.as_array().unwrap().iter().filter_map(Value::as_str).collect();
    for used in ["Teal", "Sunset", "Red", "Dots", "White"] {
        assert!(!names.contains(&used), "{used} is used: {names:?}");
    }
    for unused in ["Orange", "Ocean", "Bright Red", "K=50"] {
        assert!(names.contains(&unused), "{unused} is unused: {names:?}");
    }
}

#[test]
fn merge_keeps_the_first_and_relinks_the_others_art() {
    let mut s = session();
    for n in ["Red", "Orange"] {
        run(&mut s, "swatch.edit", json!({"name": n, "global": true}));
    }
    let a = rect_filled(&mut s, json!({"swatch": "Red"}));
    let b = rect_filled(&mut s, json!({"swatch": "Orange"}));
    let red = doc(&s).swatch("Red").unwrap().paint.color().unwrap();
    let before = doc(&s).clone();
    let undo = undo_len(&s);
    let r = run(&mut s, "swatch.merge", json!({"names": ["Red", "Orange"]}));
    assert_eq!(r, json!({"name": "Red", "merged": ["Orange"], "relinked": 1}));
    assert!(doc(&s).swatch("Orange").is_none());
    assert_eq!(fill(&s, b), Paint::Solid { color: red, swatch: Some("Red".into()) });
    assert_eq!(undo_len(&s), undo + 1);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(*doc(&s), before);
    // A kept swatch that isn't global leaves the merged swatches' art unlinked, in its colour.
    run(&mut s, "swatch.merge", json!({"names": ["Lime", "Red", "Bright Red"]}));
    let lime = doc(&s).swatch("Lime").unwrap().paint.color().unwrap();
    assert_eq!(fill(&s, a), Paint::solid(lime));
    assert!(doc(&s).swatch("Red").is_none() && doc(&s).swatch("Bright Red").is_none());
    for bad in [json!({"names": ["Lime"]}), json!({"names": ["Lime", "Sunset"]}), json!({"names": ["Lime", "Nope"]})] {
        assert!(s.execute("swatch.merge", &bad).is_err(), "{bad}");
    }
}

#[test]
fn ungroup_and_sort_by_kind_are_one_undo_step_each() {
    let mut s = session();
    let undo = undo_len(&s);
    assert_eq!(run(&mut s, "swatch.ungroup", json!({"name": "Brights"}))["swatches"].as_array().unwrap().len(), 5);
    assert!(doc(&s).swatch_groups.iter().all(|g| g.name != "Brights"));
    assert_eq!(names(&s, None).last().map(String::as_str), Some("Bright Violet"));
    assert!(s.execute("swatch.ungroup", &json!({"name": "Red"})).is_err());
    // A gradient first, a spot colour among the process ones.
    run(&mut s, "swatch.move", json!({"name": "Ocean", "to": 0}));
    run(&mut s, "swatch.edit", json!({"name": "Orange", "spot": true}));
    run(&mut s, "swatch.sortByKind", json!({}));
    assert_eq!(undo_len(&s), undo + 4);
    let rank = |w: &vectorcraft_color::Swatch| match &w.paint {
        Paint::None => 0,
        Paint::Solid { .. } if !w.spot => 1,
        Paint::Solid { .. } => 2,
        Paint::Gradient(_) => 3,
        Paint::Pattern { .. } => 4,
    };
    let ranks: Vec<u8> = doc(&s).swatches.iter().map(rank).collect();
    assert!(ranks.is_sorted(), "{ranks:?}");
    let n = names(&s, None);
    assert_eq!((n[0].as_str(), n[1].as_str()), ("[None]", "White"), "the order within a kind is kept");
    assert_eq!(n[n.iter().position(|x| x == "Orange").unwrap() + 1], "Ocean", "the spot colour comes after the process ones, then the gradients");
}
