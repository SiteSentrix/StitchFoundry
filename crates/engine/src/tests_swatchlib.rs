//! Swatch libraries: listing, reading, adding to the document (deduped, one undo step, applied)
//! and Default Swatches.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};

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

#[test]
fn list_and_get_every_builtin_library() {
    let mut s = Session::new();
    let libs = run(&mut s, "swatch.library.list", json!({}));
    let libs = libs["libraries"].as_array().unwrap();
    let ids: Vec<&str> = libs.iter().map(|l| l["id"].as_str().unwrap()).collect();
    for id in [
        "web-safe-216",
        "grays-neutrals",
        "earth-tones",
        "skin-tone-ramps",
        "pastels",
        "brights",
        "metallic-gradients",
        "perceptual-scales",
        "harmony-sets",
    ] {
        assert!(ids.contains(&id), "{id} missing from {ids:?}");
    }
    let web = libs.iter().find(|l| l["id"] == "web-safe-216").unwrap();
    assert_eq!((web["name"].as_str(), web["count"].as_u64()), (Some("Web Safe 216"), Some(216)));
    // By id or by name in any case; groups list their swatches.
    let g = run(&mut s, "swatch.library.get", json!({"library": "earth tones"}));
    assert_eq!(g["id"], "earth-tones");
    let clay = g["groups"].as_array().unwrap().iter().find(|x| x["name"] == "Clay").unwrap();
    assert_eq!(clay["swatches"].as_array().unwrap().len(), 6);
    assert!(g["swatches"].as_array().unwrap().iter().all(|w| w["kind"] == "color" && w["group"].is_string()));
    let m = run(&mut s, "swatch.library.get", json!({"library": "metallic-gradients"}));
    assert!(m["swatches"].as_array().unwrap().iter().all(|w| w["kind"] == "gradient"));
    assert!(s.execute("swatch.library.get", &json!({"library": "nope"})).is_err());
}

#[test]
fn add_dedupes_as_one_undo_step() {
    let mut s = session();
    let before = undo_len(&s);
    let r = run(&mut s, "swatch.library.add", json!({"library": "earth-tones", "names": ["Clay", "Ochre 2", "Clay 3"]}));
    // The Clay group comes whole (its third swatch once); Ochre 2 comes ungrouped.
    let added: Vec<&str> = r["added"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(added.len(), 7, "{added:?}");
    assert_eq!(undo_len(&s), before + 1, "one undo step");
    let d = doc(&s);
    assert_eq!(d.swatch_groups.iter().find(|g| g.name == "Clay").map(|g| g.swatches.len()), Some(6));
    assert!(d.swatches.iter().any(|w| w.name == "Ochre 2"));
    // Adding again finds them all and changes nothing (no undo step).
    let r = run(&mut s, "swatch.library.add", json!({"library": "earth-tones", "names": ["Clay", "Ochre 2"]}));
    assert_eq!((r["added"].as_array().unwrap().len(), r["existing"].as_array().unwrap().len()), (0, 7));
    assert_eq!(undo_len(&s), before + 1);
    // A name taken by a different colour gets a number.
    run(&mut s, "swatch.new", json!({"name": "Ochre 3", "color": "#123456"}));
    let r = run(&mut s, "swatch.library.add", json!({"library": "earth-tones", "names": ["Ochre 3"]}));
    assert_eq!(r["added"], json!(["Ochre 3 2"]));
    // Undo removes the whole first add.
    for _ in 0..3 {
        run(&mut s, "edit.undo", json!({}));
    }
    assert!(doc(&s).swatch_groups.iter().all(|g| g.name != "Clay") && doc(&s).swatch("Ochre 2").is_none());
    assert!(s.execute("swatch.library.add", &json!({"library": "earth-tones", "names": ["Nope"]})).is_err());
}

#[test]
fn add_with_apply_paints_the_selection_in_the_same_undo_step() {
    let mut s = session();
    let id = run(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50}))["id"].clone();
    let before = undo_len(&s);
    let r = run(&mut s, "swatch.library.add", json!({"library": "web-safe-216", "names": ["#FF6600"], "apply": "fill"}));
    assert_eq!((r["added"].clone(), r["applied"].clone()), (json!(["#FF6600"]), json!("#FF6600")));
    let fill = doc(&s).node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap().appearance.fill_paint();
    assert_eq!(fill.color().map(|c| c.to_hex()), Some("#ff6600".into()));
    assert_eq!(undo_len(&s), before + 1, "added and applied as one step");
    run(&mut s, "edit.undo", json!({}));
    assert!(doc(&s).swatch("#FF6600").is_none());
    // A gradient swatch applied to the stroke; an existing swatch is applied without an add.
    run(&mut s, "swatch.library.add", json!({"library": "metallic-gradients", "names": ["Gold"], "apply": "stroke"}));
    let n = doc(&s).node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap();
    assert!(matches!(n.appearance.stroke_paint(), Paint::Gradient(_)));
    let r = run(&mut s, "swatch.library.add", json!({"library": "metallic-gradients", "names": ["Gold"], "apply": "fill"}));
    assert_eq!((r["added"].clone(), r["applied"].clone()), (json!([]), json!("Gold")));
}

#[test]
fn reset_defaults_restores_missing_swatches_or_replaces_them() {
    let mut s = session();
    let defaults: Vec<String> = doc(&s).swatches_iter().map(|w| w.name.clone()).collect();
    run(&mut s, "swatch.delete", json!({"names": ["Red", "Grays"]}));
    run(&mut s, "swatch.new", json!({"name": "Mine", "color": "#abcdef"}));
    let r = run(&mut s, "swatch.resetDefaults", json!({}));
    assert!(r["added"].as_array().unwrap().iter().any(|n| n == "Red"));
    let d = doc(&s);
    assert!(d.swatch("Mine").is_some(), "the user's swatches stay");
    assert_eq!(d.swatch_groups.iter().find(|g| g.name == "Grays").map(|g| g.swatches.len()), Some(9));
    assert!(defaults.iter().all(|n| d.swatch(n).is_some()));
    // Replace: exactly the defaults; art linked to a removed global swatch keeps its colour.
    run(&mut s, "swatch.edit", json!({"name": "Mine", "global": true}));
    let id = run(&mut s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 5, "height": 5}))["id"].clone();
    run(&mut s, "select.set", json!({"ids": [id]}));
    run(&mut s, "paint.setFill", json!({"swatch": "Mine"}));
    assert_eq!(s.paint.fill, Paint::Solid { color: Color::from_hex("#abcdef").unwrap(), swatch: Some("Mine".into()) });
    run(&mut s, "swatch.resetDefaults", json!({"replace": true}));
    let d = doc(&s);
    assert_eq!(d.swatches_iter().map(|w| w.name.clone()).collect::<Vec<_>>(), defaults);
    let fill = d.node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap().appearance.fill_paint();
    assert_eq!(fill, Paint::solid(Color::from_hex("#abcdef").unwrap()));
    assert_eq!(s.paint.fill, Paint::solid(Color::from_hex("#abcdef").unwrap()), "the default fill is unlinked too");
}
