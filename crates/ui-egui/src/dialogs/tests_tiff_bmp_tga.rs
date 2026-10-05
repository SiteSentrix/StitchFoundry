//! TIFF Options through Export As, drawn headlessly.

use serde_json::json;

use super::tests_export::{app, frame, kind, set};
use super::*;

#[test]
fn tiff_options_pick_the_model_byte_order_and_compression() {
    let (mut app, written) = app(1);
    app.run("file.exportAs", json!({"format": "tiff"})).unwrap();
    confirm(&mut app).unwrap();
    assert_eq!(kind(&app), Some("tiffOptions"));
    let d = app.ui.dialog.as_ref().unwrap();
    assert_eq!((spec(&d.kind).heading)(d), "TIFF Options");
    assert_eq!((d.str("colorModel"), d.str("byteOrder"), d.bool("lzw"), d.bool("embedIcc")), ("rgb".into(), "little".into(), true, true));
    frame(&mut app);
    set(&mut app, "byteOrder", json!("big"));
    set(&mut app, "colorModel", json!("cmyk"));
    set(&mut app, "lzw", json!(false));
    frame(&mut app);
    confirm(&mut app).unwrap();
    let (path, tiff) = written.borrow()[0].clone();
    assert_eq!(path, "/out/Untitled-1.tif");
    assert_eq!(&tiff[..4], b"MM\0*");
    assert!(app.ui.dialog.is_none());

    // A CMYK document exports CMYK by default.
    app.run("object.convertDocumentColorMode", json!({"mode": "cmyk"})).unwrap();
    app.run("file.exportAs", json!({"format": "tiff"})).unwrap();
    confirm(&mut app).unwrap();
    assert_eq!(app.ui.dialog.as_ref().unwrap().str("colorModel"), "cmyk");
}
