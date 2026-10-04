//! Document I/O and transactions: open/save/export by path or bytes (base64), and `command.batch`.
//!
//! These live in the engine so every frontend (UI, CLI, MCP headless) shares them.

use serde_json::{Value, json};
use vectorcraft_doc::Document;

use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "document.open",
            "Open Document",
            [],
            None,
            "{path} or {name, dataBase64} → {index}; .vectorcraft, .svg, .pdf, .ai (PDF-compatible)",
            always,
            open
        ),
        cmd!("document.save", "Save Document", [], None, "{path?} native .vectorcraft (default: the document's path) → {path}", has_doc, save),
        cmd!(query "document.serialize", "Serialize Document", [], None, "{format: vectorcraft|svg|pdf|png} → {dataBase64 | text}", has_doc, serialize),
        cmd!(
            "document.export",
            "Export Document",
            [],
            None,
            "{format: svg|png|pdf|jpg|webp|vectorcraft, path, scale?: 1, artboard?: 0, outlineText?: bool (SVG: text as outlines)}",
            has_doc,
            export
        ),
        cmd!(
            "document.exportSelection",
            "Export Selection…",
            ["File"],
            None,
            "{path?, format?: png|jpg|webp|svg|pdf (default: from the extension, else png), scale?: 1} the selected objects cropped to their bounds → {path, bytes, bounds} (no path → {dataBase64, bounds})",
            has_selection,
            export_selection
        ),
        cmd!(
            "file.saveAsTemplate",
            "Save as Template…",
            ["File"],
            None,
            "{path?} a native copy that opens as a new untitled document (no path → {dataBase64})",
            has_doc,
            save_template
        ),
        cmd!(
            "document.exportForScreens",
            "Export for Screens",
            ["File", "Export"],
            None,
            "{folder, artboards?: [index…] (default all), formats?: [{format: png|jpg|webp|svg|pdf, scale?: 1, suffix?: \"@2x\"}], prefix?} → {files: [...]}",
            has_doc,
            export_for_screens
        ),
        cmd!(query "command.batch", "Batch", [], None, "{label?, commands: [{command, params}]} run several commands as ONE undo step; stops at the first error and rolls back", has_doc, batch),
    ]
}

fn load(name: &str, bytes: &[u8]) -> Result<Document> {
    let lower = name.to_ascii_lowercase();
    if vectorcraft_format::is_native_name(&lower) || vectorcraft_format::sniff(bytes) {
        return vectorcraft_format::load(bytes).map_err(|e| EngineError::Other(e.to_string()));
    }
    if lower.ends_with(".svg") || bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg") {
        let s = std::str::from_utf8(bytes).map_err(|_| EngineError::Other("SVG is not UTF-8".into()))?;
        let mut d = vectorcraft_svg::import(s).map_err(|e| EngineError::Other(e.to_string()))?;
        d.title = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| name.to_string());
        return Ok(d);
    }
    if lower.ends_with(".pdf") || lower.ends_with(".ai") || bytes.starts_with(b"%PDF") {
        let r =
            vectorcraft_pdf::import_with_report(bytes, &vectorcraft_pdf::ImportOptions::default()).map_err(|e| EngineError::Other(e.to_string()))?;
        let mut d = r.document;
        d.title = std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| name.to_string());
        return Ok(d);
    }
    Err(EngineError::Other(format!("unsupported file `{name}`")))
}

fn open(s: &mut Session, p: &Value) -> Result<Value> {
    let (name, bytes, path) = match (str_param(p, "path"), str_param(p, "dataBase64")) {
        (Some(path), _) => (path.to_string(), std::fs::read(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))?, Some(path.to_string())),
        (None, Some(b64)) => (
            str_param(p, "name").unwrap_or("untitled.vectorcraft").to_string(),
            vectorcraft_format::base64_decode(b64).ok_or_else(|| bad("document.open", "bad base64"))?,
            None,
        ),
        _ => return Err(bad("document.open", "give path or dataBase64")),
    };
    let mut doc = load(&name, &bytes)?;
    let mut native = vectorcraft_format::is_native_name(&name);
    if doc.template {
        // A template opens as a new untitled document (saving asks for a new name).
        doc.template = false;
        doc.title = s.next_untitled();
        native = false;
    }
    let i = s.add_document(doc, if native { path } else { None });
    Ok(json!({ "index": i }))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path")
        .map(str::to_string)
        .or_else(|| s.active().and_then(|d| d.path.clone()))
        .ok_or_else(|| bad("document.save", "no path"))?;
    let bytes = vectorcraft_format::save_file(&s.doc()?.doc);
    std::fs::write(&path, bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    let st = s.doc_mut()?;
    st.path = Some(path.clone());
    st.mark_saved();
    Ok(json!({ "path": path }))
}

