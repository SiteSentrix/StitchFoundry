//! Placed documents: a VectorCraft document placed linked (`file.place`'s default for one read
//! from a file), drawn and output as vectors, and saved with its preview.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, NodeKind, PlacedDocument};
use vectorcraft_geom::{Rect, shapes};

use super::tests_links::{BLUE, Folder, RED, centre_colour, near, open, save, session, write};
use super::*;

/// A VectorCraft document at `path`: one `w`×`h` artboard filled with `rgb` (and `extra` more
/// rectangles, which change the file's size).
fn source(path: &str, w: f64, h: f64, rgb: [u8; 3], extra: usize) {
    let mut d = Document::new(w, h);
    let layer = d.layers[0].id;
    let [r, g, b] = rgb;
    for i in 0..=extra {
        let id = d.alloc_id();
        let look = Appearance::basic(Paint::solid(Color::rgb8(r, g, b)), Paint::None, 0.0);
        d.insert(Some(layer), i, Node::path(id, shapes::rectangle(Rect::new(0.0, 0.0, w, h)), look)).unwrap();
    }
    write(path, &vectorcraft_format::save_file(&d));
}

/// A document drawing symbol `Star` (a blue 100×50 rectangle) at `path`, titled `title` (tests
/// that remove the file give it content of its own: files read are cached by content).
fn star_source(path: &str, title: &str) {
    let mut d = Document::new(100.0, 50.0);
    d.title = title.into();
    let layer = d.layers[0].id;
    let art = Node::path(vectorcraft_doc::NodeId(900), shapes::rectangle(Rect::new(0.0, 0.0, 100.0, 50.0)), look(BLUE));
    d.symbols.push(vectorcraft_doc::Symbol { name: "Star".into(), art: std::sync::Arc::new(art) });
    let id = d.alloc_id();
    d.insert(Some(layer), 0, Node::new(id, NodeKind::SymbolInstance { symbol: "Star".into(), xf: vectorcraft_geom::Affine::IDENTITY })).unwrap();
    write(path, &vectorcraft_format::save_file(&d));
}

fn look(c: [u8; 3]) -> Appearance {
    Appearance::basic(Paint::solid(Color::rgb8(c[0], c[1], c[2])), Paint::None, 0.0)
}

/// The active document gets a red symbol named `Star` of its own.
fn parent_star(s: &mut Session) {
    let star = Node::path(vectorcraft_doc::NodeId(901), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), look(RED));
    std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().doc)
        .symbols
        .push(vectorcraft_doc::Symbol { name: "Star".into(), art: std::sync::Arc::new(star) });
}

/// Place `path` (linked: the default), centred on the artboard → its id.
fn place(s: &mut Session, path: &str) -> NodeId {
    let r = s.execute("file.place", &json!({"path": path, "at": [200, 150]})).unwrap();
    assert_eq!(r["linked"], true, "{r}");
    NodeId(r["ids"][0].as_u64().unwrap())
}

fn placed(s: &Session, id: NodeId) -> PlacedDocument {
    match &s.doc().unwrap().doc.node(id).unwrap().kind {
        NodeKind::PlacedDocument(p) => (**p).clone(),
        k => panic!("not a placed document: {k:?}"),
    }
}

fn bounds(s: &Session, id: NodeId) -> Rect {
    s.doc().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap()
}

fn decode(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().unwrap()).unwrap()
}

/// Images and paths in `bytes`, a PDF, read back.
fn pdf_contents(bytes: &[u8]) -> (usize, usize) {
    // The page as drawn (not the VectorCraft document the PDF carries).
    let (doc, _) = super::cmd::fileio::page_document(bytes, 0, &Default::default()).unwrap();
    let (mut images, mut paths) = (0, 0);
    doc.walk(|n| match n.kind {
        NodeKind::Image(_) => images += 1,
        NodeKind::Path { .. } | NodeKind::Compound { .. } => paths += 1,
        _ => {}
    });
    (images, paths)
}

/// The JSON of a `.vectorcraft` file (the data after it left out).
fn file_json(bytes: &[u8]) -> Value {
    let end = bytes.windows(vectorcraft_format::BLOB_MAGIC.len()).position(|w| w == vectorcraft_format::BLOB_MAGIC).unwrap_or(bytes.len());
    serde_json::from_slice(&bytes[..end]).unwrap()
}

