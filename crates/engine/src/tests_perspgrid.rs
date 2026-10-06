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
