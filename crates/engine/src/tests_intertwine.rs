//! Object › Intertwine (#1763), as a tutorial does it: two thick rings that overlap twice are
//! made into an Intertwine group, then one overlap is clicked so the rings pass over and under
//! each other. Driven through `Session::execute`; the art is checked on screen, in the saved file
//! and in the SVG and PDF output.

use serde_json::{Value, json};
use vectorcraft_render::Rendered;

use super::tests_clip::{hit, id_of, render, select, session};
use super::*;

/// Ring A (red, centre 35,50) and ring B (blue, centre 65,50), radius 25, 8 pt strokes, no fill:
/// B is in front. The rings cross at (50, 30) and (50, 70).
fn rings(s: &mut Session) -> (NodeId, NodeId) {
    let ring = |s: &mut Session, cx: f64, color: &str| {
        let id = id_of(&s.execute("shape.ellipse", &json!({"x": cx - 25.0, "y": 25, "width": 50, "height": 50})).unwrap());
        s.execute("paint.setFill", &json!({"ids": [id.0], "none": true})).unwrap();
        s.execute("paint.setStroke", &json!({"ids": [id.0], "color": color})).unwrap();
        s.execute("stroke.set", &json!({"ids": [id.0], "weight": 8})).unwrap();
        id
    };
    let a = ring(s, 35.0, "#ff0000");
    let b = ring(s, 65.0, "#0000ff");
    (a, b)
}

/// The rings made into an Intertwine group: (group, a, b).
fn intertwined(s: &mut Session) -> (NodeId, NodeId, NodeId) {
    let (a, b) = rings(s);
    select(s, &[a, b]);
    let g = id_of(&s.execute("object.intertwine.make", &json!({})).unwrap());
    (g, a, b)
}

fn red(r: &Rendered, x: u32, y: u32) -> bool {
    let p = r.pixel(x, y);
    p[3] > 200 && p[0] > 200 && p[2] < 60
}

fn blue(r: &Rendered, x: u32, y: u32) -> bool {
    let p = r.pixel(x, y);
    p[3] > 200 && p[2] > 200 && p[0] < 60
}

fn arrange(s: &mut Session, p: Value) -> Value {
    s.execute("object.intertwine.arrange", &p).unwrap()
}

fn area(s: &Session, id: NodeId) -> Option<vectorcraft_geom::PathData> {
    s.doc().unwrap().doc.node(id).unwrap().extras().intertwine.as_deref().cloned()
}

#[test]
fn make_puts_the_objects_in_an_intertwine_group_drawn_as_before() {
    let mut s = session();
    let (g, a, b) = intertwined(&mut s);
    let d = &s.doc().unwrap().doc;
    let n = d.node(g).unwrap();
    assert!(matches!(n.kind, NodeKind::Intertwine { .. }));
    assert_eq!(n.kind_label(), "Intertwine");
    assert_eq!(n.children().unwrap().iter().map(|c| c.id).collect::<Vec<_>>(), [a, b], "the stacking order is kept");
    assert_eq!(s.doc().unwrap().selection.objects.iter().copied().collect::<Vec<_>>(), [g]);
    let r = render(&s);
    assert!(blue(&r, 50, 30) && blue(&r, 50, 70), "B is in front at both crossings until an area is arranged");
}

#[test]
fn clicking_an_overlap_brings_the_object_below_to_the_front_there_only() {
    let mut s = session();
    let (_, a, b) = intertwined(&mut s);
    let v = arrange(&mut s, json!({"at": [50, 30]}));
    assert_eq!(v["count"], 1);
    assert_eq!(v["top"], json!([a.0]));
    let r = render(&s);
    assert!(red(&r, 50, 30), "A passes over B at the top crossing");
    assert!(blue(&r, 50, 70), "and under it at the bottom one");
    // Elsewhere each ring looks as it did.
    assert!(red(&r, 10, 50) && blue(&r, 90, 50));
    assert!(area(&s, a).is_some() && area(&s, b).is_none());
    // A click there picks the ring drawn on top.
    assert_eq!(hit(&s, 50.0, 30.0), Some(a));
    assert_eq!(hit(&s, 50.0, 70.0), Some(b));
}

#[test]
fn clicking_the_same_overlap_again_swaps_back() {
    let mut s = session();
    let (_, a, b) = intertwined(&mut s);
    arrange(&mut s, json!({"at": [50, 30]}));
    let v = arrange(&mut s, json!({"at": [50, 30]}));
    assert_eq!(v["top"], json!([b.0]));
    assert!(area(&s, a).is_none() && area(&s, b).is_none(), "no area is left over");
    assert!(blue(&render(&s), 50, 30));
}

#[test]
fn a_path_drawn_around_overlaps_arranges_every_overlap_inside_it() {
    let mut s = session();
    let (_, a, _) = intertwined(&mut s);
    // Around the bottom crossing only.
    let v = arrange(&mut s, json!({"area": [[40, 60], [60, 60], [60, 80], [40, 80]]}));
    assert_eq!(v["count"], 1);
    let r = render(&s);
    assert!(blue(&r, 50, 30) && red(&r, 50, 70));
    // Around both: each one swaps.
    let v = arrange(&mut s, json!({"area": [[40, 20], [60, 20], [60, 80], [40, 80]]}));
    assert_eq!(v["count"], 2);
    let r = render(&s);
    assert!(red(&r, 50, 30) && blue(&r, 50, 70));
    assert!(area(&s, a).is_some());
}

