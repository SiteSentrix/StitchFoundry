//! Linked measurements, history, portable artwork and hostile command input.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use serde_json::{Value, json};
use vectorcraft_engine::{
    Session,
    doc::{Document, NodeId, NodeKind},
};

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":600,"height":500})).unwrap();
    s
}
fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap()
}
fn edge(s: &mut Session) -> u64 {
    run(s, "shape.line", json!({"x1":100,"y1":100,"x2":172,"y2":100}))["id"].as_u64().unwrap()
}
fn dim(s: &mut Session, node: u64) -> u64 {
    run(
        s,
        "dimension.create",
        json!({"source":{"kind":"segment","edge":{"node":node,"subpath":0,"segment":0}},"options":{"units":"in","precision":2,"scale":"1:10"}}),
    )["id"]
        .as_u64()
        .unwrap()
}
fn text(s: &Session, id: u64) -> String {
    let n = s.doc().unwrap().doc.node(NodeId(id)).unwrap();
    n.children().unwrap().iter().find_map(|n| if let NodeKind::Text(t) = &n.kind { Some(t.plain_text()) } else { None }).unwrap()
}
#[test]
fn dimension_length_scale_live_edit_undo_and_roundtrip() {
    let mut s = session();
    let line = edge(&mut s);
    let id = dim(&mut s, line);
    assert_eq!(text(&s, id), "10.00 in");
    run(&mut s, "select.set", json!({"ids":[line]}));
    run(&mut s, "object.scale", json!({"sx":200,"sy":100}));
    assert_eq!(text(&s, id), "20.00 in");
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(text(&s, id), "10.00 in");
    run(&mut s, "edit.redo", json!({}));
    assert_eq!(text(&s, id), "20.00 in");
    let json = serde_json::to_string(&*s.doc().unwrap().doc).unwrap();
    let doc: Document = serde_json::from_str(&json).unwrap();
    let mut reopened = Session::new();
    reopened.add_document(doc, None);
    run(&mut reopened, "select.set", json!({"ids":[line]}));
    run(&mut reopened, "object.scale", json!({"sx":50,"sy":100}));
    assert_eq!(text(&reopened, id), "10.00 in");
    run(&mut reopened, "edit.clear", json!({}));
    assert!(reopened.doc().unwrap().doc.node(NodeId(id)).is_none());
    run(&mut reopened, "edit.undo", json!({}));
    assert_eq!(text(&reopened, id), "10.00 in");
}
#[test]
fn dimension_selected_rectangle_and_styles_are_one_undo() {
    let mut s = session();
    run(&mut s, "shape.rectangle", json!({"x":100,"y":100,"width":72,"height":144}));
    let result = run(
        &mut s,
        "dimension.selection",
        json!({"options":{"units":"in","precision":0,"lineType":"dashed","arrowStyle":"open","hideExtensions":true,"textPosition":"center"}}),
    );
    let ids = result["ids"].as_array().unwrap();
    assert_eq!(ids.len(), 4);
    let labels: Vec<_> = ids.iter().map(|id| text(&s, id.as_u64().unwrap())).collect();
    assert_eq!(labels.iter().filter(|s| s.as_str() == "1 in").count(), 2);
    let id = NodeId(ids[0].as_u64().unwrap());
    let node = s.doc().unwrap().doc.node(id).unwrap();
    assert_eq!(node.children().unwrap().len(), 3); // Split line and text, no extension lines.
    let stroke = node.children().unwrap()[0].appearance.stroke().unwrap();
    assert!(stroke.dash.is_some());
    assert_eq!(stroke.start_arrow, Some(vectorcraft_doc::Arrowhead::TriangleOpen));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.doc().unwrap().doc.node(id).is_none());
    run(&mut s, "edit.redo", json!({}));
    assert!(s.doc().unwrap().doc.node(id).is_some());
}
#[test]
fn dimension_angles_circles_and_curved_lengths() {
    let mut s = session();
    let a = run(
        &mut s,
        "dimension.create",
        json!({"mode":"angular","a":[100,0],"vertex":[0,0],"b":[0,100],"options":{"precision":1,"scale":"1:10"}}),
    )["id"]
        .as_u64()
        .unwrap();
    assert_eq!(text(&s, a), "90.0°");
    let circle = run(&mut s, "shape.ellipse", json!({"x":100,"y":100,"width":144,"height":144}))["id"].as_u64().unwrap();
    for (mode, want) in [("radius", "R 1.0 in"), ("diameter", "Ø 2.0 in")] {
        let id = run(&mut s, "dimension.create", json!({"mode":mode,"source":{"kind":"circle","node":circle},"options":{"units":"in"}}))["id"]
            .as_u64()
            .unwrap();
        assert_eq!(text(&s, id), want);
    }
    let curve = run(&mut s, "path.create", json!({"d":"M 0 0 C 0 100 100 100 100 0"}))["id"].as_u64().unwrap();
    let id = dim(&mut s, curve);
    assert!(text(&s, id).starts_with("Arc "));
    let d = s.doc().unwrap().doc.node(NodeId(id)).unwrap().extras().dimension.as_ref().unwrap();
    assert!((d.length - 200.0).abs() < 0.001);
}
#[test]
fn dimension_rejects_bad_input_without_mutation() {
    let mut s = session();
    let line = edge(&mut s);
    let id = dim(&mut s, line);
    let original = serde_json::to_value(&*s.doc().unwrap().doc).unwrap();
    for p in [
        json!({}),
        json!({"a":[0,0],"b":[0,0]}),
        json!({"a":[0,0],"b":[1e100,0]}),
        json!({"a":[0,0],"b":[10,0],"options":{"precision":99999}}),
        json!({"a":[0,0],"b":[10,0],"options":{"scale":"0:10"}}),
        json!({"a":[0,0],"b":[10,0],"options":{"scale":"1:NaN"}}),
        json!({"source":{"kind":"segment","edge":{"node":line,"subpath":0,"segment":18446744073709551615u64}}}),
        json!({"mode":"angular","source":{"kind":"segment","edge":{"node":line,"subpath":0,"segment":0}}}),
    ] {
        assert!(s.execute("dimension.create", &p).is_err(), "{p}");
        assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), original);
    }
    assert!(s.execute("dimension.update", &json!({"ids":[id],"options":{"fontSize":-1}})).is_err());
    assert_eq!(serde_json::to_value(&*s.doc().unwrap().doc).unwrap(), original);
}

