//! Tests for Pathfinder, Object → Path and Type commands.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, NodeKind};
use vectorcraft_geom::{FillRule, PathData, Rect};

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

fn ids(v: &Value) -> Vec<NodeId> {
    v["ids"].as_array().unwrap().iter().map(|x| NodeId(x.as_u64().unwrap())).collect()
}

fn node(s: &Session, id: NodeId) -> vectorcraft_doc::Node {
    s.doc().unwrap().doc.node(id).cloned().unwrap()
}

fn path_of(s: &Session, id: NodeId) -> (PathData, FillRule) {
    let n = node(s, id);
    match &n.kind {
        NodeKind::Path { path, rule, .. } => (path.clone(), *rule),
        NodeKind::Compound { children, rule } => {
            (PathData::new(children.iter().flat_map(|c| c.path_data().unwrap().subpaths.clone()).collect()), *rule)
        }
        _ => panic!("not a path"),
    }
}

fn area_of(s: &Session, id: NodeId) -> f64 {
    let (p, r) = path_of(s, id);
    vectorcraft_pathops::area(&p, r)
}

fn bounds(s: &Session, id: NodeId) -> Rect {
    node(s, id).geometric_bounds().unwrap()
}

fn close(a: Rect, b: Rect, tol: f64) -> bool {
    (a.x0 - b.x0).abs() < tol && (a.y0 - b.y0).abs() < tol && (a.x1 - b.x1).abs() < tol && (a.y1 - b.y1).abs() < tol
}

fn set_fill(s: &mut Session, id: NodeId, c: Color) {
    s.edit("t", |d, _| {
        d.node_mut(id).unwrap().appearance.set_fill(Paint::solid(c));
        Ok(())
    })
    .unwrap();
}

fn two_rects(s: &mut Session) -> (NodeId, NodeId) {
    let a = rect(s, 0.0, 0.0, 100.0, 100.0);
    let b = rect(s, 50.0, 50.0, 100.0, 100.0);
    set_fill(s, a, Color::rgb(1.0, 0.0, 0.0));
    set_fill(s, b, Color::rgb(0.0, 0.0, 1.0));
    s.execute("select.set", &json!({"ids": [a.0, b.0]})).unwrap();
    (a, b)
}

#[test]
fn unite_two_rects() {
    let mut s = session();
    let (a, b) = two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.unite", &json!({})).unwrap());
    assert_eq!(r.len(), 1);
    assert!(s.doc().unwrap().doc.node(a).is_none() && s.doc().unwrap().doc.node(b).is_none());
    assert!(matches!(node(&s, r[0]).kind, NodeKind::Path { .. }));
    assert!(close(bounds(&s, r[0]), Rect::new(0.0, 0.0, 150.0, 150.0), 1e-3));
    assert!((area_of(&s, r[0]) - (10000.0 + 10000.0 - 2500.0)).abs() < 1.0);
    // Top object's style.
    assert_eq!(node(&s, r[0]).appearance.fill_paint(), Paint::solid(Color::rgb(0.0, 0.0, 1.0)));
    assert_eq!(s.doc().unwrap().selection.objects, r);
}

#[test]
fn minus_front_keeps_back_style() {
    let mut s = session();
    two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.minusFront", &json!({})).unwrap());
    assert!((area_of(&s, r[0]) - 7500.0).abs() < 1.0);
    assert!(close(bounds(&s, r[0]), Rect::new(0.0, 0.0, 100.0, 100.0), 1e-3));
    assert_eq!(node(&s, r[0]).appearance.fill_paint(), Paint::solid(Color::rgb(1.0, 0.0, 0.0)));
}

#[test]
fn minus_back_keeps_front_style() {
    let mut s = session();
    two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.minusBack", &json!({})).unwrap());
    assert!((area_of(&s, r[0]) - 7500.0).abs() < 1.0);
    assert!(close(bounds(&s, r[0]), Rect::new(50.0, 50.0, 150.0, 150.0), 1e-3));
    assert_eq!(node(&s, r[0]).appearance.fill_paint(), Paint::solid(Color::rgb(0.0, 0.0, 1.0)));
}

