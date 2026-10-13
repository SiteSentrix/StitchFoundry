//! Save As › `.ai` with `aiVersion: "7"`: legacy `.ai` files of version 7, one per artboard, that open with
//! their layers and groups (#868).

use serde_json::{Value, json};

use super::*;
use crate::tests_links::Folder;

/// Three artboards (named A, B and C) with a rectangle on each, on two layers: the first
/// rectangle's group (with a second one) on `Layer 1`, the others on `Top`.
fn three_boards() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    s.execute("artboard.setProps", &json!({"index": 0, "name": "A"})).unwrap();
    for (i, name) in ["B", "C"].iter().enumerate() {
        let x = 200.0 * (i + 1) as f64;
        s.execute("artboard.new", &json!({"x": x, "y": 0, "width": 100, "height": 100, "name": name})).unwrap();
    }
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 30, "height": 30})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 50, "y": 10, "width": 30, "height": 30})).unwrap();
    s.execute("select.all", &json!({})).unwrap();
    s.execute("object.group", &json!({})).unwrap();
    s.execute("layer.new", &json!({"name": "Top"})).unwrap();
    for x in [210, 410] {
        s.execute("shape.rectangle", &json!({"x": x, "y": 10, "width": 30, "height": 30})).unwrap();
    }
    s
}

fn layer_names(d: &vectorcraft_doc::Document) -> Vec<String> {
    d.layers.iter().map(|l| l.name.clone().unwrap_or_default()).collect()
}

fn warnings(r: &Value) -> Vec<String> {
    r["warnings"].as_array().map(|w| w.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

#[test]
fn each_artboard_is_a_file_that_opens_with_its_layers() {
    let dir = Folder::new("ai8");
    let mut s = three_boards();
    let path = dir.file("Poster.ai");
    let r = s.execute("file.saveAs", &json!({"path": path, "format": "ai", "aiVersion": "7"})).unwrap();
    let files: Vec<&str> = r["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert_eq!(files, [dir.file("Poster-A.ai"), dir.file("Poster-B.ai"), dir.file("Poster-C.ai")], "{r}");
    assert!(warnings(&r).iter().any(|w| w.contains("file of its own")), "{r}");
    for (f, objects) in [(files[0], 1), (files[1], 1), (files[2], 1)] {
        let bytes = std::fs::read(f).unwrap();
        assert!(bytes.starts_with(b"%!PS-Adobe-3.0\n"), "{f}");
        s.execute("document.open", &json!({"path": f})).unwrap();
        let d = &s.doc().unwrap().doc;
        assert_eq!(d.artboards.len(), 1, "{f}");
        assert_eq!(layer_names(d), ["Layer 1", "Top"], "{f}");
        let count: usize = d.layers.iter().map(|l| l.children().map_or(0, Vec::len)).sum();
        assert_eq!(count, objects, "{f}");
    }
    // Artboard A's file: the group of two rectangles in Layer 1, nothing on Top.
    s.execute("document.open", &json!({"path": files[0]})).unwrap();
    let d = &s.doc().unwrap().doc;
    let group = &d.layers[0].children().unwrap()[0];
    assert!(matches!(&group.kind, vectorcraft_doc::NodeKind::Group { children, clip: false } if children.len() == 2), "{:?}", group.kind);
    assert!(d.layers[1].children().unwrap().is_empty());
}

#[test]
fn one_artboard_is_the_file_itself_and_save_writes_it_again() {
    let dir = Folder::new("ai8-one");
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 30, "height": 30})).unwrap();
    let path = dir.file("One.ai");
    let r = s.execute("file.saveAs", &json!({"path": path, "format": "ai", "aiVersion": "7"})).unwrap();
    assert!(r["files"].is_null(), "{r}");
    assert!(std::fs::read(&path).unwrap().starts_with(b"%!PS"));
    // Save remembers the version.
    std::fs::remove_file(&path).unwrap();
    s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 30, "height": 30})).unwrap();
    s.execute("document.save", &json!({})).unwrap();
    assert!(std::fs::read(&path).unwrap().starts_with(b"%!PS"));
}

#[test]
fn the_default_stays_pdf_compatible_and_other_versions_are_refused() {
    let mut s = three_boards();
    for p in [json!({"format": "ai"}), json!({"format": "ai", "aiVersion": "pdf"})] {
        let r = s.execute("file.saveCopy", &p).unwrap();
        let bytes = vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "{p}");
    }
    assert!(s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "3"})).is_err());
    let v = s.execute("file.formatOptions", &json!({"format": "ai"})).unwrap();
    assert_eq!(v["options"]["aiVersion"]["value"], "pdf", "{v}");
}

