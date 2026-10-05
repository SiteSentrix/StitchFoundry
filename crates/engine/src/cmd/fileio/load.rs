//! `document.open`: every readable format into a new document.

use std::io::Cursor;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Document, ImageBlob, ImageObject, Node, NodeKind};
use vectorcraft_geom::Affine;

use super::super::*;
use super::{Format, format, format_for_name, read_file};
use crate::EngineError;

/// A file read into a document, with its format and non-fatal import notes.
pub struct Loaded {
    pub doc: Document,
    pub format: &'static Format,
    pub warnings: Vec<String>,
}

/// An image ready to embed: PNG/JPEG/GIF/WebP keep their bytes, other formats are stored as PNG
/// (what browsers, PDF and SVG viewers show).
pub struct RasterImage {
    /// Content key for `Document::images` (identical files share one blob).
    pub key: String,
    pub blob: ImageBlob,
    pub width: u32,
    pub height: u32,
    /// The resolution the file declares (see [`super::ppi::resolution`]).
    pub ppi: Option<(f64, f64)>,
}

fn err(e: impl std::fmt::Display) -> EngineError {
    EngineError::Other(e.to_string())
}

/// The last component of a path or file name.
pub fn file_name(name: &str) -> String {
    std::path::Path::new(name).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| name.to_string())
}

/// The readable format of a file: from its content (magic bytes) when that tells, else from the
/// extension of `name`.
pub fn detect(name: &str, bytes: &[u8]) -> Option<&'static Format> {
    let by_name = format_for_name(name).filter(|f| f.read);
    if vectorcraft_format::sniff(bytes) {
        return format("vectorcraft");
    }
    if vectorcraft_svg::is_svgz(bytes) {
        return format("svgz");
    }
    if bytes.starts_with(b"%PDF") {
        // .ai and .ait files are PDF inside; the extension keeps the template meaning.
        return by_name.filter(|f| matches!(f.id, "ai" | "ait")).or_else(|| format("pdf"));
    }
    if let Some(f) = image::guess_format(bytes).ok().and_then(image_format) {
        return Some(f);
    }
    if clipboard::looks_like_svg(&String::from_utf8_lossy(&bytes[..bytes.len().min(4096)])) {
        return format("svg");
    }
    by_name
}

/// The readable raster format behind an `image` crate format.
fn image_format(f: image::ImageFormat) -> Option<&'static Format> {
    f.extensions_str().iter().find_map(|e| format(e)).filter(|f| f.read && f.raster)
}

/// Read a file of any readable format (`name`: its file name or path, for the extension and title).
pub fn load(name: &str, bytes: &[u8]) -> Result<Loaded> {
    let format = detect(name, bytes).ok_or_else(|| err(format!("can't open `{name}`: not a format VectorCraft reads (see document.formats)")))?;
    let title = file_name(name);
    let mut warnings = vec![];
    let mut doc = match format.id {
        "vectorcraft" => vectorcraft_format::load(bytes).map_err(err)?,
        "svg" | "svgz" => {
            let text = vectorcraft_svg::text_of(bytes).map_err(err)?;
            // An SVG saved with Preserve Editing carries the native document: open that.
            match vectorcraft_svg::editing_data(&text).and_then(|b64| vectorcraft_format::base64_decode(&b64)) {
                Some(native) => vectorcraft_format::load(&native).map_err(err)?,
                None => {
                    let (d, w) = vectorcraft_svg::import_with_report(&text).map_err(err)?;
                    warnings = w;
                    d
                }
            }
        }
        "pdf" | "ai" | "ait" => {
            let r = vectorcraft_pdf::import_with_report(bytes, &vectorcraft_pdf::ImportOptions::default()).map_err(err)?;
            warnings = r.warnings;
            r.document
        }
        _ if format.raster => raster_doc(&title, bytes)?,
        _ => return Err(err(format!("{} files can't be opened yet", format.label))),
    };
    // Imports are named after the file; a native document keeps its own title (the tab shows the
    // file name once it has a path).
    if format.id != "vectorcraft" || doc.title.is_empty() {
        doc.title = title;
    }
    Ok(Loaded { doc, format, warnings })
}

