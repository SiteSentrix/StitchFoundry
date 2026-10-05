//! Menu long-tail commands (Object/Edit/Select/Type/View/File), driven through `Session::execute`.

use serde_json::{Value, json};
use vectorcraft_color::Color;
use vectorcraft_doc::{ColorMode, LiveShape, Node, NodeKind, TextKind};
use vectorcraft_geom::Rect;

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
    s
}

fn id_of(v: &Value) -> NodeId {
    NodeId(v["id"].as_u64().unwrap())
}

fn rect(s: &mut Session, x: f64, y: f64, w: f64, h: f64) -> NodeId {
    id_of(&s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": w, "height": h})).unwrap())
}

fn text(s: &mut Session, t: &str) -> NodeId {
    id_of(&s.execute("text.create", &json!({"x": 50, "y": 50, "text": t})).unwrap())
}

fn node(s: &Session, id: NodeId) -> Node {
    s.doc().unwrap().doc.node(id).unwrap().clone()
}

fn bounds(s: &Session, id: NodeId) -> Rect {
    node(s, id).geometric_bounds().unwrap()
}

fn sel(s: &mut Session, ids: &[NodeId]) {
    s.execute("select.set", &json!({"ids": ids.iter().map(|i| i.0).collect::<Vec<_>>()})).unwrap();
}

fn fill(s: &mut Session, id: NodeId, hex: &str) {
    s.execute("paint.setFill", &json!({"ids": [id.0], "color": hex})).unwrap();
}

fn fill_of(s: &Session, id: NodeId) -> Color {
    node(s, id).appearance.fill_paint().color().unwrap()
}

fn plain(s: &Session, id: NodeId) -> String {
    match &node(s, id).kind {
        NodeKind::Text(t) => t.plain_text(),
        _ => panic!("not text"),
    }
}

fn undo_len(s: &Session) -> usize {
    s.doc().unwrap().history.undo.len()
}

fn selected(s: &Session) -> Vec<NodeId> {
    s.doc().unwrap().selection.objects.clone()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

// ---------- Object → Lock / Hide ----------

#[test]
fn lock_all_artwork_above_only_overlapping_objects_above() {
    let mut s = session();
    let below = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    let a = rect(&mut s, 10.0, 10.0, 50.0, 50.0);
    let over = rect(&mut s, 30.0, 30.0, 50.0, 50.0);
    let far = rect(&mut s, 400.0, 400.0, 20.0, 20.0);
    sel(&mut s, &[a]);
    let n = undo_len(&s);
    let r = s.execute("object.lock.above", &json!({})).unwrap();
    assert_eq!(r["count"], 1);
    assert!(node(&s, over).locked);
    assert!(!node(&s, below).locked && !node(&s, far).locked && !node(&s, a).locked);
    assert_eq!(undo_len(&s), n + 1);
}

#[test]
fn hide_all_artwork_above() {
    let mut s = session();
    let a = rect(&mut s, 10.0, 10.0, 50.0, 50.0);
    let over = rect(&mut s, 30.0, 30.0, 50.0, 50.0);
    sel(&mut s, &[a]);
    s.execute("object.hide.above", &json!({})).unwrap();
    assert!(!node(&s, over).visible);
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(node(&s, over).visible);
}

#[test]
fn lock_and_hide_other_layers() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let l2 = id_of(&s.execute("layer.new", &json!({})).unwrap_or(json!({"id": 0})));
    let l1 = s.doc().unwrap().doc.layers[0].id;
    assert_ne!(l1, l2);
    sel(&mut s, &[a]);
    s.execute("object.lock.otherLayers", &json!({})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert!(d.node(l2).unwrap().locked);
    assert!(!d.node(l1).unwrap().locked);
    s.execute("object.hide.otherLayers", &json!({})).unwrap();
    assert!(!s.doc().unwrap().doc.node(l2).unwrap().visible);
}

// ---------- Transform Each ----------

#[test]
fn transform_each_scales_each_about_its_own_center() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    let b = rect(&mut s, 200.0, 0.0, 100.0, 100.0);
    sel(&mut s, &[a, b]);
    let n = undo_len(&s);
    s.execute("object.transformEach", &json!({"scaleH": 50, "scaleV": 50})).unwrap();
    assert_eq!(bounds(&s, a), Rect::new(25.0, 25.0, 75.0, 75.0));
    assert_eq!(bounds(&s, b), Rect::new(225.0, 25.0, 275.0, 75.0));
    assert_eq!(undo_len(&s), n + 1);
}

#[test]
fn transform_each_move_reference_and_copy() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    sel(&mut s, &[a]);
    // Top-left reference: scaling keeps the top-left corner.
    let r = s.execute("object.transformEach", &json!({"scaleH": 50, "scaleV": 50, "moveH": 10, "moveV": 5, "reference": 0, "copy": true})).unwrap();
    let copy = NodeId(r["ids"][0].as_u64().unwrap());
    assert_ne!(copy, a);
    assert_eq!(bounds(&s, a), Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(bounds(&s, copy), Rect::new(10.0, 5.0, 60.0, 55.0));
}

#[test]
fn transform_each_random_is_deterministic_with_seed() {
    let run = || {
        let mut s = session();
        let a = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
        sel(&mut s, &[a]);
        s.execute("object.transformEach", &json!({"moveH": 50, "random": true, "seed": 7})).unwrap();
        bounds(&s, a)
    };
    let (x, y) = (run(), run());
    assert_eq!(x, y);
    assert!(x.x0.abs() <= 50.0 && x.width() == 100.0);
}

#[test]
fn reset_bounding_box_is_a_noop() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    sel(&mut s, &[a]);
    let n = undo_len(&s);
    assert_eq!(s.execute("object.resetBoundingBox", &json!({})).unwrap()["changed"], 0);
    assert_eq!(undo_len(&s), n);
}

