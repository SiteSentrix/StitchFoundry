//! File I/O through the injected services (pick, read, write, download); the engine's `fileio`
//! decodes and encodes every format.

use serde_json::Value;
use vectorcraft_doc::{ImageObject, Node, NodeKind};
use vectorcraft_engine::cmd::fileio;
use vectorcraft_geom::Affine;

use crate::VectorcraftApp;

/// Open bytes of any readable format as a new document (templates open untitled).
pub fn open_bytes(app: &mut VectorcraftApp, name: &str, bytes: &[u8], path: Option<String>) -> Result<(), String> {
    let r = fileio::open_bytes(&mut app.session, name, bytes, path).map_err(|e| e.to_string())?;
    app.sync_views();
    if let Some(w) = r["warnings"].as_array().filter(|w| !w.is_empty()) {
        app.status(format!("Opened with {} note(s): {}", w.len(), w[0].as_str().unwrap_or_default()));
    }
    Ok(())
}

/// File → Open…
pub fn open_dialog(app: &mut VectorcraftApp) -> Result<(), String> {
    if let Some(f) = app.services.open_async.as_mut() {
        f();
        return Ok(());
    }
    let path = app.services.pick_open.as_mut().and_then(|f| f()).ok_or("cancelled")?;
    open_path(app, &path)
}

pub fn open_path(app: &mut VectorcraftApp, path: &str) -> Result<(), String> {
    let read = app.services.read.as_ref().ok_or("no file reader")?;
    let bytes = read(path)?;
    open_bytes(app, path, &bytes, Some(path.to_string()))?;
    note_recent(app, path);
    Ok(())
}

fn write_out(app: &mut VectorcraftApp, path: &str, bytes: &[u8]) -> Result<(), String> {
    if let Some(dl) = app.services.download.as_mut() {
        let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(path.to_string());
        dl(&name, bytes);
        return Ok(());
    }
    let w = app.services.write.as_mut().ok_or("no file writer")?;
    w(path, bytes)
}

fn suggested(app: &VectorcraftApp, ext: &str) -> String {
    let t = app.session.active().map(|d| d.title()).unwrap_or_else(|| "Untitled".into());
    let stem = std::path::Path::new(&t).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or(t);
    format!("{stem}.{ext}")
}

/// `path`, else a suggested name (web download) or one picked in a save dialog.
fn target_path(app: &mut VectorcraftApp, path: Option<String>, ext: &str) -> Result<String, String> {
    match path {
        Some(p) => Ok(p),
        None if app.services.download.is_some() => Ok(suggested(app, ext)),
        None => {
            let s = suggested(app, ext);
            app.services.pick_save.as_mut().and_then(|f| f(&s)).ok_or_else(|| "cancelled".into())
        }
    }
}

/// File → Save / Save As (native format).
pub fn save(app: &mut VectorcraftApp, path: Option<String>, save_as: bool) -> Result<String, String> {
    let st = app.session.active().ok_or("no document")?;
    let existing = if save_as { None } else { st.path.clone() };
    let path = target_path(app, path.or(existing), vectorcraft_format::EXTENSION)?;
    let doc = app.session.active().ok_or("no document")?.doc.clone();
    let bytes = fileio::encode(&doc, "vectorcraft", &Value::Null).map_err(|e| e.to_string())?;
    write_out(app, &path, &bytes)?;
    if let Some(st) = app.session.active_mut() {
        st.path = Some(path.clone());
        st.mark_saved();
    }
    app.status(format!("Saved {path}"));
    note_recent(app, &path);
    Ok(path)
}

/// Put `path` at the top of File → Open Recent Files (capped by Preferences → Recent Files).
pub fn note_recent(app: &mut VectorcraftApp, path: &str) {
    let r = &mut app.ui.recent_files;
    r.retain(|p| p != path);
    r.insert(0, path.to_string());
    r.truncate((app.session.prefs.recent_files_count as usize).clamp(1, 10));
}

