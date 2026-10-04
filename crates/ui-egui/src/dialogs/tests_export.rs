//! The raster options dialogs, drawn headlessly.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::fileio;

use super::*;
use crate::Services;

type Written = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

/// An app with `artboards` 60×40 pt artboards whose save dialog answers `/out/<suggested>` and
/// whose writer records what it is given.
fn app(artboards: usize) -> (VectorcraftApp, Written) {
    let written = Written::default();
    let w = written.clone();
    let services = Services {
        pick_save: Some(Box::new(|name: &str| Some(format!("/out/{name}")))),
        write: Some(Box::new(move |p: &str, b: &[u8]| {
            w.borrow_mut().push((p.to_string(), b.to_vec()));
            Ok(())
        })),
        ..Default::default()
    };
    let mut app = VectorcraftApp::new(Session::new(), services);
    app.run("file.new", json!({"width": 60, "height": 40, "artboards": artboards})).unwrap();
    app.run("paint.setStroke", json!({"none": true})).unwrap();
    app.run("shape.ellipse", json!({"x": 5, "y": 4, "width": 30, "height": 20})).unwrap();
    (app, written)
}

/// One headless frame of the dialog layer.
fn frame(app: &mut VectorcraftApp) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let mut out = ctx.run_ui(Default::default(), |ui| show(app, ui.ctx()));
    out.textures_delta.clear();
}

fn kind(app: &VectorcraftApp) -> Option<&str> {
    app.ui.dialog.as_ref().map(|d| d.kind.as_str())
}

fn set(app: &mut VectorcraftApp, field: &str, value: serde_json::Value) {
    app.ui.dialog.as_mut().expect("a dialog is open").fields.insert(field.into(), value);
}

#[test]
fn png_options_draw_and_export_with_the_chosen_options() {
    let (mut app, written) = app(2);
    png_options::open(&mut app, fileio::format("png").unwrap(), json!({"format": "png", "path": "/out/a.png", "artboard": 1}));
    assert_eq!(kind(&app), Some("pngOptions"));
    let d = app.ui.dialog.as_ref().unwrap();
    assert_eq!((d.f64("ppi", 0.0), d.str("background"), d.str("antiAlias"), d.bool("interlaced")), (72.0, "transparent".into(), "art".into(), false));
    assert_eq!(d.fields["__size"], json!([60.0, 40.0]), "the chosen artboard");
    frame(&mut app);
    // Other resolution and an Other background colour draw their own field and colour button.
    set(&mut app, "__otherPpi", json!(true));
    set(&mut app, "ppi", json!(200));
    set(&mut app, "background", json!("#204080"));
    set(&mut app, "antiAlias", json!("type"));
    set(&mut app, "interlaced", json!(true));
    frame(&mut app);
    assert_eq!(confirm(&mut app).unwrap()["path"], "/out/a.png");
    assert!(app.ui.dialog.is_none());
    let png = written.borrow()[0].1.clone();
    assert_eq!(png[28], 1, "interlaced");
    let phys = png.windows(4).position(|w| w == b"pHYs").unwrap();
    assert_eq!(&png[phys + 4..phys + 8], ((200.0f64 / 0.0254).round() as u32).to_be_bytes());
    let img = image::load_from_memory(&png).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (167, 111), "60 × 40 pt at 200 ppi");
    assert_eq!(img.get_pixel(0, 0).0, [0x20, 0x40, 0x80, 255]);
    // JPEG Options: quality, and no transparent background.
    png_options::open(&mut app, fileio::format("jpg").unwrap(), json!({"format": "jpg", "path": "/out/a.jpg"}));
    assert_eq!(kind(&app), Some("jpgOptions"));
    assert_eq!(app.ui.dialog.as_ref().unwrap().str("background"), "white");
    frame(&mut app);
    set(&mut app, "quality", json!(40));
    confirm(&mut app).unwrap();
    assert_eq!(&written.borrow()[1].1[..2], [0xFF, 0xD8]);
}

#[test]
fn export_as_png_picks_the_file_then_shows_png_options() {
    let (mut app, written) = app(1);
    app.run("file.export.png", json!({})).unwrap();
    assert_eq!(kind(&app), Some("pngOptions"));
    assert_eq!(app.ui.dialog.as_ref().unwrap().str("path"), "/out/Untitled-1.png");
    assert!(written.borrow().is_empty());
    app.ui.dialog = None;
    let r = app.run("file.export.png", json!({"path": "/x/a.png", "ppi": 144})).unwrap();
    assert_eq!(r["path"], "/x/a.png");
    assert_eq!(&written.borrow()[0].1[16..20], 120u32.to_be_bytes(), "60 pt at 144 ppi");
}