#[test]
fn dimension_gesture_highlights_preview_commits_and_cancels() {
    use vectorcraft_engine::{
        ViewInfo,
        tools::{Mods, PointerEvent, PointerKind, ToolKey},
    };
    let mut s = session();
    edge(&mut s);
    let view = ViewInfo::default();
    s.select_tool("dimension", view).unwrap();
    let before = s.doc().unwrap().doc.node_count();
    s.pointer(&PointerEvent::new(PointerKind::Move, 130.0, 100.0), view).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Down, 130.0, 100.0), view).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Move, 130.0, 60.0), view).unwrap();
    assert!(s.in_interaction());
    s.tool_key(ToolKey::Escape, Mods::default(), view).unwrap();
    assert_eq!(s.doc().unwrap().doc.node_count(), before);
    assert!(!s.in_interaction());
    s.pointer(&PointerEvent::new(PointerKind::Down, 130.0, 100.0), view).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Move, 130.0, 60.0), view).unwrap();
    s.pointer(&PointerEvent::new(PointerKind::Down, 130.0, 60.0), view).unwrap();
    assert!(!s.in_interaction());
    assert!(s.doc().unwrap().doc.node_count() > before);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(s.doc().unwrap().doc.node_count(), before);
}

#[test]
fn dimension_anchor_links_and_native_file_survive_source_moves() {
    let mut s = session();
    let first = edge(&mut s);
    let second = edge(&mut s);
    let id=run(&mut s,"dimension.create",json!({"a":[100,100],"b":[172,100],"source":{"kind":"points","a":{"node":first,"subpath":0,"anchor":0},"b":{"node":second,"subpath":0,"anchor":1},"vertex":null},"options":{"units":"pt","precision":0}}))["id"].as_u64().unwrap();
    assert_eq!(text(&s, id), "72 pt");
    let bytes = vectorcraft_format::save(&s.doc().unwrap().doc, false);
    let doc = vectorcraft_format::load(&bytes).unwrap();
    let mut s = Session::new();
    s.add_document(doc, None);
    run(&mut s, "select.set", json!({"ids":[second]}));
    run(&mut s, "object.move", json!({"dx":28,"dy":0}));
    assert_eq!(text(&s, id), "100 pt");
}

