//! Perspective Grid definition, presets and view options (`perspective.grid.get` / `define`,
//! `perspective.presets.*`, View → Perspective Grid toggles, the grid widgets).

use serde_json::{Value, json};
use vectorcraft_tools::distort::perspective::{PerspectiveGrid, Rgb};

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
    s
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn grid(s: &Session) -> PerspectiveGrid {
    PerspectiveGrid::effective(&s.doc().unwrap().doc)
}

fn undo_len(s: &Session) -> usize {
    s.doc().unwrap().history.undo.len()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// ---------- M8.8: Define Grid ----------

#[test]
fn grid_get_reports_the_model_the_define_fields_and_the_station_point() {
    let mut s = session();
    let r = run(&mut s, "perspective.grid.get", json!({}));
    assert_eq!(r["defined"], json!(false), "the default grid until one is set");
    assert_eq!(r["grid"]["kind"], json!(2));
    assert_eq!(r["define"]["name"], json!("[2P-Normal View]"));
    assert_eq!((r["define"]["angle"].clone(), r["define"]["units"].clone()), (json!(45.0), json!("points")));
    let g = grid(&s);
    assert!(near(r["station"]["x"].as_f64().unwrap(), g.origin[0]), "the normal view looks at the corner");
    assert!(near(r["station"]["distance"].as_f64().unwrap(), (g.vp_right - g.vp_left) / 2.0));
    // A query: nothing to undo.
    assert_eq!(undo_len(&s), 0);
    run(&mut s, "perspective.grid.preset", json!({"kind": 1}));
    assert_eq!(run(&mut s, "perspective.grid.get", json!({}))["defined"], json!(true));
}

#[test]
fn define_sets_the_grid_from_the_dialog_fields_in_one_undo_step() {
    let mut s = session();
    run(&mut s, "perspective.grid.preset", json!({"kind": 2}));
    let before = grid(&s);
    let n = undo_len(&s);
    let r = run(
        &mut s,
        "perspective.grid.define",
        json!({"units": "inches", "gridline": 0.5, "angle": 30, "distance": 4, "horizonHeight": 2, "leftColor": "#00ff00", "opacity": 75}),
    );
    assert_eq!(undo_len(&s), n + 1);
    assert_eq!(r["units"], json!("inches"));
    let g = grid(&s);
    assert!(near(g.cell, 36.0) && near(g.origin[1] - g.horizon, 144.0));
    let st = g.station();
    assert!(near(st.distance, 288.0) && near(st.x, before.station().x), "the viewer stays put");
    assert!(near(g.viewing_angle(), 30.0));
    assert_eq!((g.left_color, g.opacity, g.name.as_str()), (Rgb([0, 255, 0]), 75.0, ""), "no longer the preset");
    // Unchanged fields leave the grid alone: no undo step.
    let same = run(&mut s, "perspective.grid.get", json!({}))["define"].clone();
    run(&mut s, "perspective.grid.define", same);
    assert_eq!(undo_len(&s), n + 1);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(grid(&s), before);
    // Bad values are refused and change nothing.
    for bad in [json!({"angle": 0}), json!({"distance": -1}), json!({"units": "parsecs"}), json!({"kind": 5}), json!({"rightColor": "red"})] {
        assert!(s.execute("perspective.grid.define", &bad).is_err(), "{bad}");
    }
    assert_eq!(grid(&s), before);
}

#[test]
fn define_switches_the_type_around_the_station_point() {
    let mut s = session();
    run(&mut s, "perspective.grid.preset", json!({"kind": 2}));
    let st = grid(&s).station();
    run(&mut s, "perspective.grid.define", json!({"kind": 1}));
    let g = grid(&s);
    assert_eq!(g.kind, 1);
    assert!(near(g.vp_left, st.x) && near(g.distance, st.distance), "one-point looks straight at the station's centre of vision");
    run(&mut s, "perspective.grid.define", json!({"kind": 3, "thirdVp": [0, 500]}));
    let g = grid(&s);
    assert!(near(g.vp_vertical[0], g.station().x) && near(g.horizon - g.vp_vertical[1], 500.0));
}

#[test]
fn grid_definition_fields_survive_a_native_round_trip() {
    let mut s = session();
    run(&mut s, "perspective.grid.define", json!({"units": "cm", "scale": [1, 4], "angle": 40, "groundColor": "#123456", "opacity": 20}));
    let doc = s.doc().unwrap().doc.clone();
    vectorcraft_testkit::invariants::check_native_roundtrip_exact(&doc).unwrap();
    let back = vectorcraft_format::load(&vectorcraft_format::save(&doc, false)).unwrap();
    assert_eq!(PerspectiveGrid::from_doc(&back), PerspectiveGrid::from_doc(&doc));
    let g = PerspectiveGrid::from_doc(&back).unwrap();
    assert_eq!((g.units.as_str(), g.scale, g.ground_color, g.opacity), ("centimeters", [1.0, 4.0], Rgb([0x12, 0x34, 0x56]), 20.0));
    // A grid saved before these fields loads with their defaults.
    let mut old = serde_json::to_value(&g).unwrap();
    for k in ["name", "angle", "units", "scale", "leftColor", "rightColor", "groundColor", "opacity"] {
        old.as_object_mut().unwrap().remove(k);
    }
    let g: PerspectiveGrid = serde_json::from_value(old).unwrap();
    assert_eq!((g.angle, g.units.as_str(), g.scale, g.opacity), (None, "points", [1.0, 1.0], 50.0));
}

// ---------- M8.9: presets ----------

fn preset_names(s: &mut Session) -> Vec<String> {
    let r = run(s, "perspective.presets.list", json!({}));
    r["presets"].as_array().unwrap().iter().map(|p| p["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn built_in_views_apply_by_name_or_type() {
    let mut s = session();
    let names = preset_names(&mut s);
    assert_eq!(&names[..3], ["[1P-Normal View]", "[1P-Low View]", "[1P-High View]"]);
    assert!(names.contains(&"[3P-Normal View]".to_string()));
    let r = run(&mut s, "perspective.presets.list", json!({}));
    assert!(r["presets"].as_array().unwrap().iter().all(|p| p["builtIn"] == json!(true)));
    run(&mut s, "perspective.grid.preset", json!({"name": "[2P-low view]"}));
    let low = grid(&s);
    assert_eq!((low.name.as_str(), low.kind, low.visible), ("[2P-Low View]", 2, true));
    run(&mut s, "perspective.grid.preset", json!({"kind": 2}));
    let normal = grid(&s);
    assert_eq!(normal.name, "[2P-Normal View]");
    assert!(low.origin[1] - low.horizon < normal.origin[1] - normal.horizon, "the low view's horizon is nearer the ground");
    assert!(s.execute("perspective.grid.preset", &json!({"name": "[9P-Fisheye]"})).is_err());
    assert!(s.execute("perspective.grid.preset", &json!({})).is_err());
}

#[test]
fn presets_save_rename_delete_and_protect_the_built_in_ones() {
    let mut s = session();
    run(&mut s, "perspective.grid.preset", json!({"kind": 3}));
    run(&mut s, "perspective.grid.define", json!({"angle": 35, "opacity": 30}));
    // Save Grid as Preset: the document's grid under a name, not an undo step.
    let n = undo_len(&s);
    let r = run(&mut s, "perspective.presets.save", json!({"name": "Tower"}));
    assert_eq!((r["name"].clone(), r["created"].clone(), r["definition"]["kind"].clone()), (json!("Tower"), json!(true), json!(3)));
    assert_eq!(undo_len(&s), n);
    let saved = &s.prefs.perspective_presets[0];
    assert!((saved.angle - 35.0).abs() < 1e-9 && saved.opacity == 30.0);
    // A grid with its fields is that preset; changing one makes it custom again.
    run(&mut s, "perspective.grid.preset", json!({"kind": 2}));
    run(&mut s, "perspective.grid.preset", json!({"name": "tower"}));
    assert_eq!(grid(&s).name, "Tower");
    assert!((grid(&s).viewing_angle() - 35.0).abs() < 1e-9);
    run(&mut s, "perspective.grid.define", json!({"opacity": 31}));
    assert_eq!(grid(&s).name, "");
    run(&mut s, "perspective.grid.define", json!({"name": "Tower", "opacity": 30}));
    assert_eq!(grid(&s).name, "Tower");
    // Edit, rename, start from another preset.
    let r = run(&mut s, "perspective.presets.save", json!({"name": "Tower", "newName": "Spire", "gridline": 12}));
    assert_eq!((r["name"].clone(), r["created"].clone()), (json!("Spire"), json!(false)));
    assert_eq!(s.prefs.perspective_presets[0].gridline, 12.0);
    assert_eq!(run(&mut s, "perspective.presets.save", json!({"preset": "[1P-Normal View]"}))["name"], json!("Perspective Preset 1"));
    assert_eq!(s.prefs.perspective_presets[1].kind, 1);
    // Built-in names are protected and names stay unique.
    assert!(s.execute("perspective.presets.save", &json!({"name": "[2P-Normal View]"})).is_err());
    assert!(s.execute("perspective.presets.save", &json!({"name": "Spire", "newName": "[1P-Low View]"})).is_err());
    assert!(s.execute("perspective.presets.save", &json!({"name": "Spire", "newName": "perspective preset 1"})).is_err());
    assert!(s.execute("perspective.presets.save", &json!({"name": "Spire", "angle": 95})).is_err());
    assert!(s.execute("perspective.presets.delete", &json!({"name": "[3P-Low View]"})).is_err());
    assert_eq!(run(&mut s, "perspective.presets.delete", json!({"name": "spire"}))["deleted"], json!("Spire"));
    assert!(s.execute("perspective.presets.delete", &json!({"name": "Spire"})).is_err());
    assert_eq!(preset_names(&mut s).last().unwrap(), "Perspective Preset 1");
}

#[test]
fn presets_export_import_and_live_in_the_preferences() {
    let mut s = session();
    run(&mut s, "perspective.presets.save", json!({"name": "Street", "units": "inches", "gridline": 1, "distance": 8, "horizonHeight": 4}));
    let text = run(&mut s, "perspective.presets.export", json!({"names": ["Street", "[2P-High View]"]}))["data"].as_str().unwrap().to_string();
    assert!(text.contains("vcperspective"));
    let mut t = session();
    let r = run(&mut t, "perspective.presets.import", json!({"data": text}));
    assert_eq!(r["imported"], json!(["Street", "2P-High View"]), "a built-in view comes in as a saved copy");
    assert_eq!(t.prefs.perspective_presets[0], s.prefs.perspective_presets[0]);
    // Again: names in use get a number, or replace.
    assert_eq!(run(&mut t, "perspective.presets.import", json!({"data": text}))["imported"], json!(["Street 2", "2P-High View 2"]));
    assert_eq!(run(&mut t, "perspective.presets.import", json!({"data": text, "replace": true}))["imported"], json!(["Street", "2P-High View"]));
    assert_eq!(t.prefs.perspective_presets.len(), 4);
    // Bad files are refused whole.
    for bad in [
        json!({"data": "{}"}),
        json!({"data": "{\"format\": \"vcprintpresets\", \"presets\": []}"}),
        json!({"data": "{\"format\": \"vcperspective\", \"presets\": [{\"name\": \"ok\"}, {\"name\": \"x\", \"angle\": 120}]}"}),
        json!({"data": "{\"format\": \"vcperspective\", \"presets\": [{\"kind\": \"two\"}]}"}),
        json!({"dataBase64": "%%%"}),
        json!({}),
    ] {
        assert!(t.execute("perspective.presets.import", &bad).is_err(), "{bad}");
    }
    assert_eq!(t.prefs.perspective_presets.len(), 4);
    assert!(Session::new().execute("perspective.presets.export", &json!({})).is_err(), "nothing saved to export");
    // Saved with the preferences; older preferences have none.
    let back: Prefs = serde_json::from_value(serde_json::to_value(&t.prefs).unwrap()).unwrap();
    assert_eq!(back.perspective_presets, t.prefs.perspective_presets);
    assert!(serde_json::from_value::<Prefs>(json!({})).unwrap().perspective_presets.is_empty());
}
