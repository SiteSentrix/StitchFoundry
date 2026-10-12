//! Commands that remove, insert or copy many objects, and an Alt-drag that copies them, each finish
//! within 20 s on one layer of 25,000 to 100,000 objects, and objects in two layers and inside a
//! group keep their order and ids.

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use vectorcraft_doc::{Appearance, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{FillRule, Rect, shapes};
use vectorcraft_tools::{Mods, PointerEvent, PointerKind};

use super::*;

/// Squares in a row of [`square_at`].
const ROW: usize = 200;

/// Square `k`: 10 pt wide, in rows of 200 squares 20 pt apart.
fn square_at(k: usize) -> Rect {
    let (x, y) = ((k % ROW) as f64 * 20.0, (k / ROW) as f64 * 20.0);
    Rect::new(x, y, x + 10.0, y + 10.0)
}

/// Square `k` ([`square_at`]) as a path with a new id.
fn square(d: &mut Document, k: usize) -> Node {
    Node::path(d.alloc_id(), shapes::rectangle(square_at(k)), Appearance::default_art())
}

/// A session whose document holds `make(d, k)` for each `k` below `n` on its one layer, all of it
/// selected.
fn session(n: usize, mut make: impl FnMut(&mut Document, usize) -> Node) -> Session {
    let mut d = Document::new(ROW as f64 * 20.0, (n / ROW + 1) as f64 * 20.0);
    let art: Vec<Arc<Node>> = (0..n).map(|k| Arc::new(make(&mut d, k))).collect();
    if let Some(ch) = Arc::make_mut(&mut d.layers[0]).children_mut() {
        *ch = art;
    }
    let mut s = Session::new();
    s.add_document(d, None);
    s.execute("select.all", &json!({})).unwrap();
    s
}

/// The ids of `n`'s children.
fn ids_of(n: &Node) -> Vec<NodeId> {
    n.children().into_iter().flatten().map(|c| c.id).collect()
}

/// The ids of the objects on layer `i`.
fn layer(s: &Session, i: usize) -> Vec<NodeId> {
    ids_of(&s.doc().unwrap().doc.layers[i])
}

/// The ids of the children of `id`.
fn children(s: &Session, id: NodeId) -> Vec<NodeId> {
    ids_of(s.doc().unwrap().doc.node(id).unwrap())
}

/// `n` ids counting up from `first`.
fn ids_from(first: u64, n: usize) -> Vec<NodeId> {
    (first..).take(n).map(NodeId).collect()
}

/// The ids of `a` and `b` in turn: each id of `a` followed by the id of `b` at the same place.
fn in_turn(a: &[NodeId], b: &[NodeId]) -> Vec<NodeId> {
    a.iter().zip(b).flat_map(|(a, b)| [*a, *b]).collect()
}

/// Runs `f` on `s` on a thread of its own and fails the test when it takes more than 20 s.
fn soon<T: Send + 'static>(mut s: Session, what: &'static str, f: impl FnOnce(&mut Session) -> T + Send + 'static) -> (Session, T) {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let r = f(&mut s);
        tx.send((s, r)).ok();
    });
    rx.recv_timeout(Duration::from_secs(20)).unwrap_or_else(|_| panic!("{what} did not finish within 20 s"))
}

/// Runs command `cmd` with `params` through [`soon`] and returns its result.
fn run_soon(s: Session, cmd: &'static str, params: Value) -> (Session, Value) {
    let (s, r) = soon(s, cmd, move |s| s.execute(cmd, &params));
    (s, r.unwrap())
}

/// The ids in the `ids` array of a command's result.
fn ids_in(r: &Value) -> Vec<NodeId> {
    r["ids"].as_array().unwrap().iter().map(|v| NodeId(v.as_u64().unwrap())).collect()
}

/// Runs Undo and checks that it gives back `before`.
fn undo_gives_back(s: &mut Session, before: &Document) {
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(*s.doc().unwrap().doc == *before, "Undo gives back the document");
}

/// Whether the selected objects are `ids`, in that order.
fn selected(s: &Session, ids: &[NodeId]) -> bool {
    s.doc().unwrap().selection.objects == ids
}

/// The geometric bounds of object `id`.
fn bounds(s: &Session, id: NodeId) -> Rect {
    s.doc().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap()
}

#[test]
fn delete_of_100000_objects_finishes_within_20_s() {
    let s = session(100_000, square);
    let before = s.doc().unwrap().doc.clone();
    let (mut s, _) = run_soon(s, "edit.clear", json!({}));
    assert!(layer(&s, 0).is_empty());
    assert!(selected(&s, &[]));
    undo_gives_back(&mut s, &before);
}