/// Export the active document in `format` (default: the path's extension, else PNG) with the
/// `document.export` options in `params` (artboard, range, scale…). The document keeps its path.
pub fn export(app: &mut VectorcraftApp, format: Option<&str>, path: Option<String>, params: &Value) -> Result<String, String> {
    let f = fileio::writable_format(format, path.as_deref())?;
    let doc = app.session.active().ok_or("no document")?.doc.clone();
    let path = target_path(app, path, f.extensions[0])?;
    let bytes = fileio::encode(&doc, f.id, params).map_err(|e| e.to_string())?;
    write_out(app, &path, &bytes)?;
    app.status(format!("Exported {path}"));
    Ok(path)
}

/// Run an engine command that returns `{dataBase64}` and write the bytes to a picked path
/// (Export Selection, Save as Template).
pub fn save_command_output(app: &mut VectorcraftApp, id: &str, ext: &str, mut params: Value) -> Result<String, String> {
    let path = target_path(app, params.get("path").and_then(Value::as_str).map(str::to_string), ext)?;
    if let Some(o) = params.as_object_mut() {
        o.remove("path");
        if o.get("format").is_none() && id == "document.exportSelection" {
            let e = Some(fileio::extension(&path)).filter(|e| !e.is_empty()).unwrap_or_else(|| ext.into());
            o.insert("format".into(), serde_json::json!(e));
        }
    }
    let v = app.session.execute(id, &params).map_err(|e| e.to_string())?;
    let bytes = v["dataBase64"].as_str().and_then(vectorcraft_format::base64_decode).ok_or("no data")?;
    write_out(app, &path, &bytes)?;
    app.status(format!("Saved {path}"));
    Ok(path)
}