// ---------- Expand / Rasterize / Crop / Trim marks ----------

#[test]
fn expand_live_shape_and_text_in_one_undo_step() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let t = text(&mut s, "Hi");
    sel(&mut s, &[a, t]);
    let n = undo_len(&s);
    // Stroke off: the rectangle stays one path (its stroke isn't outlined).
    s.execute("object.expand", &json!({"stroke": false})).unwrap();
    assert!(matches!(node(&s, a).kind, NodeKind::Path { live: None, .. }));
    assert!(s.doc().unwrap().doc.node(t).is_none(), "text became outlines");
    assert_eq!(undo_len(&s), n + 1);
    assert_eq!(s.doc().unwrap().history.undo.last().unwrap().label, "Expand");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(matches!(node(&s, a).kind, NodeKind::Path { live: Some(_), .. }));
    assert!(s.doc().unwrap().doc.node(t).is_some());
}

#[test]
fn expand_nothing_errors() {
    let mut s = session();
    let p = id_of(&s.execute("path.create", &json!({"anchors": [{"x": 0, "y": 0}, {"x": 10, "y": 10}]})).unwrap());
    sel(&mut s, &[p]);
    // A plain path: nothing to expand for object/fill.
    let r = s.execute("object.expand", &json!({"stroke": false}));
    assert!(r.is_err());
}

#[test]
fn rasterize_replaces_selection_with_image() {
    let mut s = session();
    let a = rect(&mut s, 10.0, 20.0, 40.0, 30.0);
    fill(&mut s, a, "#ff0000");
    sel(&mut s, &[a]);
    let vb = s.doc().unwrap().doc.bounds_of(&[a], true).unwrap();
    let r = s.execute("object.rasterize", &json!({"ppi": 144, "padding": 0})).unwrap();
    let img = id_of(&r);
    assert!(s.doc().unwrap().doc.node(a).is_none());
    let n = node(&s, img);
    let NodeKind::Image(im) = &n.kind else { panic!() };
    // Visual bounds (stroke included) at 2 px/pt.
    assert_eq!((im.width as f64, im.height as f64), ((vb.width() * 2.0).ceil(), (vb.height() * 2.0).ceil()));
    assert!(s.doc().unwrap().doc.images.contains_key(&im.key));
    let b = n.geometric_bounds().unwrap();
    assert!(close(b.x0, vb.x0) && close(b.y0, vb.y0) && b.width() >= vb.width());
    assert_eq!(selected(&s), vec![img]);
}