#[test]
fn a_document_places_linked_by_default_at_its_artboard_size() {
    let dir = Folder::new("placed-basic");
    let src = dir.file("logo.vectorcraft");
    source(&src, 100.0, 50.0, RED, 0);
    let mut s = session();
    let id = place(&mut s, &src);
    let p = placed(&s, id);
    assert!((p.width - 100.0).abs() < 1e-6 && (p.height - 50.0).abs() < 1e-6, "{p:?}");
    assert_eq!(bounds(&s, id), Rect::new(150.0, 125.0, 250.0, 175.0));
    assert_eq!((p.link.path.as_str(), p.link.page), (src.as_str(), Some(1)));
    let n = s.doc().unwrap().doc.node(id).unwrap();
    assert_eq!((n.name.as_deref(), n.kind_label()), (Some("logo.vectorcraft"), "Placed Document"));
    assert!(near(centre_colour(&s.doc().unwrap().doc), RED));
    // It moves and scales like any object.
    s.execute("object.move", &json!({"dx": 10, "dy": 0})).unwrap();
    assert_eq!(bounds(&s, id), Rect::new(160.0, 125.0, 260.0, 175.0));
    // Its art's bounds instead of the artboard.
    let mut d = Document::new(100.0, 50.0);
    let layer = d.layers[0].id;
    let id2 = d.alloc_id();
    d.insert(Some(layer), 0, Node::path(id2, shapes::rectangle(Rect::new(10.0, 10.0, 40.0, 30.0)), look(RED))).unwrap();
    let small = dir.file("small.vectorcraft");
    write(&small, &vectorcraft_format::save_file(&d));
    let r = s.execute("file.place", &json!({"path": small, "crop": "bounding", "at": [200, 150]})).unwrap();
    let p = placed(&s, NodeId(r["ids"][0].as_u64().unwrap()));
    assert!(p.bounding && (p.width - 30.0).abs() < 1e-6 && (p.height - 20.0).abs() < 1e-6, "{p:?}");
}

#[test]
fn without_link_or_a_file_a_document_places_as_an_editable_copy() {
    let dir = Folder::new("placed-copy");
    let src = dir.file("logo.vectorcraft");
    source(&src, 100.0, 50.0, RED, 0);
    let mut s = session();
    let data = vectorcraft_format::base64_encode(&std::fs::read(&src).unwrap());
    for p in [json!({"path": src, "link": false}), json!({"name": "logo.vectorcraft", "dataBase64": data})] {
        let r = s.execute("file.place", &p).unwrap();
        assert_eq!(r["linked"], false, "{r}");
        let n = s.doc().unwrap().doc.node(NodeId(r["ids"][0].as_u64().unwrap())).unwrap();
        assert_eq!(n.kind_label(), "Clip Group", "{p}");
    }
    assert!(!s.doc().unwrap().doc.has_placed());
}

#[test]
fn a_placed_document_is_vectors_in_every_output() {
    let dir = Folder::new("placed-outputs");
    let src = dir.file("logo.vectorcraft");
    source(&src, 100.0, 50.0, RED, 0);
    let mut s = session();
    place(&mut s, &src);
    let svg = s.execute("document.serialize", &json!({"format": "svg"})).unwrap();
    let text = svg["text"].as_str().unwrap();
    assert!(!text.contains("<image") && (text.contains("<path") || text.contains("<rect")), "drawn as paths: {text}");
    let (images, paths) = pdf_contents(&decode(&s.execute("document.serialize", &json!({"format": "pdf"})).unwrap()));
    assert_eq!(images, 0, "no image in the PDF");
    assert!(paths > 0);
    let png = decode(&s.execute("document.serialize", &json!({"format": "png"})).unwrap());
    let [r, g, b, _] = image::load_from_memory(&png).unwrap().to_rgba8().get_pixel(200, 150).0;
    assert!(near([r, g, b], RED), "{:?}", [r, g, b]);
    for format in ["eps", "emf", "dxf"] {
        let out = s.execute("document.serialize", &json!({"format": format})).unwrap();
        assert!(out.to_string().len() > 200, "{format}: {out}");
    }
    let info = s.execute("document.info", &json!({})).unwrap();
    assert!(info.to_string().contains("\"placedDocuments\""), "{info}");
}

