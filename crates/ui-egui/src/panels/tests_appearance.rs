//! Headless frames of the Appearance panel and the panels that follow its active item.

use serde_json::{Value, json};
use vectorcraft_doc::Unit;
use vectorcraft_engine::Session;

use super::*;

fn app_with_rect() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    run(&mut app, "file.new", json!({"width": 200, "height": 200}));
    let id = run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 100}))["id"].clone();
    run(&mut app, "select.set", json!({ "ids": [id] }));
    app
}

fn run(app: &mut VectorcraftApp, id: &str, p: Value) -> Value {
    app.session.execute(id, &p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

/// Every text drawn while `f` runs one headless frame.
fn frame_texts(app: &mut VectorcraftApp, f: impl FnMut(&mut VectorcraftApp, &mut egui::Ui)) -> Vec<String> {
    let ctx = egui::Context::default();
    frame_in(&ctx, app, f)
}

fn frame_in(ctx: &egui::Context, app: &mut VectorcraftApp, mut f: impl FnMut(&mut VectorcraftApp, &mut egui::Ui)) -> Vec<String> {
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| f(app, ui));
    out.textures_delta.clear();
    fn collect(s: &egui::Shape, out: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let mut texts = vec![];
    for c in &out.shapes {
        collect(&c.shape, &mut texts);
    }
    texts
}

#[test]
fn stroke_panel_shows_the_active_rows_weight() {
    let mut app = app_with_rect();
    run(&mut app, "appearance.addStroke", json!({})); // [Fill, Stroke 1 pt, Stroke 1 pt]
    run(&mut app, "stroke.set", json!({"item": 1, "weight": 6}));
    let six = Unit::Points.format(6.0);
    let texts = frame_texts(&mut app, stroke::show);
    assert!(!texts.contains(&six), "top stroke shown: {texts:?}");
    // Clicking the lower stroke row in the Appearance panel targets it.
    let ctx = egui::Context::default();
    frame_in(&ctx, &mut app, appearance::show);
    appearance::select_row(&mut app, &ctx, appearance::Sel::Item(1));
    assert_eq!(app.session.appearance_item(), Some(1));
    assert!(frame_texts(&mut app, stroke::show).contains(&six));
    let (_, stroke) = current_paints(&app);
    assert_eq!(stroke, current_stroke(&app).unwrap().paint);
    // The Appearance panel and the proxies draw with the row active; the object row clears it.
    frame_in(&ctx, &mut app, |app, ui| {
        appearance::show(app, ui);
        crate::toolbar::show(app, ui);
        transparency::show(app, ui);
        properties::show(app, ui);
    });
    appearance::select_row(&mut app, &ctx, appearance::Sel::None);
    assert_eq!(app.session.appearance_item(), None);
    assert!(!frame_texts(&mut app, stroke::show).contains(&six));
}

#[test]
fn transparency_panel_reads_the_active_item() {
    let mut app = app_with_rect();
    run(&mut app, "transparency.set", json!({"item": 0, "opacity": 40}));
    assert_eq!(current_transparency(&app).unwrap().0, 1.0);
    run(&mut app, "appearance.setActiveItem", json!({"index": 0}));
    assert!((current_transparency(&app).unwrap().0 - 0.4).abs() < 1e-6);
    assert!(frame_texts(&mut app, transparency::show).iter().any(|t| t.starts_with("40")));
}
