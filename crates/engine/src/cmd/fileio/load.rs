//! `document.open`: every readable format into a new document.

use std::io::Cursor;

use serde_json::{Value, json};
use vectorcraft_doc::{Document, ImageBlob, ImageObject, Node, NodeKind};
use vectorcraft_geom::Affine;

use super::super::*;
use super::pdfimport::LoadOptions;
use super::{Format, SAVE_FORMATS, format, format_for_name, read_file};
use crate::EngineError;

/// A file read into a document, with its format and non-fatal import notes.
pub struct Loaded {
    pub doc: Document,
    pub format: &'static Format,
    pub warnings: Vec<String>,
    /// The document is the native one the file carries (SVG or PDF saved with Preserve Editing).
    pub restored: bool,
    /// An older native file (the former `.drawcraft` name or format version): saving it again
    /// rewrites it in today's format.
    pub converted: bool,
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

pub(super) fn err(e: impl std::fmt::Display) -> EngineError {
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
        // A template keeps its meaning from the extension, like .ait.
        return by_name.filter(|f| f.id == "template").or_else(|| format("vectorcraft"));
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

/// Why a file carrying editing data (an SVG or PDF saved with Preserve Editing) opened as plain
/// artwork: it was changed in another app after it was saved.
pub const EDITING_STALE: &str = "this file was changed in another app after it was saved with its editing data: it opened as plain artwork";
/// Why a file carrying editing data opened as plain artwork: the data can't be read.
pub const EDITING_DAMAGED: &str = "this file's editing data can't be read: it opened as plain artwork";

/// Preserve Editing, for every format that carries the native document: that document when the
/// file around it is unchanged (`editing`: whether it is, and how to get the document's bytes) and
/// it reads, else `import()`'s document with a first warning saying why the editing data wasn't
/// used → (document, warnings, restored).
fn restore_or_import(
    editing: Option<(bool, impl FnOnce() -> Option<Vec<u8>>)>,
    import: impl FnOnce() -> Result<(Document, Vec<String>)>,
) -> Result<(Document, Vec<String>, bool)> {
    let mut fallback = None;
    if let Some((intact, bytes)) = editing {
        let native = if intact { bytes() } else { None };
        match native.and_then(|b| vectorcraft_format::load(&b).ok()) {
            Some(doc) => return Ok((doc, vec![], true)),
            None => fallback = Some(if intact { EDITING_DAMAGED } else { EDITING_STALE }),
        }
    }
    let (doc, mut warnings) = import()?;
    if let Some(why) = fallback {
        warnings.insert(0, why.to_string());
    }
    Ok((doc, warnings, false))
}

/// Read a file of any readable format (`name`: its file name or path, for the extension and title).
pub fn load(name: &str, bytes: &[u8]) -> Result<Loaded> {
    load_with(name, bytes, &LoadOptions::default())
}

/// [`load`] with `document.open` options (the PDF pages, box and password).
pub fn load_with(name: &str, bytes: &[u8], opts: &LoadOptions) -> Result<Loaded> {
    // PostScript (.ai saved in an older format or without PDF compatibility, .eps) says so, by
    // any name.
    if vectorcraft_pdf::is_postscript(bytes) {
        return Err(err(format!("can't open `{}`: {}", file_name(name), vectorcraft_pdf::PdfError::PostScript)));
    }
    let format = detect(name, bytes).ok_or_else(|| match super::unsupported(&super::extension(name)) {
        Some(u) => err(format!("can't open `{}`: {}", file_name(name), u.hint)),
        None => err(format!("can't open `{name}`: not a format VectorCraft reads (see document.formats)")),
    })?;
    let title = file_name(name);
    let mut converted = false;
    let (mut doc, warnings, restored) = match format.id {
        "vectorcraft" | "template" => {
            let (d, info) = vectorcraft_format::load_info(bytes).map_err(err)?;
            converted = info.is_old() || super::extension(name) == vectorcraft_format::LEGACY_EXTENSION;
            (d, vec![], false)
        }
        "svg" | "svgz" => {
            let text = vectorcraft_svg::text_of(bytes).map_err(err)?;
            let editing = vectorcraft_svg::editing(&text).map(|e| (e.intact, move || vectorcraft_format::base64_decode(&e.data)));
            restore_or_import(editing, || vectorcraft_svg::import_with_report(&text).map_err(err))?
        }
        "pdf" | "ai" | "ait" => {
            // Saved with Preserve Editing: the document it carries, unless some pages are picked.
            let editing = opts.pages.is_none().then(|| vectorcraft_pdf::editing(bytes)).flatten().map(|e| (e.intact, move || Some(e.data)));
            restore_or_import(editing, || super::pdfimport::import(bytes, opts))?
        }
        _ if format.raster => (raster_doc(&title, bytes)?, vec![], false),
        _ => return Err(err(format!("{} files can't be opened yet", format.label))),
    };
    if let Some(mode) = opts.color_mode.filter(|m| *m != doc.color_mode) {
        super::super::colormgmt::set_color_mode(&mut doc, mode, true, None);
    }
    // Imports are named after the file; a native document keeps its own title (the tab shows the
    // file name once it has a path).
    if !matches!(format.id, "vectorcraft" | "template") || doc.title.is_empty() {
        doc.title = title;
    }
    Ok(Loaded { doc, format, warnings, restored, converted })
}

/// Open a file's bytes as the new active document (what `document.open` does) →
/// `{index, title, format, warnings, restored, missingLinks, modifiedLinks, updatedLinks}`. The
/// document keeps `path` (and its format, for Save) when Save can write that format back: not a
/// template, a partly read PDF, or a `.ai` file whose native document didn't come back. The
/// document's linked files are looked for from `path` ([`crate::cmd::links::resolve`]).
pub fn open_bytes(s: &mut Session, name: &str, bytes: &[u8], path: Option<String>) -> Result<Value> {
    open_bytes_with(s, name, bytes, path, &Value::Null)
}

/// [`open_bytes`] with the `document.open` options in `p` ([`LoadOptions::from_params`]).
pub fn open_bytes_with(s: &mut Session, name: &str, bytes: &[u8], path: Option<String>, p: &Value) -> Result<Value> {
    let opts = LoadOptions::from_params("document.open", p)?;
    open_loaded(s, load_with(name, bytes, &opts)?, path, &opts, false)
}

/// File → New from Template: any readable file (at `path`, if it is one: its links are looked for
/// from there) as a new untitled document.
pub fn open_template(s: &mut Session, name: &str, bytes: &[u8], path: Option<&str>) -> Result<Value> {
    open_loaded(s, load(name, bytes)?, path.map(str::to_string), &LoadOptions::default(), true)
}

/// Make a loaded file the new active document; `opts` are the options it was read with.
fn open_loaded(s: &mut Session, loaded: Loaded, path: Option<String>, opts: &LoadOptions, as_template: bool) -> Result<Value> {
    let Loaded { mut doc, format, warnings, restored, converted } = loaded;
    let links = crate::cmd::links::resolve(&mut doc, path.as_deref(), s.prefs.update_links == "automatically");
    // A template (saved by Save as Template, or an .ait/.vctemplate file) opens as a new untitled
    // document.
    let template = as_template || doc.template || matches!(format.id, "ait" | "template");
    if template {
        doc.template = false;
        doc.title = s.next_untitled();
    }
    // Save writes the file back only in a format it writes, only a whole PDF (see
    // [`LoadOptions::is_partial`]), and a .ai file only when it carried the native document (Save
    // writes .ai that way).
    let partial = format.id == "pdf" && opts.is_partial();
    let lossy_ai = format.id == "ai" && !restored;
    let path = path.filter(|_| !template && !partial && !lossy_ai && SAVE_FORMATS.contains(&format.id));
    let saves_back = path.is_some();
    let converted = saves_back && converted && s.prefs.append_converted;
    let index = s.add_document(doc, path);
    let st = s.doc_mut()?;
    if saves_back {
        st.format = format.id;
    }
    st.converted = converted;
    let title = s.documents()[index].title();
    Ok(super::merge(json!({ "index": index, "title": title, "format": format.id, "warnings": warnings, "restored": restored }), links.to_json()))
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
    open_bytes_with(s, src.name, &src.bytes, src.path.map(str::to_string), p)
}

pub(super) fn new_from_template(s: &mut Session, p: &Value) -> Result<Value> {
    let src = source(p, "file.newFromTemplate")?;
    open_template(s, src.name, &src.bytes, src.path)
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
    let blob = ImageBlob::new(mime, bytes);
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
