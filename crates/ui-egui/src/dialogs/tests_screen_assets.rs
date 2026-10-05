//! Export for Screens' Assets tab, drawn headlessly: the checked assets, the OK label, desktop
//! folders and web downloads.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};
use vectorcraft_engine::Session;

use super::tests_export::{frame, kind, set};
use super::*;
use crate::Services;
use crate::tests_labels::painted_text;

type Log = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// An app with two unstroked squares (selected); `web` gives it a download service (recorded in
/// the log) instead of a file manager.
fn app(web: bool) -> (VectorcraftApp, Log, [u64; 2]) {
    let log = Log::default();
    let l = log.clone();
    let mut services = Services::default();
    if web {
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| l.borrow_mut().push((name.to_string(), bytes.to_vec()))));
    } else {
        services.reveal = Some(Box::new(move |path: &str| {
            l.borrow_mut().push((path.to_string(), vec![]));
            Ok(())
        }));
    }
    let mut app = VectorcraftApp::new(Session::new(), services);
    app.run("file.new", json!({"width": 200, "height": 100})).unwrap();
    app.session.paint.stroke = vectorcraft_color::Paint::None;
    let mut rect = |x: f64| app.run("shape.rectangle", json!({"x": x, "y": 10, "width": 30, "height": 20})).unwrap()["id"].as_u64().unwrap();
    let ids = [rect(10.0), rect(100.0)];
    app.run("select.set", json!({"ids": ids})).unwrap();
    (app, log, ids)
}

fn field(app: &VectorcraftApp, k: &str) -> Value {
    app.ui.dialog.as_ref().expect("a dialog is open").fields.get(k).cloned().unwrap_or(Value::Null)
}

fn assets(app: &mut VectorcraftApp) -> Vec<Value> {
    app.run("assets.list", json!({})).unwrap()["assets"].as_array().unwrap().clone()
}

#[test]
fn the_assets_tab_exports_the_checked_assets_into_the_folder() {
    let (mut app, log, _) = app(false);
    app.run("assets.add", json!({})).unwrap();
    let ids: Vec<u64> = assets(&mut app).iter().map(|a| a["id"].as_u64().unwrap()).collect();
    app.run("file.exportForScreens", json!({})).unwrap();
    assert_eq!((field(&app, "tab"), field(&app, "assets")), (json!("artboards"), json!(ids)), "every asset checked");
    assert_eq!((spec(export_for_screens::KIND).ok_label.unwrap())(&app), "Export Artboard");
    set(&mut app, "tab", json!("assets"));
    assert_eq!((spec(export_for_screens::KIND).ok_label.unwrap())(&app), "Export Asset");
    frame(&mut app);
    // Nothing checked keeps the dialog open.
    set(&mut app, "assets", json!([]));
    assert!(confirm(&mut app).is_err());
    assert_eq!(kind(&app), Some(export_for_screens::KIND));
    let dir = std::env::temp_dir().join(format!("vc-efs-assets-{}", std::process::id()));
    let folder = dir.to_string_lossy().replace('\\', "/");
    set(&mut app, "folder", json!(folder));
    set(&mut app, "assets", json!([ids[1], 999]));
    set(&mut app, "formats", json!([{"format": "png", "scale": "2x", "suffix": "@2x"}]));
    let r = confirm(&mut app).unwrap();
    assert_eq!(r["files"], json!([format!("{folder}/Asset-2@2x.png")]));
    let png = image::load_from_memory(&std::fs::read(dir.join("Asset-2@2x.png")).unwrap()).unwrap();
    assert_eq!((png.width(), png.height()), (60, 40), "the asset's art at 2x");
    assert_eq!(log.borrow().len(), 1, "Open Location shows the file");
    // The rows are remembered for the dialog and the Asset Export panel alike.
    assert_eq!(app.run("document.exportSettings", json!({})).unwrap()["settings"]["formats"][0]["scale"], "2x");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn an_empty_assets_tab_says_how_to_collect_art() {
    let (mut app, _, _) = app(false);
    app.run("file.exportForScreens", json!({})).unwrap();
    set(&mut app, "tab", json!("assets"));
    let text = painted_text(&mut app, |app, ui| super::show(app, ui.ctx()));
    assert!(text.contains("Collect for Export"), "{text}");
}

#[test]
fn the_menus_reach_asset_export() {
    let (mut app, _, _) = app(false);
    let entries = crate::menus::menu_entries(&app);
    let find = |cmd: &str, label: &str| entries.iter().find(|e| e.command.as_deref() == Some(cmd) && e.label == label).map(|e| e.path.clone());
    assert_eq!(find("assets.add", "As Single Asset"), Some(vec!["Object".to_string(), "Collect for Export".to_string()]));
    assert_eq!(find("assets.add", "As Multiple Assets"), Some(vec!["Object".to_string(), "Collect for Export".to_string()]));
    let window = entries.iter().find(|e| e.label == "Asset Export").unwrap();
    assert_eq!(window.params, json!({"panel": crate::panels::asset_export::ID}));
    app.run("window.panel", window.params.clone()).unwrap();
    assert_eq!(app.ui.open_panel.as_deref(), Some(crate::panels::asset_export::ID));
    app.run("assets.add", json!({"multiple": false})).unwrap();
    assert_eq!(assets(&mut app).len(), 1);
}
