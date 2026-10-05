//! File I/O through the injected services (pick, read, write, download); the engine's `fileio`
//! decodes and encodes every format.

use serde_json::{Value, json};
use vectorcraft_engine::cmd::fileio;

use crate::VectorcraftApp;
use crate::dialogs::svg_options;

/// Open bytes of any readable format as a new document (templates open untitled); swatch and
/// graphic style library files open in the library panel and flattener presets files are imported.
pub fn open_bytes(app: &mut VectorcraftApp, name: &str, bytes: &[u8], path: Option<String>) -> Result<(), String> {
    let ext = fileio::extension(name);
    if vectorcraft_engine::cmd::flatten::PRESET_EXTS.contains(&ext.as_str()) {
        let r = app.run("flattener.presets.import", serde_json::json!({"data": String::from_utf8_lossy(bytes)}))?;
        let names: Vec<&str> = r["imported"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        app.status(format!("Imported flattener presets: {}", names.join(", ")));
        return Ok(());
    }
    let swatches = vectorcraft_engine::cmd::swatchlib::LIBRARY_EXTS.contains(&ext.as_str());
    if swatches || ext == vectorcraft_doc::style_libs::STYLES_EXT {
        let p = match path {
            Some(path) => serde_json::json!({ "path": path }),
            None => serde_json::json!({"name": name, "data": String::from_utf8_lossy(bytes)}),
        };
        let load = if swatches { crate::panels::swatches::load_library } else { crate::panels::graphic_styles::load_library };
        return load(app, p).map(|_| ());
    }
    // A PDF with several pages or a password asks first (the Import PDF dialog).
    if crate::dialogs::import_pdf::offer(app, name, bytes, path.clone(), None) {
        return Ok(());
    }
    open_document(app, name, bytes, path, &Value::Null)
}

/// Open a document through the engine loader with the `document.open` options in `p`.
pub fn open_document(app: &mut VectorcraftApp, name: &str, bytes: &[u8], path: Option<String>, p: &Value) -> Result<(), String> {
    let r = fileio::open_bytes_with(&mut app.session, name, bytes, path, p).map_err(|e| e.to_string())?;
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
        dl(&fileio::file_name(path), bytes);
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
pub(crate) fn target_path(app: &mut VectorcraftApp, path: Option<String>, ext: &str) -> Result<String, String> {
    match path {
        Some(p) => Ok(p),
        None if app.services.download.is_some() => Ok(suggested(app, ext)),
        None => {
            let s = suggested(app, ext);
            app.services.pick_save.as_mut().and_then(|f| f(&s)).ok_or_else(|| "cancelled".into())
        }
    }
}

/// Write every file of an export (one per artboard, linked images) for the destination `path`.
/// → the export's own files (without the linked images).
fn write_encoded(app: &mut VectorcraftApp, doc: &vectorcraft_doc::Document, path: &str, enc: &fileio::Encoded) -> Result<Vec<String>, String> {
    let named = enc.named(doc, path);
    for (p, bytes) in &named {
        write_out(app, p, bytes)?;
    }
    Ok(named.into_iter().take(enc.files.len()).map(|(p, _)| p).collect())
}

fn format_param(p: &Value) -> Option<&str> {
    p.get("format").and_then(Value::as_str)
}

/// File → Save / Save As: native, or SVG for an .svg path (with the SVG options in `params`, else
/// the ones the document was last saved with).
pub fn save(app: &mut VectorcraftApp, path: Option<String>, save_as: bool, params: &Value) -> Result<String, String> {
    let st = app.session.active().ok_or("no document")?;
    let existing = if save_as { None } else { st.path.clone() };
    let path = target_path(app, path.or(existing), vectorcraft_format::EXTENSION)?;
    let f = fileio::save_format(format_param(params), Some(&path))?;
    fileio::stamp_save_dates(app.session.active_mut().ok_or("no document")?);
    let st = app.session.active().ok_or("no document")?;
    let (enc, opts) = fileio::save_encoding(st, f, params).map_err(|e| e.to_string())?;
    let doc = st.doc.clone();
    write_encoded(app, &doc, &path, &enc)?;
    if let Some(st) = app.session.active_mut() {
        st.path = Some(path.clone());
        st.save_options = opts;
        st.mark_saved();
    }
    app.status(format!("Saved {path}"));
    note_recent(app, &path);
    Ok(path)
}

/// File → Save As / Save a Copy: the path (asked for when missing); an SVG path without SVG
/// options in `p` opens SVG Options, whose OK saves. A copy leaves the document's path alone.
pub fn save_as(app: &mut VectorcraftApp, copy: bool, p: &Value) -> Result<Value, String> {
    let path = target_path(app, p.get("path").and_then(Value::as_str).map(str::to_string), vectorcraft_format::EXTENSION)?;
    let f = fileio::save_format(format_param(p), Some(&path))?;
    let given = p.get("svg").is_some_and(|v| !v.is_null()) || !fileio::svg_options(p)?.is_empty();
    if f.id != "vectorcraft" && !given {
        let mode = if copy { svg_options::Mode::SaveCopy } else { svg_options::Mode::Save };
        svg_options::open(app, mode, Some(&path));
        return Ok(json!({ "dialog": svg_options::KIND, "path": path }));
    }
    if !copy {
        return save(app, Some(path), true, p).map(|p| json!({ "path": p }));
    }
    let doc = app.session.active().ok_or("no document")?.doc.clone();
    let enc = fileio::encode_all(&doc, f.id, p).map_err(|e| e.to_string())?;
    write_encoded(app, &doc, &path, &enc)?;
    app.status(format!("Saved a copy as {path}"));
    Ok(json!({ "path": path }))
}

/// Put `path` at the top of File → Open Recent Files (capped by Preferences → Recent Files).
pub fn note_recent(app: &mut VectorcraftApp, path: &str) {
    let r = &mut app.ui.recent_files;
    r.retain(|p| p != path);
    r.insert(0, path.to_string());
    r.truncate((app.session.prefs.recent_files_count as usize).clamp(1, 10));
}

/// Export the active document in `format` (default: the path's extension, else PNG) with the
/// `document.export` options in `params` (artboard, range, useArtboards, ppi, SVG options…):
/// every file it writes (one per artboard, linked images) goes next to `path`. The document keeps
/// its path. → `{path, warnings, files?}` (`files`: every file when there are several).
pub fn export(app: &mut VectorcraftApp, format: Option<&str>, path: Option<String>, params: &Value) -> Result<Value, String> {
    let f = fileio::writable_format(format, path.as_deref())?;
    let doc = app.session.active().ok_or("no document")?.doc.clone();
    let path = target_path(app, path, f.extensions[0])?;
    // An SVG given a .svgz name is written compressed.
    let f = match fileio::format_for_name(&path) {
        Some(z) if f.id == "svg" && z.id == "svgz" => z,
        _ => f,
    };
    let enc = fileio::encode_all(&doc, f.id, params).map_err(|e| e.to_string())?;
    let files = write_encoded(app, &doc, &path, &enc)?;
    let what = match files.as_slice() {
        [one] => one.clone(),
        _ => format!("{} files", files.len()),
    };
    app.status(match enc.warnings.first() {
        Some(first) => format!("Exported {what} with {} note(s): {first}", enc.warnings.len()),
        None => format!("Exported {what}"),
    });
    let mut out = json!({ "path": files.first().unwrap_or(&path), "warnings": enc.warnings });
    if files.len() > 1 {
        out["files"] = json!(files);
    }
    Ok(out)
}

/// Run an engine command that returns `{dataBase64}` and write the bytes to a picked path
/// (Export Selection, Save as Template).
pub fn save_command_output(app: &mut VectorcraftApp, id: &str, ext: &str, params: Value) -> Result<String, String> {
    let (path, _) = run_to_file(app, id, ext, params)?;
    app.status(format!("Saved {path}"));
    Ok(path)
}

/// Run an engine command that returns `{dataBase64}` without its `path` and write the bytes to
/// that path (else a picked or suggested name) → (path, the command's result without the data).
/// The command runs before a path is asked for, so bad params never open a save dialog, except
/// for Export Selection without a `format`, which takes it from the picked path.
fn run_to_file(app: &mut VectorcraftApp, id: &str, ext: &str, mut params: Value) -> Result<(String, Value), String> {
    let mut path = params.as_object_mut().and_then(|o| o.remove("path")).and_then(|p| p.as_str().map(str::to_string));
    if let Some(o) = params.as_object_mut()
        && o.get("format").is_none()
        && id == "document.exportSelection"
    {
        let picked = target_path(app, path, ext)?;
        let e = Some(fileio::extension(&picked)).filter(|e| !e.is_empty()).unwrap_or_else(|| ext.into());
        o.insert("format".into(), serde_json::json!(e));
        path = Some(picked);
    }
    let mut v = app.session.execute(id, &params).map_err(|e| e.to_string())?;
    // Binary output comes as base64, text (swatch libraries) as is; the result keeps the rest.
    let o = v.as_object_mut().ok_or("no data")?;
    let bytes = match (o.remove("dataBase64"), o.remove("data")) {
        (Some(Value::String(b64)), _) => vectorcraft_format::base64_decode(&b64),
        (_, Some(Value::String(text))) => Some(text.into_bytes()),
        _ => None,
    };
    let bytes = bytes.ok_or("no data")?;
    let path = match path {
        Some(p) => p,
        None => target_path(app, None, ext)?,
    };
    write_out(app, &path, &bytes)?;
    Ok((path, v))
}

/// File → Save as PDF: `document.exportPdf` with `params` (the Save PDF dialog's options), written
/// to `path` (else a picked or suggested name) → `{path, bytes, warnings}`. With
/// `viewAfterSaving`, the written file opens in the system viewer (not on the web, which
/// downloads it).
pub fn export_pdf(app: &mut VectorcraftApp, params: Value) -> Result<Value, String> {
    let view = params.get("viewAfterSaving").and_then(Value::as_bool).unwrap_or(false);
    let (path, mut v) = run_to_file(app, "document.exportPdf", "pdf", params)?;
    if view && app.services.download.is_none() {
        app.open_url(&file_url(&path));
    }
    let warnings: Vec<&str> = v["warnings"].as_array().map(|w| w.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    app.status(match warnings.first() {
        Some(first) => format!("Saved {path} with {} note(s): {first}", warnings.len()),
        None => format!("Saved {path}"),
    });
    v["path"] = Value::String(path);
    Ok(v)
}

/// `path` as an absolute `file://` URL for the system opener (bytes other than letters, digits and
/// `/-._~:` percent-encoded).
fn file_url(path: &str) -> String {
    let abs = std::path::absolute(path).map_or_else(|_| path.to_string(), |p| p.to_string_lossy().into_owned());
    let abs = abs.replace('\\', "/");
    let mut url = String::from(if abs.starts_with('/') { "file://" } else { "file:///" });
    for b in abs.bytes() {
        if b.is_ascii_alphanumeric() || b"/-._~:".contains(&b) {
            url.push(char::from(b));
        } else {
            url.push_str(&format!("%{b:02X}"));
        }
    }
    url
}

/// Place a file's bytes (no path, so embedded) centred in the view: `file.place`.
pub fn place_bytes(app: &mut VectorcraftApp, name: &str, bytes: &[u8]) -> Result<(), String> {
    crate::place::run(app, &serde_json::json!({ "name": name, "dataBase64": vectorcraft_format::base64_encode(bytes) })).map(|_| ())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;
    use crate::Services;
    use vectorcraft_doc::NodeKind;

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
    fn export_selection_takes_its_format_from_the_picked_path() {
        let (mut app, written) = app();
        app.session.execute("file.new", &json!({"width": 60, "height": 40})).unwrap();
        app.session.execute("shape.rectangle", &json!({"x": 5, "y": 5, "width": 20, "height": 10})).unwrap();
        app.services.pick_save = Some(Box::new(|_: &str| Some("/tmp/sel.svg".into())));
        assert_eq!(app.run("document.exportSelection", json!({})).unwrap()["path"], "/tmp/sel.svg");
        app.run("document.exportSelection", json!({"path": "/tmp/sel.png"})).unwrap();
        let w = written.borrow();
        assert!(String::from_utf8_lossy(&w[0].1).contains("<svg"), "SVG from the picked name");
        assert_eq!(&w[1].1[1..4], b"PNG", "PNG from the given path");
    }

    #[test]
    fn written_files_open_as_file_urls() {
        let url = super::file_url("/tmp/My Art #1.pdf");
        assert!(url.starts_with("file:///") && url.ends_with("/tmp/My%20Art%20%231.pdf"), "{url}");
        assert!(!url.contains('\\'), "{url}");
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
