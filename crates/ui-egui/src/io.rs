//! File I/O through the injected services: open, save, export, place.

use std::sync::Arc;

use vectorcraft_doc::{Document, ImageBlob, ImageObject, Node, NodeKind};
use vectorcraft_geom::Affine;

use crate::VectorcraftApp;

fn ext(name: &str) -> String {
    std::path::Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// Decode an image and make a new document (or place into the active one when `place`).
fn image_doc(name: &str, bytes: &[u8]) -> Result<Document, String> {
    let (w, h) = image_size(bytes)?;
    let mut d = Document::new(w as f64, h as f64);
    d.title = name.to_string();
    let key = format!("img{:016x}", hash(bytes));
    d.images.insert(key.clone(), ImageBlob { mime: mime_of(name), bytes: Arc::new(bytes.to_vec()) });
    let l = d.layers[0].id;
    let id = d.alloc_id();
    let mut n = Node::new(id, NodeKind::Image(ImageObject { key, width: w, height: h, xf: Affine::IDENTITY, link: None }));
    n.name = Some(name.to_string());
    d.insert(Some(l), 0, n).map_err(|e| e.to_string())?;
    Ok(d)
}

pub fn image_size(bytes: &[u8]) -> Result<(u32, u32), String> {
    // PNG / JPEG / GIF / WebP headers — enough to size a placed image without decoding.
    if bytes.len() > 24 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" {
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        return Ok((w, h));
    }
    if bytes.len() > 10 && &bytes[..6] == b"GIF89a" || bytes.len() > 10 && &bytes[..6] == b"GIF87a" {
        return Ok((u16::from_le_bytes([bytes[6], bytes[7]]) as u32, u16::from_le_bytes([bytes[8], bytes[9]]) as u32));
    }
    if bytes.len() > 4 && bytes[0] == 0xFF && bytes[1] == 0xD8 {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xFF {
                i += 1;
                continue;
            }
            let marker = bytes[i + 1];
            let len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            if (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC {
                let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                return Ok((w, h));
            }
            i += 2 + len;
        }
    }
    Err("unsupported image format".into())
}

fn mime_of(name: &str) -> String {
    match ext(name).as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    }
    .to_string()
}

