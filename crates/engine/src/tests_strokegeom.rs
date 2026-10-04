//! Stroke geometry from the Stroke panel's commands (`cmd/stroke.rs`): the width-profile presets.

use serde_json::json;
use vectorcraft_doc::{StrokeLayer, WidthProfile};

use super::*;

fn session_with_line() -> (Session, NodeId) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
    let r = s.execute("shape.line", &json!({"x1": 10, "y1": 100, "x2": 150, "y2": 100})).unwrap();
    (s, NodeId(r["id"].as_u64().unwrap()))
}

fn stroke(s: &Session, id: NodeId) -> StrokeLayer {
    s.doc().unwrap().doc.node(id).unwrap().appearance.stroke().unwrap().clone()
}

#[test]
fn profiles_come_from_the_preset_catalogue() {
    let (mut s, id) = session_with_line();
    for p in WidthProfile::PRESETS {
        s.execute("stroke.set", &json!({"profile": p.id})).unwrap();
        let st = stroke(&s, id);
        assert_eq!(WidthProfile::id_of(st.profile.as_ref()), p.id);
    }
    assert!(stroke(&s, id).profile.is_some());
    s.execute("stroke.set", &json!({"profile": "uniform"})).unwrap();
    assert!(stroke(&s, id).profile.is_none(), "uniform is the plain stroke");
    assert!(s.execute("stroke.set", &json!({"profile": "zigzag"})).is_err());
    // The params doc (what agents read) lists every preset.
    let spec = command_specs().iter().find(|c| c.id == "stroke.set").unwrap();
    for p in WidthProfile::PRESETS {
        assert!(spec.params.contains(&format!("\"{}\"", p.id)), "{}", p.id);
    }
}

#[test]
fn profiles_survive_the_native_format() {
    let (mut s, id) = session_with_line();
    s.execute("stroke.set", &json!({"startArrow": "CircleOpen", "endArrow": "Arrow", "profile": "taperEnd"})).unwrap();
    let doc = &s.doc().unwrap().doc;
    let back = vectorcraft_format::load(&vectorcraft_format::save(doc, false)).unwrap();
    let st = back.node(id).unwrap().appearance.stroke().unwrap().clone();
    assert_eq!(st, stroke(&s, id));
    assert_eq!(WidthProfile::id_of(st.profile.as_ref()), "taperEnd");
}
