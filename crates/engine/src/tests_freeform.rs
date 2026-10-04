//! Freeform gradients through commands: the first application, transforms and warps (M3.81).

use serde_json::{Value, json};
use vectorcraft_color::{Freeform, GradientKind, GradientPaint, Paint};
use vectorcraft_geom::Point;

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
    s
}

fn shape(s: &mut Session, cmd: &str, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    let r = s.execute(cmd, &json!({"x": x, "y": y, "width": w, "height": h})).unwrap();
    NodeId(r["id"].as_u64().unwrap())
}

fn node(s: &Session, id: NodeId) -> &vectorcraft_doc::Node {
    s.doc().unwrap().doc.node(id).unwrap()
}

fn fill_gradient(s: &Session, id: NodeId) -> GradientPaint {
    match node(s, id).appearance.fill_paint() {
        Paint::Gradient(g) => *g,
        p => panic!("expected a gradient fill, got {p:?}"),
    }
}

fn points(s: &Session, id: NodeId) -> Freeform {
    fill_gradient(s, id).freeform.expect("placed freeform points")
}

/// A selected 100 × 100 rectangle at (100, 100) with a freeform fill.
fn freeform_rect() -> (Session, NodeId) {
    let mut s = session();
    let id = shape(&mut s, "shape.rectangle", 100.0, 100.0, 100.0, 100.0);
    s.execute("paint.editGradient", &json!({"kind": "freeform"})).unwrap();
    (s, id)
}

fn run(s: &mut Session, cmd: &str, p: Value) -> Value {
    s.execute(cmd, &p).unwrap_or_else(|e| panic!("{cmd} {p}: {e}"))
}

fn close(a: Point, b: Point) -> bool {
    a.distance(b) < 1e-6
}

// ---------- M3.81: the first application, transforms, warps and saving ----------

#[test]
fn the_first_application_places_points_inside_the_shape() {
    let mut s = session();
    // An ellipse: the box's corners are outside it.
    let id = shape(&mut s, "shape.ellipse", 100.0, 100.0, 300.0, 100.0);
    s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
    s.execute("paint.editGradient", &json!({"kind": "freeform"})).unwrap();
    let g = fill_gradient(&s, id);
    assert_eq!(g.gradient.kind, GradientKind::Freeform);
    let f = g.freeform.as_ref().unwrap();
    let inside = cmd::freeform::inside_fn(node(&s, id));
    assert!(f.points.len() >= 4 && f.points.iter().all(|p| inside(p.at)), "{:?}", f.points);
    // The stops follow the points' colours (swatch chips and exports show them).
    assert_eq!(g.gradient.stops.iter().map(|st| st.color).collect::<Vec<_>>(), f.points.iter().map(|p| p.color).collect::<Vec<_>>());
    // Draw mode: freeform only.
    s.execute("paint.editGradient", &json!({"mode": "lines"})).unwrap();
    assert_eq!(points(&s, id).mode, vectorcraft_color::FreeformMode::Lines);
    assert_eq!(points(&s, id).points, f.points, "the points stay");
    s.execute("paint.editGradient", &json!({"kind": "linear"})).unwrap();
    assert!(fill_gradient(&s, id).freeform.is_none(), "a linear gradient drops the points");
    let e = s.execute("paint.editGradient", &json!({"mode": "lines"})).unwrap_err().to_string();
    assert!(e.contains("freeform"), "{e}");
}

#[test]
fn new_stops_recolour_the_points() {
    let (mut s, id) = freeform_rect();
    let stops = json!([{"offset": 0, "color": "#ff0000"}, {"offset": 1, "color": "#0000ff"}]);
    s.execute("paint.editGradient", &json!({"stops": stops})).unwrap();
    let f = points(&s, id);
    assert_eq!((f.points[0].color.to_hex(), f.points.last().unwrap().color.to_hex()), ("#ff0000".into(), "#0000ff".into()));
}

#[test]
fn transforms_and_warps_move_the_points() {
    let (mut s, id) = freeform_rect();
    let before = points(&s, id);
    run(&mut s, "object.move", json!({"dx": 10, "dy": 5}));
    let moved = points(&s, id);
    assert!(before.points.iter().zip(&moved.points).all(|(a, b)| close(b.at, a.at + vectorcraft_geom::Vec2::new(10.0, 5.0))));
    run(&mut s, "object.rotate", json!({"angle": 90, "origin": [160, 155]}));
    let turned = points(&s, id);
    // The y-down document turns (x, y) about the origin to (x', y') = (160 + (y − 155), 155 − (x − 160)).
    for (a, b) in moved.points.iter().zip(&turned.points) {
        let want = Point::new(160.0 + (a.at.y - 155.0), 155.0 - (a.at.x - 160.0));
        assert!(close(b.at, want), "{:?} → {:?}, want {want:?}", a.at, b.at);
    }
    // A perspective distort maps each point exactly: a point on a corner stays on it.
    let (mut s, id) = freeform_rect();
    let corner = json!({"kind": "freeform", "freeform": {"points": [{"at": [200, 200], "color": "#ff0000"}]}});
    run(&mut s, "paint.setFill", json!({ "gradient": corner }));
    run(&mut s, "object.distort", json!({"corners": [[100, 100], [200, 100], [260, 240], [100, 200]]}));
    let f = points(&s, id);
    assert!(close(f.points.last().unwrap().at, Point::new(260.0, 240.0)), "{:?}", f.points.last());
}

#[test]
fn freeform_paints_survive_saving_and_old_files_load() {
    let (mut s, id) = freeform_rect();
    run(&mut s, "paint.editGradient", json!({"mode": "lines"}));
    let mut g = fill_gradient(&s, id);
    g.freeform.as_mut().unwrap().add_line(vec![0, 1, 2]).unwrap();
    run(&mut s, "paint.setFill", json!({"gradient": vectorcraft_tools::params::gradient_params(&g)}));
    assert_eq!(points(&s, id).lines, vec![vec![0, 1, 2]]);
    vectorcraft_testkit::invariants::check_native_roundtrip_exact(&s.doc().unwrap().doc).unwrap();
    // A gradient saved before freeform points existed has none.
    let old: GradientPaint = serde_json::from_value(json!({"gradient": {"kind": "Freeform", "stops": []}, "angle": 0.0})).unwrap();
    assert!(old.freeform.is_none());
    // The command params are lossless: setting the paint read back reproduces it.
    let g = fill_gradient(&s, id);
    let other = shape(&mut s, "shape.rectangle", 300.0, 300.0, 50.0, 50.0);
    run(&mut s, "paint.setFill", json!({"gradient": vectorcraft_tools::params::gradient_params(&g), "ids": [other.0]}));
    assert_eq!(fill_gradient(&s, other), g);
}
