//! The canvas draws the transparency grid behind the artboards of the documents that show it
//! (M3.88), and the View menu item follows the active document.

use serde_json::json;
use vectorcraft_engine::Session;

use crate::{VectorcraftApp, canvas, menus};

/// Whether one headless canvas frame paints the grid's grey squares.
fn draws_grid(app: &mut VectorcraftApp) -> bool {
    fn grey(s: &egui::Shape) -> bool {
        match s {
            egui::Shape::Rect(r) => r.fill == egui::Color32::from_gray(204),
            egui::Shape::Vec(v) => v.iter().any(grey),
            _ => false,
        }
    }
    let ctx = egui::Context::default();
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| canvas::show(app, ui));
    out.textures_delta.clear();
    out.shapes.iter().any(|c| grey(&c.shape))
}

#[test]
fn the_canvas_shows_the_grid_only_for_documents_that_turn_it_on() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
    app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
    assert!(!draws_grid(&mut app));
    assert_eq!(menus::dynamic_label(&app, "view.transparencyGrid", ""), "Show Transparency Grid");
    // The menu's id reaches the engine command.
    assert_eq!(app.run("view.transparencyGrid", json!({})).unwrap(), json!({"on": true}));
    assert!(draws_grid(&mut app));
    assert_eq!(menus::dynamic_label(&app, "view.transparencyGrid", ""), "Hide Transparency Grid");
    app.run("document.activate", json!({"index": 0})).unwrap();
    assert!(!draws_grid(&mut app), "the other document keeps its own setting");
    assert_eq!(menus::dynamic_label(&app, "view.transparencyGrid", ""), "Show Transparency Grid");
    app.run("document.activate", json!({"index": 1})).unwrap();
    assert!(draws_grid(&mut app));
}
