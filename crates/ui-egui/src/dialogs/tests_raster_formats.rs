//! The options of the raster formats beyond PNG, drawn headlessly: JPEG.

use serde_json::json;

use super::tests_export::{app, frame, kind, set};
use super::*;

/// A JPEG's SOF marker and component count.
fn jpeg_frame(file: &[u8]) -> (u8, u8) {
    let mut i = 2;
    while !(0xC0..=0xC2).contains(&file[i + 1]) {
        i += 2 + u16::from_be_bytes([file[i + 2], file[i + 3]]) as usize;
    }
    (file[i + 1], file[i + 9])
}

#[test]
fn jpeg_options_pick_the_colour_model_method_and_profile() {
    let (mut app, written) = app(1);
    app.run("file.exportAs", json!({"format": "jpg"})).unwrap();
    confirm(&mut app).unwrap();
    assert_eq!(kind(&app), Some("jpgOptions"));
    let d = app.ui.dialog.as_ref().unwrap();
    assert_eq!(
        (d.str("colorModel"), d.str("method"), d.f64("scans", 0.0), d.bool("embedIcc"), d.str("imageMap")),
        ("rgb".into(), "baseline".into(), 3.0, true, "none".into())
    );
    frame(&mut app);
    set(&mut app, "method", json!("progressive"));
    set(&mut app, "scans", json!(4));
    set(&mut app, "colorModel", json!("cmyk"));
    set(&mut app, "quality", json!(30));
    frame(&mut app);
    confirm(&mut app).unwrap();
    let file = written.borrow()[0].1.clone();
    assert_eq!(jpeg_frame(&file), (0xC2, 4), "progressive CMYK");
    assert!(file.windows(12).any(|w| w == b"ICC_PROFILE\0"));

    // A CMYK document exports CMYK by default.
    app.run("object.convertDocumentColorMode", json!({"mode": "cmyk"})).unwrap();
    app.run("file.exportAs", json!({"format": "jpg"})).unwrap();
    confirm(&mut app).unwrap();
    assert_eq!(app.ui.dialog.as_ref().unwrap().str("colorModel"), "cmyk");
}
