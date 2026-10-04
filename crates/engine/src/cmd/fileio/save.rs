//! Saves: `document.save` (native or SVG) and `file.saveAsTemplate`.

use serde_json::{Value, json};

use super::super::*;
use super::{Encoded, Format, default_name, encode_all, format_for_name, writable_format, write_encoded, write_or_return};
use crate::DocState;

/// The format Save writes: `format` (an id or extension), else the format the path's extension
/// names, else native. Save writes .vectorcraft, SVG and SVGZ; other formats are exports.
pub fn save_format(format: Option<&str>, path: Option<&str>) -> std::result::Result<&'static Format, String> {
    let f = match (format, path.and_then(format_for_name)) {
        (Some(_), _) => writable_format(format, None)?,
        (None, Some(f)) => f,
        (None, None) => super::format("vectorcraft").ok_or("no native format")?,
    };
    match f.id {
        "vectorcraft" | "svg" | "svgz" => Ok(f),
        _ => Err(format!("Save writes .{}, .svg or .svgz files: write {} with document.export", vectorcraft_format::EXTENSION, f.label)),
    }
}

/// What Save writes for `st` in format `f` (see [`save_format`]) → the encoded file and the
/// options to remember for the next Save. An SVG save takes the SVG options in `p`, else the ones
/// the document was last saved with.
pub fn save_encoding(st: &DocState, f: &Format, p: &Value) -> Result<(Encoded, Value)> {
    const C: &str = "document.save";
    if f.id == "vectorcraft" {
        return Ok((encode_all(&st.doc, f.id, p)?, Value::Null));
    }
    let given = super::svg_options(p).map_err(|e| bad(C, e))?;
    let opts = if given.is_empty() { st.save_options.clone() } else { Value::Object(given) };
    let enc = encode_all(&st.doc, f.id, &json!({ "svg": opts }))?;
    if enc.files.len() != 1 {
        return Err(bad(C, "Save writes one artboard: name one, or export several with document.export"));
    }
    Ok((enc, opts))
}

pub(super) fn save(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "document.save";
    let path = str_param(p, "path").map(str::to_string).or_else(|| s.active().and_then(|d| d.path.clone()));
    let f = save_format(str_param(p, "format"), path.as_deref()).map_err(|e| bad(C, e))?;
    let st = s.doc()?;
    let (enc, opts) = save_encoding(st, f, p)?;
    let Some(path) = path else {
        // Nowhere to save to (web, agents): hand the bytes back; the document stays modified.
        return write_encoded(None, &default_name(&st.doc, f.extensions[0]), &st.doc, &enc, json!({}));
    };
    let out = write_encoded(Some(&path), &path, &st.doc, &enc, json!({}))?;
    let st = s.doc_mut()?;
    st.path = Some(path.clone());
    st.save_options = opts;
    st.mark_saved();
    let mut r = json!({ "path": path });
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
