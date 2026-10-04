//! `document.export`, `document.serialize`, `document.exportSelection`, `document.exportForScreens`.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::Node;

use super::super::*;
use super::{create_dir, encode, writable, write_file, write_or_return};

pub(super) fn serialize(s: &mut Session, p: &Value) -> Result<Value> {
    let f = writable("document.serialize", Some(str_param(p, "format").unwrap_or("vectorcraft")), None)?;
    let bytes = encode(&s.doc()?.doc, f.id, p)?;
    if f.id == "svg" {
        return Ok(json!({ "text": String::from_utf8_lossy(&bytes) }));
    }
    Ok(json!({ "dataBase64": vectorcraft_format::base64_encode(&bytes) }))
}

pub(super) fn export(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").ok_or_else(|| bad("document.export", "missing path"))?;
    let f = writable("document.export", str_param(p, "format"), Some(path))?;
    let bytes = encode(&s.doc()?.doc, f.id, p)?;
    write_or_return(Some(path), &bytes, json!({ "format": f.id }))
}

/// `p` without its artboard choice (for documents made of one synthetic artboard).
fn without_artboards(p: &Value) -> Value {
    let mut q = p.clone();
    if let Some(o) = q.as_object_mut() {
        for k in ["artboard", "artboards", "range"] {
            o.remove(k);
        }
    }
    q
}

/// File → Export Selection: the selected objects alone, cropped to their visual bounds.
pub(super) fn export_selection(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.exportSelection";
    let path = str_param(p, "path");
    let f = writable(C, str_param(p, "format"), path)?;
    let ids = edit::selected_roots(s)?;
    let st = s.doc()?;
    let nodes: Vec<Arc<Node>> = ids.into_iter().filter_map(|id| st.doc.node(id).cloned().map(Arc::new)).collect();
    let bounds = nodes.iter().filter_map(|n| n.visual_bounds()).reduce(|a, b| a.union(b)).ok_or_else(|| bad(C, "select something to export"))?;
    let mut d = (*st.doc).clone();
    let mut layer = Node::layer(d.alloc_id(), "Selection", vectorcraft_doc::LayerColor::Preset(0));
    if let Some(ch) = layer.children_mut() {
        *ch = nodes;
    }
    d.layers = vec![Arc::new(layer)];
    let mut ab = d.artboards.first().cloned().ok_or_else(|| bad(C, "the document has no artboard"))?;
    ab.rect = bounds;
    ab.name = "Selection".into();
    d.artboards = vec![ab];
    let bytes = encode(&d, f.id, &without_artboards(p))?;
    write_or_return(path, &bytes, json!({ "bounds": [bounds.x0, bounds.y0, bounds.width(), bounds.height()] }))
}

pub(super) fn export_for_screens(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.exportForScreens";
    let folder = str_param(p, "folder").ok_or_else(|| bad(C, "missing folder"))?;
    let doc = s.doc()?.doc.clone();
    let n = doc.artboards.len();
    let boards: Vec<usize> = match p.get("artboards").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(Value::as_u64).map(|v| v as usize).filter(|i| *i < n).collect(),
        None => (0..n).collect(),
    };
    let formats: Vec<(&str, f64, String)> = match p.get("formats").and_then(Value::as_array) {
        Some(a) => a
            .iter()
            .map(|f| {
                let sc = f64_or(f, "scale", 1.0);
                let suffix = str_param(f, "suffix")
                    .map(str::to_string)
                    .unwrap_or_else(|| if (sc - 1.0).abs() < 1e-9 { String::new() } else { format!("@{sc}x") });
                (str_param(f, "format").unwrap_or("png"), sc, suffix)
            })
            .collect(),
        None => vec![("png", 1.0, String::new())],
    };
    let prefix = str_param(p, "prefix").unwrap_or("");
    create_dir(folder)?;
    let mut files = vec![];
    for b in boards {
        let name: String = doc.artboards[b].name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
        for (fmt, sc, suffix) in &formats {
            let f = writable(C, Some(fmt), None)?;
            let bytes = encode(&doc, f.id, &json!({ "artboard": b, "scale": sc }))?;
            let path = format!("{folder}/{prefix}{name}{suffix}.{}", f.extensions[0]);
            write_file(&path, &bytes)?;
            files.push(path);
        }
    }
    Ok(json!({ "files": files }))
}
