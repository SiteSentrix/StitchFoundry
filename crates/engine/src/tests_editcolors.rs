//! Edit → Edit Colors: Adjust Color Balance modes and the live preview.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
    s
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

/// A selected rectangle filled with `fill`.
fn rect(s: &mut Session, fill: Value) -> NodeId {
    let id = id_of(&s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap());
    s.execute("paint.setFill", &json!({"ids": [id.0], "color": fill})).unwrap();
    s.execute("select.set", &json!({"ids": [id.0]})).unwrap();
    id
}

fn fill_of(s: &Session, id: NodeId) -> Color {
    s.doc().unwrap().doc.node(id).unwrap().appearance.fill_paint().color().unwrap()
}

#[test]
fn adjust_balance_modes() {
    let mut s = session();
    let a = rect(&mut s, json!({"c": 20, "m": 40, "y": 0, "k": 10}));
    // CMYK channels on a CMYK colour stay CMYK.
    s.execute("edit.colors.adjustBalance", &json!({"mode": "cmyk", "c": 10, "k": -10})).unwrap();
    let Color::Cmyk { c, m, k, .. } = fill_of(&s, a) else { panic!("kept CMYK") };
    assert!((c - 0.3).abs() < 1e-5 && (m - 0.4).abs() < 1e-5 && k.abs() < 1e-5, "{c} {m} {k}");
    // RGB channels keep the colour's model unless Convert is on.
    s.execute("edit.colors.adjustBalance", &json!({"mode": "rgb", "r": 10})).unwrap();
    assert!(matches!(fill_of(&s, a), Color::Cmyk { .. }));
    s.execute("edit.colors.adjustBalance", &json!({"mode": "rgb", "convert": true})).unwrap();
    assert!(matches!(fill_of(&s, a), Color::Rgb { .. }), "converted to RGB");
    // Gray with Convert makes a grey of the colour darkened by the shift.
    let b = rect(&mut s, json!("#808080"));
    s.execute("edit.colors.adjustBalance", &json!({"mode": "gray", "gray": 20, "convert": true})).unwrap();
    let Color::Gray { k } = fill_of(&s, b) else { panic!("converted to grey") };
    assert!((k - 0.698).abs() < 0.01, "{k}");
    // Mode follows the channels given when it is left out.
    s.execute("edit.colors.adjustBalance", &json!({"gray": -20})).unwrap();
    assert!(matches!(fill_of(&s, b), Color::Gray { .. }));
    assert!(s.execute("edit.colors.adjustBalance", &json!({"mode": "lab"})).is_err());
}

#[test]
fn global_mode_needs_tints_and_says_so() {
    let mut s = session();
    s.execute("swatch.new", &json!({"name": "Brand", "color": "#3366cc", "global": true})).unwrap();
    let a = id_of(&s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap());
    s.execute("paint.setFill", &json!({"ids": [a.0], "swatch": "Brand"})).unwrap();
    s.execute("select.set", &json!({"ids": [a.0]})).unwrap();
    let before = s.doc().unwrap().doc.clone();
    let e = s.execute("edit.colors.adjustBalance", &json!({"mode": "global"})).unwrap_err().to_string();
    assert!(e.contains("global and spot"), "{e}");
    assert_eq!(*s.doc().unwrap().doc, *before, "nothing changed");
    let p = s.doc().unwrap().doc.node(a).unwrap().appearance.fill_paint();
    assert!(matches!(p, Paint::Solid { swatch: Some(ref n), .. } if n == "Brand"), "the link is kept");
}

#[test]
fn balance_previews_and_commits_one_step() {
    let mut s = session();
    let a = rect(&mut s, json!("#808080"));
    let undo = s.doc().unwrap().history.undo.len();
    s.begin_interaction("Adjust Colors").unwrap();
    for r in [10, 30, 20] {
        s.preview("edit.colors.adjustBalance", &json!({"mode": "rgb", "r": r})).unwrap();
    }
    assert_eq!(fill_of(&s, a).to_hex(), "#b38080", "each preview starts from the original");
    s.commit_interaction().unwrap();
    assert_eq!(s.doc().unwrap().history.undo.len(), undo + 1);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(fill_of(&s, a).to_hex(), "#808080");
}
