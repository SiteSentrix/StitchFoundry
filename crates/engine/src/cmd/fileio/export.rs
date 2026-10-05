//! `document.export`, `document.serialize`, `document.exportSelection`, `document.exportForScreens`.

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node, NodeKind};

use super::super::*;
use super::encode::single_artboard;
use super::{
    ARTBOARD_PARAMS, ArtboardPick, Format, artboard_file_names, create_dir, default_name, encode, encode_all, merge, writable, write_encoded,
    write_file, write_or_return,
};

pub(super) fn serialize(s: &mut Session, p: &Value) -> Result<Value> {
    let f = writable("document.serialize", Some(str_param(p, "format").unwrap_or("vectorcraft")), None)?;
    let expanded = super::pdf::expand_preset(s, "document.serialize", p)?;
    let p = &*expanded;
    let doc = &*source(s, f, p, "document.serialize")?;
    let enc = encode_all(doc, f.id, p)?;
    let files = enc.named(doc, &default_name(doc, f.extensions[0]));
    let (main, linked) = files.split_at(enc.files.len());
    // Text formats come back as text.
    let data = |bytes: &[u8]| match f.id {
        "svg" => json!({ "text": String::from_utf8_lossy(bytes) }),
        _ => json!({ "dataBase64": vectorcraft_format::base64_encode(bytes) }),
    };
    let mut out = merge(data(main.first().map_or(&[][..], |m| &m.1)), json!({ "warnings": enc.warnings }));
    if main.len() > 1 {
        out["files"] = main.iter().map(|(name, bytes)| merge(json!({ "name": name }), data(bytes))).collect();
    }
    if !linked.is_empty() {
        out["linked"] = linked.iter().map(|(name, bytes)| json!({ "name": name, "dataBase64": vectorcraft_format::base64_encode(bytes) })).collect();
    }
    Ok(out)
}

pub(super) fn export(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path");
    let f = writable("document.export", str_param(p, "format"), path)?;
    let expanded = super::pdf::expand_preset(s, "document.export", p)?;
    let p = &*expanded;
    let doc = &*source(s, f, p, "document.export")?;
    let enc = encode_all(doc, f.id, p)?;
    write_encoded(path, &default_name(doc, f.extensions[0]), doc, &enc, json!({ "format": f.id, "warnings": enc.warnings }))
}

/// `p` without its artboard choice, also inside its SVG options (for documents made of one
/// synthetic artboard, and for callers that pick the artboard themselves).
pub(super) fn without_artboards(p: &Value) -> Value {
    let strip = |o: &mut serde_json::Map<String, Value>| ARTBOARD_PARAMS.iter().for_each(|k| _ = o.remove(*k));
    let mut q = p.clone();
    if let Some(o) = q.as_object_mut() {
        strip(o);
        if let Some(svg) = o.get_mut("svg").and_then(Value::as_object_mut) {
            strip(svg);
        }
    }
    q
}

/// The document an export of `f` writes: the active one, or for text with `selectionOnly` the
/// selected objects alone.
fn source<'a>(s: &'a mut Session, f: &Format, p: &Value, cmd: &str) -> Result<Cow<'a, Document>> {
    if f.id == "txt" && bool_or(p, "selectionOnly", false) {
        return Ok(Cow::Owned(selection(s, cmd)?.0));
    }
    Ok(Cow::Borrowed(&s.doc()?.doc))
}

/// The selected objects (not on template layers) alone on one layer, with one artboard: their
/// visual bounds.
fn selection(s: &mut Session, cmd: &str) -> Result<(Document, vectorcraft_geom::Rect)> {
    let ids = edit::selected_roots(s)?;
    let st = s.doc()?;
    let is_template = |id| st.doc.node(id).is_some_and(|l| matches!(l.kind, NodeKind::Layer { template: true, .. }));
    // On a template layer or sublayer, at any depth.
    let on_template = |id| st.doc.ancestry(id).is_some_and(|a| a.into_iter().any(is_template));
    let nodes: Vec<Arc<Node>> = ids.into_iter().filter(|id| !on_template(*id)).filter_map(|id| st.doc.node(id).cloned().map(Arc::new)).collect();
    let bounds = nodes.iter().filter_map(|n| n.visual_bounds()).reduce(|a, b| a.union(b)).ok_or_else(|| bad(cmd, "select something to export"))?;
    let mut d = single_artboard(&st.doc, bounds, "Selection");
    let mut layer = Node::layer(d.alloc_id(), "Selection", vectorcraft_doc::LayerColor::Preset(0));
    if let Some(ch) = layer.children_mut() {
        *ch = nodes;
    }
    d.layers = vec![Arc::new(layer)];
    Ok((d, bounds))
}

/// File → Export Selection: the selected objects alone, cropped to their visual bounds. Objects on
/// template layers are guides, not artwork, and are left out.
pub(super) fn export_selection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.exportSelection";
    let path = str_param(p, "path");
    let f = writable(C, str_param(p, "format"), path)?;
    let expanded = super::pdf::expand_preset(s, C, p)?;
    let p = &*expanded;
    let (d, bounds) = selection(s, C)?;
    let bytes = encode(&d, f.id, &without_artboards(p))?;
    write_or_return(path, &bytes, json!({ "bounds": [bounds.x0, bounds.y0, bounds.width(), bounds.height()] }))
}

