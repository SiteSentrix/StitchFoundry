//! `document.exportPdf` and the PDF options of every PDF export.

use serde_json::{Value, json};

use super::pdf::DEFAULT_PRESET;
use super::*;

fn session(artboards: usize) -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 80, "artboards": artboards})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 40, "height": 30})).unwrap();
    s
}

fn b64(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().expect("dataBase64")).unwrap()
}

fn export(s: &mut Session, p: Value) -> Value {
    s.execute("document.exportPdf", &p).unwrap_or_else(|e| panic!("{p}: {e}"))
}

fn warnings(v: &Value) -> Vec<String> {
    v["warnings"].as_array().expect("warnings").iter().map(|w| w.as_str().unwrap().to_string()).collect()
}

fn pages(bytes: &[u8]) -> usize {
    vectorcraft_pdf::import(bytes).unwrap().artboards.len()
}

/// `p` plus `format: pdf` (the same options through `document.export`).
fn as_export(p: &Value) -> Value {
    let mut o = p.as_object().cloned().unwrap_or_default();
    o.insert("format".into(), json!("pdf"));
    Value::Object(o)
}

#[test]
fn export_pdf_writes_the_compatibility_header_and_the_range() {
    let mut s = session(3);
    let v = export(&mut s, json!({}));
    let pdf = b64(&v);
    assert!(pdf.starts_with(b"%PDF-1.7"));
    assert_eq!(pages(&pdf), 3);
    assert_eq!(v["bytes"], pdf.len());
    assert!(warnings(&v).is_empty(), "{v}");
    assert!(b64(&export(&mut s, json!({"compatibility": "1.5"}))).starts_with(b"%PDF-1.5"));
    assert!(b64(&export(&mut s, json!({"compatibility": "2.0"}))).starts_with(b"%PDF-2.0"));
    assert_eq!(pages(&b64(&export(&mut s, json!({"range": "1,3"})))), 2);
    assert_eq!(pages(&b64(&export(&mut s, json!({"artboards": [1]})))), 1);
    // The same options through document.export and document.serialize.
    let v = s.execute("document.export", &as_export(&json!({"compatibility": "1.4", "range": "2"}))).unwrap();
    assert!(b64(&v).starts_with(b"%PDF-1.4") && pages(&b64(&v)) == 1);
    let v = s.execute("document.serialize", &as_export(&json!({"compatibility": "1.6"}))).unwrap();
    assert!(b64(&v).starts_with(b"%PDF-1.6"));
}

#[test]
fn bad_options_are_refused() {
    let mut s = session(2);
    for p in [
        json!({"compatibility": "1.3"}),
        json!({"compatibility": 1.5}),
        json!({"range": "1-3"}),
        json!({"range": "0"}),
        json!({"artboards": []}),
        json!({"preset": "Nope"}),
        json!({"standard": "pdfX4"}),
        json!({"standard": "pdfA2b", "compatibility": "2.0"}),
        json!({"compression": {"color": {"ppi": 0}}}),
        json!({"compression": "zip"}),
        json!({"security": {"openPassword": "x"}}),
    ] {
        let e = s.execute("document.exportPdf", &p).unwrap_err();
        assert!(matches!(e, crate::EngineError::BadParams { .. }), "{p}: {e}");
        assert!(s.execute("document.export", &as_export(&p)).is_err(), "document.export {p}");
    }
}

#[test]
fn options_apply_over_the_preset() {
    let mut s = session(1);
    let a = b64(&export(&mut s, json!({"preset": DEFAULT_PRESET, "compression": {"compressText": false}})));
    let b = b64(&export(&mut s, json!({"preset": "default"})));
    assert!(a.len() > b.len(), "content streams left uncompressed");
    assert!(String::from_utf8_lossy(&a).contains(" re") || String::from_utf8_lossy(&a).contains(" l"));
    // `null` (the default document.formats lists) keeps the preset's value, at any depth.
    let p = json!({"compatibility": null, "compression": null, "security": {"openPassword": null}, "marks": {"weight": null}});
    let v = export(&mut s, p);
    assert!(b64(&v).starts_with(b"%PDF-1.7"));
    assert!(warnings(&v).is_empty(), "{v}");
}

#[test]
fn pdf_a_refuses_pdf_2() {
    let mut s = session(1);
    let e = s.execute("document.exportPdf", &json!({"standard": "pdfA2b", "compatibility": "2.0"})).unwrap_err();
    assert!(matches!(e, crate::EngineError::BadParams { .. }), "{e}");
    let pdf = b64(&export(&mut s, json!({"standard": "pdfA2b", "compatibility": "1.6"})));
    assert!(pdf.starts_with(b"%PDF-1.6") && String::from_utf8_lossy(&pdf).contains("pdfaid"));
}

#[test]
fn options_not_applied_yet_and_document_features_warn() {
    let mut s = session(1);
    let w = warnings(&export(&mut s, json!({"thumbnails": true, "marks": {"trim": true}})));
    assert_eq!(w.len(), 2, "{w:?}");
    assert!(w.iter().any(|w| w.contains("thumbnails")) && w.iter().any(|w| w.contains("marks")));
    // A pattern stroke is approximated: its warning comes back.
    let tile = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"].as_u64().unwrap();
    s.execute("select.set", &json!({"ids": [tile]})).unwrap();
    s.execute("object.pattern.make", &json!({"name": "Dots", "width": 20, "height": 20})).unwrap();
    s.execute("object.pattern.done", &json!({})).unwrap();
    let frame = s.execute("shape.rectangle", &json!({"x": 20, "y": 20, "width": 50, "height": 40})).unwrap()["id"].as_u64().unwrap();
    s.execute("paint.setStroke", &json!({"ids": [frame], "swatch": "Dots"})).unwrap();
    let pattern = |w: Vec<String>| w.iter().any(|w| w.contains("pattern strokes"));
    assert!(pattern(warnings(&export(&mut s, json!({})))));
}

#[test]
fn export_pdf_writes_a_path() {
    let mut s = session(2);
    let dir = std::env::temp_dir().join(format!("vc-exportpdf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.pdf").to_string_lossy().to_string();
    let v = export(&mut s, json!({"path": path, "range": "2"}));
    assert_eq!(v["path"], path.as_str());
    assert!(v.get("dataBase64").is_none());
    let bytes = std::fs::read(&path).unwrap();
    assert_eq!(v["bytes"], bytes.len());
    assert_eq!(pages(&bytes), 1);
    assert_eq!(s.doc().unwrap().path, None, "exporting never retargets the document");
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn formats_list_the_pdf_options() {
    let mut s = Session::new();
    let v = s.execute("document.formats", &json!({})).unwrap();
    let pdf = v["formats"].as_array().unwrap().iter().find(|f| f["id"] == "pdf").unwrap();
    for k in ["artboard", "range", "preset", "standard", "compatibility", "compression", "security"] {
        assert!(pdf["options"].get(k).is_some(), "{k}");
    }
    assert_eq!(pdf["options"]["compatibility"]["default"], "1.7");
    assert_eq!(pdf["options"]["preset"]["default"], DEFAULT_PRESET);
}
