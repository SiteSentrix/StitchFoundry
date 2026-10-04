use std::io::{Cursor, Write as _};

use serde_json::{Value, json};
use vectorcraft_doc::NodeKind;

use super::*;
use crate::cmd::parse_range;

fn session(width: f64, height: f64, artboards: usize) -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": width, "height": height, "artboards": artboards})).unwrap();
    s
}

fn tmp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("vc-fileio-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn b64(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().expect("dataBase64")).unwrap()
}

fn open(s: &mut Session, name: &str, bytes: &[u8]) -> Value {
    s.execute("document.open", &json!({"name": name, "dataBase64": vectorcraft_format::base64_encode(bytes)}))
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// A `w`×`h` image of one colour in `format`.
fn image_bytes(w: u32, h: u32, format: image::ImageFormat) -> Vec<u8> {
    let rgba = image::RgbaImage::from_pixel(w, h, image::Rgba([200, 30, 40, 255]));
    let img = if format == image::ImageFormat::Jpeg {
        image::DynamicImage::ImageRgb8(image::DynamicImage::ImageRgba8(rgba).to_rgb8())
    } else {
        rgba.into()
    };
    let mut out = Vec::new();
    img.write_to(&mut Cursor::new(&mut out), format).unwrap();
    out
}

fn image_of(s: &Session) -> vectorcraft_doc::ImageObject {
    let doc = &s.doc().unwrap().doc;
    match &doc.layers[0].children().unwrap()[0].kind {
        NodeKind::Image(im) => im.clone(),
        other => panic!("expected an image, got {other:?}"),
    }
}

#[test]
fn opens_every_readable_format() {
    let mut s = session(200.0, 100.0, 1);
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
    let native = b64(&s.execute("document.serialize", &json!({})).unwrap());
    let svg = s.execute("document.serialize", &json!({"format": "svg"})).unwrap()["text"].as_str().unwrap().to_string();
    let pdf = b64(&s.execute("document.serialize", &json!({"format": "pdf"})).unwrap());
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    gz.write_all(svg.as_bytes()).unwrap();
    let svgz = gz.finish().unwrap();

    for (name, bytes, format) in [
        ("a.vectorcraft", native.clone(), "vectorcraft"),
        ("a.drawcraft", native, "vectorcraft"),
        ("a.svg", svg.into_bytes(), "svg"),
        ("a.svgz", svgz, "svgz"),
        ("a.pdf", pdf.clone(), "pdf"),
        ("a.ai", pdf, "ai"),
    ] {
        let r = open(&mut s, name, &bytes);
        assert_eq!(r["format"], format, "{name}");
        assert_eq!(r["title"], if format == "vectorcraft" { "Untitled-1" } else { name }, "{name}");
        assert!(s.doc().unwrap().doc.node_count() >= 2, "{name}: the rectangle came through");
    }
    for (name, kind) in [
        ("p.png", image::ImageFormat::Png),
        ("p.jpg", image::ImageFormat::Jpeg),
        ("p.gif", image::ImageFormat::Gif),
        ("p.webp", image::ImageFormat::WebP),
        ("p.tif", image::ImageFormat::Tiff),
        ("p.bmp", image::ImageFormat::Bmp),
    ] {
        let r = open(&mut s, name, &image_bytes(5, 4, kind));
        assert_eq!(r["format"], format(name.rsplit('.').next().unwrap()).unwrap().id, "{name}");
        let im = image_of(&s);
        assert_eq!((im.width, im.height), (5, 4), "{name}");
        let doc = &s.doc().unwrap().doc;
        assert_eq!((doc.artboards[0].rect.width(), doc.artboards[0].rect.height()), (5.0, 4.0));
        // Browser-safe formats keep their bytes; TIFF and BMP are stored as PNG.
        let mime = &doc.images[&im.key].mime;
        assert_eq!(mime, if name.ends_with("tif") || name.ends_with("bmp") { "image/png" } else { format_for_name(name).unwrap().mime });
    }
    assert!(s.execute("document.open", &json!({"name": "x.txt", "dataBase64": "aGVsbG8="})).is_err());
}

#[test]
fn webp_opens_at_its_pixel_size() {
    let mut s = Session::new();
    open(&mut s, "tiny.webp", &image_bytes(3, 2, image::ImageFormat::WebP));
    let im = image_of(&s);
    assert_eq!((im.width, im.height), (3, 2));
    let ab = s.doc().unwrap().doc.artboards[0].rect;
    assert_eq!((ab.width(), ab.height()), (3.0, 2.0));
    let r = raster_image(&image_bytes(3, 2, image::ImageFormat::WebP)).unwrap();
    assert_eq!((r.width, r.height, r.blob.mime.as_str()), (3, 2, "image/webp"));
}

#[test]
fn content_beats_a_wrong_extension() {
    let png = image_bytes(2, 2, image::ImageFormat::Png);
    assert_eq!(detect("photo.jpg", &png).unwrap().id, "png");
    assert_eq!(detect("noext", b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap().id, "svg");
    assert_eq!(detect("x.ait", b"%PDF-1.7").unwrap().id, "ait");
    assert_eq!(detect("x.bin", b"%PDF-1.7").unwrap().id, "pdf");
    assert!(detect("x.bin", b"nothing").is_none());
}

#[test]
fn templates_open_untitled() {
    let mut s = session(100.0, 100.0, 1);
    let dir = tmp_dir("tpl");
    let path = dir.join("t.vectorcraft").to_string_lossy().to_string();
    s.execute("file.saveAsTemplate", &json!({"path": path})).unwrap();
    let r = s.execute("document.open", &json!({"path": path})).unwrap();
    assert!(r["title"].as_str().unwrap().starts_with("Untitled-"), "{r}");
    assert_eq!(s.doc().unwrap().path, None);
    assert!(!s.doc().unwrap().doc.template);

    let pdf = b64(&s.execute("document.serialize", &json!({"format": "pdf"})).unwrap());
    let ait = dir.join("t.ait").to_string_lossy().to_string();
    std::fs::write(&ait, pdf).unwrap();
    let r = s.execute("document.open", &json!({"path": ait})).unwrap();
    assert_eq!(r["format"], "ait");
    assert!(r["title"].as_str().unwrap().starts_with("Untitled-"), "{r}");
    assert_eq!(s.doc().unwrap().path, None);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn open_exts_cover_every_readable_format() {
    for f in FORMATS.iter().filter(|f| f.read) {
        for e in f.extensions {
            assert!(OPEN_EXTS.contains(e), "OPEN_EXTS lacks .{e} ({})", f.label);
        }
    }
    for e in OPEN_EXTS {
        assert!(format(e).is_some_and(|f| f.read), ".{e} in OPEN_EXTS is no readable format");
    }
    let filters: Vec<_> = open_filters().collect();
    assert_eq!(filters[0], ("All readable files", OPEN_EXTS));
    assert_eq!(filters.len(), 1 + FORMATS.iter().filter(|f| f.read).count());
}

#[test]
fn formats_query_lists_readers_writers_and_options() {
    let mut s = Session::new();
    let r = s.execute("document.formats", &json!({})).unwrap();
    let ids = |k: &str| r[k].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(ids("writable"), ["vectorcraft", "svg", "pdf", "png", "jpg", "webp"]);
    assert!(ids("readable").contains(&"tiff".to_string()) && ids("readable").contains(&"ait".to_string()));
    let png = r["formats"].as_array().unwrap().iter().find(|f| f["id"] == "png").unwrap();
    assert_eq!(png["options"]["scale"]["default"], 1);
    assert_eq!(r["openExtensions"].as_array().unwrap().len(), OPEN_EXTS.len());
}

#[test]
fn ranges_parse_one_based() {
    assert_eq!(parse_range("1-3, 5", 5).unwrap(), [0, 1, 2, 4]);
    assert_eq!(parse_range("3-, 1", 4).unwrap(), [2, 3, 0]);
    assert_eq!(parse_range("-2", 4).unwrap(), [0, 1]);
    assert_eq!(parse_range("2,2,1\u{2013}2", 4).unwrap(), [1, 0]);
    for bad in ["", "0", "6", "3-1", "a", "-", "1-9"] {
        assert!(parse_range(bad, 5).is_err(), "{bad:?}");
    }
}
