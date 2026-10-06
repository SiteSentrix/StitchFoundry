//! The Liquify tools: their options (Brush Affects, Simplify, Use Pressure Pen, Show Brush Size),
//! Alt-drag brush sizing, holding still, what they leave alone and incremental strokes.

use serde_json::{Value, json};
use vectorcraft_geom::PathData;
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

fn path(s: &Session, id: NodeId) -> PathData {
    s.doc().unwrap().doc.node(id).unwrap().path_data().unwrap().clone()
}

fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
    PointerEvent::new(kind, x, y)
}

/// Down at the first point, drags through the middle ones, up at the last.
fn stroke(s: &mut Session, pts: &[(f64, f64)], pressure: f32) {
    let v = ViewInfo::default();
    for (i, &(x, y)) in pts.iter().enumerate() {
        let kind = match i {
            0 => PointerKind::Down,
            _ if i + 1 == pts.len() => PointerKind::Up,
            _ => PointerKind::Drag,
        };
        s.pointer(&PointerEvent { pressure, ..ev(kind, x, y) }, v).unwrap();
    }
}

fn last_journal(s: &Session) -> (String, Value) {
    s.journal.last().unwrap().clone()
}

#[test]
fn brush_affects_reach_the_command() {
    let mut s = session();
    let a = rect(&mut s, 100.0, 100.0, 200.0, 200.0);
    s.select_tool("scallop", ViewInfo::default()).unwrap();
    s.set_tool_option("affectAnchors", &json!(false));
    s.set_tool_option("affectOut", &json!(false));
    stroke(&mut s, &[(300.0, 150.0), (300.0, 200.0), (300.0, 250.0)], 1.0);
    let (cmd, p) = last_journal(&s);
    assert_eq!((cmd.as_str(), &p["affectAnchors"], &p["affectIn"], &p["affectOut"]), ("object.liquify", &json!(false), &json!(true), &json!(false)));
    // Replaying it gives the same path, and the boxes change the result.
    let mut t = session();
    let b = rect(&mut t, 100.0, 100.0, 200.0, 200.0);
    t.execute("select.set", &json!({"ids": [b.0]})).unwrap();
    t.execute(&cmd, &p).unwrap();
    assert_eq!(path(&t, b), path(&s, a));
    let mut all = p.clone();
    for k in ["affectAnchors", "affectIn", "affectOut"] {
        all[k] = json!(true);
    }
    t.execute("edit.undo", &json!({})).unwrap();
    t.execute(&cmd, &all).unwrap();
    assert_ne!(path(&t, b), path(&s, a));
}

#[test]
fn simplify_can_be_turned_off() {
    let run = |on: bool| {
        let mut s = session();
        let a = rect(&mut s, 100.0, 100.0, 200.0, 200.0);
        // Pucker centred on an edge pulls its new anchors along the edge: they stay flat.
        let p = json!({"tool": "pucker", "points": [[300, 200]], "intensity": 1, "detail": 10, "simplify": 100, "simplifyOn": on});
        s.execute("object.liquify", &p).unwrap();
        path(&s, a).anchor_count()
    };
    assert!(run(false) > run(true), "{} vs {}", run(false), run(true));
}