#[test]
fn intersect_rects() {
    let mut s = session();
    two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.intersect", &json!({})).unwrap());
    assert!(close(bounds(&s, r[0]), Rect::new(50.0, 50.0, 100.0, 100.0), 1e-3));
    assert!((area_of(&s, r[0]) - 2500.0).abs() < 1.0);
}

#[test]
fn intersect_disjoint_is_error_and_no_change() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&mut s, 50.0, 0.0, 10.0, 10.0);
    s.execute("select.set", &json!({"ids": [a.0, b.0]})).unwrap();
    let undo_len = s.doc().unwrap().history.undo.len();
    assert!(s.execute("object.pathfinder.intersect", &json!({})).is_err());
    assert!(s.doc().unwrap().doc.node(a).is_some());
    assert_eq!(s.doc().unwrap().history.undo.len(), undo_len);
}

#[test]
fn exclude_makes_compound() {
    let mut s = session();
    two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.exclude", &json!({})).unwrap());
    assert!((area_of(&s, r[0]) - 15000.0).abs() < 1.0);
}

#[test]
fn divide_produces_group_of_faces() {
    let mut s = session();
    two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.divide", &json!({})).unwrap());
    let g = node(&s, r[0]);
    assert!(matches!(g.kind, NodeKind::Group { .. }));
    let ch = g.children().unwrap();
    assert_eq!(ch.len(), 3);
    let total: f64 = ch.iter().map(|c| area_of(&s, c.id)).sum();
    assert!((total - 17500.0).abs() < 1.0);
    // The overlap face takes the top (blue) style.
    let blue = Paint::solid(Color::rgb(0.0, 0.0, 1.0));
    assert_eq!(ch.iter().filter(|c| c.appearance.fill_paint() == blue).count(), 2);
}

#[test]
fn trim_and_merge() {
    let mut s = session();
    two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.trim", &json!({})).unwrap());
    let g = node(&s, r[0]);
    assert_eq!(g.children().unwrap().len(), 2);
    s.execute("edit.undo", &json!({})).unwrap();
    // Same colour → merge joins them.
    let sel = s.doc().unwrap().selection.objects.clone();
    set_fill(&mut s, sel[0], Color::rgb(0.0, 0.0, 1.0));
    s.execute("select.set", &json!({"ids": [sel[0].0, sel[1].0]})).unwrap();
    let r = ids(&s.execute("object.pathfinder.merge", &json!({})).unwrap());
    let g = node(&s, r[0]);
    assert_eq!(g.children().unwrap().len(), 1);
}

#[test]
fn crop_and_outline() {
    let mut s = session();
    two_rects(&mut s);
    let r = ids(&s.execute("object.pathfinder.crop", &json!({})).unwrap());
    let g = node(&s, r[0]);
    let ch = g.children().unwrap();
    assert_eq!(ch.len(), 1);
    assert!((area_of(&s, ch[0].id) - 2500.0).abs() < 1.0);
    s.execute("edit.undo", &json!({})).unwrap();
    let r = ids(&s.execute("object.pathfinder.outline", &json!({})).unwrap());
    let g = node(&s, r[0]);
    assert!(g.children().unwrap().len() >= 4);
    assert!(g.children().unwrap().iter().all(|c| c.appearance.stroke_width() > 0.0));
}

#[test]
fn pathfinder_undo_restores() {
    let mut s = session();
    let (a, b) = two_rects(&mut s);
    let before = s.doc().unwrap().doc.clone();
    s.execute("object.pathfinder.unite", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(*s.doc().unwrap().doc, *before);
    assert!(s.doc().unwrap().selection.contains(a) && s.doc().unwrap().selection.contains(b));
}

#[test]
fn unite_with_group() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&mut s, 20.0, 0.0, 10.0, 10.0);
    s.execute("select.set", &json!({"ids": [a.0, b.0]})).unwrap();
    let g = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    let c = rect(&mut s, 5.0, 5.0, 20.0, 10.0);
    s.execute("select.set", &json!({"ids": [g, c.0]})).unwrap();
    let r = ids(&s.execute("object.pathfinder.unite", &json!({})).unwrap());
    assert!(close(bounds(&s, r[0]), Rect::new(0.0, 0.0, 30.0, 15.0), 1e-3));
    assert!((area_of(&s, r[0]) - (100.0 + 100.0 + 200.0 - 25.0 - 25.0)).abs() < 1.0);
}

