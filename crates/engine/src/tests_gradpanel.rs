//! The Gradient panel's engine side (M3.33): sampling a colour into a gradient stop.

use serde_json::json;
use vectorcraft_color::Paint;

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

fn stop_hexes(s: &Session, id: NodeId, stroke: bool) -> Vec<String> {
    let a = &s.doc().unwrap().doc.node(id).unwrap().appearance;
    match if stroke { a.stroke_paint() } else { a.fill_paint() } {
        Paint::Gradient(g) => g.gradient.stops.iter().map(|s| s.color.to_hex()).collect(),
        p => panic!("expected a gradient, got {p:?}"),
    }
}

#[test]
fn sample_color_with_a_stop_recolours_that_stop_in_one_undo_step() {
    let mut s = session();
    let id = rect(&mut s, 10.0, 10.0, 100.0, 100.0);
    // A solid fill: no stop to recolour.
    let err = s.execute("paint.sampleColor", &json!({"color": "#ff0000", "stop": 0})).unwrap_err();
    assert!(err.to_string().contains("not a gradient"), "{err}");
    s.execute("paint.setFill", &json!({"gradient": {}})).unwrap();
    s.execute("paint.sampleColor", &json!({"color": "#ff0000", "stop": 1})).unwrap();
    assert_eq!(stop_hexes(&s, id, false), ["#ffffff", "#ff0000"]);
    assert!(s.execute("paint.sampleColor", &json!({"color": "#ff0000", "stop": 2})).unwrap_err().to_string().contains("no stop 2"));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(stop_hexes(&s, id, false), ["#ffffff", "#000000"]);
    // The stroke's gradient through `stroke`, the fill left alone.
    s.execute("paint.setStroke", &json!({"gradient": {}})).unwrap();
    s.execute("paint.sampleColor", &json!({"color": "#00ff00", "stop": 0, "stroke": true})).unwrap();
    assert_eq!(
        (stop_hexes(&s, id, true), stop_hexes(&s, id, false)),
        (vec!["#00ff00".into(), "#000000".into()], vec!["#ffffff".into(), "#000000".into()])
    );
}