#[test]
fn pen_pressure_is_the_intensity_with_use_pressure_pen() {
    let run = |pressures: [f64; 2], use_pressure: bool| {
        let mut s = session();
        let a = rect(&mut s, 100.0, 100.0, 200.0, 200.0);
        let pts = json!([[260, 150, pressures[0]], [260, 250, pressures[1]]]);
        s.execute("object.liquify", &json!({"tool": "pucker", "points": pts, "width": 120, "height": 120, "usePressure": use_pressure})).unwrap();
        path(&s, a)
    };
    let square = {
        let mut s = session();
        let a = rect(&mut s, 100.0, 100.0, 200.0, 200.0);
        path(&s, a)
    };
    // No pressure: nothing moves; more pressure moves more; pressure is ignored without the option.
    assert_eq!(run([0.0, 0.0], true), square);
    let x = |p: &PathData| p.anchors().map(|(_, _, an)| an.p.x).fold(f64::MAX, |m, v| if v > 200.0 { m.min(v) } else { m });
    assert!(x(&run([1.0, 1.0], true)) < x(&run([0.3, 0.3], true)));
    assert_eq!(run([0.0, 0.0], false), run([1.0, 1.0], false));

    // The tool sends each sample's pressure while the option is on.
    let mut s = session();
    rect(&mut s, 100.0, 100.0, 200.0, 200.0);
    s.select_tool("bloat", ViewInfo::default()).unwrap();
    stroke(&mut s, &[(300.0, 150.0), (300.0, 200.0), (300.0, 210.0)], 0.4);
    assert_eq!(last_journal(&s).1["points"][0].as_array().unwrap().len(), 2);
    s.set_tool_option("usePressure", &json!(true));
    stroke(&mut s, &[(300.0, 150.0), (300.0, 200.0), (300.0, 210.0)], 0.4);
    let (_, p) = last_journal(&s);
    assert_eq!((p["usePressure"].as_bool(), p["points"][1][2].as_f64().map(|f| (f * 1e6).round() / 1e6)), (Some(true), Some(0.4)));
}

#[test]
fn alt_drag_sizes_the_brush_from_its_size() {
    let mut s = session();
    let v = ViewInfo::default();
    s.select_tool("warp", v).unwrap();
    s.set_tool_option("height", &json!(60));
    let alt = Mods { alt: true, ..Mods::default() };
    let shift_alt = Mods { shift: true, ..alt };
    let size = |s: &Session| (s.tool_options()["width"].as_f64().unwrap(), s.tool_options()["height"].as_f64().unwrap());
    s.pointer(&ev(PointerKind::Down, 50.0, 50.0).with_mods(alt), v).unwrap();
    // The first sample barely moves: the brush stays about as it was (it used to collapse to 1 pt).
    s.pointer(&ev(PointerKind::Drag, 51.0, 50.0).with_mods(alt), v).unwrap();
    assert_eq!(size(&s), (102.0, 60.0));
    s.pointer(&ev(PointerKind::Drag, 60.0, 45.0).with_mods(alt), v).unwrap();
    assert_eq!(size(&s), (120.0, 50.0));
    s.pointer(&ev(PointerKind::Up, 60.0, 45.0).with_mods(alt), v).unwrap();
    assert_eq!(s.journal.iter().filter(|(c, _)| c == "object.liquify").count(), 0, "sizing doesn't liquify");
    // Shift keeps the proportions.
    s.pointer(&ev(PointerKind::Down, 0.0, 0.0).with_mods(shift_alt), v).unwrap();
    s.pointer(&ev(PointerKind::Drag, 60.0, 5.0).with_mods(shift_alt), v).unwrap();
    assert_eq!(size(&s), (240.0, 100.0));
    s.pointer(&ev(PointerKind::Up, 60.0, 5.0).with_mods(shift_alt), v).unwrap();
    // The size is kept: the other Liquify tools share it.
    s.select_tool("pucker", v).unwrap();
    assert_eq!(size(&s), (240.0, 100.0));
}

#[test]
fn show_brush_size_hides_the_outline() {
    let mut s = session();
    let v = ViewInfo::default();
    s.select_tool("twirl", v).unwrap();
    s.pointer(&ev(PointerKind::Move, 200.0, 200.0), v).unwrap();
    assert_eq!(s.overlays(v).len(), 1);
    s.set_tool_option("showBrush", &json!(false));
    assert!(s.overlays(v).is_empty());
    assert_eq!(s.tool_options()["showBrush"], json!(false));
    // Sizing the brush shows it anyway.
    s.pointer(&ev(PointerKind::Down, 200.0, 200.0).with_mods(Mods { alt: true, ..Mods::default() }), v).unwrap();
    assert_eq!(s.overlays(v).len(), 1);
}
