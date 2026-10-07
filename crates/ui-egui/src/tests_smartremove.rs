//! Smart Remove Anchor Points: a command for direct-selected anchors, not a Pen click.

use egui::{Event, PointerButton, Pos2, Rect, Shape, vec2};
use serde_json::json;
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;
use vectorcraft_geom::Point;

use crate::canvas::Xf;
use crate::menus::{self, Item};
use crate::{VectorcraftApp, canvas, chrome};

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app
}

fn rect(app: &mut VectorcraftApp) -> NodeId {
    NodeId(app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 100, "height": 80})).unwrap()["id"].as_u64().unwrap())
}

fn anchor_count(app: &VectorcraftApp, id: NodeId) -> usize {
    app.session.active().and_then(|st| st.doc.node(id)).and_then(|n| n.path_data()).map(|p| p.anchor_count()).unwrap_or(0)
}

fn labels(items: &[Item]) -> Vec<&'static str> {
    items
        .iter()
        .flat_map(|it| match it {
            Item::Cmd(l, ..) => vec![*l],
            Item::Sub(l, ch) => std::iter::once(*l).chain(labels(ch)).collect(),
            _ => vec![],
        })
        .collect()
}

fn shapes_text(shapes: &[Shape]) -> Vec<(String, Rect)> {
    fn walk(s: &Shape, v: &mut Vec<(String, Rect)>) {
        match s {
            Shape::Text(t) => v.push((t.galley.text().to_string(), Rect::from_min_size(t.pos, t.galley.size()))),
            Shape::Vec(s) => s.iter().for_each(|s| walk(s, v)),
            _ => {}
        }
    }
    let mut v = vec![];
    shapes.iter().for_each(|s| walk(s, &mut v));
    v
}

fn control_frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>) -> Vec<(String, Rect)> {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(1400.0, 900.0));
    let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| chrome::control_bar(app, ui));
    out.textures_delta.clear();
    shapes_text(&out.shapes.iter().map(|c| c.shape.clone()).collect::<Vec<_>>())
}

fn click_control(app: &mut VectorcraftApp, ctx: &egui::Context, at: Pos2) {
    let press = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    control_frame(app, ctx, vec![Event::PointerMoved(at), press(true)]);
    control_frame(app, ctx, vec![press(false)]);
    control_frame(app, ctx, vec![]);
}

fn canvas_frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>) -> Vec<(String, Rect)> {
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0));
    let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), events, ..Default::default() }, |ui| canvas::show(app, ui));
    out.textures_delta.clear();
    shapes_text(&out.shapes.iter().map(|c| c.shape.clone()).collect::<Vec<_>>())
}

fn click_canvas(app: &mut VectorcraftApp, ctx: &egui::Context, at: Pos2, button: PointerButton) -> Vec<(String, Rect)> {
    let press = |pressed| Event::PointerButton { pos: at, button, pressed, modifiers: Default::default() };
    canvas_frame(app, ctx, vec![Event::PointerMoved(at)]);
    canvas_frame(app, ctx, vec![press(true)]);
    canvas_frame(app, ctx, vec![press(false)]);
    canvas_frame(app, ctx, vec![])
}

fn properties_frame(app: &mut VectorcraftApp, width: f32) -> Vec<(String, Rect)> {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let screen = Rect::from_min_size(Pos2::ZERO, vec2(width, 900.0));
    let mut out = ctx.run_ui(egui::RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| {
        crate::panels::properties::show(app, ui);
    });
    out.textures_delta.clear();
    shapes_text(&out.shapes.iter().map(|c| c.shape.clone()).collect::<Vec<_>>())
}

fn has(texts: &[(String, Rect)], label: &str) -> bool {
    texts.iter().any(|(t, _)| t == label)
}

fn at(texts: &[(String, Rect)], label: &str) -> Pos2 {
    texts.iter().find(|(t, _)| t == label).map(|(_, r)| r.center()).unwrap_or_else(|| panic!("no `{label}`"))
}