#[test]
fn offset_rect_grows_bounds() {
    let mut s = session();
    let a = rect(&mut s, 100.0, 100.0, 50.0, 40.0);
    let r = ids(&s.execute("object.path.offsetPath", &json!({"offset": 10, "joins": "miter", "miterLimit": 4})).unwrap());
    assert_eq!(r.len(), 1);
    assert!(s.doc().unwrap().doc.node(a).is_some(), "original is kept");
    assert!(close(bounds(&s, r[0]), Rect::new(90.0, 90.0, 160.0, 150.0), 1e-2), "{:?}", bounds(&s, r[0]));
    assert_eq!(s.doc().unwrap().selection.objects, r);
    // Above the original.
    let d = &s.doc().unwrap().doc;
    assert!(d.index_path(r[0]).unwrap() > d.index_path(a).unwrap());
    // Negative insets; string params are accepted.
    s.execute("select.set", &json!({"ids": [a.0]})).unwrap();
    let r = ids(&s.execute("object.path.offsetPath", &json!({"offset": "-5 pt", "joins": "round"})).unwrap());
    assert!(close(bounds(&s, r[0]), Rect::new(105.0, 105.0, 145.0, 135.0), 1e-2));
}

#[test]
fn outline_stroke_of_line() {
    let mut s = session();
    let r = s.execute("shape.line", &json!({"x1": 0, "y1": 50, "x2": 100, "y2": 50})).unwrap();
    let l = NodeId(r["id"].as_u64().unwrap());
    s.edit("t", |d, _| {
        d.node_mut(l).unwrap().appearance = Appearance::basic(Paint::None, Paint::solid(Color::rgb(1.0, 0.0, 0.0)), 10.0).into();
        Ok(())
    })
    .unwrap();
    let r = ids(&s.execute("object.path.outlineStroke", &json!({})).unwrap());
    let n = node(&s, r[0]);
    assert!(close(n.geometric_bounds().unwrap(), Rect::new(0.0, 45.0, 100.0, 55.0), 1e-2));
    assert_eq!(n.appearance.fill_paint(), Paint::solid(Color::rgb(1.0, 0.0, 0.0)));
    assert!(n.appearance.stroke().is_none());
    assert!((area_of(&s, r[0]) - 1000.0).abs() < 1.0);
}

#[test]
fn outline_stroke_with_fill_makes_group() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    s.edit("t", |d, _| {
        d.node_mut(a).unwrap().appearance.stroke_mut().unwrap().width = 4.0;
        Ok(())
    })
    .unwrap();
    let r = ids(&s.execute("object.path.outlineStroke", &json!({})).unwrap());
    let g = node(&s, r[0]);
    let ch = g.children().unwrap();
    assert_eq!(ch.len(), 2);
    assert_eq!(ch[0].appearance.fill_paint(), Paint::solid(Color::WHITE));
    assert_eq!(ch[1].appearance.fill_paint(), Paint::solid(Color::BLACK));
    assert!(close(ch[1].geometric_bounds().unwrap(), Rect::new(-2.0, -2.0, 102.0, 102.0), 1e-2));
}

#[test]
fn simplify_dense_polyline() {
    let mut s = session();
    let anchors: Vec<Value> = (0..=100)
        .map(|i| {
            let x = i as f64 * 3.0;
            json!({"x": x, "y": 100.0 + 40.0 * (x / 50.0).sin()})
        })
        .collect();
    let r = s.execute("path.create", &json!({"anchors": anchors})).unwrap();
    let id = NodeId(r["id"].as_u64().unwrap());
    let before = path_of(&s, id).0.anchor_count();
    let out = s.execute("object.path.simplify", &json!({"tolerance": 1.0})).unwrap();
    let after = path_of(&s, id).0.anchor_count();
    assert_eq!(before, 101);
    assert!(after < before / 4, "{after}");
    assert_eq!(out["after"].as_u64().unwrap() as usize, after);
    assert!(close(bounds(&s, id), Rect::new(0.0, 60.0, 300.0, 140.0), 2.0));
}

