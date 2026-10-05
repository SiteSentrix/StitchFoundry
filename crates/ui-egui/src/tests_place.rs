//! File → Place in the app: the Place dialog and the Control bar's image details.

use serde_json::{Value, json};
use vectorcraft_doc::NodeKind;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;

/// A `w`×`h` red PNG declaring `ppi`.
fn png(w: u32, h: u32, ppi: f64) -> Vec<u8> {
    let mut out = vec![];
    image::RgbaImage::from_pixel(w, h, image::Rgba([255, 0, 0, 255])).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    vectorcraft_engine::cmd::fileio::ppi::with_png_resolution(&out, (ppi, ppi))
}

/// `bytes` written to a fresh temporary file named `name` → its path.
fn temp_file(name: &str, bytes: &[u8]) -> String {
    let dir = std::env::temp_dir().join(format!("vectorcraft-ui-place-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path.to_string_lossy().to_string()
}

/// An app reading files from disk, with a 400×300 document.
fn app() -> VectorcraftApp {
    let services = crate::Services { read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))), ..Default::default() };
    let mut app = VectorcraftApp::new(Session::new(), services);
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app
}

/// File → Place… with `path` chosen in the file picker.
fn place_picked(app: &mut VectorcraftApp, path: &str) {
    let path = path.to_string();
    app.services.pick_open = Some(Box::new(move || Some(path.clone())));
    app.run("file.place", json!({})).unwrap();
}

fn selected_image(app: &VectorcraftApp) -> vectorcraft_doc::Node {
    let st = app.session.active().unwrap();
    let n = st.doc.node(st.selection.objects[0]).unwrap().clone();
    assert!(matches!(n.kind, NodeKind::Image(_)), "{:?}", n.kind);
    n
}

#[test]
fn one_file_places_centred_in_the_view_and_replace_swaps_the_selection() {
    let mut app = app();
    app.view_mut().unwrap().center = vectorcraft_geom::Point::new(120.0, 80.0);
    place_picked(&mut app, &temp_file("one.png", &png(300, 150, 300.0)));
    let d = app.ui.dialog.as_ref().expect("the Place dialog");
    assert_eq!(d.kind, crate::dialogs::place::KIND);
    assert!(d.bool("link") && !d.bool("__replace"), "Link on; Replace needs one selected object");
    assert_eq!(d.fields["__info"][0], "300 × 150 px, 300 ppi, RGB (72 pt × 36 pt)");
    let text = crate::tests_labels::painted_text(&mut app, |app, ui| crate::dialogs::show(app, ui.ctx()));
    for label in ["Place", "one.png", "Link", "Template", "Replace", "Cancel"] {
        assert!(text.contains(label), "{label} in {text}");
    }
    crate::dialogs::confirm(&mut app).unwrap();
    assert!(app.ui.dialog.is_none());
    let first = selected_image(&app);
    assert_eq!(first.geometric_bounds().unwrap().center(), vectorcraft_geom::Point::new(120.0, 80.0), "centred in the view");
    // With one object selected, Replace applies.
    place_picked(&mut app, &temp_file("two.png", &png(40, 40, 144.0)));
    assert!(app.ui.dialog.as_ref().unwrap().bool("__replace"));
    app.ui.dialog.as_mut().unwrap().fields.insert("replace".into(), json!(true));
    app.ui.dialog.as_mut().unwrap().fields.insert("link".into(), json!(false));
    crate::dialogs::confirm(&mut app).unwrap();
    let second = selected_image(&app);
    assert_eq!(second.name.as_deref(), Some("two.png"));
    assert_eq!(app.session.active().unwrap().doc.layers[0].children().unwrap().len(), 1, "replaced");
    assert!(!app.ui.place_link, "Link is remembered");
}

#[test]
fn the_control_bar_shows_the_image_file_link_colour_mode_and_ppi() {
    let mut app = app();
    let path = temp_file("photo.png", &png(30, 30, 300.0));
    app.run("file.place", json!({"path": path})).unwrap();
    let text = crate::tests_labels::painted_text(&mut app, crate::chrome::control_bar);
    for s in ["Linked File", "photo.png", "RGB   PPI: 300"] {
        assert!(text.contains(s), "{s} in {text}");
    }
    let v: Value = app.run("file.place", json!({"path": path, "link": false})).unwrap();
    assert_eq!(v["linked"], false);
    let text = crate::tests_labels::painted_text(&mut app, crate::chrome::control_bar);
    assert!(text.contains("Embedded"), "{text}");
}
