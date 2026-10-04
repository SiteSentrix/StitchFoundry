//! Appearance stacks: the active item that paint, stroke, gradient and transparency edits target
//! (`appearance.setActiveItem`, the `item` param).

use serde_json::{Value, json};
use vectorcraft_color::{BlendMode, Color, GradientKind, Paint};
use vectorcraft_doc::{AppearanceItem, Node};

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    s
}

/// A selected 100×100 rectangle with the default appearance `[Fill white, Stroke black]`.
fn rect(s: &mut Session, x: f64) -> NodeId {
    let r = s.execute("shape.rectangle", &json!({"x": x, "y": 100, "width": 100, "height": 100})).unwrap();
    let id = NodeId(r["id"].as_u64().unwrap());
    s.execute("select.set", &json!({"ids": [id.0]})).unwrap();
    id
}

fn node(s: &Session, id: NodeId) -> Node {
    s.doc().unwrap().doc.node(id).unwrap().clone()
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn stroke(n: &Node, i: usize) -> &vectorcraft_doc::StrokeLayer {
    n.appearance.stroke_at(Some(i)).unwrap_or_else(|| panic!("item {i} is not a stroke"))
}

fn fill_paint(n: &Node, i: usize) -> &Paint {
    &n.appearance.fill_at(Some(i)).unwrap_or_else(|| panic!("item {i} is not a fill")).paint
}

#[test]
fn stroke_set_with_item_changes_only_that_stroke() {
    let mut s = session();
    let id = rect(&mut s, 100.0);
    run(&mut s, "appearance.addStroke", json!({})); // [Fill, Stroke, Stroke]
    run(&mut s, "stroke.set", json!({"item": 1, "dash": [4, 2], "weight": 6}));
    let n = node(&s, id);
    assert_eq!(stroke(&n, 1).dash.as_ref().unwrap().pattern, vec![4.0, 2.0]);
    assert_eq!(stroke(&n, 1).width, 6.0);
    assert!(stroke(&n, 2).dash.is_none());
    assert_eq!(stroke(&n, 2).width, 1.0);
    run(&mut s, "stroke.setAdvanced", json!({"item": 1, "arrowScale": [50, 200]}));
    let n = node(&s, id);
    assert_eq!((stroke(&n, 1).arrow_scale, stroke(&n, 2).arrow_scale), ((50.0, 200.0), (100.0, 100.0)));
    // An explicit item must be a stroke; nothing changes when it is not.
    assert!(s.execute("stroke.set", &json!({"item": 0, "weight": 9})).is_err());
    assert!(s.execute("stroke.set", &json!({"item": 7, "weight": 9})).is_err());
    assert!(s.execute("stroke.set", &json!({"item": "top", "weight": 9})).is_err());
    assert_eq!(stroke(&node(&s, id), 2).width, 1.0);
    // Without an item the top stroke changes.
    run(&mut s, "stroke.set", json!({"weight": 3}));
    let n = node(&s, id);
    assert_eq!((stroke(&n, 1).width, stroke(&n, 2).width), (6.0, 3.0));
}

#[test]
fn edit_gradient_on_the_lower_fill() {
    let mut s = session();
    let id = rect(&mut s, 100.0);
    run(&mut s, "appearance.addFill", json!({})); // [Fill, Stroke, Fill]
    run(&mut s, "paint.editGradient", json!({"item": 0, "kind": "radial"}));
    let n = node(&s, id);
    let Paint::Gradient(g) = fill_paint(&n, 0) else { panic!("lower fill is not a gradient") };
    assert_eq!(g.gradient.kind, GradientKind::Radial);
    assert_eq!(*fill_paint(&n, 2), Paint::solid(Color::WHITE));
    // The gradient vector of the lower fill, through the active item (the gradient tool's path).
    run(&mut s, "appearance.setActiveItem", json!({"index": 0}));
    run(&mut s, "paint.setGradientGeom", json!({"start": [100, 150], "end": [200, 150]}));
    let n = node(&s, id);
    let Paint::Gradient(g) = fill_paint(&n, 0) else { panic!() };
    assert_eq!(g.geom.unwrap().end.x, 200.0);
    assert_eq!(*fill_paint(&n, 2), Paint::solid(Color::WHITE));
    // A stroke row makes the gradient vector a stroke gradient.
    run(&mut s, "appearance.setActiveItem", json!({"index": 1}));
    run(&mut s, "paint.setGradientGeom", json!({"start": [100, 150], "end": [100, 250]}));
    assert!(matches!(stroke(&node(&s, id), 1).paint, Paint::Gradient(_)));
}

#[test]
fn active_item_targets_paint_and_proxies_and_clears_on_selection_change() {
    let mut s = session();
    let other = rect(&mut s, 250.0);
    let id = rect(&mut s, 100.0);
    run(&mut s, "appearance.addFill", json!({})); // [Fill, Stroke, Fill]
    assert!(s.execute("appearance.setActiveItem", &json!({"index": 5})).is_err());
    assert_eq!(run(&mut s, "appearance.setActiveItem", json!({"index": 0}))["index"], 0);
    assert_eq!(s.appearance_item(), Some(0));
    assert_eq!(inspect::document(&s)["paint"]["appearanceItem"], 0);
    assert!(s.fill_active);
    run(&mut s, "paint.setFill", json!({"color": "#ff0000"}));
    let n = node(&s, id);
    assert_eq!(fill_paint(&n, 0).color().unwrap().to_hex(), "#ff0000");
    assert_eq!(*fill_paint(&n, 2), Paint::solid(Color::WHITE));
    // The stroke edits fall back to the top stroke while a fill row is active.
    run(&mut s, "paint.setStroke", json!({"color": "#00ff00"}));
    assert_eq!(stroke(&node(&s, id), 1).paint.color().unwrap().to_hex(), "#00ff00");
    // A stroke row brings the Stroke proxy forward.
    run(&mut s, "appearance.setActiveItem", json!({"index": 1}));
    assert!(!s.fill_active);
    // Changing the selection clears it, also when the same object is selected again.
    run(&mut s, "select.set", json!({"ids": [other.0]}));
    assert_eq!(s.appearance_item(), None);
    run(&mut s, "select.set", json!({"ids": [id.0]}));
    assert_eq!(s.appearance_item(), None);
    run(&mut s, "paint.setFill", json!({"color": "#0000ff"}));
    let n = node(&s, id);
    assert_eq!(fill_paint(&n, 2).color().unwrap().to_hex(), "#0000ff");
    assert_eq!(fill_paint(&n, 0).color().unwrap().to_hex(), "#ff0000");
    // Explicit ids never use the active item, and `null` forces the top one.
    run(&mut s, "appearance.setActiveItem", json!({"index": 0}));
    run(&mut s, "paint.setFill", json!({"color": "#222222", "ids": [id.0]}));
    assert_eq!(fill_paint(&node(&s, id), 2).color().unwrap().to_hex(), "#222222");
    run(&mut s, "paint.setFill", json!({"color": "#111111", "item": null}));
    let n = node(&s, id);
    assert_eq!(fill_paint(&n, 2).color().unwrap().to_hex(), "#111111");
    assert_eq!(fill_paint(&n, 0).color().unwrap().to_hex(), "#ff0000");
    run(&mut s, "appearance.setActiveItem", json!({"index": null}));
    assert_eq!(s.appearance_item(), None);
}

#[test]
fn item_rows_follow_remove_and_move() {
    let mut s = session();
    let id = rect(&mut s, 100.0);
    run(&mut s, "appearance.addFill", json!({})); // [Fill, Stroke, Fill]
    run(&mut s, "appearance.setActiveItem", json!({"index": 1}));
    run(&mut s, "appearance.moveItem", json!({"from": 1, "to": 2}));
    assert_eq!(s.appearance_item(), Some(2));
    assert!(matches!(node(&s, id).appearance.items[2], AppearanceItem::Stroke(_)));
    run(&mut s, "appearance.removeItem", json!({"index": 0}));
    assert_eq!(s.appearance_item(), Some(1));
    run(&mut s, "appearance.removeItem", json!({"index": 1}));
    assert_eq!(s.appearance_item(), None);
}

#[test]
fn transparency_goes_to_the_targeted_item() {
    let mut s = session();
    let id = rect(&mut s, 100.0);
    run(&mut s, "transparency.set", json!({"item": 1, "opacity": 40, "blend": "multiply"}));
    let n = node(&s, id);
    assert_eq!((stroke(&n, 1).opacity, stroke(&n, 1).blend), (0.4, BlendMode::Multiply));
    assert_eq!((n.opacity, n.blend), (1.0, BlendMode::Normal));
    run(&mut s, "appearance.setActiveItem", json!({"index": 0}));
    // Opacity and blend go to the active item, isolate stays on the object.
    run(&mut s, "transparency.set", json!({"opacity": 1, "isolate": true}));
    let n = node(&s, id);
    assert!((n.appearance.items[0].opacity() - 0.01).abs() < 1e-6);
    assert!(n.isolate);
    assert_eq!(n.opacity, 1.0);
    run(&mut s, "transparency.set", json!({"item": null, "opacity": 50}));
    assert_eq!(node(&s, id).opacity, 0.5);
    assert!(s.execute("transparency.set", &json!({"item": 9, "opacity": 50})).is_err());
}

#[test]
fn group_rows_are_the_groups_own_stack() {
    let mut s = session();
    let a = rect(&mut s, 100.0);
    let b = rect(&mut s, 250.0);
    run(&mut s, "select.set", json!({"ids": [a.0, b.0]}));
    let g = NodeId(run(&mut s, "object.group", json!({}))["id"].as_u64().unwrap());
    run(&mut s, "appearance.addFill", json!({}));
    assert_eq!(node(&s, g).appearance.items.len(), 1);
    assert_eq!(node(&s, a).appearance.items.len(), 2);
    run(&mut s, "appearance.setActiveItem", json!({"index": 0}));
    run(&mut s, "paint.setFill", json!({"color": "#ff0000"}));
    assert_eq!(fill_paint(&node(&s, g), 0).color().unwrap().to_hex(), "#ff0000");
    assert_eq!(*fill_paint(&node(&s, a), 0), Paint::solid(Color::WHITE));
    // Without a targeted item, a group's fill edit still recolours its contents.
    run(&mut s, "appearance.setActiveItem", json!({"index": null}));
    run(&mut s, "paint.setFill", json!({"color": "#00ff00"}));
    assert_eq!(fill_paint(&node(&s, b), 0).color().unwrap().to_hex(), "#00ff00");
}

#[test]
fn text_items_take_item_paint() {
    let mut s = session();
    let t = NodeId(run(&mut s, "text.create", json!({"x": 10, "y": 50, "text": "Hi"}))["id"].as_u64().unwrap());
    run(&mut s, "select.set", json!({"ids": [t.0]}));
    run(&mut s, "appearance.addFill", json!({}));
    run(&mut s, "paint.setFill", json!({"item": 0, "color": "#ff0000"}));
    let n = node(&s, t);
    assert_eq!(fill_paint(&n, 0).color().unwrap().to_hex(), "#ff0000");
    let vectorcraft_doc::NodeKind::Text(tx) = &n.kind else { panic!() };
    assert_ne!(tx.first_style().fill, Paint::solid(Color::from_hex("#ff0000").unwrap()));
}

#[test]
fn effects_land_on_the_targeted_item() {
    let mut s = session();
    let id = rect(&mut s, 100.0);
    let r = run(&mut s, "effect.apply", json!({"effect": "distort.roughen", "item": 1}));
    assert_eq!((r["index"].clone(), r["item"].clone()), (json!(0), json!(1)));
    let n = node(&s, id);
    assert_eq!(stroke(&n, 1).effects.len(), 1);
    assert!(n.appearance.effects.is_empty() && n.appearance.items[0].effects().is_empty());
    run(&mut s, "edit.undo", json!({}));
    assert!(stroke(&node(&s, id), 1).effects.is_empty());
    assert!(s.execute("effect.apply", &json!({"effect": "distort.roughen", "item": 9})).is_err());
    assert!(s.execute("effect.apply", &json!({"effect": "distort.roughen", "item": -1})).is_err());
    // The active item takes the Effect menu's effects (and its alias).
    run(&mut s, "appearance.setActiveItem", json!({"index": 0}));
    run(&mut s, "effect.apply", json!({"effect": "distort.twist"}));
    run(&mut s, "appearance.addEffect", json!({"effect": "stylize.roundCorners"}));
    let n = node(&s, id);
    assert_eq!(n.appearance.items[0].effects().iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["distort.twist", "stylize.roundCorners"]);
    let l = run(&mut s, "effect.list", json!({}));
    assert_eq!(l["activeItem"], 0);
    assert_eq!(l["applied"][0]["items"][0]["kind"], "fill");
    assert_eq!(l["applied"][0]["items"][0]["effects"].as_array().unwrap().len(), 2);
    // Sub-row eye, duplicate and delete address the item's list.
    run(&mut s, "effect.setParams", json!({"index": 1, "visible": false}));
    run(&mut s, "effect.duplicate", json!({"index": 0}));
    run(&mut s, "effect.remove", json!({"index": 2}));
    let fx = node(&s, id).appearance.items[0].effects().clone();
    assert_eq!(fx.iter().map(|e| (e.id.as_str(), e.visible)).collect::<Vec<_>>(), [("distort.twist", true), ("distort.twist", true)]);
    assert!(s.execute("effect.remove", &json!({"index": 5})).is_err());
    // `item: null` addresses the object's own effects.
    run(&mut s, "effect.apply", json!({"effect": "stylize.dropShadow", "item": null}));
    assert_eq!(node(&s, id).appearance.effects.len(), 1);
}

#[test]
fn per_fill_offset_path_renders() {
    let mut s = session();
    let id = rect(&mut s, 100.0);
    run(&mut s, "paint.setFill", json!({"color": "#ff0000"}));
    run(&mut s, "paint.setStroke", json!({"none": true}));
    let render = |s: &Session| {
        vectorcraft_render::Renderer::new().render(&s.doc().unwrap().doc, 400, 400, vectorcraft_geom::Affine::IDENTITY, &Default::default())
    };
    assert_eq!(render(&s).pixel(90, 150)[3], 0);
    run(&mut s, "effect.apply", json!({"effect": "path.offsetPath", "params": {"offset": 20}, "item": 0}));
    assert_eq!(node(&s, id).appearance.items[0].effects().len(), 1);
    let px = render(&s).pixel(90, 150);
    assert!(px[0] > 200 && px[3] > 200, "{px:?}");
}

#[test]
fn text_fill_items_composite_with_their_opacity() {
    let mut s = session();
    let t = NodeId(run(&mut s, "text.create", json!({"x": 20, "y": 200, "text": "HIM", "size": 150}))["id"].as_u64().unwrap());
    run(&mut s, "select.set", json!({"ids": [t.0]}));
    run(&mut s, "appearance.addFill", json!({}));
    run(&mut s, "appearance.setItem", json!({"index": 0, "color": "#ff0000"}));
    let render = |s: &Session| {
        vectorcraft_render::Renderer::new().render(&s.doc().unwrap().doc, 400, 400, vectorcraft_geom::Affine::IDENTITY, &Default::default())
    };
    let opaque = render(&s);
    // A pixel the red fill covers fully (over the black character fill).
    let (x, y) = (0..400u32).flat_map(|y| (0..400u32).map(move |x| (x, y))).find(|&(x, y)| opaque.pixel(x, y) == [255, 0, 0, 255]).expect("red text");
    run(&mut s, "appearance.setItem", json!({"index": 0, "opacity": 0.5, "blend": "multiply"}));
    let px = render(&s).pixel(x, y);
    // Half of red multiplied over black: black.
    assert!(px[0] < 20 && px[3] == 255, "{px:?}");
    run(&mut s, "appearance.setItem", json!({"index": 0, "blend": "normal"}));
    let px = render(&s).pixel(x, y);
    assert!((110..=145).contains(&px[0]) && px[1] < 10, "{px:?}");
}