/// Open a file's bytes as the new active document (what `document.open` does) →
/// `{index, title, format, warnings}`. `path` is kept for Save only for a native, non-template file.
pub fn open_bytes(s: &mut Session, name: &str, bytes: &[u8], path: Option<String>) -> Result<Value> {
    let Loaded { mut doc, format, warnings } = load(name, bytes)?;
    // A template (saved by Save as Template, or an .ait file) opens as a new untitled document.
    let template = doc.template || format.id == "ait";
    if template {
        doc.template = false;
        doc.title = s.next_untitled();
    }
    let keep_path = format.id == "vectorcraft" && !template;
    let index = s.add_document(doc, path.filter(|_| keep_path));
    let title = s.documents()[index].title();
    Ok(json!({ "index": index, "title": title, "format": format.id, "warnings": warnings }))
}

/// A file named by a command's params: `{path}` (read from disk) or `{name, dataBase64}`.
pub(crate) struct Source<'a> {
    /// The path, or the given name (default "Untitled"): for the extension and the title.
    pub name: &'a str,
    pub bytes: Vec<u8>,
    pub path: Option<&'a str>,
}

/// The file `p` names for command `cmd` (see [`Source`]).
pub(crate) fn source<'a>(p: &'a Value, cmd: &str) -> Result<Source<'a>> {
    match (str_param(p, "path"), str_param(p, "dataBase64")) {
        (Some(path), _) => Ok(Source { name: path, bytes: read_file(path)?, path: Some(path) }),
        (None, Some(b64)) => {
            let bytes = vectorcraft_format::base64_decode(b64).ok_or_else(|| bad(cmd, "bad base64"))?;
            Ok(Source { name: str_param(p, "name").unwrap_or("Untitled"), bytes, path: None })
        }
        _ => Err(bad(cmd, "give path, or name and dataBase64")),
    }
}

pub(super) fn open(s: &mut Session, p: &Value) -> Result<Value> {
    let src = source(p, "document.open")?;
    open_bytes(s, src.name, &src.bytes, src.path.map(str::to_string))
}

/// Decode an image's header (and, for formats stored as PNG, its pixels).
pub fn raster_image(bytes: &[u8]) -> Result<RasterImage> {
    let reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(err)?;
    let kind = reader.format().ok_or_else(|| err("not an image VectorCraft reads (see document.formats)"))?;
    let f = image_format(kind).ok_or_else(|| err(format!("{kind:?} images can't be opened (see document.formats)")))?;
    let ppi = super::ppi::resolution(bytes);
    let (bytes, mime, (width, height)) = if matches!(f.id, "png" | "jpg" | "gif" | "webp") {
        (bytes.to_vec(), f.mime, reader.into_dimensions().map_err(err)?)
    } else {
        let img = reader.decode().map_err(err)?.to_rgba8();
        let size = img.dimensions();
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).map_err(err)?;
        // The stored PNG keeps the file's resolution.
        let png = match ppi {
            Some(r) => super::ppi::with_png_resolution(&png, r),
            None => png,
        };
        (png, "image/png", size)
    };
    if width == 0 || height == 0 {
        return Err(err("the image is empty"));
    }
    let blob = ImageBlob { mime: mime.into(), bytes: Arc::new(bytes) };
    Ok(RasterImage { key: blob.content_key(), blob, width, height, ppi })
}

/// An image as a document of its pixel size (1 px = 1 pt), the image named after the file.
fn raster_doc(name: &str, bytes: &[u8]) -> Result<Document> {
    let RasterImage { key, blob, width, height, .. } = raster_image(bytes)?;
    let mut d = Document::new(width as f64, height as f64);
    let layer = d.layers[0].id;
    let id = d.alloc_id();
    let mut n = Node::new(id, NodeKind::Image(ImageObject { key: key.clone(), width, height, xf: Affine::IDENTITY, link: None }));
    n.name = Some(name.to_string());
    d.images.insert(key, blob);
    d.insert(Some(layer), 0, n).map_err(err)?;
    Ok(d)
}