#[test]
fn a_document_saved_with_previews_outputs_its_files_art() {
    let dir = Folder::new("placed-preview-output");
    let src = dir.file("logo.vectorcraft");
    source(&src, 100.0, 50.0, RED, 0);
    let mut s = session();
    let id = place(&mut s, &src);
    let doc_path = dir.file("poster.vectorcraft");
    save(&mut s, &doc_path);
    open(&mut s, &doc_path);
    let key = placed(&s, id).key;
    assert!(s.doc().unwrap().doc.images[&key].is_proxy(), "opened with the preview only");
    assert!(near(centre_colour(&s.doc().unwrap().doc), RED));
    // Output reads the file again: vectors, no warning.
    let svg = s.execute("document.serialize", &json!({"format": "svg"})).unwrap();
    assert_eq!(svg["warnings"], json!([]), "{}", svg["warnings"]);
    let text = svg["text"].as_str().unwrap();
    assert!(!text.contains("<image") && (text.contains("<path") || text.contains("<rect")), "vectors");
    // So does saving with Include Linked Files, and the document stays as it was.
    let full = file_json(&decode(&s.execute("document.serialize", &json!({"format": "vectorcraft", "includeLinked": true})).unwrap()));
    assert_eq!(full["images"][&key]["mime"], "application/json");
    assert!(s.doc().unwrap().doc.images[&key].is_proxy());
}

#[test]
fn a_placed_document_inside_a_placed_document_outputs_from_its_own_file() {
    let dir = Folder::new("placed-nested");
    let (logo, card) = (dir.file("logo.vectorcraft"), dir.file("card.vectorcraft"));
    source(&logo, 100.0, 50.0, RED, 0);
    // The card places the logo, and is saved with the logo's preview only.
    let mut s = session();
    place(&mut s, &logo);
    save(&mut s, &card);
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    place(&mut s, &card);
    assert!(near(centre_colour(&s.doc().unwrap().doc), RED));
    // The logo inside is read from its file: vectors, not its preview.
    let svg = s.execute("document.serialize", &json!({"format": "svg"})).unwrap();
    let text = svg["text"].as_str().unwrap();
    assert!(!text.contains("<image"), "the logo's file, not its preview: {text}");
    assert_eq!(pdf_contents(&decode(&s.execute("document.serialize", &json!({"format": "pdf"})).unwrap())).0, 0);
}

#[test]
fn what_cant_be_placed_linked_is_an_error() {
    let dir = Folder::new("placed-errors");
    let src = dir.file("logo.vectorcraft");
    source(&src, 100.0, 50.0, RED, 0);
    let mut s = session();
    let place = |s: &mut Session, p: Value| s.execute("file.place", &p).unwrap_err().to_string();
    let e = place(&mut s, json!({"path": src, "page": 2}));
    assert!(e.contains("page 2") && e.contains("1 artboard"), "{e}");
    let e = place(&mut s, json!({"path": src, "crop": "art"}));
    assert!(e.contains("crop"), "{e}");
    let empty = dir.file("empty.vectorcraft");
    write(&empty, &vectorcraft_format::save_file(&Document::new(100.0, 50.0)));
    let e = place(&mut s, json!({"path": empty, "crop": "bounding"}));
    assert!(e.contains("no art to place"), "{e}");
    // Junk params are errors, not crashes.
    for v in [json!(null), json!(true), json!(-1), json!(1e308), json!("x"), json!([1, 2]), json!({"a": {"b": []}})] {
        for k in ["link", "page", "crop", "at", "rect"] {
            let _ = s.execute("file.place", &json!({"path": src, k: v}));
        }
    }
}

#[test]
fn a_placed_documents_resources_stay_its_own() {
    let dir = Folder::new("placed-symbols");
    let src = dir.file("badge.vectorcraft");
    star_source(&src, "star");
    let mut s = session();
    parent_star(&mut s);
    place(&mut s, &src);
    assert!(near(centre_colour(&s.doc().unwrap().doc), BLUE), "the file's own Star");
    let doc = &s.doc().unwrap().doc;
    assert_eq!(doc.symbols.len(), 1, "nothing joins the document");
    let prefix = vectorcraft_doc::placed_document::RESOURCE_PREFIX;
    assert!(doc.images.keys().all(|k| !k.starts_with(prefix)));
    let svg = s.execute("document.serialize", &json!({"format": "svg"})).unwrap();
    assert!(!svg["text"].as_str().unwrap().contains("<image"));
    // Nor its saves.
    let saved = String::from_utf8(decode(&s.execute("document.serialize", &json!({"format": "vectorcraft"})).unwrap())).unwrap();
    assert!(!saved.contains(prefix), "{saved}");
    // Saved for older apps: the art and its resources, as plain objects.
    let old = vectorcraft_format::load(&decode(&s.execute("document.serialize", &json!({"format": "vectorcraft", "version": 2})).unwrap())).unwrap();
    assert!(!old.has_placed());
    assert!(near(centre_colour(&old), BLUE));
}