#[test]
fn dimension_options_are_persistent_and_invalid_updates_report_errors() {
    let mut s = session();
    s.set_tool_option_cmd(&json!({"tool":"dimension","values":{"style":{"scale":"10:1","units":"mm","precision":3},"mode":"diameter"}})).unwrap();
    assert_eq!(s.tool_options_of("dimension")["style"]["scale"], "10:1");
    assert!(s.set_tool_option_cmd(&json!({"tool":"dimension","values":{"style":{"scale":"1:0"}}})).is_err());
    let o: vectorcraft_doc::dimension::DimensionOptions = serde_json::from_value(s.tool_options_of("dimension")["style"].clone()).unwrap();
    assert_eq!(o.label(72.0, false).unwrap(), "2.540 mm");
    let prefs = serde_json::to_value(&s.prefs).unwrap();
    let mut fresh = Session::new();
    fresh.prefs = serde_json::from_value(prefs).unwrap();
    assert_eq!(fresh.tool_options_of("dimension")["style"], s.tool_options_of("dimension")["style"]);
}

#[test]
fn dimension_unlinked_geometry_scales_and_updates_its_label() {
    let mut s = session();
    let id = run(&mut s, "dimension.create", json!({"a":[100,100],"b":[172,100],"options":{"units":"in","precision":0}}))["id"].as_u64().unwrap();
    run(&mut s, "object.scale", json!({"sx":200,"sy":200}));
    assert_eq!(text(&s, id), "2 in");
}

#[test]
fn dimension_linked_angle_and_circle_update_and_reject_non_circles() {
    let mut s = session();
    let first = edge(&mut s);
    let second = run(&mut s, "shape.line", json!({"x1":100,"y1":100,"x2":100,"y2":172}))["id"].as_u64().unwrap();
    let id=run(&mut s,"dimension.create",json!({"mode":"angular","source":{"kind":"angle","first":{"node":first,"subpath":0,"segment":0},"second":{"node":second,"subpath":0,"segment":0}}}))["id"].as_u64().unwrap();
    assert_eq!(text(&s, id), "90.0°");
    run(&mut s, "select.set", json!({"ids":[second]}));
    run(&mut s, "object.rotate", json!({"angle":45,"origin":[100,100]}));
    assert!(["45.0°", "135.0°"].contains(&text(&s, id).as_str()));
    let circle = run(&mut s, "shape.ellipse", json!({"x":100,"y":100,"width":144,"height":144}))["id"].as_u64().unwrap();
    let id = run(
        &mut s,
        "dimension.create",
        json!({"mode":"diameter","source":{"kind":"circle","node":circle},"options":{"units":"in","precision":0}}),
    )["id"]
        .as_u64()
        .unwrap();
    run(&mut s, "select.set", json!({"ids":[circle]}));
    run(&mut s, "object.scale", json!({"sx":200,"sy":200}));
    assert_eq!(text(&s, id), "Ø 4 in");
    run(&mut s, "object.scale", json!({"sx":200,"sy":100}));
    assert!(s.doc().unwrap().doc.node(NodeId(id)).is_none());
    assert!(s.execute("dimension.create", &json!({"mode":"radius","source":{"kind":"circle","node":circle}})).is_err());
}

#[test]
fn dimension_style_update_keeps_links_and_can_be_undone() {
    let mut s = session();
    let line = edge(&mut s);
    let id = dim(&mut s, line);
    run(
        &mut s,
        "dimension.update",
        json!({"ids":[id],"offset":-42,"options":{"units":"mm","precision":2,"hideUnits":true,"textPosition":"below","lineColor":[10,20,30]}}),
    );
    assert_eq!(text(&s, id), "25.40");
    let d = s.doc().unwrap().doc.node(NodeId(id)).unwrap().extras().dimension.as_ref().unwrap();
    assert!(d.source.is_some());
    assert_eq!(d.offset, -42.0);
    run(&mut s, "edit.undo", json!({}));
    assert_eq!(text(&s, id), "10.00 in");
}

#[test]
fn dimension_cannot_measure_its_own_generated_art() {
    let mut s = session();
    let line = edge(&mut s);
    let id = dim(&mut s, line);
    let child = s.doc().unwrap().doc.node(NodeId(id)).unwrap().children().unwrap()[0].id;
    assert!(s.execute("dimension.create", &json!({"source":{"kind":"segment","edge":{"node":child,"subpath":0,"segment":0}}})).is_err());
}
