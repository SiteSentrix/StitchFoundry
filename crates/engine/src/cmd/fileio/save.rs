//! Saves: `document.save` (native, SVG or PDF-compatible .ai) and `file.saveAsTemplate`.

use serde_json::{Value, json};

use super::super::*;
use super::{Encoded, Format, default_name, encode_all, format_for_name, writable_format, write_encoded, write_or_return};
use crate::DocState;

/// The formats Save writes, in the order save dialogs offer them; other formats are exports.
pub const SAVE_FORMATS: [&str; 4] = ["vectorcraft", "ai", "svg", "svgz"];

/// The format Save writes: `format` (an id or extension), else the format the path's extension
/// names, else native (see [`SAVE_FORMATS`]). A `.ai` file is a PDF carrying the native document.
pub fn save_format(format: Option<&str>, path: Option<&str>) -> std::result::Result<&'static Format, String> {
    let f = match (format, path.and_then(format_for_name)) {
        (Some(id), _) => super::format(id).filter(|f| SAVE_FORMATS.contains(&f.id)).map_or_else(|| writable_format(format, None), Ok)?,
        (None, Some(f)) => f,
        (None, None) => super::format("vectorcraft").ok_or("no native format")?,
    };
    if SAVE_FORMATS.contains(&f.id) {
        Ok(f)
    } else {
        Err(format!("Save writes .{}, .ai, .svg or .svgz files: write {} with document.export", vectorcraft_format::EXTENSION, f.label))
    }
}

/// Save dialog filters for a file named `name`: when Save writes its format, every format Save
/// writes (that one first), else none.
pub fn save_filters(name: &str) -> Vec<(&'static str, &'static [&'static str])> {
    let Some(first) = format_for_name(name).filter(|f| SAVE_FORMATS.contains(&f.id)) else { return vec![] };
    let others = SAVE_FORMATS.iter().filter_map(|id| super::format(id)).filter(|f| f.id != first.id);
    std::iter::once(first).chain(others).map(|f| (f.label, f.extensions)).collect()
}

/// What Save writes for `st` in format `f` (see [`save_format`]) to `path` → the encoded file and
/// the options to remember for the next Save. An SVG save takes the SVG options in `p`, else the
/// ones the document was last saved with, and keeps hidden layers (hidden) unless they say
/// otherwise. A native file (and the document a `.ai` file carries) records its links' paths
/// relative to `path`.
pub fn save_encoding(st: &DocState, f: &Format, p: &Value, path: Option<&str>) -> Result<(Encoded, Value)> {
    const C: &str = "document.save";
    let relative = path.filter(|_| matches!(f.id, "vectorcraft" | "ai")).and_then(|path| crate::cmd::links::with_relative_paths(&st.doc, path));
    let doc = relative.as_ref().unwrap_or(&st.doc);
    match f.id {
        "vectorcraft" => return Ok((encode_all(doc, f.id, p)?, Value::Null)),
        // A PDF of every artboard with the PDF options in `p`, always carrying the native document.
        "ai" => {
            let mut q = if p.is_object() { p.clone() } else { json!({}) };
            if let Some(o) = q.as_object_mut() {
                o.extend([("preserveEditing".into(), json!(true)), ("range".into(), json!("all"))]);
            }
            let (bytes, warnings) = super::pdf::encode(C, &doc.without_edit_modes(), &q)?;
            return Ok((Encoded { warnings, ..Encoded::one(bytes) }, Value::Null));
        }
        _ => {}
    }
    let given = super::svg_options(p).map_err(|e| bad(C, e))?;
    let opts = if given.is_empty() { st.save_options.clone() } else { Value::Object(given) };
    let mut svg = opts.as_object().cloned().unwrap_or_default();
    svg.entry("hiddenLayers").or_insert(Value::Bool(true));
    let enc = encode_all(&st.doc, f.id, &json!({ "svg": svg }))?;
    if enc.files.len() != 1 {
        return Err(bad(C, "Save writes one artboard: name one, or export several with document.export"));
    }
    Ok((enc, opts))
}

/// File Info's dates for a save to a file: the modified date (and the created date, when it has
/// none) become now. Not an undo step; the web build, without a clock, leaves them.
pub fn stamp_save_dates(st: &mut DocState) {
    let Some(now) = vectorcraft_doc::metadata::now_unix() else { return };
    let d = std::sync::Arc::make_mut(&mut st.doc);
    d.metadata.created.get_or_insert(now);
    d.metadata.modified = Some(now);
}

pub(super) fn save(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.save";
    let path = str_param(p, "path").map(str::to_string).or_else(|| s.active().and_then(|d| d.path.clone()));
    let f = save_format(str_param(p, "format"), path.as_deref()).map_err(|e| bad(C, e))?;
    if path.is_some() {
        stamp_save_dates(s.doc_mut()?);
    }
    let expanded = super::pdf::expand_preset(s, C, p)?;
    let p = &*expanded;
    let st = s.doc()?;
    let (enc, opts) = save_encoding(st, f, p, path.as_deref())?;
    // The encoder's notes (a .ai file's PDF options not applied yet…).
    let notes = if enc.warnings.is_empty() { json!({}) } else { json!({ "warnings": enc.warnings }) };
    let Some(path) = path else {
        // Nowhere to save to (web, agents): hand the bytes back; the document stays modified.
        return write_encoded(None, &default_name(&st.doc, f.extensions[0]), &st.doc, &enc, notes);
    };
    let out = write_encoded(Some(&path), &path, &st.doc, &enc, json!({}))?;
    let st = s.doc_mut()?;
    st.path = Some(path.clone());
    st.save_options = opts;
    st.mark_saved();
    let mut r = super::merge(json!({ "path": path }), notes);
    if let Some(linked) = out.get("linked") {
        r["linked"] = linked.clone();
    }
    Ok(r)
}

/// File → Save as Template: a native copy flagged so opening it starts a new untitled document.
pub(super) fn save_template(s: &mut Session, p: &Value) -> Result<Value> {
    let mut d = (*s.doc()?.doc).clone();
    d.template = true;
    write_or_return(str_param(p, "path"), &vectorcraft_format::save_file(&d), json!({}))
}