#[test]
fn front_and_back_choose_the_member_where_three_objects_overlap() {
    let mut s = session();
    let (a, b) = rings(&mut s);
    // A green bar across the top crossing, in front of both rings.
    let c = id_of(&s.execute("shape.rectangle", &json!({"x": 46, "y": 20, "width": 8, "height": 20})).unwrap());
    s.execute("paint.setFill", &json!({"ids": [c.0], "color": "#00ff00"})).unwrap();
    s.execute("paint.setStroke", &json!({"ids": [c.0], "none": true})).unwrap();
    select(&mut s, &[a, b, c]);
    s.execute("object.intertwine.make", &json!({})).unwrap();
    let green = |r: &Rendered| {
        let p = r.pixel(50, 30);
        p[1] > 200 && p[0] < 60 && p[2] < 60
    };
    assert!(green(&render(&s)));
    arrange(&mut s, json!({"at": [50, 30], "order": "front", "member": a.0}));
    assert!(red(&render(&s), 50, 30), "A is brought to the front of all three");
    arrange(&mut s, json!({"at": [50, 30], "order": "back", "member": a.0}));
    assert!(green(&render(&s)), "sent to the back, the others are over it again in their own order");
    arrange(&mut s, json!({"at": [50, 30], "order": "back"}));
    assert!(blue(&render(&s), 50, 30), "back without a member sends the top one (C) back: B is on top");
    let e = s.execute("object.intertwine.arrange", &json!({"at": [50, 30], "order": "front", "member": 999})).unwrap_err();
    assert!(e.to_string().contains("999"), "{e}");
}

#[test]
fn release_gives_the_objects_back_as_they_were() {
    let mut s = session();
    let (g, a, b) = intertwined(&mut s);
    arrange(&mut s, json!({"at": [50, 30]}));
    let v = s.execute("object.intertwine.release", &json!({})).unwrap();
    assert_eq!(v["ids"], json!([a.0, b.0]));
    let d = &s.doc().unwrap().doc;
    assert!(d.node(g).is_none());
    assert!(d.node(a).unwrap().extras().intertwine.is_none());
    assert!(blue(&render(&s), 50, 30));
    // Undo brings the group back, arranged.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(red(&render(&s), 50, 30));
}

#[test]
fn the_arranged_areas_move_with_the_group() {
    let mut s = session();
    let (g, ..) = intertwined(&mut s);
    arrange(&mut s, json!({"at": [50, 30]}));
    select(&mut s, &[g]);
    s.execute("object.move", &json!({"dx": 0, "dy": 10})).unwrap();
    let r = render(&s);
    assert!(red(&r, 50, 40) && blue(&r, 50, 80));
}

#[test]
fn the_woven_rings_survive_save_and_export() {
    let mut s = session();
    intertwined(&mut s);
    arrange(&mut s, json!({"at": [50, 30]}));
    for format in ["vectorcraft", "svg", "pdf"] {
        let data = s.execute("document.export", &json!({"format": format})).unwrap()["dataBase64"].clone();
        let mut o = Session::new();
        o.execute("document.open", &json!({"name": format!("rings.{format}"), "dataBase64": data})).unwrap();
        let r = render(&o);
        assert!(red(&r, 50, 30) && blue(&r, 50, 70), "{format}");
    }
}

#[test]
fn what_cannot_be_intertwined_is_refused_with_a_reason() {
    let mut s = session();
    let (a, b) = rings(&mut s);
    select(&mut s, &[a]);
    let e = s.execute("object.intertwine.make", &json!({})).unwrap_err();
    assert!(e.to_string().contains("two or more"), "{e}");
    select(&mut s, &[a, b]);
    let g = id_of(&s.execute("object.intertwine.make", &json!({})).unwrap());
    let c = id_of(&s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 5, "height": 5})).unwrap());
    let e = s.execute("object.intertwine.make", &json!({"ids": [g.0, c.0]})).unwrap_err();
    assert!(e.to_string().contains("Intertwine group can't"), "{e}");
    let e = s.execute("object.intertwine.make", &json!({"ids": [a.0, c.0]})).unwrap_err();
    assert!(e.to_string().contains("already in an Intertwine group"), "{e}");
    select(&mut s, &[g]);
    let e = s.execute("object.intertwine.arrange", &json!({"at": [5, 50]})).unwrap_err();
    assert!(e.to_string().contains("no overlap"), "{e}");
    let e = s.execute("object.intertwine.arrange", &json!({"at": [50, 30], "area": [[0, 0], [1, 0], [1, 1]]})).unwrap_err();
    assert!(e.to_string().contains("either"), "{e}");
    let e = s.execute("object.intertwine.arrange", &json!({"at": [50, 30], "order": "sideways"})).unwrap_err();
    assert!(e.to_string().contains("sideways"), "{e}");
    let e = s.execute("object.intertwine.arrange", &json!({"area": [[0, 0], [1, 0]]})).unwrap_err();
    assert!(e.to_string().contains("polygon"), "{e}");
    let e = s.execute("object.intertwine.arrange", &json!({"id": c.0, "at": [1, 1]})).unwrap_err();
    assert!(e.to_string().contains("not an Intertwine group"), "{e}");
}