#[test]
fn group_of_50000_objects_finishes_within_20_s() {
    let s = session(50_000, square);
    let (before, ids) = (s.doc().unwrap().doc.clone(), layer(&s, 0));
    let (mut s, r) = run_soon(s, "object.group", json!({}));
    let g = NodeId(r["id"].as_u64().unwrap());
    assert_eq!(layer(&s, 0), [g]);
    assert!(children(&s, g) == ids, "the group holds the objects in their order");
    assert!(selected(&s, &[g]));
    undo_gives_back(&mut s, &before);
}

#[test]
fn clipping_mask_of_50000_objects_finishes_within_20_s() {
    let s = session(50_000, square);
    let (before, ids) = (s.doc().unwrap().doc.clone(), layer(&s, 0));
    let (mut s, r) = run_soon(s, "object.clippingMask.make", json!({}));
    let g = NodeId(r["id"].as_u64().unwrap());
    assert_eq!(layer(&s, 0), [g]);
    // The topmost object comes first, as the clipping path, and the others follow in their order.
    let mut want = ids.clone();
    want.rotate_right(1);
    assert!(children(&s, g) == want, "the clip group holds the objects in their order");
    let doc = &s.doc().unwrap().doc;
    assert!(matches!(doc.node(g).unwrap().kind, NodeKind::Group { clip: true, .. }));
    assert!(matches!(doc.node(want[0]).unwrap().kind, NodeKind::Path { clipping: true, .. }));
    assert!(selected(&s, &[g]));
    undo_gives_back(&mut s, &before);
}

#[test]
fn duplicate_of_50000_objects_finishes_within_20_s() {
    let s = session(50_000, square);
    let (before, ids, first) = (s.doc().unwrap().doc.clone(), layer(&s, 0), s.doc().unwrap().doc.peek_next_id());
    let (mut s, r) = run_soon(s, "edit.duplicate", json!({"dx": 5}));
    // Each copy goes directly above its object and takes the next id, in the objects' order.
    let copies = ids_from(first, ids.len());
    assert!(ids_in(&r) == copies);
    assert!(layer(&s, 0) == in_turn(&ids, &copies), "each copy directly above its object");
    assert!(selected(&s, &copies));
    assert_eq!(bounds(&s, copies[0]), square_at(0) + vectorcraft_geom::Vec2::new(5.0, 0.0));
    undo_gives_back(&mut s, &before);
}

#[test]
fn transform_with_copy_of_50000_objects_finishes_within_20_s() {
    let s = session(50_000, square);
    let (before, ids, first) = (s.doc().unwrap().doc.clone(), layer(&s, 0), s.doc().unwrap().doc.peek_next_id());
    let (mut s, r) = run_soon(s, "object.transform", json!({"matrix": [1, 0, 0, 1, 5, 0], "copy": true}));
    let copies = ids_from(first, ids.len());
    assert!(ids_in(&r) == copies);
    assert!(layer(&s, 0) == in_turn(&ids, &copies), "each copy directly above its object");
    assert!(selected(&s, &copies));
    let last = ids.len() - 1;
    assert_eq!(bounds(&s, copies[last]), square_at(last) + vectorcraft_geom::Vec2::new(5.0, 0.0));
    assert_eq!(bounds(&s, ids[last]), square_at(last));
    undo_gives_back(&mut s, &before);
}

#[test]
fn transform_each_with_copy_of_50000_objects_finishes_within_20_s() {
    let s = session(50_000, square);
    let (before, ids, first) = (s.doc().unwrap().doc.clone(), layer(&s, 0), s.doc().unwrap().doc.peek_next_id());
    let (mut s, r) = run_soon(s, "object.transformEach", json!({"moveH": 5, "copy": true}));
    let copies = ids_from(first, ids.len());
    assert!(ids_in(&r) == copies);
    assert!(layer(&s, 0) == in_turn(&ids, &copies), "each copy directly above its object");
    assert!(selected(&s, &copies));
    assert_eq!(bounds(&s, copies[1]), square_at(1) + vectorcraft_geom::Vec2::new(5.0, 0.0));
    undo_gives_back(&mut s, &before);
}

#[test]
fn compound_path_of_100000_objects_finishes_within_20_s() {
    let s = session(100_000, square);
    let (before, ids) = (s.doc().unwrap().doc.clone(), layer(&s, 0));
    let (mut s, r) = run_soon(s, "object.compoundPath.make", json!({}));
    let c = NodeId(r["id"].as_u64().unwrap());
    assert_eq!(layer(&s, 0), [c]);
    assert!(children(&s, c) == ids, "the compound path holds the paths in their order");
    assert!(selected(&s, &[c]));
    undo_gives_back(&mut s, &before);
}