#[test]
fn transparency_and_appearances_are_written_as_plain_art_and_type_as_text() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 30, "y": 30, "width": 50, "height": 50})).unwrap();
    s.execute("transparency.set", &json!({"opacity": 50})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 120, "y": 10, "width": 30, "height": 30})).unwrap();
    s.execute("appearance.addFill", &json!({})).unwrap();
    s.execute("text.create", &json!({"x": 100, "y": 80, "text": "Hi", "size": 24})).unwrap();
    let r = s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7"})).unwrap();
    let w = warnings(&r);
    assert!(w.iter().any(|w| w.contains("written opaque")), "{w:?}");
    assert!(!w.iter().any(|w| w.contains("type") && w.contains("outlines")), "{w:?}");
    let bytes = vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap();
    let text = String::from_utf8(bytes.clone()).unwrap();
    // No transparency operators: version 7 has none.
    assert!(!text.contains(" Xy\n"), "{text}");
    s.execute("document.open", &json!({"name": "t.ai", "dataBase64": r["dataBase64"]})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert!(d.layers.iter().all(|l| l.children().is_some_and(|c| !c.is_empty())));
    let mut opaque = true;
    d.walk(|n| opaque &= n.opacity == 1.0);
    assert!(opaque);
    assert_eq!(texts(d), ["Hi"]);
}

/// The plain text of every text object of `d`.
fn texts(d: &vectorcraft_doc::Document) -> Vec<String> {
    let mut out = vec![];
    d.walk(|n| {
        if let vectorcraft_doc::NodeKind::Text(t) = &n.kind {
            out.push(t.plain_text());
        }
    });
    out
}

#[test]
fn type_can_be_written_as_outlines() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
    s.execute("text.create", &json!({"x": 20, "y": 50, "text": "Café", "size": 24})).unwrap();
    let r = s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7", "outlineText": true})).unwrap();
    s.execute("document.open", &json!({"name": "t.ai", "dataBase64": r["dataBase64"]})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert!(texts(d).is_empty());
    let mut paths = 0;
    d.walk(|n| paths += usize::from(matches!(n.kind, vectorcraft_doc::NodeKind::Compound { .. } | vectorcraft_doc::NodeKind::Path { .. })));
    assert!(paths > 0);
    assert!(s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7", "outlineText": "yes"})).is_err());
    let v = s.execute("file.formatOptions", &json!({"format": "ai"})).unwrap();
    assert_eq!(v["options"]["outlineText"]["value"], false, "{v}");
}

#[test]
fn hidden_objects_go_to_a_hidden_layer_or_are_left_out_or_shown() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 30, "height": 30})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 60, "y": 10, "width": 30, "height": 30})).unwrap();
    s.execute("object.hide", &json!({})).unwrap();
    let saved = |s: &mut Session, hidden: Value| {
        let r = s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7", "hiddenObjects": hidden})).unwrap();
        s.execute("document.open", &json!({"name": "t.ai", "dataBase64": r["dataBase64"]})).unwrap();
        let d = s.doc().unwrap().doc.clone();
        s.execute("file.close", &json!({})).unwrap();
        (d, warnings(&r))
    };
    let (d, w) = saved(&mut s, Value::Null);
    assert_eq!(layer_names(&d), ["Layer 1", "Layer 1 (hidden)"], "{w:?}");
    assert!(d.layers[0].visible && !d.layers[1].visible);
    assert!(w.iter().any(|w| w.contains("hidden layer")), "{w:?}");
    let (d, _) = saved(&mut s, json!("omit"));
    assert_eq!(layer_names(&d), ["Layer 1"]);
    assert_eq!(d.layers[0].children().map(Vec::len), Some(1));
    let (d, _) = saved(&mut s, json!("visible"));
    assert_eq!(layer_names(&d), ["Layer 1"]);
    assert_eq!(d.layers[0].children().map(|c| c.iter().filter(|n| n.visible).count()), Some(2));
    assert!(s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7", "hiddenObjects": "behind"})).is_err());
    let v = s.execute("file.formatOptions", &json!({"format": "ai"})).unwrap();
    assert_eq!(v["options"]["hiddenObjects"]["value"], "layer", "{v}");
}

#[test]
fn art_under_transparent_art_stays_in_its_own_layer() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
    s.execute("layer.new", &json!({"name": "Shade"})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 40, "height": 40})).unwrap();
    s.execute("transparency.set", &json!({"opacity": 50, "blend": "multiply"})).unwrap();
    let r = s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7"})).unwrap();
    s.execute("document.open", &json!({"name": "t.ai", "dataBase64": r["dataBase64"]})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert_eq!(layer_names(d), ["Layer 1", "Shade"]);
    assert!(d.layers.iter().all(|l| l.children().is_some_and(|c| c.len() == 1)), "{:?}", d.layers);
}

/// A `w`×`h` PNG of one colour (`rgba`), 72 ppi.
fn png(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    let mut out = vec![];
    image::RgbaImage::from_pixel(w, h, image::Rgba(rgba)).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// A red square (10–50 × 10–50) half under a half-transparent black image (30–70 × 10–50), and
/// an opaque blue image (140–160 × 40–60).
fn shadow_over_square() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 40, "height": 40})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#ff0000"})).unwrap();
    for (bytes, at) in [(png(40, 40, [0, 0, 0, 128]), [50, 30]), (png(20, 20, [0, 0, 255, 255]), [150, 50])] {
        let p = json!({"name": "i.png", "dataBase64": vectorcraft_format::base64_encode(&bytes), "at": at});
        s.execute("file.place", &p).unwrap();
    }
    s
}