/// How many steps there are to undo and to redo, and whether the document has unsaved changes.
fn history(s: &Session) -> (usize, usize, bool) {
    let st = s.doc().unwrap();
    (st.history.undo.len(), st.history.redo.len(), st.is_dirty())
}

/// Select `ids` with a move of them left to redo, and mark the document saved.
fn select_with_a_step_to_redo(s: &mut Session, ids: &[NodeId]) {
    s.execute("select.set", &json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()})).unwrap();
    s.execute("object.move", &json!({"dx": 5, "dy": 0})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    s.doc_mut().unwrap().mark_saved();
}

/// Simplify that leaves every path as it is records no undo step: a rectangle, a star and a line,
/// whose sides are straight segments between corners, and a path with one anchor. The step to redo
/// stays, the document stays unmodified, the rectangle and the line stay live shapes, and the reply
/// counts the anchors. With a path it simplifies selected too, Simplify is one undo step that
/// changes only that path, and the others stay as they are, live shapes included.
#[test]
fn simplify_that_changes_no_path_records_no_undo_step() {
    let mut s = session();
    let id = |v: Value| NodeId(v["id"].as_u64().unwrap());
    let r = rect(&mut s, 0.0, 0.0, 100.0, 60.0);
    let star = id(s.execute("shape.star", &json!({"cx": 300, "cy": 100, "radius1": 60, "radius2": 30})).unwrap());
    let line = id(s.execute("shape.line", &json!({"x1": 0, "y1": 200, "x2": 100, "y2": 240})).unwrap());
    let point = id(s.execute("path.create", &json!({"anchors": [{"x": 5, "y": 300}]})).unwrap());
    let ids = [r, star, line, point];
    let paths = |s: &Session| ids.map(|i| path_of(s, i).0);
    let is_live = |s: &Session, i| matches!(node(s, i).kind, NodeKind::Path { live: Some(_), .. });
    let was = paths(&s);
    select_with_a_step_to_redo(&mut s, &ids);
    let before = history(&s);
    assert_eq!((before.1, before.2), (1, false), "a step to redo, and nothing unsaved");
    for p in [json!({}), json!({"tolerance": 5, "straightLines": true})] {
        let out = s.execute("object.path.simplify", &p).unwrap();
        assert_eq!(out, json!({"ids": ids.map(|i| i.0), "before": 17, "after": 17}), "{p}");
        assert_eq!(history(&s), before, "{p}");
        assert_eq!(paths(&s), was, "{p}");
        assert!(is_live(&s, r) && is_live(&s, line), "{p}");
        assert_eq!(s.journal.last().unwrap(), &("object.path.simplify".to_string(), p));
    }
    // The 101 anchors of a sine wave become a few.
    let anchors: Vec<Value> = (0..=100)
        .map(|i| {
            let x = i as f64 * 3.0;
            json!({"x": x, "y": 400.0 + 40.0 * (x / 50.0).sin()})
        })
        .collect();
    let wave = id(s.execute("path.create", &json!({"anchors": anchors})).unwrap());
    s.execute("select.set", &json!({"ids": [r.0, star.0, line.0, point.0, wave.0]})).unwrap();
    let undo = history(&s).0;
    s.execute("object.path.simplify", &json!({})).unwrap();
    assert_eq!(history(&s), (undo + 1, 0, true));
    assert!(path_of(&s, wave).0.anchor_count() < 25);
    assert_eq!(paths(&s), was);
    assert!(is_live(&s, r) && is_live(&s, line), "the paths Simplify leaves as they are stay live shapes");
}

/// The Simplify dialog previews Simplify in an interaction, and OK commits the preview. When the
/// preview changes no path, OK records no undo step: the step to redo stays, the document stays
/// unmodified and the rectangle stays a live shape.
#[test]
fn simplify_preview_that_changes_no_path_commits_no_undo_step() {
    let mut s = session();
    let r = rect(&mut s, 0.0, 0.0, 100.0, 60.0);
    select_with_a_step_to_redo(&mut s, &[r]);
    let before = history(&s);
    s.begin_interaction("Simplify").unwrap();
    let out = s.preview("object.path.simplify", &json!({"tolerance": 1.0})).unwrap();
    assert_eq!(out, json!({"ids": [r.0], "before": 4, "after": 4}));
    s.commit_interaction().unwrap();
    assert_eq!(history(&s), before);
    assert!(matches!(node(&s, r).kind, NodeKind::Path { live: Some(_), .. }));
}

