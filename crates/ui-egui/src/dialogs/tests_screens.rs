//! Export for Screens, drawn headlessly: desktop folders and web downloads.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::fileio;

use super::tests_export::{frame, kind, set};
use super::*;
use crate::Services;

type Log = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// An app with `artboards` 60×40 pt artboards and an ellipse on the first; `web` gives it a
/// download service (recorded in the log) instead of a folder picker.
fn app(artboards: usize, web: bool) -> (VectorcraftApp, Log) {
    let log = Log::default();
    let l = log.clone();
    let mut services = Services::default();
    if web {
        services.download = Some(Box::new(move |name: &str, bytes: &[u8]| l.borrow_mut().push((name.to_string(), bytes.to_vec()))));
    } else {
        services.pick_folder = Some(Box::new(|| None));
    }
    let mut app = VectorcraftApp::new(Session::new(), services);
    app.run("file.new", json!({"width": 60, "height": 40, "artboards": artboards})).unwrap();
    app.run("shape.ellipse", json!({"x": 5, "y": 4, "width": 30, "height": 20})).unwrap();
    (app, log)
}

fn field(app: &VectorcraftApp, k: &str) -> Value {
    app.ui.dialog.as_ref().expect("a dialog is open").fields.get(k).cloned().unwrap_or(Value::Null)
}

fn open(app: &mut VectorcraftApp) {
    app.run("file.exportForScreens", json!({})).unwrap();
    assert_eq!(kind(app), Some(export_for_screens::KIND));
}

#[test]
fn exports_into_the_folder() {
    let (mut app, _) = app(2, false);
    open(&mut app);
    assert_eq!(field(&app, "folder"), json!(fileio::export_folder().unwrap_or_default()), "the Desktop, else home");
    assert_eq!((spec(export_for_screens::KIND).ok_label.unwrap())(&app), "Export Artboard");
    frame(&mut app);
    let dir = std::env::temp_dir().join(format!("vc-efs-desk-{}", std::process::id()));
    let folder = dir.to_string_lossy().replace('\\', "/");
    set(&mut app, "boards", json!([false, true]));
    set(&mut app, "formats", json!([{"format": "png", "scale": 1, "suffix": ""}, {"format": "svg", "scale": 1, "suffix": ""}]));
    // No folder keeps the dialog open.
    set(&mut app, "folder", json!(" "));
    assert!(confirm(&mut app).is_err());
    assert_eq!(kind(&app), Some(export_for_screens::KIND));
    set(&mut app, "folder", json!(folder));
    let r = confirm(&mut app).unwrap();
    assert!(app.ui.dialog.is_none());
    let files: Vec<&str> = r["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert_eq!(files, [format!("{folder}/Artboard-2.png"), format!("{folder}/Artboard-2.svg")]);
    assert!(std::fs::metadata(dir.join("Artboard-2.png")).unwrap().len() > 50);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn the_web_downloads_one_zip_or_the_file() {
    let (mut app, log) = app(2, true);
    assert_eq!((spec(export_for_screens::KIND).ok_label.unwrap())(&app), "Download");
    open(&mut app);
    frame(&mut app);
    set(&mut app, "formats", json!([{"format": "png", "scale": 1, "suffix": ""}, {"format": "jpg", "scale": 2, "suffix": "@2x"}]));
    let r = confirm(&mut app).unwrap();
    assert!(r.get("dataBase64").is_some(), "four files: one zip");
    {
        let log = log.borrow();
        let [(name, zip)] = log.as_slice() else { panic!("one download: {:?}", log.iter().map(|(n, _)| n).collect::<Vec<_>>()) };
        assert_eq!(name, "Untitled-1.zip");
        assert_eq!(&vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap(), zip);
    }
    assert_eq!(r["files"], json!(["Artboard-1.png", "Artboard-1@2x.jpg", "Artboard-2.png", "Artboard-2@2x.jpg"]));
    // One file downloads as itself.
    log.borrow_mut().clear();
    open(&mut app);
    set(&mut app, "boards", json!([true, false]));
    set(&mut app, "formats", json!([{"format": "png", "scale": 1, "suffix": ""}]));
    confirm(&mut app).unwrap();
    let log = log.borrow();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].0, "Artboard-1.png");
    assert_eq!(image::load_from_memory(&log[0].1).unwrap().width(), 60);
}