#[test]
fn crop_image_to_rect() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 100.0, 100.0);
    sel(&mut s, &[a]);
    let img = id_of(&s.execute("object.rasterize", &json!({"ppi": 72})).unwrap());
    let r = s.execute("object.cropImage", &json!({"rect": [10, 10, 50, 40]})).unwrap();
    assert_eq!((r["width"].as_u64().unwrap(), r["height"].as_u64().unwrap()), (50, 40));
    let b = bounds(&s, img);
    assert!(close(b.x0, 10.0) && close(b.y0, 10.0) && close(b.width(), 50.0) && close(b.height(), 40.0), "{b:?}");
    // Image fully inside the artboard: default crop has nothing to do.
    assert!(s.execute("object.cropImage", &json!({})).is_err());
}

#[test]
fn crop_image_disabled_without_image() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    sel(&mut s, &[a]);
    assert!(matches!(s.execute("object.cropImage", &json!({})), Err(EngineError::Disabled(..))));
}

#[test]
fn trim_marks_around_selection() {
    let mut s = session();
    let a = rect(&mut s, 100.0, 100.0, 200.0, 100.0);
    fill(&mut s, a, "#00ff00");
    s.execute("stroke.set", &json!({"ids": [a.0], "weight": 0})).unwrap();
    sel(&mut s, &[a]);
    let g = id_of(&s.execute("object.createTrimMarks", &json!({"offset": 10, "length": 20})).unwrap());
    let n = node(&s, g);
    assert_eq!(n.children().unwrap().len(), 8);
    assert_eq!(n.name.as_deref(), Some("Trim Marks"));
    let b = n.geometric_bounds().unwrap();
    let vb = s.doc().unwrap().doc.bounds_of(&[a], true).unwrap();
    assert!(close(b.x0, vb.x0 - 30.0) && close(b.y1, vb.y1 + 30.0), "{b:?} {vb:?}");
}

// ---------- Convert to Shape ----------

#[test]
fn convert_rectangle_path_to_live_shape() {
    let mut s = session();
    let p = id_of(
        &s.execute("path.create", &json!({"anchors": [{"x": 0, "y": 0}, {"x": 40, "y": 0}, {"x": 40, "y": 20}, {"x": 0, "y": 20}], "closed": true}))
            .unwrap(),
    );
    sel(&mut s, &[p]);
    assert_eq!(s.execute("object.shape.convertToShape", &json!({})).unwrap()["converted"], 1);
    let NodeKind::Path { live: Some(LiveShape::Rectangle { w, h, .. }), path, .. } = &node(&s, p).kind else { panic!() };
    assert!(close(*w, 40.0) && close(*h, 20.0));
    assert_eq!(path.bounds(), Some(Rect::new(0.0, 0.0, 40.0, 20.0)));
}

#[test]
fn convert_rotated_rectangle_and_ellipse() {
    let mut s = session();
    let p = id_of(
        &s.execute(
            "path.create",
            &json!({"anchors": [{"x": 50, "y": 0}, {"x": 100, "y": 50}, {"x": 50, "y": 100}, {"x": 0, "y": 50}], "closed": true}),
        )
        .unwrap(),
    );
    let e = id_of(&s.execute("shape.ellipse", &json!({"x": 0, "y": 200, "width": 80, "height": 40})).unwrap());
    sel(&mut s, &[e]);
    s.execute("object.expandShape", &json!({})).unwrap();
    sel(&mut s, &[p, e]);
    assert_eq!(s.execute("object.shape.convertToShape", &json!({})).unwrap()["converted"], 2);
    let b = bounds(&s, p);
    assert!(close(b.x0, 0.0) && close(b.x1, 100.0) && close(b.y1, 100.0), "{b:?}");
    assert!(matches!(node(&s, e).kind, NodeKind::Path { live: Some(LiveShape::Ellipse { .. }), .. }));
}

#[test]
fn convert_to_shape_rejects_other_paths() {
    let mut s = session();
    let p = id_of(&s.execute("path.create", &json!({"anchors": [{"x": 0, "y": 0}, {"x": 40, "y": 0}, {"x": 10, "y": 20}], "closed": true})).unwrap());
    sel(&mut s, &[p]);
    assert!(s.execute("object.shape.convertToShape", &json!({})).is_err());
}