/// One Export for Screens format row: what to write, at which scale, with which file-name suffix.
struct ScreenFormat {
    format: &'static Format,
    /// The row's own options (scale, quality…) for the encoder.
    options: Value,
    suffix: String,
}

fn screen_format(row: &Value) -> Result<ScreenFormat> {
    const C: &str = "document.exportForScreens";
    if !row.is_object() {
        return Err(bad(C, "each format is an object {format, scale?, suffix?}"));
    }
    let format = writable(C, Some(str_param(row, "format").unwrap_or("png")), None)?;
    if matches!(format.id, "vectorcraft" | "template") {
        return Err(bad(C, "Export for Screens writes png, jpg, webp, gif, png8, svg, svgz or pdf"));
    }
    // Vector formats have no pixel size: scale doesn't apply and adds no @Nx suffix (not even one
    // left over from a raster row switched to SVG or PDF).
    // A raster row's `ppi` wins over its `scale` (the encoder does the same), so it names the suffix.
    let scale = match row.get("ppi").and_then(Value::as_f64) {
        _ if !format.raster => 1.0,
        Some(ppi) => ppi / 72.0,
        None => f64_or(row, "scale", 1.0),
    };
    let suffix = match str_param(row, "suffix") {
        Some(s) if !format.raster && is_scale_suffix(s) => String::new(),
        Some(s) => s.to_string(),
        None if (scale - 1.0).abs() < 1e-9 => String::new(),
        None => format!("@{scale}x"),
    };
    let mut options = without_artboards(row);
    if let Some(o) = options.as_object_mut() {
        o.insert("scale".into(), json!(scale));
        // A file per artboard is for viewing: it doesn't carry the whole document to edit.
        o.entry("preserveEditing").or_insert(json!(false));
    }
    Ok(ScreenFormat { format, options, suffix })
}

/// A pixel-density suffix such as `@2x` or `@0.5x`.
fn is_scale_suffix(s: &str) -> bool {
    s.strip_prefix('@').and_then(|s| s.strip_suffix(['x', 'X'])).is_some_and(|n| n.parse::<f64>().is_ok())
}

/// File → Export for Screens: every chosen artboard in every format, one file each (a PDF holds
/// its artboard alone). Artboards that share a name get `-2`, `-3`… instead of overwriting.
pub(super) fn export_for_screens(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.exportForScreens";
    let doc = s.doc()?.doc.clone();
    let n = doc.artboards.len();
    let pick = if p.is_object() { ArtboardPick::deserialize(p).map_err(|e| bad(C, e.to_string()))? } else { ArtboardPick::default() };
    let boards = pick.resolve(n).map_err(|e| bad(C, e))?.unwrap_or_else(|| (0..n).collect());
    let mut formats: Vec<ScreenFormat> = match p.get("formats").and_then(Value::as_array) {
        Some(rows) => rows.iter().map(screen_format).collect::<Result<_>>()?,
        None => vec![screen_format(&json!({}))?],
    };
    // A top-level anti-aliasing mode applies to every row that doesn't name its own.
    if let Some(aa) = p.get("antiAlias") {
        for sf in &mut formats {
            if let Some(o) = sf.options.as_object_mut() {
                o.entry("antiAlias").or_insert_with(|| aa.clone());
            }
        }
    }
    let prefix = str_param(p, "prefix").unwrap_or("");
    let folder = str_param(p, "folder");
    if let Some(dir) = folder {
        create_dir(dir)?;
    }
    let mut written = HashSet::new();
    let mut files = vec![];
    for (b, name) in boards.iter().copied().zip(artboard_file_names(&doc, &boards)) {
        for sf in &formats {
            let file = format!("{prefix}{name}{}.{}", sf.suffix, sf.format.extensions[0]);
            if !written.insert(file.to_lowercase()) {
                continue; // the same file from two identical rows
            }
            let mut options = sf.options.clone();
            options["artboard"] = json!(b);
            let bytes = encode(&doc, sf.format.id, &options)?;
            match folder {
                Some(dir) => {
                    let path = format!("{dir}/{file}");
                    write_file(&path, &bytes)?;
                    files.push(json!(path));
                }
                None => files.push(json!({ "name": file, "dataBase64": vectorcraft_format::base64_encode(&bytes) })),
            }
        }
    }
    Ok(json!({ "files": files }))
}

/// File → Save for Office Documents: one artboard as a PNG at `ppi`, on white unless
/// `transparent`.
pub(super) fn export_for_office(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.exportForOffice";
    let path = str_param(p, "path");
    let ppi = f64_or(p, "ppi", 150.0);
    let doc = &s.doc()?.doc;
    let artboard = match p.get("artboard") {
        None | Some(Value::Null) => 0,
        Some(v) => v.as_u64().and_then(|i| usize::try_from(i).ok()).ok_or_else(|| bad(C, "artboard is a 0-based index"))?,
    };
    if artboard >= doc.artboards.len() {
        return Err(bad(C, format!("no artboard {artboard} (0-based; the document has {})", doc.artboards.len())));
    }
    let background = if bool_or(p, "transparent", false) { "transparent" } else { "white" };
    let bytes = encode(doc, "png", &json!({ "ppi": ppi, "background": background, "artboard": artboard }))?;
    // The pixel size, from the PNG header.
    let side = |at: usize| bytes.get(at..at + 4).and_then(|b| b.try_into().ok()).map_or(0, u32::from_be_bytes);
    write_or_return(path, &bytes, json!({ "width": side(16), "height": side(20) }))
}
