//! The File menu around saving, in the app: Revert's confirmation.

use serde_json::json;
use vectorcraft_engine::Session;

use crate::{Services, VectorcraftApp, dialogs, menus, theme};

/// An app that reads and writes real files.
fn app() -> VectorcraftApp {
    let services = Services {
        read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))),
        write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))),
        ..Default::default()
    };
    VectorcraftApp::new(Session::new(), services)
}

/// A fresh folder for one test.
fn dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vc-ui-save-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rect(app: &mut VectorcraftApp) {
    app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 30, "height": 20})).unwrap();
}

fn objects(app: &VectorcraftApp) -> usize {
    app.session.doc().unwrap().doc.node_count()
}

/// One headless frame of the dialog layer with Enter pressed (OK).
fn press_ok(app: &mut VectorcraftApp) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let key = egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
    let mut out = ctx.run_ui(egui::RawInput { events: vec![key], ..Default::default() }, |ui| dialogs::show(app, ui.ctx()));
    out.textures_delta.clear();
}

#[test]
fn revert_asks_then_reloads_in_the_same_tab_keeping_every_zoom() {
    let d = dir("revert");
    let path = d.join("a.vectorcraft").to_string_lossy().to_string();
    let mut app = app();
    app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    rect(&mut app);
    app.run("file.saveAs", json!({"path": path})).unwrap();
    assert!(!menus::enabled(&app, "file.revert"), "nothing changed since the save");
    app.run("file.new", json!({"width": 50, "height": 50})).unwrap();
    app.views[0].zoom = 2.0;
    app.views[1].zoom = 4.0;
    app.session.set_active(0);
    rect(&mut app);
    assert!(menus::enabled(&app, "file.revert"));
    let r = app.run("file.revert", json!({})).unwrap();
    assert_eq!(r["pending"], dialogs::confirm::KIND);
    assert_eq!(objects(&app), 3, "nothing happens before OK");
    press_ok(&mut app);
    assert!(app.ui.dialog.is_none());
    assert_eq!((objects(&app), app.session.active_index(), app.session.documents().len()), (2, Some(0), 2));
    assert!(!app.session.active().unwrap().is_dirty());
    assert_eq!((app.views[0].zoom, app.views[1].zoom), (2.0, 4.0));
    // Cancel leaves the changes.
    rect(&mut app);
    app.run("file.revert", json!({})).unwrap();
    app.ui.dialog = None;
    assert_eq!(objects(&app), 3);
    // An untitled document can't revert.
    app.session.set_active(1);
    rect(&mut app);
    assert!(app.run("file.revert", json!({})).is_err() && app.ui.dialog.is_none());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn new_from_template_takes_bytes_from_agents() {
    let mut app = app();
    app.run("file.new", json!({})).unwrap();
    let bytes = app.session.execute("document.serialize", &json!({"format": "template"})).unwrap();
    let r = app.run("file.newFromTemplate", json!({"name": "flyer.vctemplate", "dataBase64": bytes["dataBase64"]})).unwrap();
    assert!(r["title"].as_str().is_some_and(|t| t.starts_with("Untitled-")), "{r}");
    assert_eq!((app.session.documents().len(), app.views.len()), (2, 2));
    assert_eq!(app.session.active().unwrap().path, None);
}