fn hash(b: &[u8]) -> u64 {
    // FNV-1a: stable content key for embedded images.
    let mut h: u64 = 0xcbf29ce484222325;
    for x in b {
        h ^= *x as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Open bytes as a new document.
pub fn open_bytes(app: &mut VectorcraftApp, name: &str, bytes: &[u8], path: Option<String>) -> Result<(), String> {
    let e = ext(name);
    let (doc, keep_path) = if vectorcraft_format::is_native_ext(&e) || vectorcraft_format::sniff(bytes) {
        let mut d = vectorcraft_format::load(bytes).map_err(|e| e.to_string())?;
        if d.template {
            // Templates open as a new untitled document.
            d.template = false;
            d.title = app.session.next_untitled();
            (d, false)
        } else {
            if d.title.is_empty() {
                d.title = name.to_string();
            }
            (d, true)
        }
    } else if e == "svg" || bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg") {
        let s = std::str::from_utf8(bytes).map_err(|_| "SVG is not UTF-8".to_string())?;
        let (mut d, warnings) = vectorcraft_svg::import_with_report(s).map_err(|e| e.to_string())?;
        d.title = name.to_string();
        if !warnings.is_empty() {
            app.status(format!("Opened with {} warning(s): {}", warnings.len(), warnings.first().cloned().unwrap_or_default()));
        }
        (d, false)
    } else if e == "pdf" || e == "ai" || bytes.starts_with(b"%PDF") {
        let vectorcraft_pdf::ImportReport { document: mut d, warnings } =
            vectorcraft_pdf::import_with_report(bytes, &vectorcraft_pdf::ImportOptions::default()).map_err(|e| e.to_string())?;
        d.title = name.to_string();
        if !warnings.is_empty() {
            app.status(format!("Opened with {} note(s): {}", warnings.len(), warnings.first().cloned().unwrap_or_default()));
        }
        (d, false)
    } else if ["png", "jpg", "jpeg", "gif", "webp"].contains(&e.as_str()) {
        (image_doc(name, bytes)?, false)
    } else {
        return Err(format!("VectorCraft can't open .{e} files yet"));
    };
    app.session.add_document(doc, if keep_path { path } else { None });
    app.sync_views();
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
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or(path.to_string());
    open_bytes(app, &name, &bytes, Some(path.to_string()))?;
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

/// File → Save / Save As (native format).
pub fn save(app: &mut VectorcraftApp, path: Option<String>, save_as: bool) -> Result<String, String> {
    let st = app.session.active().ok_or("no document")?;
    let existing = if save_as { None } else { st.path.clone() };
    let path = match path.or(existing) {
        Some(p) => p,
        None if app.services.download.is_some() => suggested(app, "vectorcraft"),
        None => {
            let s = suggested(app, "vectorcraft");
            app.services.pick_save.as_mut().and_then(|f| f(&s)).ok_or("cancelled")?
        }
    };
    let bytes = vectorcraft_format::save_file(&app.session.active().unwrap().doc);
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

/// Export the document as SVG / PDF / PNG / JPEG / WebP.
pub fn export(app: &mut VectorcraftApp, format: &str, path: Option<String>, scale: f64) -> Result<String, String> {
    let st = app.session.active().ok_or("no document")?;
    let doc = st.doc.clone();
    let path = match path {
        Some(p) => p,
        None if app.services.download.is_some() => suggested(app, format),
        None => {
            let s = suggested(app, format);
            app.services.pick_save.as_mut().and_then(|f| f(&s)).ok_or("cancelled")?
        }
    };
    let bytes = match format {
        "svg" => vectorcraft_svg::export(&doc, &vectorcraft_svg::ExportOptions { artboard: Some(0), ..Default::default() }).into_bytes(),
        "pdf" => vectorcraft_engine::export_pdf(&doc, &vectorcraft_pdf::PdfOptions::default()).map_err(|e| e.to_string())?,
        "png" | "jpg" | "jpeg" | "webp" => {
            let r = doc.artboards.first().map(|a| a.rect).ok_or("no artboard")?;
            vectorcraft_render::raster_size(r, scale)?;
            let img = app.canvas.renderer.render_region(&doc, r, scale, format == "jpg" || format == "jpeg");
            match format {
                "png" => img.to_png(),
                "webp" => img.to_webp(),
                _ => img.to_jpeg(90),
            }
        }
        other => return Err(format!("unknown export format `{other}`")),
    };
    write_out(app, &path, &bytes)?;
    app.status(format!("Exported {path}"));
    Ok(path)
}

/// Run an engine command that returns `{dataBase64}` and write the bytes to a picked path
/// (Export Selection, Save as Template).
pub fn save_command_output(app: &mut VectorcraftApp, id: &str, ext: &str, mut params: serde_json::Value) -> Result<String, String> {
    let path = match params.get("path").and_then(serde_json::Value::as_str) {
        Some(p) => p.to_string(),
        None if app.services.download.is_some() => suggested(app, ext),
        None => {
            let s = suggested(app, ext);
            app.services.pick_save.as_mut().and_then(|f| f(&s)).ok_or("cancelled")?
        }
    };
    if let Some(o) = params.as_object_mut() {
        o.remove("path");
        if o.get("format").is_none() && id == "document.exportSelection" {
            let e = std::path::Path::new(&path).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| ext.into());
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
    let e = ext(name);
    if e == "svg" {
        let s = std::str::from_utf8(bytes).map_err(|_| "SVG is not UTF-8".to_string())?;
        let src = vectorcraft_svg::import(s).map_err(|e| e.to_string())?;
        let nodes: Vec<Node> = src.layers.iter().flat_map(|l| l.children().cloned().unwrap_or_default()).map(|n| (*n).clone()).collect();
        app.session.clipboard = nodes;
        // Straight to the engine: `app.run` would let the system clipboard replace these nodes.
        app.session.execute("edit.pasteInPlace", &serde_json::json!({})).map_err(|e| e.to_string())?;
        app.sync_views();
        return Ok(());
    }
    let (w, h) = image_size(bytes)?;
    let key = format!("img{:016x}", hash(bytes));
    let st = app.session.active().ok_or("no document")?;
    let ab = st.doc.artboards.first().map(|a| a.rect).unwrap_or_default();
    let s = (ab.width() / w as f64).min(ab.height() / h as f64).min(1.0);
    let xf = Affine::translate((ab.center().x - w as f64 * s / 2.0, ab.center().y - h as f64 * s / 2.0)) * Affine::scale(s);
    let mime = mime_of(name);
    let bytes = Arc::new(bytes.to_vec());
    let parent = st.insertion_parent();
    let name = name.to_string();
    app.session
        .edit("Place", |d, sel| {
            d.images.insert(key.clone(), ImageBlob { mime, bytes });
            let id = d.alloc_id();
            let mut n = Node::new(id, NodeKind::Image(ImageObject { key, width: w, height: h, xf, link: None }));
            n.name = Some(name);
            d.insert(parent, usize::MAX, n)?;
            sel.set([id]);
            Ok(())
        })
        .map_err(|e| e.to_string())
}
