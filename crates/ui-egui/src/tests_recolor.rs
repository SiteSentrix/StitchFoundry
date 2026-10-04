//! Recolor Artwork (M3.78), drawn headlessly.

use egui::{Event, Pos2, Rect, vec2};
use serde_json::{Value, json};
use vectorcraft_engine::Session;

use crate::{VectorcraftApp, dialogs};

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 200})).unwrap();
    app
}

fn context() -> egui::Context {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    ctx
}

fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>, time: f64, draw: fn(&mut VectorcraftApp, &mut egui::Ui)) {
    let screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(900.0, 800.0)));
    let input = egui::RawInput { events, time: Some(time), screen_rect, ..Default::default() };
    let mut out = ctx.run_ui(input, |ui| draw(app, ui));
    out.textures_delta.clear();
}

fn dialog_frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let time = ctx.input(|i| i.time) + 1.0;
    frame(app, ctx, vec![], time, |app, ui| dialogs::show(app, ui.ctx()));
}

/// Rectangles filled with `colors`, all selected.
fn art(app: &mut VectorcraftApp, colors: &[&str]) {
    for (i, c) in colors.iter().enumerate() {
        let id = app.run("shape.rectangle", json!({"x": 10 + 30 * i, "y": 10, "width": 20, "height": 20})).unwrap()["id"].clone();
        app.run("paint.setFill", json!({"ids": [id], "color": c})).unwrap();
        app.run("paint.setStroke", json!({"ids": [id], "none": true})).unwrap();
    }
    app.run("select.all", json!({})).unwrap();
}

fn colors(app: &mut VectorcraftApp) -> usize {
    app.session.execute("recolor.colors", &json!({})).unwrap()["colors"].as_array().unwrap().len()
}

fn field(app: &VectorcraftApp, k: &str) -> Value {
    app.ui.dialog.as_ref().and_then(|d| d.fields.get(k).cloned()).unwrap_or(Value::Null)
}

fn set(app: &mut VectorcraftApp, k: &str, v: Value) {
    app.ui.dialog.as_mut().unwrap().fields.insert(k.into(), v);
}

#[test]
fn the_dialog_reduces_previews_and_applies_as_one_undo_step() {
    let mut app = app();
    let ctx = context();
    art(&mut app, &["#ff0000", "#e01010", "#0000ff", "#1010e0", "#ff3030"]);
    app.run("ui.recolorDialog", json!({})).unwrap();
    dialog_frame(&mut app, &ctx);
    assert_eq!(field(&app, "rows").as_array().unwrap().len(), 5, "Auto: a row per colour");
    assert!(app.session.in_interaction(), "the dialog previews");
    // Two colours, Exact; OK without another frame still reduces first.
    set(&mut app, "colors", json!(2));
    set(&mut app, "method", json!("exact"));
    let undo = app.session.doc().unwrap().history.undo.len();
    dialogs::confirm(&mut app).unwrap();
    assert!(app.ui.dialog.is_none());
    assert_eq!(colors(&mut app), 2);
    assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 1);
    app.run("edit.undo", json!({})).unwrap();
    assert_eq!(colors(&mut app), 5);
    // Cancel rolls the preview back.
    app.run("ui.recolorDialog", json!({"colors": 1})).unwrap();
    set(&mut app, "method", json!("exact"));
    dialog_frame(&mut app, &ctx);
    assert_eq!(colors(&mut app), 1, "previewed");
    dialogs::cancel(&mut app);
    assert_eq!(colors(&mut app), 5);
}

#[test]
fn presets_open_colour_jobs_and_libraries_and_both_tabs_draw() {
    let mut app = app();
    let ctx = context();
    art(&mut app, &["#ff0000", "#00ff00", "#0000ff"]);
    let entries = crate::menus::menu_entries(&app);
    let presets: Vec<(&str, &Value)> =
        entries.iter().filter(|e| e.path.last().is_some_and(|p| p == "Recolor with Preset")).map(|e| (e.label.as_str(), &e.params)).collect();
    assert_eq!(presets.iter().map(|p| p.0).collect::<Vec<_>>(), ["1 Color Job…", "2 Color Job…", "3 Color Job…", "Color Library…"]);
    app.run("ui.recolorDialog", presets[1].1.clone()).unwrap();
    dialog_frame(&mut app, &ctx);
    assert_eq!((field(&app, "colors"), field(&app, "method")), (json!(2), json!("scaleTints")));
    assert_eq!(field(&app, "rows").as_array().unwrap().len(), 2);
    set(&mut app, "tab", json!("edit"));
    dialog_frame(&mut app, &ctx);
    dialogs::cancel(&mut app);
    app.run("ui.recolorDialog", presets[3].1.clone()).unwrap();
    assert_eq!(field(&app, "limitTo"), "web-safe-216", "Color Library starts on the first library");
    dialog_frame(&mut app, &ctx);
    let web = |k: &Value| vectorcraft_engine::cmd::color_value(k).unwrap().to_rgb().iter().all(|v| ((v * 5.0).round() - v * 5.0).abs() < 1e-3);
    assert!(field(&app, "rows").as_array().unwrap().iter().all(|r| web(&r["to"])));
    dialogs::cancel(&mut app);
    // The Color Guide's colours become the new colours.
    app.run("ui.recolorDialog", json!({"colors": ["#123456", "#abcdef"]})).unwrap();
    dialog_frame(&mut app, &ctx);
    let tos: Vec<Value> = field(&app, "rows").as_array().unwrap().iter().map(|r| r["to"].clone()).collect();
    assert_eq!(tos, [json!("rgb 18 52 86"), json!("rgb 171 205 239")]);
    dialogs::cancel(&mut app);
}