// ---------- Blend ----------

#[test]
fn blend_make_release_options() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&mut s, 100.0, 0.0, 30.0, 30.0);
    fill(&mut s, a, "#000000");
    fill(&mut s, b, "#ffffff");
    sel(&mut s, &[a, b]);
    let n = undo_len(&s);
    let g = id_of(&s.execute("object.blend.make", &json!({"steps": 3})).unwrap());
    assert_eq!(undo_len(&s), n + 1);
    let gn = node(&s, g);
    assert!(matches!(gn.kind, NodeKind::Blend { .. }), "live blend");
    let ch = vectorcraft_doc::live::expand_live(&gn);
    assert_eq!(ch.len(), 5);
    // Middle step: halfway in position, size and colour.
    let mid = &ch[2];
    let mb = mid.geometric_bounds().unwrap();
    assert!(close(mb.x0, 50.0) && close(mb.width(), 20.0), "{mb:?}");
    let c = mid.appearance.fill_paint().color().unwrap().to_rgb();
    assert!((c[0] - 0.5).abs() < 1e-3);
    // Options: 1 step.
    s.execute("object.blend.options", &json!({"steps": 1})).unwrap();
    assert_eq!(vectorcraft_doc::live::expand_live(&node(&s, g)).len(), 3);
    // Release: keys only, back in the layer.
    let r = s.execute("object.blend.release", &json!({})).unwrap();
    assert_eq!(r["ids"].as_array().unwrap().len(), 2);
    assert!(s.doc().unwrap().doc.node(g).is_none());
    assert!(s.doc().unwrap().doc.node(a).is_some() && s.doc().unwrap().doc.node(b).is_some());
}

#[test]
fn blend_expand_and_reverse() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&mut s, 100.0, 0.0, 10.0, 10.0);
    sel(&mut s, &[a, b]);
    let g = id_of(&s.execute("object.blend.make", &json!({"steps": 2})).unwrap());
    s.execute("object.blend.reverseFrontToBack", &json!({})).unwrap();
    assert_eq!(node(&s, g).children().unwrap().last().unwrap().id, a);
    s.execute("object.blend.reverseSpine", &json!({})).unwrap();
    assert!(close(bounds(&s, a).x0, 100.0));
    s.execute("object.blend.expand", &json!({})).unwrap();
    assert!(matches!(node(&s, g).kind, NodeKind::Group { .. }));
    assert!(s.execute("object.blend.release", &json!({})).is_err(), "no longer a blend");
}

// ---------- Artboards ----------

#[test]
fn convert_to_artboards_and_rearrange() {
    let mut s = session();
    let a = rect(&mut s, 900.0, 0.0, 200.0, 100.0);
    sel(&mut s, &[a]);
    assert_eq!(s.execute("artboard.convertToArtboards", &json!({})).unwrap()["artboards"], 1);
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.artboards.len(), 2);
    assert_eq!(d.artboards[1].rect, Rect::new(900.0, 0.0, 1100.0, 100.0));
    assert!(d.node(a).is_none());
    // Art on artboard 2 moves with it.
    let b = rect(&mut s, 950.0, 25.0, 10.0, 10.0);
    s.execute("artboard.rearrange", &json!({"columns": 1, "spacing": 50})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert_eq!(d.artboards[1].rect, Rect::new(0.0, 650.0, 200.0, 750.0));
    assert_eq!(bounds(&s, b), Rect::new(50.0, 675.0, 60.0, 685.0));
}

// ---------- Edit Colors ----------

#[test]
fn invert_and_convert_colors() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    fill(&mut s, a, "#ff0000");
    sel(&mut s, &[a]);
    s.execute("edit.colors.invert", &json!({"stroke": false})).unwrap();
    assert_eq!(fill_of(&s, a).to_hex(), "#00ffff");
    s.execute("edit.colors.toCMYK", &json!({})).unwrap();
    assert!(matches!(fill_of(&s, a), Color::Cmyk { .. }));
    s.execute("edit.colors.toGrayscale", &json!({})).unwrap();
    assert!(matches!(fill_of(&s, a), Color::Gray { .. }));
    s.execute("edit.colors.toRGB", &json!({})).unwrap();
    assert!(matches!(fill_of(&s, a), Color::Rgb { .. }));
}