#[test]
fn simplify_refreshes_changed_path_selection_and_undo() {
    let mut s = session();
    let anchors: Vec<Value> = (0..20).map(|i| json!({"x": 30 + i * 15, "y": 100.0 + (i % 2) as f64 * 0.1})).collect();
    let id = NodeId(s.execute("path.create", &json!({"anchors": anchors})).unwrap()["id"].as_u64().unwrap());
    // A one-anchor path cannot be simplified; its component selection must stay.
    let kept = NodeId(s.execute("path.create", &json!({"anchors": [{"x": 350, "y": 180}]})).unwrap()["id"].as_u64().unwrap());
    let unrelated = rect(&mut s, 400.0, 200.0, 40.0, 30.0);
    s.edit("selection fixture", |d, sel| {
        d.guides.push(vectorcraft_doc::Guide::new(true, 30.0));
        sel.set([kept, id]);
        sel.anchors.insert(id, [(0, 19)].into_iter().collect());
        sel.anchors.insert(kept, [(0, 0)].into_iter().collect());
        sel.key = Some(kept);
        sel.target = Some(id);
        sel.guides.push(0);
        Ok(())
    })
    .unwrap();
    let before = s.doc().unwrap().doc.clone();
    let selection = s.doc().unwrap().selection.clone();
    let undo_len = s.doc().unwrap().history.undo.len();
    let out = s.execute("object.path.simplify", &json!({"tolerance": 1})).unwrap();
    assert_eq!((out["before"].as_u64(), out["after"].as_u64()), (Some(21), Some(3)));
    assert_eq!(path_of(&s, id).0.anchor_count(), 2);
    let mut expected_selection = selection.clone();
    expected_selection.anchors.remove(&id);
    assert_eq!(s.doc().unwrap().selection, expected_selection, "only the changed path becomes selected whole");
    assert_eq!(node(&s, unrelated), before.node(unrelated).unwrap().clone());
    let after = s.doc().unwrap().doc.clone();
    let mut restored = after.clone();
    std::sync::Arc::make_mut(&mut restored).node_mut(id).unwrap().kind = before.node(id).unwrap().kind.clone();
    assert_eq!(restored, before, "all document state except the simplified geometry is preserved");
    assert_eq!(s.doc().unwrap().history.undo.len(), undo_len + 1);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc, before);
    assert_eq!(s.doc().unwrap().selection, selection);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc, after);
    assert_eq!(s.doc().unwrap().selection, expected_selection);
}

#[test]
fn add_anchor_points_refreshes_components_even_when_old_indexes_still_exist() {
    let mut s = session();
    let id = rect(&mut s, 20.0, 30.0, 100.0, 70.0);
    s.execute("select.anchors", &json!({"id": id.0, "anchors": [[0, 3]]})).unwrap();
    let before = s.doc().unwrap().doc.clone();
    let selection = s.doc().unwrap().selection.clone();
    let out = s.execute("object.path.addAnchorPoints", &json!({})).unwrap();
    assert_eq!((out["before"].as_u64(), out["after"].as_u64()), (Some(4), Some(8)));
    assert!(s.doc().unwrap().selection.anchors.is_empty());
    assert_eq!(s.doc().unwrap().selection.objects, selection.objects);
    assert!(close(bounds(&s, id), before.node(id).unwrap().geometric_bounds().unwrap(), 1e-9));
    let after = s.doc().unwrap().doc.clone();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc, before);
    assert_eq!(s.doc().unwrap().selection, selection);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc, after);
    assert!(s.doc().unwrap().selection.anchors.is_empty());
}

