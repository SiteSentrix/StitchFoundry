//! File → Export → Export for Screens (`document.exportForScreens`): the chosen artboards in every
//! format row, into a folder or returned for download (each file, or one ZIP).

use std::collections::HashSet;

use serde::Deserialize;
use serde_json::{Value, json};

use super::super::*;
use super::export::without_artboards;
use super::{ArtboardPick, Format, artboard_file_names, create_dir, encode, writable, write_file};

const C: &str = "document.exportForScreens";

/// One Export for Screens format row: what to write, at which scale, with which file-name suffix.
struct ScreenFormat {
    format: &'static Format,
    /// The row's own options (scale, quality…) for the encoder.
    options: Value,
    suffix: String,
}

fn screen_format(row: &Value) -> Result<ScreenFormat> {
    if !row.is_object() {
        return Err(bad(C, "each format is an object {format, scale?, suffix?}"));
    }
    let format = writable(C, Some(str_param(row, "format").unwrap_or("png")), None)?;
    if matches!(format.id, "vectorcraft" | "template" | "dxf") {
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

/// A name safe as one file name: letters, digits, `-`, `_` and `.` kept, anything else `-` (`None`
/// when nothing but dots is left).
fn safe_name(s: &str) -> Option<String> {
    let n: String = s.trim().chars().map(|c| if c.is_alphanumeric() || "-_.".contains(c) { c } else { '-' }).collect();
    (!n.trim_matches('.').is_empty()).then_some(n)
}

/// File → Export for Screens: every chosen artboard in every format, one file each (a PDF holds
/// its artboard alone). Artboards that share a name get `-2`, `-3`… instead of overwriting.
pub(super) fn export_for_screens(s: &mut Session, p: &Value) -> Result<Value> {
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
    let folder = str_param(p, "folder").filter(|f| !f.is_empty());
    let zip = bool_or(p, "zip", false);
    if let Some(dir) = folder {
        create_dir(dir)?;
    }
    let mut written = HashSet::new();
    // Written paths, or (name, bytes) to return or zip.
    let mut paths = vec![];
    let mut kept: Vec<(String, Vec<u8>)> = vec![];
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
                Some(dir) if !zip => {
                    let path = format!("{dir}/{file}");
                    write_file(&path, &bytes)?;
                    paths.push(path);
                }
                _ => kept.push((file, bytes)),
            }
        }
    }
    let names: Vec<&str> = kept.iter().map(|(n, _)| n.as_str()).collect();
    if zip {
        let archive = super::zip::store(&kept).map_err(|e| bad(C, e))?;
        let zip_name = format!("{prefix}{}.zip", safe_name(&super::file_stem(&doc.title)).unwrap_or_else(|| "Untitled".into()));
        return match folder {
            Some(dir) => {
                let path = format!("{dir}/{zip_name}");
                write_file(&path, &archive)?;
                Ok(json!({ "path": path, "bytes": archive.len(), "files": names }))
            }
            None => {
                Ok(json!({ "name": zip_name, "dataBase64": vectorcraft_format::base64_encode(&archive), "bytes": archive.len(), "files": names }))
            }
        };
    }
    Ok(match folder {
        Some(_) => json!({ "files": paths }),
        None => {
            json!({ "files": kept.iter().map(|(name, bytes)| json!({ "name": name, "dataBase64": vectorcraft_format::base64_encode(bytes) })).collect::<Vec<_>>() })
        }
    })
}