#[test]
fn send_to_current_layer_of_100000_objects_finishes_within_20_s() {
    let mut s = session(100_000, square);
    let ids = layer(&s, 0);
    let to = NodeId(s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap());
    s.execute("select.all", &json!({})).unwrap();
    s.execute("layer.setCurrent", &json!({"id": to.0})).unwrap();
    let before = s.doc().unwrap().doc.clone();
    let (mut s, _) = run_soon(s, "object.arrange.sendToCurrentLayer", json!({}));
    assert!(layer(&s, 0).is_empty());
    assert!(children(&s, to) == ids, "the layer holds the objects in their order");
    assert!(selected(&s, &ids));
    undo_gives_back(&mut s, &before);
}

#[test]
fn ungroup_of_25000_groups_finishes_within_20_s() {
    // Each group holds two squares, and ungrouping one adds an object to the layer.
    let s = session(25_000, |d, k| {
        let pair = [square(d, 2 * k), square(d, 2 * k + 1)].map(Arc::new);
        Node::group(d.alloc_id(), pair.to_vec())
    });
    let before = s.doc().unwrap().doc.clone();
    let squares: Vec<NodeId> = layer(&s, 0).into_iter().flat_map(|g| children(&s, g)).collect();
    let (mut s, _) = run_soon(s, "object.ungroup", json!({}));
    assert!(layer(&s, 0) == squares, "each group's objects where the group was");
    assert!(selected(&s, &squares));
    undo_gives_back(&mut s, &before);
}

#[test]
fn compound_path_release_of_25000_compound_paths_finishes_within_20_s() {
    let s = session(25_000, |d, k| {
        let pair = [square(d, 2 * k), square(d, 2 * k + 1)].map(Arc::new);
        Node::new(d.alloc_id(), NodeKind::Compound { children: pair.to_vec(), rule: FillRule::NonZero })
    });
    let before = s.doc().unwrap().doc.clone();
    let paths: Vec<NodeId> = layer(&s, 0).into_iter().flat_map(|c| children(&s, c)).collect();
    let (mut s, _) = run_soon(s, "object.compoundPath.release", json!({}));
    assert!(layer(&s, 0) == paths, "each compound path's paths where it was");
    assert!(selected(&s, &paths));
    undo_gives_back(&mut s, &before);
}

/// With Alt held, a Selection tool press on one of 30,000 selected squares, four moves of 20 pt
/// and the release copy the squares once, as one undo step.
#[test]
fn alt_drag_of_30000_objects_finishes_within_20_s() {
    let s = session(30_000, square);
    let (before, ids, first) = (s.doc().unwrap().doc.clone(), layer(&s, 0), s.doc().unwrap().doc.peek_next_id());
    // The press lands on a square in the middle of the art, away from the handles of the
    // selection's box.
    let grab = square_at(15_100).center();
    let (mut s, r) = soon(s, "the Alt-drag", move |s| {
        let (v, alt) = (ViewInfo::default(), Mods { alt: true, ..Mods::default() });
        s.select_tool("selection", v)?;
        let at = |kind, dx: f64| PointerEvent::new(kind, grab.x + dx, grab.y).with_mods(alt);
        s.pointer(&at(PointerKind::Down, 0.0), v)?;
        for dx in [20.0, 40.0, 60.0, 80.0] {
            s.pointer(&at(PointerKind::Drag, dx), v)?;
        }
        s.pointer(&at(PointerKind::Up, 80.0), v)
    });
    r.unwrap();
    let copies = ids_from(first, ids.len());
    assert!(layer(&s, 0) == in_turn(&ids, &copies), "each copy directly above its square");
    assert!(selected(&s, &copies));
    assert_eq!(s.doc().unwrap().history.undo.last().map(|e| e.label.as_str()), Some("Copy"));
    // The squares stay; their copies move together.
    let moved = bounds(&s, copies[0]).origin() - square_at(0).origin();
    assert!(moved.x > 0.0, "{moved:?}");
    for (k, (id, copy)) in ids.iter().zip(&copies).enumerate() {
        assert_eq!(bounds(&s, *id), square_at(k));
        assert_eq!(bounds(&s, *copy), square_at(k) + moved);
    }
    undo_gives_back(&mut s, &before);
}