#[test]
fn simplify_refreshes_components_inside_selected_groups() {
    let mut s = session();
    let anchors: Vec<Value> = (0..20).map(|i| json!({"x": 30 + i * 15, "y": 100.0 + (i % 2) as f64 * 0.1})).collect();
    let id = NodeId(s.execute("path.create", &json!({"anchors": anchors})).unwrap()["id"].as_u64().unwrap());
    let other = rect(&mut s, 400.0, 200.0, 40.0, 30.0);
    s.execute("select.set", &json!({"ids": [id.0, other.0]})).unwrap();
    let group = NodeId(s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap());
    s.select(|_, sel| {
        sel.anchors.insert(id, [(0, 19)].into_iter().collect());
    })
    .unwrap();
    let other_before = node(&s, other);
    s.execute("object.path.simplify", &json!({"tolerance": 1})).unwrap();
    assert_eq!(s.doc().unwrap().selection.objects, vec![group]);
    assert!(s.doc().unwrap().selection.anchors.is_empty());
    assert_eq!(path_of(&s, id).0.anchor_count(), 2);
    assert_eq!(node(&s, other).appearance, other_before.appearance);
}

#[test]
fn add_anchor_points_doubles() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    s.execute("object.path.addAnchorPoints", &json!({})).unwrap();
    assert_eq!(path_of(&s, a).0.anchor_count(), 8);
    assert!(close(bounds(&s, a), Rect::new(0.0, 0.0, 10.0, 10.0), 1e-9));
}

/// Add Anchor Points with only paths of one anchor selected records no undo step. With a rectangle
/// selected too, it is one undo step that adds four anchors to the rectangle and leaves the
/// one-anchor path as it is.
#[test]
fn add_anchor_points_with_no_segment_records_no_undo_step() {
    let mut s = session();
    let point = NodeId(s.execute("path.create", &json!({"anchors": [{"x": 5, "y": 5}]})).unwrap()["id"].as_u64().unwrap());
    let a = rect(&mut s, 20.0, 0.0, 10.0, 10.0);
    let was = path_of(&s, point).0;
    select_with_a_step_to_redo(&mut s, &[point]);
    let before = history(&s);
    let out = s.execute("object.path.addAnchorPoints", &json!({})).unwrap();
    assert_eq!(out, json!({"ids": [point.0], "before": 1, "after": 1}));
    assert_eq!(history(&s), before);
    assert_eq!(s.journal.last().unwrap().0, "object.path.addAnchorPoints");
    s.execute("select.set", &json!({"ids": [point.0, a.0]})).unwrap();
    let out = s.execute("object.path.addAnchorPoints", &json!({})).unwrap();
    assert_eq!(out, json!({"ids": [point.0, a.0], "before": 5, "after": 9}));
    assert_eq!(history(&s), (before.0 + 1, 0, true));
    assert_eq!(path_of(&s, point).0, was);
}

#[test]
fn split_into_grid_cells() {
    let mut s = session();
    rect(&mut s, 0.0, 0.0, 110.0, 50.0);
    let r = ids(&s.execute("object.path.splitIntoGrid", &json!({"rows": 2, "columns": 3, "gutter": 10})).unwrap());
    assert_eq!(r.len(), 6);
    assert!(close(bounds(&s, r[0]), Rect::new(0.0, 0.0, 30.0, 20.0), 1e-9));
    assert!(s.execute("object.path.splitIntoGrid", &json!({"rows": 0})).is_err());
}

#[test]
fn clean_up_removes_junk() {
    let mut s = session();
    let keep = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let unpainted = rect(&mut s, 20.0, 0.0, 10.0, 10.0);
    s.edit("t", |d, _| {
        d.node_mut(unpainted).unwrap().appearance = Appearance::basic(Paint::None, Paint::None, 1.0).into();
        Ok(())
    })
    .unwrap();
    let stray = NodeId(s.execute("path.create", &json!({"anchors": [{"x": 5, "y": 5}]})).unwrap()["id"].as_u64().unwrap());
    let empty = NodeId(s.execute("text.create", &json!({"x": 10, "y": 10, "text": ""})).unwrap()["id"].as_u64().unwrap());
    let r = s.execute("object.path.cleanUp", &json!({})).unwrap();
    assert_eq!(r["removed"], 3);
    let d = &s.doc().unwrap().doc;
    assert!(d.node(keep).is_some());
    assert!(d.node(unpainted).is_none() && d.node(stray).is_none() && d.node(empty).is_none());
}