pub(crate) fn encode(s: &Session, format: &str, scale: f64, artboard: usize) -> Result<Vec<u8>> {
    encode_doc(&s.doc()?.doc, format, scale, artboard)
}

fn encode_doc(doc: &vectorcraft_doc::Document, format: &str, scale: f64, artboard: usize) -> Result<Vec<u8>> {
    encode_doc_with(doc, format, scale, artboard, false)
}

/// `outline_text`: SVG text as glyph outlines (Illustrator's SVG Options → Fonts → Convert to Outlines).
fn encode_doc_with(doc: &vectorcraft_doc::Document, format: &str, scale: f64, artboard: usize, outline_text: bool) -> Result<Vec<u8>> {
    Ok(match format {
        "vectorcraft" => vectorcraft_format::save_file(doc),
        "svg" => vectorcraft_svg::export(doc, &vectorcraft_svg::ExportOptions { artboard: Some(artboard), outline_text, ..Default::default() })
            .into_bytes(),
        "pdf" => super::rasterfx::export_pdf(doc, &vectorcraft_pdf::PdfOptions::default()).map_err(|e| EngineError::Other(e.to_string()))?,
        "png" | "jpg" | "jpeg" | "webp" => {
            let r = doc.artboards.get(artboard).map(|a| a.rect).ok_or_else(|| EngineError::Other("no such artboard".into()))?;
            let scale = scale.clamp(0.01, 64.0);
            vectorcraft_render::raster_size(r, scale).map_err(EngineError::Other)?;
            let img = vectorcraft_render::Renderer::new().render_region(doc, r, scale, format != "png" && format != "webp");
            match format {
                "png" => img.to_png(),
                "webp" => img.to_webp(),
                _ => img.to_jpeg(90),
            }
        }
        other => return Err(EngineError::Other(format!("unknown format `{other}`"))),
    })
}

fn serialize(s: &mut Session, p: &Value) -> Result<Value> {
    let f = str_param(p, "format").unwrap_or("vectorcraft");
    let bytes = encode(s, f, f64_or(p, "scale", 1.0), p.get("artboard").and_then(Value::as_u64).unwrap_or(0) as usize)?;
    if f == "svg" {
        return Ok(json!({ "text": String::from_utf8_lossy(&bytes) }));
    }
    Ok(json!({ "dataBase64": vectorcraft_format::base64_encode(&bytes) }))
}

fn export(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("document.export", "missing path"))?;
    let f = str_param(p, "format")
        .map(str::to_string)
        .unwrap_or_else(|| std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| "png".into()));
    let artboard = p.get("artboard").and_then(Value::as_u64).unwrap_or(0) as usize;
    let bytes = encode_doc_with(&s.doc()?.doc, &f, f64_or(p, "scale", 1.0), artboard, bool_or(p, "outlineText", false))?;
    std::fs::write(path, &bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
    Ok(json!({ "path": path, "bytes": bytes.len() }))
}

/// File → Export Selection: the selected objects alone, cropped to their visual bounds.
fn export_selection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.exportSelection";
    let path = str_param(p, "path");
    let f = str_param(p, "format")
        .map(str::to_string)
        .or_else(|| path.and_then(|p| std::path::Path::new(p).extension()).map(|e| e.to_string_lossy().to_ascii_lowercase()))
        .unwrap_or_else(|| "png".into());
    let ids = super::edit::selected_roots(s)?;
    let st = s.doc()?;
    let nodes: Vec<std::sync::Arc<vectorcraft_doc::Node>> = ids.iter().filter_map(|id| st.doc.node(*id).cloned().map(std::sync::Arc::new)).collect();
    let bounds = nodes.iter().filter_map(|n| n.visual_bounds()).reduce(|a, b| a.union(b)).ok_or_else(|| bad(C, "select something to export"))?;
    let mut d = (*st.doc).clone();
    let mut layer = vectorcraft_doc::Node::layer(d.alloc_id(), "Selection", vectorcraft_doc::LayerColor::Preset(0));
    if let Some(ch) = layer.children_mut() {
        *ch = nodes;
    }
    d.layers = vec![std::sync::Arc::new(layer)];
    let mut ab = d.artboards.first().cloned().ok_or_else(|| bad(C, "the document has no artboard"))?;
    ab.rect = bounds;
    ab.name = "Selection".into();
    d.artboards = vec![ab];
    let bytes = encode_doc_with(&d, &f, f64_or(p, "scale", 1.0), 0, bool_or(p, "outlineText", false))?;
    let b = [bounds.x0, bounds.y0, bounds.width(), bounds.height()];
    match path {
        Some(path) => {
            std::fs::write(path, &bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            Ok(json!({ "path": path, "bytes": bytes.len(), "bounds": b }))
        }
        None => Ok(json!({ "dataBase64": vectorcraft_format::base64_encode(&bytes), "bounds": b })),
    }
}