#[test]
fn saturate_and_adjust_balance() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    s.execute("paint.setFill", &json!({"ids": [a.0], "color": [0.8, 0.4, 0.4]})).unwrap();
    sel(&mut s, &[a]);
    s.execute("edit.colors.saturate", &json!({"intensity": -100})).unwrap();
    let [_, sat, _] = fill_of(&s, a).to_hsb();
    assert!(sat < 1e-4);
    s.execute("edit.colors.adjustBalance", &json!({"r": 10, "g": -10})).unwrap();
    let [r, g, _] = fill_of(&s, a).to_rgb();
    assert!((r - 0.9).abs() < 1e-3 && (g - 0.7).abs() < 1e-3, "{r} {g}");
    assert!(s.execute("edit.colors.adjustBalance", &json!({})).is_err());
}

#[test]
fn blend_colors_horizontally() {
    let mut s = session();
    let ids: Vec<NodeId> = (0..3).map(|i| rect(&mut s, 200.0 - i as f64 * 100.0, 0.0, 10.0, 10.0)).collect();
    fill(&mut s, ids[2], "#000000"); // leftmost
    fill(&mut s, ids[0], "#ffffff"); // rightmost
    fill(&mut s, ids[1], "#ff0000");
    sel(&mut s, &ids);
    s.execute("edit.colors.blendHorizontally", &json!({})).unwrap();
    assert_eq!(fill_of(&s, ids[1]).to_hex(), "#808080");
    sel(&mut s, &ids[..2]);
    assert!(s.execute("edit.colors.blendVertically", &json!({})).is_err(), "needs three");
}

#[test]
fn blend_front_to_back_uses_stacking() {
    let mut s = session();
    let ids: Vec<NodeId> = (0..3).map(|_| rect(&mut s, 0.0, 0.0, 10.0, 10.0)).collect();
    fill(&mut s, ids[0], "#000000");
    fill(&mut s, ids[2], "#ffffff");
    sel(&mut s, &ids);
    s.execute("edit.colors.blendFrontToBack", &json!({})).unwrap();
    assert_eq!(fill_of(&s, ids[1]).to_hex(), "#808080");
}

// ---------- Paste without formatting / Find ----------

#[test]
fn paste_without_formatting_resets_text_style() {
    let mut s = session();
    let t = id_of(&s.execute("text.create", &json!({"x": 10, "y": 10, "text": "Bold", "size": 40})).unwrap());
    sel(&mut s, &[t]);
    s.execute("edit.copy", &json!({})).unwrap();
    let n = undo_len(&s);
    let r = s.execute("edit.pasteWithoutFormatting", &json!({})).unwrap();
    let p = NodeId(r["ids"][0].as_u64().unwrap());
    let NodeKind::Text(tt) = &node(&s, p).kind else { panic!() };
    assert_eq!(tt.plain_text(), "Bold");
    assert_eq!(tt.first_style().size, 12.0);
    assert_eq!(undo_len(&s), n + 1);
    // Clipboard keeps its formatting.
    let NodeKind::Text(ct) = &s.clipboard.nodes[0].kind else { panic!() };
    assert_eq!(ct.first_style().size, 40.0);
}

#[test]
fn find_and_replace_options() {
    let mut s = session();
    let a = text(&mut s, "Cat cat catalog");
    let b = text(&mut s, "the CAT");
    let r = s.execute("edit.findReplace", &json!({"find": "cat", "replace": "dog", "wholeWord": true})).unwrap();
    assert_eq!(r["count"], 3);
    assert_eq!(plain(&s, a), "dog dog catalog");
    assert_eq!(plain(&s, b), "the dog");
    let r = s.execute("edit.findReplace", &json!({"find": "Dog", "replace": "x", "matchCase": true})).unwrap();
    assert_eq!(r["count"], 0);
    assert!(s.execute("edit.findReplace", &json!({"find": ""})).is_err());
}