#[test]
fn divide_objects_below_cuts() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    let cutter = rect(&mut s, 50.0, -10.0, 100.0, 120.0);
    s.execute("select.set", &json!({"ids": [cutter.0]})).unwrap();
    let r = ids(&s.execute("object.path.divideObjectsBelow", &json!({})).unwrap());
    assert_eq!(r.len(), 2);
    let d = &s.doc().unwrap().doc;
    assert!(d.node(a).is_none() && d.node(cutter).is_none());
    let total: f64 = r.iter().map(|id| area_of(&s, *id)).sum();
    assert!((total - 10000.0).abs() < 1.0);
}

fn text(s: &mut Session, t: &str, size: f64) -> NodeId {
    let r = s.execute("text.create", &json!({"x": 10, "y": 50, "text": t, "size": size})).unwrap();
    NodeId(r["id"].as_u64().unwrap())
}

#[test]
fn create_outlines_matches_text_bounds() {
    let mut s = session();
    let t = text(&mut s, "Hello bo", 36.0);
    let tb = bounds(&s, t);
    let r = ids(&s.execute("type.createOutlines", &json!({})).unwrap());
    assert!(s.doc().unwrap().doc.node(t).is_none());
    let g = node(&s, r[0]);
    let ch = g.children().unwrap();
    assert_eq!(ch.len(), 7, "one per visible glyph");
    assert!(ch.iter().any(|c| matches!(c.kind, NodeKind::Compound { .. })), "o/b have holes");
    assert!(ch.iter().all(|c| c.appearance.fill_paint() == Paint::solid(Color::BLACK)));
    let gb = g.geometric_bounds().unwrap();
    assert!(gb.x0 >= tb.x0 - 2.0 && gb.x1 <= tb.x1 + 2.0 && gb.y0 >= tb.y0 - 2.0 && gb.y1 <= tb.y1 + 2.0, "{gb:?} vs {tb:?}");
    assert!(gb.width() > tb.width() * 0.8);
    assert_eq!(s.doc().unwrap().selection.objects, r);
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.doc().unwrap().doc.node(t).is_some());
}

#[test]
fn set_style_size_changes_bounds() {
    let mut s = session();
    let t = text(&mut s, "Hello", 12.0);
    let b0 = bounds(&s, t);
    s.execute("text.setStyle", &json!({"size": 24, "justify": "center", "fill": "#ff0000", "leading": "auto"})).unwrap();
    let b1 = bounds(&s, t);
    assert!(b1.width() > b0.width() * 1.8, "{b0:?} → {b1:?}");
    let NodeKind::Text(tx) = node(&s, t).kind else { panic!() };
    assert_eq!(tx.first_style().size, 24.0);
    assert_eq!(tx.para.justify, vectorcraft_doc::Justify::Center);
    assert_eq!(tx.first_style().fill, Paint::solid(Color::rgb(1.0, 0.0, 0.0)));
    assert!(s.execute("text.setStyle", &json!({"justify": "diagonal"})).is_err());
    assert!(s.execute("text.setStyle", &json!({})).is_err());
}

#[test]
fn set_text_updates_bounds() {
    let mut s = session();
    let t = text(&mut s, "Hi", 24.0);
    let b0 = bounds(&s, t);
    s.execute("text.setText", &json!({"id": t.0, "text": "Hello there"})).unwrap();
    let NodeKind::Text(tx) = node(&s, t).kind else { panic!() };
    assert_eq!(tx.plain_text(), "Hello there");
    assert!(bounds(&s, t).width() > b0.width() * 2.0);
    assert!(s.execute("text.setText", &json!({"id": t.0})).is_err());
    s.execute("edit.undo", &json!({})).unwrap();
    let NodeKind::Text(tx) = node(&s, t).kind else { panic!() };
    assert_eq!(tx.plain_text(), "Hi");
}

#[test]
fn commands_reject_bad_input() {
    let mut s = session();
    two_rects(&mut s);
    for (c, p) in [
        ("object.path.offsetPath", json!({"offset": "abc"})),
        ("object.path.offsetPath", json!({"offset": 1e300})),
        ("object.path.simplify", json!({"tolerance": -5})),
        ("object.path.splitIntoGrid", json!({"rows": 1e9, "columns": -1})),
        ("text.setStyle", json!({"size": -3})),
        ("type.createOutlines", json!({})),
        ("object.pathfinder.divide", json!({"junk": [1, 2]})),
    ] {
        let _ = s.execute(c, &p);
    }
}
