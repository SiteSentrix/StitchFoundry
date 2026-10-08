//! The Home icon shows the Home screen over open documents; Cmd+N opens the New Document dialog.

use serde_json::json;

use crate::{VectorcraftApp, canvas};

fn app_with_doc() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
    app.run("file.new", json!({})).unwrap();
    app
}

/// Draw one canvas frame; whether it drew the document (else the Home screen).
fn shows_document(app: &mut VectorcraftApp) -> bool {
    app.canvas_rect = None;
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| canvas::show(app, ui));
    out.textures_delta.clear();
    app.canvas_rect.is_some()
}

#[test]
fn home_shows_over_open_documents_until_a_document_is_chosen() {
    let mut app = app_with_doc();
    assert!(shows_document(&mut app));
    app.run("app.home", json!({})).unwrap();
    assert!(!shows_document(&mut app), "Home replaces the canvas");
    assert_eq!(app.session.documents().len(), 1, "the document stays open");
    assert!(app.ui.dialog.is_none(), "Home is not the New Document dialog");
    // Choosing the document's tab returns to it.
    app.ui.home = None;
    assert!(shows_document(&mut app));
    // A new document (from Home's presets, New… or Open) replaces Home too.
    app.run("app.home", json!({})).unwrap();
    app.run("file.new", json!({})).unwrap();
    assert!(shows_document(&mut app));
    assert!(app.ui.home.is_none());
}

/// Draw one canvas frame; whether it drew any text (the Home screen's labels; an empty window
/// draws none).
fn draws_text(app: &mut VectorcraftApp) -> bool {
    fn has_text(s: &egui::Shape) -> bool {
        match s {
            egui::Shape::Text(_) => true,
            egui::Shape::Vec(v) => v.iter().any(has_text),
            _ => false,
        }
    }
    app.canvas_rect = None;
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| canvas::show(app, ui));
    out.textures_delta.clear();
    out.shapes.iter().any(|c| has_text(&c.shape))
}

/// General › Show The Home Screen When No Documents Are Open (#394): off, an app with no document
/// shows an empty window instead of the Home screen, and the Home button still opens it.
#[test]
fn home_screen_without_documents_follows_its_preference() {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
    assert!(app.session.active().is_none());
    assert!(draws_text(&mut app), "Home with no document open (the default)");
    assert!(crate::menus::home_showing(&app));
    app.session.execute("prefs.set", &json!({"key": "showHomeScreen", "value": false})).unwrap();
    assert!(!draws_text(&mut app), "off: an empty window");
    assert!(!shows_document(&mut app));
    assert!(!crate::menus::home_showing(&app), "the Home button isn't lit");
    app.run("app.home", json!({})).unwrap();
    assert!(draws_text(&mut app), "the Home button still shows the Home screen");
    app.run("file.new", json!({})).unwrap();
    assert!(shows_document(&mut app));
    assert!(app.ui.home.is_none());
    app.run("file.close", json!({})).unwrap();
    assert!(app.session.active().is_none());
    assert!(!draws_text(&mut app), "closing the last document leaves the window empty");
    app.session.execute("prefs.set", &json!({"key": "showHomeScreen", "value": true})).unwrap();
    assert!(draws_text(&mut app), "on again: Home");
}

#[test]
fn cmd_n_opens_the_new_document_dialog() {
    assert_eq!(crate::shortcut_editor::command_for_key("Cmd+N"), Some("file.newDialog"));
    let mut app = app_with_doc();
    app.run("file.newDialog", json!({})).unwrap();
    assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("newDocument"));
    assert_eq!(app.session.documents().len(), 1, "no document is made until the dialog's OK");
}