#[test]
fn find_next_cycles_through_text_objects() {
    let mut s = session();
    let a = text(&mut s, "apple");
    let _ = text(&mut s, "banana");
    let c = text(&mut s, "pineapple");
    s.execute("select.none", &json!({})).unwrap();
    assert_eq!(id_of(&s.execute("edit.findNext", &json!({"find": "APPLE"})).unwrap()), a);
    assert_eq!(id_of(&s.execute("edit.findNext", &json!({"find": "apple"})).unwrap()), c);
    assert_eq!(id_of(&s.execute("edit.findNext", &json!({"find": "apple"})).unwrap()), a);
    assert!(s.execute("edit.findNext", &json!({"find": "zzz"})).unwrap()["id"].is_null());
}

// ---------- Select ----------

#[test]
fn select_same_font_size_and_fill() {
    let mut s = session();
    let a = id_of(&s.execute("text.create", &json!({"x": 0, "y": 20, "text": "a", "size": 20})).unwrap());
    let b = id_of(&s.execute("text.create", &json!({"x": 0, "y": 60, "text": "b", "size": 20})).unwrap());
    let c = id_of(&s.execute("text.create", &json!({"x": 0, "y": 90, "text": "c", "size": 30, "font": "Inter"})).unwrap());
    sel(&mut s, &[a]);
    assert_eq!(s.execute("select.same.fontSize", &json!({})).unwrap()["count"], 2);
    assert_eq!(selected(&s), vec![a, b]);
    sel(&mut s, &[c]);
    assert_eq!(s.execute("select.same.fontFamily", &json!({})).unwrap()["count"], 1);
    sel(&mut s, &[a]);
    assert_eq!(s.execute("select.same.textFillColor", &json!({})).unwrap()["count"], 3);
    assert_eq!(s.execute("select.same.fontFamilyStyle", &json!({})).unwrap()["count"], 2);
    let r = rect(&mut s, 0.0, 0.0, 5.0, 5.0);
    sel(&mut s, &[r]);
    assert!(s.execute("select.same.fontSize", &json!({})).is_err());
}

#[test]
fn select_direction_handles_selects_all_anchors() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    sel(&mut s, &[a]);
    assert_eq!(s.execute("select.object.directionHandles", &json!({})).unwrap()["anchors"], 4);
    assert_eq!(s.doc().unwrap().selection.anchors.get(&a).unwrap().len(), 4);
}

#[test]
fn select_point_and_area_text() {
    let mut s = session();
    let p = text(&mut s, "point");
    let a = id_of(&s.execute("text.create", &json!({"x": 0, "y": 0, "text": "area", "area": {"width": 100, "height": 50}})).unwrap());
    let _ = rect(&mut s, 0.0, 0.0, 5.0, 5.0);
    assert_eq!(s.execute("select.object.pointText", &json!({})).unwrap()["count"], 1);
    assert_eq!(selected(&s), vec![p]);
    s.execute("select.object.areaText", &json!({})).unwrap();
    assert_eq!(selected(&s), vec![a]);
    assert_eq!(s.execute("select.object.brushStrokes", &json!({})).unwrap()["count"], 0);
}

#[test]
fn save_and_recall_selection() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    let b = rect(&mut s, 20.0, 0.0, 10.0, 10.0);
    sel(&mut s, &[a, b]);
    assert_eq!(s.execute("select.save", &json!({})).unwrap()["name"], "Selection 1");
    s.execute("select.none", &json!({})).unwrap();
    assert_eq!(s.execute("select.recall", &json!({"name": "Selection 1"})).unwrap()["count"], 2);
    s.execute("select.editSaved", &json!({"name": "Selection 1", "newName": "Pair"})).unwrap();
    assert_eq!(s.execute("select.savedList", &json!({})).unwrap(), json!(["Pair"]));
    s.execute("select.editSaved", &json!({"name": "Pair", "delete": true})).unwrap();
    assert!(s.execute("select.recall", &json!({"name": "Pair"})).is_err());
}

// ---------- Type ----------