/// The ids under `n`: a child that can hold children is a list of its id and theirs.
fn outline(n: &Node) -> Value {
    let ids = n.children().into_iter().flatten().map(|c| match c.children() {
        Some(_) => {
            let mut v = vec![json!(c.id.0)];
            v.extend(outline(c).as_array().cloned().unwrap_or_default());
            Value::Array(v)
        }
        None => json!(c.id.0),
    });
    Value::Array(ids.collect())
}

/// [`outline`] of each layer.
fn layers(s: &Session) -> Vec<Value> {
    s.doc().unwrap().doc.layers.iter().map(|l| outline(l)).collect()
}

/// A session whose first layer holds a, b, c and a group of d, e and f, and whose second layer
/// holds g and h, with a, c, e, g and h selected. Returns the session and the ids of a to h and of
/// the group.
fn two_layers() -> (Session, [u64; 9]) {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
    let rect = |s: &mut Session| {
        let x = 20.0 * s.doc().unwrap().doc.node_count() as f64;
        s.execute("shape.rectangle", &json!({"x": x, "y": 10, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap()
    };
    let [a, b, c, d, e, f] = [(); 6].map(|()| rect(&mut s));
    s.execute("select.set", &json!({"ids": [d, e, f]})).unwrap();
    let group = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    s.execute("layer.new", &json!({})).unwrap();
    let [g, h] = [(); 2].map(|()| rect(&mut s));
    s.execute("select.set", &json!({"ids": [a, c, e, g, h]})).unwrap();
    assert_eq!(layers(&s), [json!([a, b, c, [group, d, e, f]]), json!([g, h])]);
    (s, [a, b, c, d, e, f, g, h, group])
}

/// After Group, Delete and Duplicate of objects in two layers and inside a group, the layers hold
/// the objects and the new ones in the order, and with the ids, that each check lists, and Undo
/// gives back the document.
#[test]
fn group_delete_and_duplicate_across_layers_and_inside_a_group_keep_order_and_ids() {
    let (mut s, [a, b, c, d, e, f, g, h, group]) = two_layers();
    let before = s.doc().unwrap().doc.clone();
    let next = before.peek_next_id();

    let r = s.execute("object.group", &json!({})).unwrap();
    assert_eq!(r["id"], json!(next));
    assert_eq!(layers(&s), [json!([b, [group, d, f]]), json!([[next, a, c, e, g, h]])]);
    assert!(selected(&s, &[NodeId(next)]));
    undo_gives_back(&mut s, &before);

    s.execute("edit.clear", &json!({})).unwrap();
    assert_eq!(layers(&s), [json!([b, [group, d, f]]), json!([])]);
    undo_gives_back(&mut s, &before);

    let r = s.execute("edit.duplicate", &json!({})).unwrap();
    let [a2, c2, e2, g2, h2] = [0, 1, 2, 3, 4].map(|k| next + k);
    assert_eq!(r["ids"], json!([a2, c2, e2, g2, h2]));
    assert_eq!(layers(&s), [json!([a, a2, b, c, c2, [group, d, e, e2, f]]), json!([g, g2, h, h2])]);
    assert!(selected(&s, &ids_from(next, 5)));
    undo_gives_back(&mut s, &before);
}

/// `object.transform` copies ids listed twice, and an object listed before a group around it.
/// Each copy goes directly above its original, a later copy of the same object goes below an
/// earlier one, and the copy of the group holds a copy of the copy made inside the group before it.
#[test]
fn copies_of_ids_listed_twice_or_inside_another_listed_id_keep_their_order_and_ids() {
    let (mut s, [a, b, c, d, e, f, g, h, group]) = two_layers();
    let before = s.doc().unwrap().doc.clone();
    let n = before.peek_next_id();

    let r = s.execute("object.transform", &json!({"matrix": [1, 0, 0, 1, 5, 0], "copy": true, "ids": [e, group]})).unwrap();
    assert_eq!(r["ids"], json!([n, n + 1]));
    let copied = json!([n + 1, n + 2, n + 3, n + 4, n + 5]);
    assert_eq!(layers(&s), [json!([a, b, c, [group, d, e, n, f], copied]), json!([g, h])]);
    undo_gives_back(&mut s, &before);

    let r = s.execute("object.transform", &json!({"matrix": [1, 0, 0, 1, 5, 0], "copy": true, "ids": [a, a]})).unwrap();
    assert_eq!(r["ids"], json!([n, n + 1]));
    assert_eq!(layers(&s), [json!([a, n + 1, n, b, c, [group, d, e, f]]), json!([g, h])]);
    undo_gives_back(&mut s, &before);
}
