//! The options of the raster formats beyond PNG, drawn headlessly: JPEG, PNG-8 and GIF.

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

#[test]
fn palette_options_reduce_the_colours() {
    let (mut app, written) = app(1);
    app.run("file.exportAs", json!({"format": "gif"})).unwrap();
    confirm(&mut app).unwrap();
    assert_eq!(kind(&app), Some("gifOptions"));
    let d = app.ui.dialog.as_ref().unwrap();
    assert_eq!((spec(&d.kind).heading)(d), "GIF Options");
    assert_eq!(
        (d.str("reduction"), d.f64("colors", 0.0), d.str("dither"), d.bool("transparency"), d.str("matte")),
        ("selective".into(), 256.0, "diffusion".into(), true, "white".into())
    );
    frame(&mut app);
    set(&mut app, "colors", json!(4));
    set(&mut app, "dither", json!("pattern"));
    set(&mut app, "matte", json!("#336699"));
    set(&mut app, "interlaced", json!(true));
    frame(&mut app);
    confirm(&mut app).unwrap();
    let (path, gif) = written.borrow()[0].clone();
    assert_eq!(path, "/out/Untitled-1.gif");
    assert_eq!(&gif[..6], b"GIF89a");
    assert_eq!(gif[10] & 0x07, 1, "a global colour table of 4 entries");

    app.run("file.exportAs", json!({"format": "png8"})).unwrap();
    confirm(&mut app).unwrap();
    assert_eq!(kind(&app), Some("png8Options"));
    set(&mut app, "reduction", json!("gray"));
    frame(&mut app);
    confirm(&mut app).unwrap();
    let (path, png) = written.borrow()[1].clone();
    assert_eq!(path, "/out/Untitled-1.png");
    assert_eq!(png[25], 3, "indexed");
}