#[test]
fn the_path_menu_and_bars_run_it_for_direct_selected_anchors() {
    let mut app = app();
    let id = rect(&mut app);
    assert_eq!(anchor_count(&app, id), 4);
    let entry = menus::menu_entries(&app).into_iter().find(|e| e.command.as_deref() == Some("path.smartRemoveAnchor")).unwrap();
    assert_eq!(entry.path, ["Object", "Path"]);
    assert_eq!(entry.label, "Smart Remove Anchor Points");
    assert!(!entry.enabled, "an object selection is not enough");
    assert!(!labels(&menus::context_items(&app)).contains(&"Smart Remove Anchor Points"));
    assert!(crate::palette::items().iter().any(|(_, cmd, _)| cmd == "path.smartRemoveAnchor"));

    app.select_tool("directSelection");
    app.run("select.anchors", json!({"id": id.0, "anchors": [[0, 1]]})).unwrap();
    assert!(menus::enabled(&app, "path.smartRemoveAnchor"));
    assert!(labels(&menus::context_items(&app)).contains(&"Smart Remove Anchor Points"));
    assert!(menus::menu_strings().contains("Smart Remove Anchor Points"));

    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let bar = control_frame(&mut app, &ctx, vec![]);
    assert!(has(&bar, "Smart Remove Anchor Points"), "control bar: {bar:?}");
    let label_w = bar.iter().find(|(t, _)| t == "Smart Remove Anchor Points").unwrap().1.width();
    assert!(label_w <= 196.0, "the Control bar button clips the label ({label_w} pt)");
    click_control(&mut app, &ctx, at(&bar, "Smart Remove Anchor Points"));
    assert_eq!(anchor_count(&app, id), 3, "the Control bar button removes the anchor");
    assert_eq!(app.session.active().unwrap().history.undo.last().unwrap().label, "Smart Remove Anchor Point");
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(anchor_count(&app, id), 4);

    app.run("select.anchors", json!({"id": id.0, "anchors": [[0, 1]]})).unwrap();
    // 230 pt is the dock's minimum width. The label has to sit inside that row.
    let props = properties_frame(&mut app, 230.0);
    let row = props.iter().find(|(t, _)| t == "Smart Remove Anchor Points").expect("properties");
    assert!(row.1.min.x >= -0.5 && row.1.max.x <= 230.5, "properties clips the label: {:?}", row.1);

    // The task bar's area settles on the second frame.
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    canvas_frame(&mut app, &ctx, vec![]);
    let texts = canvas_frame(&mut app, &ctx, vec![]);
    assert!(has(&texts, "Smart Remove Anchor Points"), "task bar: {texts:?}");

    // Right-click keeps the anchor selection (the object is already selected) and runs the item.
    let p = Xf::new(app.canvas_rect.unwrap(), app.view().unwrap()).to_screen(Point::new(100.0, 90.0));
    let texts = click_canvas(&mut app, &ctx, p, PointerButton::Secondary);
    assert!(has(&texts, "Smart Remove Anchor Points"), "context menu: {texts:?}");
    let texts = click_canvas(&mut app, &ctx, at(&texts, "Smart Remove Anchor Points"), PointerButton::Primary);
    assert_eq!(anchor_count(&app, id), 3);
    assert!(!has(&texts, "Smart Remove Anchor Points"), "the menu closes after the command");

    app.select_tool("pen");
    let hint = crate::tests_labels::painted_text(&mut app, chrome::hint_bar);
    assert!(!hint.contains("Smart Remove") && !hint.contains("keep the shape"), "pen hint: {hint}");
}

#[test]
fn the_control_bar_hides_the_button_without_anchors() {
    let mut app = app();
    rect(&mut app);
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let bar = control_frame(&mut app, &ctx, vec![]);
    assert!(!has(&bar, "Smart Remove Anchor Points"), "{bar:?}");
    let props = crate::tests_labels::painted_text(&mut app, crate::panels::properties::show);
    assert!(!props.contains("Smart Remove Anchor Points"), "{props}");
}