/// The colour `d` shows at (`x`, `y`), over white.
fn colour_at(d: &vectorcraft_doc::Document, x: f64, y: f64) -> [u8; 3] {
    let px = vectorcraft_render::Renderer::new().render_region(d, vectorcraft_geom::Rect::new(x, y, x + 1.0, y + 1.0), 1.0, true).pixels;
    [px[0], px[1], px[2]]
}

fn images(d: &vectorcraft_doc::Document) -> (usize, usize) {
    let (mut all, mut clipped) = (0, 0);
    d.walk(|n| {
        if let vectorcraft_doc::NodeKind::Group { children, clip: true } = &n.kind {
            clipped += children.iter().filter(|c| matches!(c.kind, vectorcraft_doc::NodeKind::Image(_))).count();
        }
        all += usize::from(matches!(n.kind, vectorcraft_doc::NodeKind::Image(_)));
    });
    (all, clipped)
}

#[test]
fn a_transparent_image_is_flattened_with_the_art_under_it_which_stays() {
    let mut s = shadow_over_square();
    let before = [colour_at(&s.doc().unwrap().doc, 40.0, 30.0), colour_at(&s.doc().unwrap().doc, 60.0, 30.0)];
    let r = s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7"})).unwrap();
    assert!(warnings(&r).iter().any(|w| w.contains("flattened with the art under them")), "{r}");
    assert!(!warnings(&r).iter().any(|w| w.contains("transparent image pixels are written over white")), "{r}");
    s.execute("document.open", &json!({"name": "t.ai", "dataBase64": r["dataBase64"]})).unwrap();
    let d = &s.doc().unwrap().doc;
    // The square is still there, under an opaque image clipped to the transparent one's place;
    // the opaque image stays as it was.
    let mut squares = 0;
    d.walk(|n| squares += usize::from(matches!(n.kind, vectorcraft_doc::NodeKind::Path { clipping: false, .. })));
    assert_eq!(squares, 1);
    assert_eq!(images(d), (2, 1));
    let after = [colour_at(d, 40.0, 30.0), colour_at(d, 60.0, 30.0)];
    for (a, b) in before.iter().zip(&after) {
        assert!(a.iter().zip(b).all(|(a, b)| a.abs_diff(*b) <= 3), "{before:?} {after:?}");
    }
    assert_eq!(colour_at(d, 20.0, 30.0), [255, 0, 0], "the square outside the image");
    assert_eq!(colour_at(d, 80.0, 30.0), [255, 255, 255], "nothing beyond the image");
}

#[test]
fn transparent_images_can_be_left_out_and_opacity_makes_one_transparent() {
    let mut s = shadow_over_square();
    s.execute("transparency.set", &json!({"opacity": 50})).unwrap(); // the blue image
    let r = s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7", "transparentImages": "omit"})).unwrap();
    assert!(warnings(&r).iter().any(|w| w.contains("left out")), "{r}");
    s.execute("document.open", &json!({"name": "t.ai", "dataBase64": r["dataBase64"]})).unwrap();
    let d = &s.doc().unwrap().doc;
    assert_eq!(images(d), (0, 0));
    assert_eq!(colour_at(d, 40.0, 30.0), [255, 0, 0]);
    assert!(s.execute("file.saveCopy", &json!({"format": "ai", "aiVersion": "7", "transparentImages": "blur"})).is_err());
    let v = s.execute("file.formatOptions", &json!({"format": "ai"})).unwrap();
    assert_eq!(v["options"]["transparentImages"]["value"], "flatten", "{v}");
}

#[test]
fn a_range_saves_those_artboards_and_save_writes_them_again() {
    let dir = Folder::new("ai7-range");
    let mut s = three_boards();
    let path = dir.file("Poster.ai");
    let r = s.execute("file.saveAs", &json!({"path": path, "format": "ai", "aiVersion": "7", "range": "1, 3"})).unwrap();
    let files: Vec<&str> = r["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert_eq!(files, [dir.file("Poster-A.ai"), dir.file("Poster-C.ai")], "{r}");
    // One artboard: the file itself. An empty range: every artboard.
    let r = s.execute("file.saveAs", &json!({"path": path, "format": "ai", "aiVersion": "7", "range": "2"})).unwrap();
    assert!(r["files"].is_null(), "{r}");
    let r = s.execute("file.saveAs", &json!({"path": path, "format": "ai", "aiVersion": "7", "range": ""})).unwrap();
    assert_eq!(r["files"].as_array().map(Vec::len), Some(3), "{r}");
    // Save keeps the range it was saved with.
    s.execute("file.saveAs", &json!({"path": path, "format": "ai", "aiVersion": "7", "range": "1-2"})).unwrap();
    let r = s.execute("document.save", &json!({})).unwrap();
    assert_eq!(r["files"].as_array().map(Vec::len), Some(2), "{r}");
}
