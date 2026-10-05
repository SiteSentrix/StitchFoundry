//! The native format's options: compression, the version to write (older apps read older
//! versions) and an embedded preview.

use serde::Deserialize;
use serde_json::Value;
use vectorcraft_doc::Document;
use vectorcraft_format::{PREVIEW_MAX, SaveOptions};

use super::super::*;
use super::{Format, FormatOption};

/// The options `document.save` and the native encoder read.
pub const OPTIONS: &[FormatOption] = &[
    FormatOption {
        name: "compress",
        ty: "boolean",
        default: "false",
        description: "gzip the file: smaller, not readable as text (saves default to Preferences → File Handling → Use Compression)",
    },
    FormatOption {
        name: "version",
        ty: "integer",
        default: "3",
        description: "the format version to write: 3 (current), or 2 or 1 for older VectorCraft versions (not compressed; newer features they don't know are lost there)",
    },
    FormatOption {
        name: "preview",
        ty: "boolean",
        default: "false",
        description: "embed a PNG preview of the first artboard (at most 256 pixels on its longer side) for file browsers",
    },
];

#[derive(Default, Deserialize)]
#[serde(default)]
struct NativeOptions {
    compress: Option<bool>,
    version: Option<u32>,
    preview: bool,
}

/// The `.vectorcraft` file of `doc` with the options in `p`.
pub(super) fn encode(cmd: &str, f: &Format, doc: &Document, p: &Value) -> Result<Vec<u8>> {
    let o: NativeOptions = super::encode::options(f, p)?;
    let mut so = SaveOptions::for_doc(doc);
    so.compress = o.compress.unwrap_or(false);
    so.version = o.version.unwrap_or(vectorcraft_format::VERSION);
    if o.preview {
        so.preview = preview_png(doc)?;
    }
    vectorcraft_format::save_with(doc, &so).map_err(|e| bad(cmd, e.to_string()))
}

/// The first artboard (else the art) as a PNG fitted into [`PREVIEW_MAX`] pixels (`None`: nothing
/// to show).
fn preview_png(doc: &Document) -> Result<Option<Vec<u8>>> {
    let Some(r) = doc.artboards.first().map(|a| a.rect).or_else(|| vectorcraft_render::encode::art_bounds(doc)) else { return Ok(None) };
    let scale = f64::from(PREVIEW_MAX) / r.width().max(r.height());
    if vectorcraft_render::raster_size(r, scale).is_err() {
        return Ok(None);
    }
    vectorcraft_render::Renderer::new().render_region(doc, r, scale, false).to_png().map(Some).map_err(EngineError::Other)
}

/// `p` with `compress` from the Use Compression preference when it doesn't say (saves only;
/// exports and serializing write what they're told).
pub fn with_compression_pref(prefs: &crate::Prefs, p: &Value) -> Value {
    let mut q = if p.is_object() { p.clone() } else { Value::Object(Default::default()) };
    if let Some(o) = q.as_object_mut()
        && o.get("compress").is_none_or(Value::is_null)
        // Older versions can't read compressed files: those saves stay plain.
        && o.get("version").and_then(Value::as_u64).is_none_or(|v| v >= u64::from(vectorcraft_format::COMPRESSED_SINCE))
    {
        o.insert("compress".into(), Value::Bool(prefs.use_compression));
    }
    q
}
