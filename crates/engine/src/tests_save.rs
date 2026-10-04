//! The save pipeline: Save writes a document's own format, Save As / a Copy / as Template, Revert
//! and templates.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::*;
use crate::cmd::fileio::{SaveMode, save_plan};

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
    s
}

fn rect(s: &mut Session) {
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 40, "height": 30})).unwrap();
}

/// A fresh folder for one test.
fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("vc-save-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn path(d: &std::path::Path, name: &str) -> String {
    d.join(name).to_string_lossy().to_string()
}

fn b64(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().expect("dataBase64")).unwrap()
}

fn objects(s: &Session) -> usize {
    s.doc().unwrap().doc.node_count()
}

#[test]
fn an_svg_path_writes_svg_and_save_writes_it_back() {
    let d = dir("svg");
    let mut s = session();
    rect(&mut s);
    let svg = path(&d, "art.svg");
    let r = s.execute("document.save", &json!({"path": svg})).unwrap();
    assert_eq!(r["format"], "svg");
    assert!(r["warnings"][0].as_str().unwrap().contains("SVG keeps the artwork"), "{r}");
    assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<?xml"));
    let st = s.doc().unwrap();
    assert_eq!((st.path.as_deref(), st.format, st.is_dirty()), (Some(svg.as_str()), "svg", false));
    // Save writes the same file in the same format.
    rect(&mut s);
    s.execute("document.save", &json!({})).unwrap();
    assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<?xml"));
    assert!(!s.doc().unwrap().is_dirty());
    // A reopened SVG keeps its path and saves as SVG again.
    let mut s = Session::new();
    s.execute("document.open", &json!({"path": svg})).unwrap();
    assert_eq!((s.doc().unwrap().path.as_deref(), s.doc().unwrap().format), (Some(svg.as_str()), "svg"));
    rect(&mut s);
    assert_eq!(s.execute("document.save", &json!({})).unwrap()["format"], "svg");
    assert!(std::fs::read_to_string(&svg).unwrap().starts_with("<?xml"));
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn save_as_takes_on_path_format_and_options() {
    let d = dir("saveas");
    let mut s = session();
    let pdf = path(&d, "doc.pdf");
    s.execute("file.saveAs", &json!({"path": pdf, "options": {"range": "1", "bogus": 1}})).unwrap();
    let st = s.doc().unwrap();
    assert_eq!((st.format, st.title()), ("pdf", "doc.pdf".to_string()));
    assert_eq!(Value::Object(st.save_options.clone()), json!({"range": "1"}), "only the format's own options are kept");
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    // The remembered options are what Save and the options dialog use.
    let o = s.execute("file.formatOptions", &json!({})).unwrap();
    assert_eq!((o["id"].as_str(), o["options"]["range"]["value"].as_str()), (Some("pdf"), Some("1")));
    assert_eq!(o["options"]["artboards"]["value"], Value::Null);
    let svg = s.execute("file.formatOptions", &json!({"format": "svg"})).unwrap();
    assert_eq!(svg["options"]["outlineText"]["value"], false, "another format shows its defaults");
    let ids: Vec<&str> = o["saveFormats"].as_array().unwrap().iter().map(|f| f["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["vectorcraft", "template", "pdf", "svg", "svgz", "ai"]);
    // SVGZ is gzip-compressed SVG and reopens as SVGZ.
    let svgz = path(&d, "doc.svgz");
    s.execute("file.saveAs", &json!({"path": svgz})).unwrap();
    assert_eq!(&std::fs::read(&svgz).unwrap()[..2], [0x1f, 0x8b]);
    s.execute("document.open", &json!({"path": svgz})).unwrap();
    assert_eq!(s.doc().unwrap().format, "svgz");
    // Export formats aren't save formats.
    let e = s.execute("file.saveAs", &json!({"path": path(&d, "x.png")})).unwrap_err().to_string();
    assert!(e.contains("document.export"), "{e}");
    assert!(s.execute("document.save", &json!({"format": "webp"})).is_err());
    assert!(s.execute("file.saveAs", &json!({"path": svgz, "options": 3})).is_err());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn another_format_without_a_path_hands_the_bytes_back() {
    let d = dir("otherfmt");
    let mut s = session();
    let native = path(&d, "a.vectorcraft");
    s.execute("document.save", &json!({"path": native})).unwrap();
    let r = s.execute("document.save", &json!({"format": "svg"})).unwrap();
    assert!(String::from_utf8(b64(&r)).unwrap().starts_with("<?xml"));
    assert_eq!(r["name"], "a.svg");
    assert!(vectorcraft_format::sniff(&std::fs::read(&native).unwrap()), "the native file isn't overwritten with SVG");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn save_a_copy_keeps_the_path_title_and_modified_state() {
    let d = dir("copy");
    let mut s = session();
    let native = path(&d, "poster.vectorcraft");
    s.execute("document.save", &json!({"path": native})).unwrap();
    rect(&mut s);
    let r = s.execute("file.saveCopy", &json!({})).unwrap();
    assert_eq!((r["name"].as_str(), r["folder"].as_str()), (Some("poster copy.vectorcraft"), Some(d.to_string_lossy().as_ref())));
    let copy = path(&d, "poster copy.svg");
    s.execute("file.saveCopy", &json!({"path": copy})).unwrap();
    assert!(std::fs::read_to_string(&copy).unwrap().starts_with("<?xml"));
    let st = s.doc().unwrap();
    assert_eq!((st.path.as_deref(), st.format, st.title()), (Some(native.as_str()), "vectorcraft", "poster.vectorcraft".to_string()));
    assert!(st.is_dirty(), "a copy leaves the document modified");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn revert_reloads_the_saved_file_in_the_same_tab() {
    let d = dir("revert");
    let mut s = session();
    rect(&mut s);
    let native = path(&d, "a.vectorcraft");
    s.execute("document.save", &json!({"path": native})).unwrap();
    assert!(s.execute("file.revert", &json!({})).is_err(), "nothing to revert");
    s.execute("file.new", &json!({})).unwrap();
    s.set_active(0);
    let uid = s.doc().unwrap().uid;
    rect(&mut s);
    rect(&mut s);
    assert_eq!(objects(&s), 4);
    s.execute("file.revert", &json!({})).unwrap();
    let st = s.doc().unwrap();
    assert_eq!((s.active_index(), s.documents().len(), objects(&s)), (Some(0), 2, 2));
    assert!(!st.is_dirty() && st.history.undo.is_empty() && st.selection.is_empty());
    assert_eq!((st.uid, st.path.as_deref()), (uid, Some(native.as_str())));
    // With the file gone the document is left as it is, still modified.
    rect(&mut s);
    std::fs::remove_file(&native).unwrap();
    assert!(s.execute("file.revert", &json!({})).is_err());
    let st = s.doc().unwrap();
    assert!(st.is_dirty());
    assert_eq!((objects(&s), st.history.undo.len()), (3, 1));
    // Never saved: nothing to revert to.
    s.set_active(1);
    rect(&mut s);
    assert!(s.execute("file.revert", &json!({})).is_err());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn templates_suggest_a_name_in_the_templates_folder_and_open_untitled() {
    let d = dir("tpl");
    let mut s = session();
    let source = path(&d, "flyer.vectorcraft");
    s.execute("document.save", &json!({"path": source})).unwrap();
    s.prefs.templates_folder = path(&d, "Templates");
    let r = s.execute("file.saveAsTemplate", &json!({})).unwrap();
    assert_eq!(r["name"], "flyer template.vctemplate");
    assert_eq!(r["folder"].as_str(), Some(s.prefs.templates_folder.as_str()));
    assert_ne!(r["name"].as_str(), std::path::Path::new(&source).file_name().and_then(|n| n.to_str()), "never the source file");
    // Writing it leaves the document alone; opening it gives an untitled document.
    rect(&mut s);
    let tpl = path(&d, "flyer template.vctemplate");
    s.execute("file.saveAsTemplate", &json!({"path": tpl})).unwrap();
    assert_eq!(s.doc().unwrap().path.as_deref(), Some(source.as_str()));
    assert!(s.doc().unwrap().is_dirty());
    let r = s.execute("document.open", &json!({"path": tpl})).unwrap();
    assert_eq!(r["format"], "template");
    assert!(r["title"].as_str().unwrap().starts_with("Untitled-"), "{r}");
    assert_eq!(s.doc().unwrap().path, None);
    assert!(!s.doc().unwrap().doc.template);
    // New from Template opens any file untitled.
    let r = s.execute("file.newFromTemplate", &json!({"path": source})).unwrap();
    assert!(r["title"].as_str().unwrap().starts_with("Untitled-"), "{r}");
    assert_eq!(s.doc().unwrap().path, None);
    // With no preference the folder is under the user's home (where there is one).
    s.prefs.templates_folder.clear();
    if let Some(f) = crate::cmd::fileio::templates_folder(&s.prefs) {
        assert!(f.ends_with("VectorCraft Templates"), "{f}");
    }
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn save_plans_suggest_names_and_folders() {
    let mut s = session();
    s.execute("file.info", &json!({"title": "Map"})).unwrap();
    let plan = |s: &Session, mode, p: Value| save_plan(s, mode, &p).unwrap();
    let p = plan(&s, SaveMode::Save, json!({}));
    assert_eq!((p.path, p.name.as_str(), p.format.id, p.folder), (None, "Map.vectorcraft", "vectorcraft", None));
    assert_eq!(plan(&s, SaveMode::Copy, json!({"format": "pdf"})).name, "Map copy.pdf");
    assert_eq!(plan(&s, SaveMode::Template, json!({"format": "svg"})).format.id, "template", "a template is always native");
    assert_eq!(SaveMode::of("file.saveCopy"), Some(SaveMode::Copy));
    assert_eq!(crate::cmd::fileio::save_filters("svg")[0], ("SVG", &["svg"][..]));
}

#[test]
fn a_pdf_opened_in_part_is_not_saved_back_over_the_whole_file() {
    let d = dir("pdfpart");
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100, "artboards": 2})).unwrap();
    rect(&mut s);
    let pdf = path(&d, "two.pdf");
    s.execute("document.export", &json!({"path": pdf, "format": "pdf"})).unwrap();
    // The whole file: Save writes it back as PDF.
    s.execute("document.open", &json!({"path": pdf})).unwrap();
    let st = s.doc().unwrap();
    assert_eq!((st.path.as_deref(), st.format), (Some(pdf.as_str()), "pdf"));
    // One page of it: Save asks for a name instead of dropping the other page.
    s.execute("document.open", &json!({"path": pdf, "pages": "1"})).unwrap();
    let st = s.doc().unwrap();
    assert_eq!((st.path.as_deref(), st.format), (None, "vectorcraft"));
    assert!(s.execute("document.save", &json!({})).unwrap().get("dataBase64").is_some());
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn a_template_places_like_a_native_file() {
    let mut s = session();
    rect(&mut s);
    let tpl = s.execute("document.serialize", &json!({"format": "template"})).unwrap();
    s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
    let empty = s.doc().unwrap().doc.node_count();
    s.execute("file.place", &json!({"name": "card.vctemplate", "dataBase64": tpl["dataBase64"]})).unwrap();
    let st = s.doc().unwrap();
    assert!(st.doc.node_count() > empty && !st.selection.is_empty(), "the template's art is placed and selected");
}
