//! Puppet Warp: turning the art around a pin (`object.puppetWarp {angles}`) and the tool's pins.

use serde_json::json;
use vectorcraft_geom::{Point, Rect};
use vectorcraft_tools::{Mods, PointerEvent, PointerKind};

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
    s
}

fn rect(s: &mut Session, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    let r = s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap();
    NodeId(r["id"].as_u64().unwrap())
}

fn bounds(s: &Session, id: NodeId) -> Rect {
    s.doc().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap()
}

#[test]
fn a_pin_with_an_angle_turns_the_art_around_it() {
    let mut s = session();
    let a = rect(&mut s, 100.0, 100.0, 200.0, 100.0);
    // One pin at the centre, held at a quarter turn: the 200 × 100 rectangle stands up.
    s.execute("object.puppetWarp", &json!({"id": a.0, "pins": [[200, 150]], "moved": [[200, 150]], "angles": [90]})).unwrap();
    let b = bounds(&s, a);
    assert!((b.width() - 100.0).abs() < 4.0 && (b.height() - 200.0).abs() < 4.0, "{b:?}");
    assert!(b.center().distance(Point::new(200.0, 150.0)) < 2.0, "{b:?}");
    // null leaves a pin free; the list must be as long as the pins.
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("object.puppetWarp", &json!({"id": a.0, "pins": [[200, 150]], "moved": [[210, 150]], "angles": [null]})).unwrap();
    assert!((bounds(&s, a).x0 - 110.0).abs() < 0.5, "a free single pin only moves the art");
    for bad in [json!([90, 0]), json!("90"), json!({})] {
        assert!(s.execute("object.puppetWarp", &json!({"id": a.0, "pins": [[200, 150]], "moved": [[200, 150]], "angles": bad})).is_err(), "{bad}");
    }
}

#[test]
fn alt_drag_with_the_tool_turns_the_art_in_one_undo_step() {
    let mut s = session();
    let a = rect(&mut s, 100.0, 100.0, 200.0, 100.0);
    s.execute("select.set", &json!({"ids": [a.0]})).unwrap();
    let v = ViewInfo::default();
    s.select_tool("puppetWarp", v).unwrap();
    s.set_tool_option("selectAllPins", &json!(false));
    // Select the pin in the middle, then Alt-drag about a quarter turn around it, 20 pt out.
    s.pointer(&PointerEvent::new(PointerKind::Down, 200.0, 150.0), v).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Up, 200.0, 150.0), v).unwrap();
    let n = s.doc().unwrap().history.undo.len();
    let alt = Mods { alt: true, ..Default::default() };
    for (k, x, y) in
        [(PointerKind::Down, 220.0, 150.0), (PointerKind::Drag, 214.0, 164.0), (PointerKind::Drag, 200.0, 170.0), (PointerKind::Up, 200.0, 170.0)]
    {
        s.pointer(&PointerEvent::new(k, x, y).with_mods(alt), v).unwrap();
    }
    assert_eq!(s.doc().unwrap().history.undo.len(), n + 1);
    let (cmd, p) = s.journal.last().unwrap();
    assert_eq!(cmd, "object.puppetWarp");
    let angles = p["angles"].as_array().unwrap();
    assert!(angles.iter().any(|a| a.as_f64().is_some_and(|d| (d - 90.0).abs() < 15.0)), "about a quarter turn: {p}");
    let b = bounds(&s, a);
    assert!(b.height() > 110.0, "the art turned: {b:?}");
}