#[test]
fn change_case_variants() {
    let mut s = session();
    let t = text(&mut s, "hello wORLD. again here");
    sel(&mut s, &[t]);
    s.execute("type.changeCase", &json!({"case": "upper"})).unwrap();
    assert_eq!(plain(&s, t), "HELLO WORLD. AGAIN HERE");
    s.execute("type.changeCase", &json!({"case": "lower"})).unwrap();
    assert_eq!(plain(&s, t), "hello world. again here");
    s.execute("type.changeCase", &json!({"case": "title"})).unwrap();
    assert_eq!(plain(&s, t), "Hello World. Again Here");
    s.execute("type.changeCase", &json!({"case": "sentence"})).unwrap();
    assert_eq!(plain(&s, t), "Hello world. Again here");
    assert!(s.execute("type.changeCase", &json!({"case": "weird"})).is_err());
}

#[test]
fn smart_punctuation_quotes_dashes_ellipsis() {
    let mut s = session();
    let t = text(&mut s, "\"Hi\" it's 1--2 and 3---4...");
    sel(&mut s, &[t]);
    assert_eq!(s.execute("type.smartPunctuation", &json!({})).unwrap()["changed"], 1);
    assert_eq!(plain(&s, t), "“Hi” it’s 1–2 and 3—4…");
}

#[test]
fn convert_point_area_roundtrip() {
    let mut s = session();
    let t = text(&mut s, "Some words");
    sel(&mut s, &[t]);
    s.execute("type.convertToAreaType", &json!({})).unwrap();
    let NodeKind::Text(tt) = &node(&s, t).kind else { panic!() };
    assert!(matches!(tt.kind, TextKind::Area { .. }));
    assert_eq!(tt.plain_text(), "Some words");
    s.execute("type.convertToPointType", &json!({})).unwrap();
    let NodeKind::Text(tt) = &node(&s, t).kind else { panic!() };
    assert!(matches!(tt.kind, TextKind::Point));
    assert_eq!(tt.plain_text(), "Some words");
}

#[test]
fn area_to_point_turns_wraps_into_breaks() {
    let mut s = session();
    let a = id_of(
        &s.execute("text.create", &json!({"x": 0, "y": 0, "text": "one two three four five six", "area": {"width": 40, "height": 200}})).unwrap(),
    );
    sel(&mut s, &[a]);
    s.execute("type.convertToPointType", &json!({})).unwrap();
    let txt = plain(&s, a);
    assert!(txt.contains('\n'), "{txt:?}");
    assert_eq!(txt.replace('\n', " "), "one two three four five six");
}

#[test]
fn placeholder_and_insert_characters() {
    let mut s = session();
    let t = text(&mut s, "x");
    sel(&mut s, &[t]);
    s.execute("type.fillPlaceholder", &json!({})).unwrap();
    assert!(plain(&s, t).len() > 10);
    s.execute("text.setText", &json!({"text": "A"})).unwrap();
    s.execute("type.insert", &json!({"char": "emDash"})).unwrap();
    s.execute("type.insert", &json!({"char": "copyright"})).unwrap();
    s.execute("type.insert", &json!({"text": "z"})).unwrap();
    assert_eq!(plain(&s, t), "A—©z");
    assert!(s.execute("type.insert", &json!({"char": "nope"})).is_err());
}

#[test]
fn area_placeholder_fills_frame() {
    let mut s = session();
    let a = id_of(&s.execute("text.create", &json!({"x": 0, "y": 0, "text": "", "area": {"width": 300, "height": 300}})).unwrap());
    sel(&mut s, &[a]);
    s.execute("type.fillPlaceholder", &json!({})).unwrap();
    assert!(plain(&s, a).len() > 400);
}

#[test]
fn font_and_size_menu_items_use_set_style() {
    let mut s = session();
    let t = text(&mut s, "x");
    sel(&mut s, &[t]);
    s.execute("text.setStyle", &json!({"size": 36})).unwrap();
    s.execute("text.setStyle", &json!({"font": "Inter"})).unwrap();
    let NodeKind::Text(tt) = &node(&s, t).kind else { panic!() };
    assert_eq!(tt.first_style().size, 36.0);
    assert_eq!(tt.first_style().font_family, "Inter");
}

// ---------- View → Guides ----------

