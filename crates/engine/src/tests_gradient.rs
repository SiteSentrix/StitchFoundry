//! Gradients through commands: lossless, validated params (M3.14).

use serde_json::{Value, json};
use vectorcraft_color::{Color, Gradient, GradientGeom, GradientKind, GradientPaint, GradientStop, Paint};
use vectorcraft_geom::Point;

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

fn node(s: &Session, id: NodeId) -> &vectorcraft_doc::Node {
    s.doc().unwrap().doc.node(id).unwrap()
}

fn fill_gradient(s: &Session, id: NodeId) -> GradientPaint {
    match node(s, id).appearance.fill_paint() {
        Paint::Gradient(g) => *g,
        p => panic!("expected a gradient fill, got {p:?}"),
    }
}

/// A three-stop freeform gradient with a 0.3 midpoint, partial opacity and a placed vector.
fn three_stops() -> GradientPaint {
    let stop = |offset, color, opacity, midpoint| GradientStop { offset, color, opacity, midpoint };
    let mut g = GradientPaint::new(Gradient {
        kind: GradientKind::Freeform,
        stops: vec![
            stop(0.0, Color::rgb(1.0, 0.0, 0.0), 1.0, 0.3),
            stop(0.4, Color::cmyk(0.1, 0.2, 0.3, 0.4), 0.5, 0.5),
            stop(1.0, Color::gray(0.25), 0.75, 0.5),
        ],
    });
    g.geom = Some(GradientGeom { start: Point::new(10.0, 20.0), end: Point::new(110.0, 70.0), aspect: 0.5 });
    g.angle = g.geom.unwrap().angle_deg();
    g
}

fn gradient_param(g: &GradientPaint) -> Value {
    json!({ "gradient": vectorcraft_tools::params::gradient_params(g) })
}

#[test]
fn gradient_params_round_trip_through_fill_and_swatches() {
    let mut s = session();
    let id = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    let g = three_stops();
    s.execute("paint.setFill", &gradient_param(&g)).unwrap();
    let got = fill_gradient(&s, id);
    assert_eq!(got.gradient.kind, GradientKind::Freeform);
    assert_eq!(got.gradient.stops, g.gradient.stops);
    let (a, b) = (got.geom.unwrap(), g.geom.unwrap());
    assert!(a.start.distance(b.start) < 1e-9 && a.end.distance(b.end) < 1e-9 && (a.aspect - b.aspect).abs() < 1e-9, "{a:?}");
    // The default proxy keeps it too, and saving it as a swatch is lossless.
    assert_eq!(s.paint.fill, Paint::Gradient(Box::new(got.clone())));
    let mut q = gradient_param(&g);
    q["name"] = json!("Three");
    s.execute("swatch.new", &q).unwrap();
    assert_eq!(s.doc().unwrap().doc.swatch("Three").unwrap().paint, Paint::Gradient(Box::new(got)));
}

#[test]
fn bad_gradients_are_rejected_without_editing() {
    let mut s = session();
    let id = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    let before = node(&s, id).appearance.clone();
    let undo = s.doc().unwrap().history.undo.len();
    for bad in [
        json!({"stops": [{"offset": 0, "color": "#ff0000"}]}),
        json!({"stops": [{"offset": 0, "color": "#ff0000"}, {"offset": 1}]}),
        json!({"stops": [{"offset": 0, "color": "#ff0000"}, {"color": "#000000"}]}),
        json!({"stops": [{"offset": "a", "color": "#ff0000"}, {"offset": 1, "color": "#000000"}]}),
        json!({"stops": "red"}),
        json!({"kind": "conic"}),
        json!({"start": [0, 0]}),
        json!({"aspect": "wide"}),
    ] {
        assert!(s.execute("paint.setFill", &json!({ "gradient": bad })).is_err(), "accepted {bad}");
        // `start` isn't a paint.editGradient param (it would just apply the gradient).
        if bad.get("start").is_none() {
            assert!(s.execute("paint.editGradient", &bad).is_err(), "editGradient accepted {bad}");
        }
    }
    assert_eq!(node(&s, id).appearance, before);
    assert_eq!(s.doc().unwrap().history.undo.len(), undo);
}

#[test]
fn stop_opacity_and_offsets_are_normalised() {
    let mut s = session();
    let id = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    s.execute(
        "paint.setFill",
        &json!({"gradient": {"stops": [{"offset": 1.5, "color": "#000000", "opacity": 50}, {"offset": -1, "color": "#ffffff", "opacity": 0.25, "midpoint": 0.99}]}}),
    )
    .unwrap();
    let g = fill_gradient(&s, id);
    // Sorted, offsets clamped, 0..100 opacity normalised, midpoint clamped to the diamond's range.
    let st = &g.gradient.stops;
    assert_eq!((st[0].offset, st[0].opacity, st[0].midpoint), (0.0, 0.25, 0.87));
    assert_eq!((st[1].offset, st[1].opacity), (1.0, 0.5));
    assert!(g.geom.is_none());
}