/// File → Save as Template: a native copy flagged so opening it starts a new untitled document.
fn save_template(s: &mut Session, p: &Value) -> Result<Value> {
    let mut d = (*s.doc()?.doc).clone();
    d.template = true;
    let bytes = vectorcraft_format::save_file(&d);
    match str_param(p, "path") {
        Some(path) => {
            std::fs::write(path, bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            Ok(json!({ "path": path }))
        }
        None => Ok(json!({ "dataBase64": vectorcraft_format::base64_encode(&bytes) })),
    }
}

fn batch(s: &mut Session, p: &Value) -> Result<Value> {
    let cmds = p.get("commands").and_then(Value::as_array).ok_or_else(|| bad("command.batch", "missing commands"))?.clone();
    let label = str_param(p, "label").unwrap_or("Batch").to_string();
    s.begin_interaction(&label)?;
    // Session-level state a step may change (paint defaults, drawing mode) rolls back too.
    let saved = (s.paint.clone(), s.fill_active, s.draw_mode, s.draw_inside, s.clipboard.clone());
    let mut results = vec![];
    for c in &cmds {
        let id = c.get("command").and_then(Value::as_str).unwrap_or("");
        if id == "command.batch" {
            s.cancel_interaction()?;
            return Err(bad("command.batch", "batches cannot nest"));
        }
        let params = c.get("params").cloned().unwrap_or(json!({}));
        match s.execute(id, &params) {
            Ok(v) => results.push(v),
            Err(e) => {
                s.cancel_interaction()?;
                (s.paint, s.fill_active, s.draw_mode, s.draw_inside, s.clipboard) = saved;
                return Err(EngineError::Other(format!("batch step {} (`{id}`) failed: {e}", results.len())));
            }
        }
    }
    // Commit as one undo step (the interaction's "preview" is the batch itself).
    if let Some(it) = s.doc_mut()?.interaction.as_mut() {
        it.preview = Some(("command.batch".into(), p.clone()));
    }
    s.commit_interaction()?;
    Ok(json!({ "results": results }))
}

fn export_for_screens(s: &mut Session, p: &Value) -> Result<Value> {
    let folder = str_param(p, "folder").ok_or_else(|| bad("document.exportForScreens", "missing folder"))?.to_string();
    let n = s.doc()?.doc.artboards.len();
    let boards: Vec<usize> = match p.get("artboards").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(Value::as_u64).map(|v| v as usize).filter(|i| *i < n).collect(),
        None => (0..n).collect(),
    };
    let formats: Vec<(String, f64, String)> = match p.get("formats").and_then(Value::as_array) {
        Some(a) => a
            .iter()
            .map(|f| {
                let fmt = str_param(f, "format").unwrap_or("png").to_string();
                let sc = f64_or(f, "scale", 1.0);
                let suffix = str_param(f, "suffix")
                    .map(str::to_string)
                    .unwrap_or_else(|| if (sc - 1.0).abs() < 1e-9 { String::new() } else { format!("@{sc}x") });
                (fmt, sc, suffix)
            })
            .collect(),
        None => vec![("png".into(), 1.0, String::new())],
    };
    let prefix = str_param(p, "prefix").unwrap_or("").to_string();
    std::fs::create_dir_all(&folder).map_err(|e| EngineError::Other(format!("{folder}: {e}")))?;
    let mut files = vec![];
    for b in boards {
        let name: String =
            s.doc()?.doc.artboards[b].name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        for (fmt, sc, suffix) in &formats {
            let bytes = encode(s, fmt, *sc, b)?;
            let ext = if fmt == "jpeg" { "jpg" } else { fmt.as_str() };
            let path = format!("{folder}/{prefix}{name}{suffix}.{ext}");
            std::fs::write(&path, &bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            files.push(path);
        }
    }
    Ok(json!({ "files": files }))
}