#[test]
fn make_release_clear_guides() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    sel(&mut s, &[a]);
    assert_eq!(s.execute("view.guides.make", &json!({})).unwrap()["count"], 1);
    assert!(matches!(node(&s, a).kind, NodeKind::Path { guide: true, .. }));
    assert!(selected(&s).is_empty());
    assert_eq!(s.execute("view.guides.release", &json!({})).unwrap()["count"], 1);
    assert!(matches!(node(&s, a).kind, NodeKind::Path { guide: false, .. }));
    sel(&mut s, &[a]);
    s.execute("view.guides.make", &json!({})).unwrap();
    s.execute("guide.add", &json!({"vertical": true, "pos": 100})).unwrap();
    let n = undo_len(&s);
    assert_eq!(s.execute("view.guides.clear", &json!({})).unwrap()["count"], 2);
    assert!(s.doc().unwrap().doc.guides.is_empty());
    assert!(s.doc().unwrap().doc.node(a).is_none());
    assert_eq!(undo_len(&s), n + 1);
}

#[test]
fn ruler_guides_and_lock() {
    let mut s = session();
    assert_eq!(s.execute("guide.add", &json!({"vertical": false, "pos": 50})).unwrap()["index"], 0);
    s.execute("guide.move", &json!({"index": 0, "pos": 70})).unwrap();
    assert_eq!(s.doc().unwrap().doc.guides[0].pos, 70.0);
    assert_eq!(s.execute("view.guides.lock", &json!({})).unwrap()["locked"], true);
    assert!(s.guides_locked());
    assert!(matches!(s.execute("guide.remove", &json!({"index": 0})), Err(EngineError::Disabled(..))));
    s.execute("view.guides.lock", &json!({})).unwrap();
    s.execute("guide.remove", &json!({"index": 0})).unwrap();
    assert!(s.doc().unwrap().doc.guides.is_empty());
    assert!(s.execute("guide.remove", &json!({"index": 0})).is_err());
}

// ---------- File ----------

#[test]
fn close_all_documents() {
    let mut s = session();
    s.execute("file.new", &json!({})).unwrap();
    assert_eq!(s.execute("file.closeAll", &json!({})).unwrap()["closed"], 2);
    assert!(s.active().is_none());
    assert!(s.execute("file.closeAll", &json!({})).is_err());
}

#[test]
fn document_color_mode_converts_colours() {
    let mut s = session();
    let a = rect(&mut s, 0.0, 0.0, 10.0, 10.0);
    fill(&mut s, a, "#ff0000");
    s.execute("file.documentColorMode", &json!({"mode": "cmyk"})).unwrap();
    assert_eq!(s.doc().unwrap().doc.color_mode, ColorMode::Cmyk);
    // Converted through the colour settings: a press red, mostly magenta and yellow.
    let Some(Color::Cmyk { c, m, y, k }) = node(&s, a).appearance.fill_paint().color() else { panic!("not CMYK") };
    assert!(m > 0.8 && y > 0.8 && c < 0.05 && k < 0.05, "{c} {m} {y} {k}");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.doc().unwrap().doc.color_mode, ColorMode::Rgb);
    assert!(s.execute("file.documentColorMode", &json!({"mode": "lab"})).is_err());
}

#[test]
fn file_info_sets_title() {
    let mut s = session();
    let r = s.execute("file.info", &json!({"title": "Poster"})).unwrap();
    assert_eq!(r["title"], "Poster");
    assert_eq!(s.doc().unwrap().doc.title, "Poster");
    assert_eq!(s.execute("file.info", &json!({})).unwrap()["colorMode"], "rgb");
}

#[test]
fn every_new_command_has_params_doc_and_menu() {
    for id in [
        "object.lock.above",
        "object.transformEach",
        "object.rasterize",
        "object.blend.make",
        "edit.colors.invert",
        "edit.findReplace",
        "select.same.fontSize",
        "type.changeCase",
        "view.guides.make",
        "file.closeAll",
    ] {
        let c = find_command(id).unwrap_or_else(|| panic!("{id}"));
        assert!(!c.params.is_empty() && !c.menu.is_empty(), "{id}");
    }
}