#[test]
fn geometry_and_aspect_in_one_call() {
    let mut s = session();
    let id = rect(&mut s, 100.0, 100.0, 200.0, 100.0);
    s.execute("paint.setFill", &json!({"gradient": {"kind": "radial", "start": [200, 150], "end": [260, 150], "aspect": 40}})).unwrap();
    let geom = fill_gradient(&s, id).geom.unwrap();
    assert_eq!((geom.start, geom.end), (Point::new(200.0, 150.0), Point::new(260.0, 150.0)));
    assert!((geom.aspect - 0.4).abs() < 1e-9);
    // An aspect without a vector places the gradient on the object's bounds so the aspect sticks.
    s.execute("paint.setStroke", &json!({"gradient": {"kind": "radial", "aspect": 25}})).unwrap();
    let Paint::Gradient(sg) = node(&s, id).appearance.stroke_paint() else { panic!() };
    let geom = sg.geom.unwrap();
    assert_eq!(geom.start, Point::new(200.0, 150.0));
    assert!((geom.aspect - 0.25).abs() < 1e-9 && (geom.length() - 100.5).abs() < 1e-9, "fits the stroke-inflated bounds: {geom:?}");
    // appearance.setItem takes the same gradient object.
    let g = three_stops();
    let mut q = gradient_param(&g);
    q["index"] = json!(0);
    s.execute("appearance.setItem", &q).unwrap();
    assert_eq!(fill_gradient(&s, id).gradient, g.gradient);
}

// ---------- M3.15: gradients follow every transform ----------

fn fill_geom(s: &Session, id: NodeId) -> GradientGeom {
    fill_gradient(s, id).geom.expect("placed")
}

#[test]
fn rotate_reflect_and_scale_carry_unplaced_gradients() {
    let mut s = session();
    let id = rect(&mut s, 0.0, 0.0, 100.0, 50.0);
    s.execute("paint.setFill", &json!({"gradient": {}})).unwrap();
    s.execute("object.move", &json!({"dx": 10, "dy": 0})).unwrap();
    assert!(fill_gradient(&s, id).geom.is_none(), "a move keeps refitting");
    s.execute("object.rotate", &json!({"angle": 90})).unwrap();
    let g = fill_geom(&s, id);
    assert!((g.end.x - g.start.x).abs() < 1e-9 && (g.length() - 100.0).abs() < 1e-9, "vertical after a 90° turn: {g:?}");
    s.execute("object.reflect", &json!({"axis": "horizontal"})).unwrap();
    let r = fill_geom(&s, id);
    // The vertical vector is centred on the object, so the reflection swaps its ends.
    assert!(r.start.distance(g.end) < 1e-9 && r.end.distance(g.start) < 1e-9, "{g:?} → {r:?}");

    // A circle's radial gradient becomes an ellipse under a non-uniform scale.
    let c = s.execute("shape.ellipse", &json!({"x": 300, "y": 300, "width": 100, "height": 100})).unwrap();
    let c = NodeId(c["id"].as_u64().unwrap());
    s.execute("paint.setFill", &json!({"gradient": {"kind": "radial"}})).unwrap();
    s.execute("object.scale", &json!({"sx": 200, "sy": 100})).unwrap();
    let e = fill_geom(&s, c);
    assert!((e.aspect - 0.5).abs() < 1e-9 && (e.length() - 100.0).abs() < 1e-9, "{e:?}");
}

#[test]
fn free_distort_maps_gradients_once() {
    let mut s = session();
    let id = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    s.execute("paint.setFill", &json!({"gradient": {}})).unwrap();
    // An affine distort (a sheared parallelogram) maps the gradient exactly like object.transform.
    s.execute("object.distort", &json!({"corners": [[20, 0], [120, 0], [100, 100], [0, 100]]})).unwrap();
    let g = fill_geom(&s, id);
    let mut want = GradientGeom::fit(GradientKind::Linear, vectorcraft_geom::Rect::new(0.0, 0.0, 100.0, 100.0), 0.0);
    want.transform(vectorcraft_geom::Affine::new([1.0, 0.0, -0.2, 1.0, 20.0, 0.0]), GradientKind::Linear);
    assert!(g.start.distance(want.start) < 1e-6 && g.end.distance(want.end) < 1e-6, "{g:?} vs {want:?}");
}