/// File → Place… (embed an image or SVG into the active document).
pub fn place_bytes(app: &mut VectorcraftApp, name: &str, bytes: &[u8]) -> Result<(), String> {
    let f = fileio::detect(name, bytes).ok_or_else(|| format!("can't place `{name}`: not a format VectorCraft reads"))?;
    if !f.raster {
        if !matches!(f.id, "svg" | "svgz") {
            return Err(format!("Place doesn't take {} files yet", f.label));
        }
        let src = fileio::load(name, bytes).map_err(|e| e.to_string())?.doc;
        let nodes: Vec<Node> = src.layers.iter().flat_map(|l| l.children().cloned().unwrap_or_default()).map(|n| (*n).clone()).collect();
        app.session.clipboard = nodes;
        // Straight to the engine: `app.run` would let the system clipboard replace these nodes.
        app.session.execute("edit.pasteInPlace", &serde_json::json!({})).map_err(|e| e.to_string())?;
        app.sync_views();
        return Ok(());
    }
    let fileio::RasterImage { key, blob, width: w, height: h } = fileio::raster_image(bytes).map_err(|e| e.to_string())?;
    let st = app.session.active().ok_or("no document")?;
    let ab = st.doc.artboards.first().map(|a| a.rect).unwrap_or_default();
    let s = (ab.width() / w as f64).min(ab.height() / h as f64).min(1.0);
    let xf = Affine::translate((ab.center().x - w as f64 * s / 2.0, ab.center().y - h as f64 * s / 2.0)) * Affine::scale(s);
    let parent = st.insertion_parent();
    let name = name.to_string();
    app.session
        .edit("Place", |d, sel| {
            d.images.insert(key.clone(), blob);
            let id = d.alloc_id();
            let mut n = Node::new(id, NodeKind::Image(ImageObject { key, width: w, height: h, xf, link: None }));
            n.name = Some(name);
            d.insert(parent, usize::MAX, n)?;
            sel.set([id]);
            Ok(())
        })
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;
    use crate::Services;

    type Written = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

    /// An app whose writer records what it is given.
    fn app() -> (VectorcraftApp, Written) {
        let written = Written::default();
        let w = written.clone();
        let services = Services {
            write: Some(Box::new(move |p: &str, b: &[u8]| {
                w.borrow_mut().push((p.to_string(), b.to_vec()));
                Ok(())
            })),
            ..Default::default()
        };
        (VectorcraftApp::new(Session::new(), services), written)
    }

    fn bytes_of(app: &mut VectorcraftApp, cmd: &str, p: Value) -> Vec<u8> {
        let v = app.session.execute(cmd, &p).unwrap();
        vectorcraft_format::base64_decode(v["dataBase64"].as_str().unwrap()).unwrap()
    }

    fn image_size(app: &VectorcraftApp, id: vectorcraft_doc::NodeId) -> (u32, u32) {
        match &app.session.doc().unwrap().doc.node(id).unwrap().kind {
            NodeKind::Image(im) => (im.width, im.height),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn webp_opens_and_places_at_its_pixel_size() {
        let (mut app, _) = app();
        app.session.execute("file.new", &json!({"width": 3, "height": 2})).unwrap();
        let webp = bytes_of(&mut app, "document.serialize", json!({"format": "webp"}));
        open_bytes(&mut app, "tiny.webp", &webp, None).unwrap();
        assert_eq!(app.session.documents().len(), 2);
        assert_eq!(app.views.len(), 2, "views follow the documents");
        let first = app.session.doc().unwrap().doc.layers[0].children().unwrap()[0].id;
        assert_eq!(image_size(&app, first), (3, 2));
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        place_bytes(&mut app, "tiny.webp", &webp).unwrap();
        let placed = app.session.doc().unwrap().selection.objects[0];
        assert_eq!(image_size(&app, placed), (3, 2));
        assert!(place_bytes(&mut app, "x.txt", b"hello").is_err());
    }

    #[test]
    fn templates_open_untitled() {
        let (mut app, _) = app();
        app.session.execute("file.new", &json!({"width": 50, "height": 50})).unwrap();
        let pdf = bytes_of(&mut app, "document.serialize", json!({"format": "pdf"}));
        open_bytes(&mut app, "brochure.ait", &pdf, Some("/tmp/brochure.ait".into())).unwrap();
        let st = app.session.active().unwrap();
        assert!(st.title().starts_with("Untitled-"), "{}", st.title());
        assert_eq!(st.path, None);
    }

    #[test]
    fn export_uses_the_engine_options_and_keeps_the_path() {
        let (mut app, written) = app();
        app.session.execute("file.new", &json!({"width": 100, "height": 50, "artboards": 2})).unwrap();
        app.session.execute("artboard.setProps", &json!({"index": 1, "width": 40})).unwrap();
        app.session.doc_mut().unwrap().path = Some("/tmp/doc.vectorcraft".into());
        export(&mut app, None, Some("/tmp/b.png".into()), &json!({"artboard": 1, "scale": 2})).unwrap();
        export(&mut app, None, Some("/tmp/copy.vectorcraft".into()), &Value::Null).unwrap();
        let w = written.borrow();
        assert_eq!(&w[0].1[16..20], 80u32.to_be_bytes(), "artboard 1 (40 pt) at scale 2");
        assert!(vectorcraft_format::sniff(&w[1].1));
        assert_eq!(app.session.active().unwrap().path.as_deref(), Some("/tmp/doc.vectorcraft"));
        drop(w);
        assert!(export(&mut app, None, Some("/tmp/x.bmp".into()), &Value::Null).is_err(), "BMP is read-only");
    }

    #[test]
    fn export_for_screens_with_params_runs_the_engine() {
        let (mut app, _) = app();
        app.session.execute("file.new", &json!({"width": 60, "height": 40, "artboards": 3})).unwrap();
        let r = app.run("file.exportForScreens", json!({"range": "2-3", "formats": [{"format": "pdf"}]})).unwrap();
        let files = r["files"].as_array().unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0]["name"], "Artboard-2.pdf");
        assert!(app.ui.dialog.is_none());
        app.run("file.exportForScreens", Value::Null).unwrap();
        assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some("exportForScreens"));
    }

    #[test]
    fn control_export_without_path_returns_bytes() {
        let (mut app, written) = app();
        app.session.execute("file.new", &json!({"width": 30, "height": 20})).unwrap();
        let (req, _) = crate::control::ControlRequest::new("app.export", json!({"format": "png", "scale": 2}));
        let crate::control::Outcome::Done(r) = crate::control::handle(&mut app, &egui::Context::default(), &req) else { panic!("not done") };
        assert_eq!(r["ok"], true, "{r}");
        let png = vectorcraft_format::base64_decode(r["result"]["dataBase64"].as_str().unwrap()).unwrap();
        assert_eq!(&png[16..20], 60u32.to_be_bytes(), "30 pt at scale 2");
        assert!(written.borrow().is_empty(), "no file and no save dialog");
    }
}
