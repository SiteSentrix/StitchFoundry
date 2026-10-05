//! Document file I/O for every frontend (desktop, web, CLI, control channel, headless MCP): open,
//! save, export and serialize by path or bytes (base64), the format table, and `command.batch`.
//!
//! - `load`: `document.open` (native, legacy, SVG/SVGZ, PDF/.ai/.ait, raster images).
//! - `encode`: one encoder per writable format, with typed options parsed from the params.
//! - `svg`: the SVG Options (styling, fonts, images, object ids, artboards…).
//! - `export`: `document.export` / `serialize` / `exportSelection` / `exportForScreens`.
//! - `save`: `document.save`, Save As / a Copy / as Template, New from Template, Revert and the
//!   per-format options (`file.formatOptions`); [`save_with`] is the one save path of every frontend.
//! - `pdf`: PDF settings and presets for every PDF export, `document.exportPdf`.
//! - `pdfimport`: the PDF pages, box and password `document.open` and Place read, `document.pdfInfo`.
//!
//! [`FORMATS`] is the single list of formats (append-only); open dialogs use [`open_filters`],
//! agents query `document.formats`.

mod batch;
mod encode;
mod export;
mod imagemap;
mod load;
pub mod pdf;
mod pdfimport;
pub mod pngtext;
pub mod ppi;
mod save;
mod svg;

use serde_json::{Value, json};

pub use encode::{ARTBOARD_PARAMS, ArtboardPick, Encoded, encode, encode_all, encode_with_warnings};
pub(crate) use encode::{anti_alias, background, with_single_artboard};
use load::err;
pub(crate) use load::source;
pub use load::{Loaded, RasterImage, detect, file_name, load, load_with, open_bytes, open_bytes_with, open_template, raster_image};
pub use pdfimport::{LoadOptions, page_document};
pub use save::{SAVE_FORMATS, SaveMode, SavePlan, save_filters, save_format, save_plan, save_with, stamp_save_dates, templates_folder};
pub use svg::options_map as svg_options;

use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "document.open",
            "Open Document",
            [],
            None,
            "{path} or {name, dataBase64}, PDF/.ai: pages?: \"2-3, 5\" (1-based, default all; one artboard and layer each) | page?: n, cropTo?: bounding|art|crop (default)|trim|bleed|media (the box each artboard gets; bounding: the art's bounds), password? (encrypted PDFs; see document.pdfInfo), colorMode?: rgb|cmyk (the mode the document opens in, its colours converted as file.documentColorMode does; default: the file's — a PDF keeps CMYK, Gray and spot inks (spot swatches at a tint) and opens in CMYK when painted mostly in CMYK) → {index, title, format, warnings, restored, missingLinks, modifiedLinks, updatedLinks: [{name, path, ids}]}; any readable format (see document.formats): .vectorcraft/.drawcraft, .svg/.svgz, .pdf/.ai, .ait, PNG/JPEG/GIF/WebP/TIFF/BMP (an image opens as a document of its pixel size). A PDF/.ai/.ait or SVG saved with Preserve Editing restores the native document it carries (restored: true; a PDF only when no pages are picked), unless the file was changed elsewhere since or the data can't be read: then its artwork is imported and the first warning says why. Templates (native templates, .ait) open as a new untitled document; a restored .ai keeps its path (Save writes .ai again). Linked images are read from their files (looked for at their path, then relative to the document): missing ones show their saved preview (links.relink), modified ones are read again only with Preferences › Update Links: Automatically (else links.update)",
            always,
            load::open
        ),
        cmd!(
            query "document.serialize",
            "Serialize Document",
            [],
            None,
            "{format?: vectorcraft (default)|template|svg|svgz|pdf|png|jpg|webp, …the format's options (see document.formats; SVG ones also as svg: {…})} → {text, warnings} for svg, else {dataBase64, warnings}; an SVG of several artboards also gives files: [{name, text}], linked images linked: [{name, dataBase64}]",
            has_doc,
            export::serialize
        ),
        cmd!(
            "document.export",
            "Export Document",
            [],
            None,
            "{path?, format?: svg|svgz|pdf|png|jpg|webp|vectorcraft|template (default: from the path's extension, else png), artboard?: 0, artboards?: [i…], range?: \"1-3, 5\" | \"all\" (1-based; PDF writes one page per artboard, default all; SVG writes one file per artboard, {stem}-{artboard}.svg; raster formats write one artboard), useArtboards?: true (raster: one file per chosen artboard, default all, {stem}-{artboard}.{ext}; pdf: every page) | false (pdf/raster: the bounds of the visible art; SVG has it as an SVG option), raster: ppi?: 72 (pixels per inch, stored in the file; wins over scale), scale?: 1 (pixels per point), background?: transparent|white|black|\"#rrggbb\" (jpg: white when transparent), antiAlias?: none|art (default)|type (text snapped to pixels), interlaced?: false (png, Adam7), jpg: quality?: 90 (0–100), colorModel?: rgb|cmyk|gray, method?: baseline|optimized|progressive, scans?: 3 (3–5, progressive), embedIcc?: true, imageMap?: none|client|server (an HTML or NCSA map of the objects with a URL, written as <stem>.html / <stem>.map); SVG options flat or as svg: {styling, outlineText, images, objectIds, decimals, minify, responsive, useArtboards, preserveEditing, metadata, fewerTspans, hiddenLayers} (see document.formats), …the PDF options of document.exportPdf} → {path, format, bytes, warnings, files?: [path…] (several), linked?: [path…] (linked images, image maps)}; no path → {dataBase64, format, bytes, warnings, files?: [{name, dataBase64}], linked?: [{name, dataBase64}]}. Never changes the document's path",
            has_doc,
            export::export
        ),
        cmd!(
            "document.exportSelection",
            "Export Selection…",
            ["File"],
            None,
            "{path?, format?: png|jpg|webp|svg|svgz|pdf (default: from the extension, else png), scale?: 1, …the format's options} the selected objects cropped to their bounds (template layers left out) → {path, bytes, bounds} (no path → {dataBase64, bounds})",
            has_selection,
            export::export_selection
        ),
        cmd!(
            "document.exportForScreens",
            "Export for Screens",
            ["File", "Export"],
            None,
            "{folder?, artboards?: [index…] | range?: \"1-3\" (default all), formats?: [{format: png|jpg|webp|svg|svgz|pdf, scale?: 1 (raster only), ppi?: (raster: wins over scale, scale = ppi / 72), suffix?: \"@2x\" (raster default: @{scale}x when scale ≠ 1; vector formats drop @Nx suffixes), …the format's options (antiAlias, background…)}], prefix?, antiAlias?: none|art|type (raster rows without their own)} one file per artboard and format (a PDF holds its artboard alone; artboards with the same name, in any case, get -2, -3…; an unnamed one is Artboard-N) → {files: [path…]}; no folder → {files: [{name, dataBase64}]}",
            has_doc,
            export::export_for_screens
        ),
        cmd!(query "command.batch", "Batch", [], None, "{label?, commands: [{command, params}]} run several commands as ONE undo step; stops at the first error and rolls back", has_doc, batch::batch),
        cmd!(
            query "document.formats",
            "File Formats",
            [],
            None,
            "{} → {formats: [{id, label, extensions, mime, read, write, raster, options: {name: {type, default, description}}}], readable: [id…], writable: [id…], openExtensions: [ext…]}",
            always,
            formats
        ),
        cmd!(
            query "document.pdfInfo",
            "PDF Info",
            [],
            None,
            "{path} or {name?, dataBase64}, password?, thumbnail?: page (1-based), thumbnailSize?: 160 (px, longest side), cropTo?: crop (the thumbnail's box) → {pages, needsPassword, wrongPassword?, pageInfo: [{width, height (pt, as shown), rotation, boxes: {media, crop, bleed, trim, art: [x0, y0, x1, y1] (PDF space, pt)}}], thumbnail?: PNG dataBase64}; an encrypted PDF without its password → {pages: 0, needsPassword: true}",
            always,
            pdfimport::pdf_info
        ),
    ]
    .into_iter()
    .chain(save::specs())
    .collect()
}

/// One option a format's encoder reads from the export params.
#[derive(Clone, Copy, Debug)]
pub struct FormatOption {
    pub name: &'static str,
    /// JSON type: `number`, `integer`, `boolean`, `string`, `array` or `object`.
    pub ty: &'static str,
    /// The default as a JSON literal (`"1"`, `"false"`, `"null"`).
    pub default: &'static str,
    pub description: &'static str,
}

/// One file format: how `document.open` recognises it and what `document.export` writes.
#[derive(Clone, Copy, Debug)]
pub struct Format {
    /// The `format` param value (`svg`, `png`, …).
    pub id: &'static str,
    /// Open/save dialog filter name.
    pub label: &'static str,
    /// Lower-case extensions without the dot; the first is the one exports use.
    pub extensions: &'static [&'static str],
    pub mime: &'static str,
    pub read: bool,
    pub write: bool,
    /// Pixels (export `scale` applies) rather than vectors.
    pub raster: bool,
    /// Options the encoder reads (writable formats).
    pub options: &'static [FormatOption],
}

impl Format {
    pub fn to_json(&self) -> Value {
        let options: serde_json::Map<String, Value> = self
            .options
            .iter()
            .map(|o| {
                let default: Value = serde_json::from_str(o.default).unwrap_or(Value::Null);
                (o.name.to_string(), json!({"type": o.ty, "default": default, "description": o.description}))
            })
            .collect();
        json!({
            "id": self.id,
            "label": self.label,
            "extensions": self.extensions,
            "mime": self.mime,
            "read": self.read,
            "write": self.write,
            "raster": self.raster,
            "options": options,
        })
    }
}

const ARTBOARD: FormatOption = FormatOption {
    name: "artboard",
    ty: "integer",
    default: "0",
    description: "0-based artboard to export (`artboards: [i]` or `range: \"n\"` naming one artboard work too)",
};
const ARTBOARDS: FormatOption = FormatOption {
    name: "artboards",
    ty: "array",
    default: "null",
    description: "0-based artboards (PDF: one page each, default all; SVG, and raster formats with useArtboards: one file each)",
};
const RANGE: FormatOption = FormatOption {
    name: "range",
    ty: "string",
    default: "null",
    description: "1-based artboards such as \"1-3, 5\", or \"all\" (wins over artboards and artboard)",
};
const SCALE: FormatOption = FormatOption { name: "scale", ty: "number", default: "1", description: "pixels per point (0.01–64)" };
const QUALITY: FormatOption =
    FormatOption { name: "quality", ty: "integer", default: "90", description: "JPEG quality 0–100 (the JPEG Options dialog shows 0–10)" };
const USE_ARTBOARDS: FormatOption = FormatOption {
    name: "useArtboards",
    ty: "boolean",
    default: "null",
    description: "true: every chosen artboard (default all), one file each named <file>-<artboard>.<ext> (PDF: one page each); false: the bounds of the visible art",
};
const PPI: FormatOption = FormatOption {
    name: "ppi",
    ty: "number",
    default: "72",
    description: "resolution in pixels per inch (72 = one pixel per point; 150, 300…), stored in the file; wins over scale",
};
const BACKGROUND: FormatOption = FormatOption {
    name: "background",
    ty: "string",
    default: "\"transparent\"",
    description: "transparent, white, black or a colour such as \"#ff8800\" (JPEG: white when transparent)",
};
const ANTI_ALIAS: FormatOption = FormatOption {
    name: "antiAlias",
    ty: "string",
    default: "\"art\"",
    description: "none (hard pixel edges), art (smooth edges) or type (smooth, text snapped to whole pixels)",
};
const INTERLACED: FormatOption =
    FormatOption { name: "interlaced", ty: "boolean", default: "false", description: "Adam7 interlacing (the image builds up while it loads)" };
const COLOR_MODEL: FormatOption = FormatOption {
    name: "colorModel",
    ty: "string",
    default: "\"rgb\"",
    description: "rgb, cmyk (ink amounts in the working CMYK space: CMYK colours keep their inks) or gray",
};
const METHOD: FormatOption = FormatOption {
    name: "method",
    ty: "string",
    default: "\"baseline\"",
    description: "baseline (standard), optimized (smaller Huffman tables) or progressive (builds up in scans)",
};
const SCANS: FormatOption = FormatOption { name: "scans", ty: "integer", default: "3", description: "progressive scans, 3–5" };
const EMBED_ICC: FormatOption = FormatOption {
    name: "embedIcc",
    ty: "boolean",
    default: "true",
    description: "embed the colour profile: sRGB (RGB), the working CMYK space (CMYK) or gray with sRGB's tone curve",
};
const IMAGE_MAP: FormatOption = FormatOption {
    name: "imageMap",
    ty: "string",
    default: "\"none\"",
    description: "none, client (an HTML page with <map>, <stem>.html) or server (an NCSA <stem>.map): the areas of objects with a URL and an Image Map shape (attributes.set)",
};

/// A format `document.open` reads but nothing writes yet.
const fn reader(id: &'static str, label: &'static str, extensions: &'static [&'static str], mime: &'static str, raster: bool) -> Format {
    Format { id, label, extensions, mime, read: true, write: false, raster, options: &[] }
}

/// Every format VectorCraft reads or writes. Append-only: new formats go at the end.
pub const FORMATS: &[Format] = &[
    Format {
        id: "vectorcraft",
        label: "VectorCraft",
        extensions: &[vectorcraft_format::EXTENSION, vectorcraft_format::LEGACY_EXTENSION],
        mime: "application/json",
        read: true,
        write: true,
        raster: false,
        options: &[],
    },
    Format { id: "svg", label: "SVG", extensions: &["svg"], mime: "image/svg+xml", read: true, write: true, raster: false, options: svg::OPTIONS },
    Format {
        id: "svgz",
        label: "SVG Compressed",
        extensions: &["svgz"],
        mime: "image/svg+xml",
        read: true,
        write: true,
        raster: false,
        options: svg::OPTIONS,
    },
    Format { id: "pdf", label: "PDF", extensions: &["pdf"], mime: "application/pdf", read: true, write: true, raster: false, options: pdf::OPTIONS },
    reader("ai", "PDF-compatible .ai", &["ai"], "application/pdf", false),
    reader("ait", "PDF-compatible .ait template", &["ait"], "application/pdf", false),
    Format {
        id: "png",
        label: "PNG",
        extensions: &["png"],
        mime: "image/png",
        read: true,
        write: true,
        raster: true,
        options: &[ARTBOARD, ARTBOARDS, RANGE, USE_ARTBOARDS, PPI, SCALE, BACKGROUND, ANTI_ALIAS, INTERLACED],
    },
    Format {
        id: "jpg",
        label: "JPEG",
        extensions: &["jpg", "jpeg"],
        mime: "image/jpeg",
        read: true,
        write: true,
        raster: true,
        options: &[
            ARTBOARD,
            ARTBOARDS,
            RANGE,
            USE_ARTBOARDS,
            PPI,
            SCALE,
            BACKGROUND,
            ANTI_ALIAS,
            QUALITY,
            COLOR_MODEL,
            METHOD,
            SCANS,
            EMBED_ICC,
            IMAGE_MAP,
        ],
    },
    reader("gif", "GIF", &["gif"], "image/gif", true),
    Format {
        id: "webp",
        label: "WebP",
        extensions: &["webp"],
        mime: "image/webp",
        read: true,
        write: true,
        raster: true,
        options: &[ARTBOARD, ARTBOARDS, RANGE, USE_ARTBOARDS, PPI, SCALE, BACKGROUND, ANTI_ALIAS],
    },
    reader("tiff", "TIFF", &["tif", "tiff"], "image/tiff", true),
    reader("bmp", "BMP", &["bmp"], "image/bmp", true),
    Format {
        id: "template",
        label: "VectorCraft Template",
        extensions: &["vctemplate"],
        mime: "application/json",
        read: true,
        write: true,
        raster: false,
        options: &[],
    },
];

/// Every extension `document.open` reads (the "All readable files" filter of open dialogs).
pub const OPEN_EXTS: &[&str] =
    &["vectorcraft", "drawcraft", "svg", "svgz", "pdf", "ai", "ait", "png", "jpg", "jpeg", "gif", "webp", "tif", "tiff", "bmp", "vctemplate"];

/// Text files: File → Place sets them as area type (Text Import Options).
pub const TEXT_EXTS: &[&str] = &["txt"];

/// Every extension File → Place reads: [`OPEN_EXTS`] and [`TEXT_EXTS`].
pub const PLACE_EXTS: &[&str] =
    &["vectorcraft", "drawcraft", "svg", "svgz", "pdf", "ai", "ait", "png", "jpg", "jpeg", "gif", "webp", "tif", "tiff", "bmp", "vctemplate", "txt"];

/// One dialog filter per readable format.
fn format_filters() -> impl Iterator<Item = (&'static str, &'static [&'static str])> {
    FORMATS.iter().filter(|f| f.read).map(|f| (f.label, f.extensions))
}

/// Open-dialog filters: "All readable files" first, then one per readable format, then swatch
/// libraries (which open in the library panel), flattener presets and PDF presets (imported).
pub fn open_filters() -> impl Iterator<Item = (&'static str, &'static [&'static str])> {
    std::iter::once(("All readable files", OPEN_EXTS))
        .chain(format_filters())
        .chain(std::iter::once(("Swatch libraries", super::swatchlib::LIBRARY_EXTS)))
        .chain(std::iter::once(("Flattener presets", super::flatten::PRESET_EXTS)))
        .chain(std::iter::once(("PDF presets", super::pdfcmds::PRESET_EXTS)))
}

/// File → Place dialog filters: "All placeable files", then one per readable format, then text.
pub fn place_filters() -> impl Iterator<Item = (&'static str, &'static [&'static str])> {
    std::iter::once(("All placeable files", PLACE_EXTS)).chain(format_filters()).chain(std::iter::once(("Text", TEXT_EXTS)))
}

/// A format by id or extension (any case, leading dot allowed; `jpeg` finds `jpg`).
pub fn format(id_or_ext: &str) -> Option<&'static Format> {
    let k = id_or_ext.trim_start_matches('.').to_ascii_lowercase();
    FORMATS.iter().find(|f| f.id == k).or_else(|| FORMATS.iter().find(|f| f.extensions.contains(&k.as_str())))
}

/// The lower-case extension of a file name or path (empty when it has none).
pub fn extension(name: &str) -> String {
    std::path::Path::new(name).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// A file name or path without its folder and extension (`/a/Poster.svg` → `Poster`).
pub fn file_stem(name: &str) -> String {
    std::path::Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| name.to_string())
}

/// The format a file name's extension names.
pub fn format_for_name(name: &str) -> Option<&'static Format> {
    Some(extension(name)).filter(|e| !e.is_empty()).and_then(|e| format(&e))
}

/// The format to write: `format` (an id or extension), else the path's extension, else PNG.
pub fn writable_format(format_param: Option<&str>, path: Option<&str>) -> std::result::Result<&'static Format, String> {
    let f = match (format_param, path.map(extension).filter(|e| !e.is_empty())) {
        (Some(f), _) => format(f).ok_or_else(|| format!("unknown format `{f}` (see document.formats)"))?,
        (None, Some(e)) => format(&e).ok_or_else(|| format!("unknown extension `.{e}`: pass `format` (see document.formats)"))?,
        (None, None) => format("png").ok_or("no PNG encoder")?,
    };
    if f.write { Ok(f) } else { Err(format!("{} files can be opened but not written (see document.formats)", f.label)) }
}

/// [`writable_format`] for command `cmd`.
fn writable(cmd: &str, format_param: Option<&str>, path: Option<&str>) -> Result<&'static Format> {
    writable_format(format_param, path).map_err(|e| bad(cmd, e))
}

fn formats(_: &mut Session, _: &Value) -> Result<Value> {
    let ids = |pick: fn(&Format) -> bool| FORMATS.iter().filter(|f| pick(f)).map(|f| f.id).collect::<Vec<_>>();
    Ok(json!({
        "formats": FORMATS.iter().map(Format::to_json).collect::<Vec<_>>(),
        "readable": ids(|f| f.read),
        "writable": ids(|f| f.write),
        "openExtensions": OPEN_EXTS,
    }))
}

// ---------- the file system (none on the web, where commands take and return bytes) ----------

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn read_file(path: &str) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn write_file(path: &str, bytes: &[u8]) -> Result<()> {
    std::fs::write(path, bytes).map_err(|e| EngineError::Other(format!("{path}: {e}")))
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn create_dir(path: &str) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|e| EngineError::Other(format!("{path}: {e}")))
}

/// A file's size (bytes) and modification time (ms since the Unix epoch, when the file system
/// keeps one); `None` when there is no file at `path`.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn file_stamp(path: &str) -> Option<(u64, Option<u64>)> {
    let m = std::fs::metadata(path).ok().filter(std::fs::Metadata::is_file)?;
    let modified = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as u64);
    Some((m.len(), modified))
}

/// `path` made absolute against the working directory (as given when absolute already, or when
/// that fails).
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn absolute_path(path: &str) -> String {
    if std::path::Path::new(path).is_absolute() {
        return path.to_string();
    }
    std::path::absolute(path).map_or_else(|_| path.to_string(), |p| p.to_string_lossy().into_owned())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn file_stamp(_: &str) -> Option<(u64, Option<u64>)> {
    None
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn absolute_path(path: &str) -> String {
    path.to_string()
}

#[cfg(target_arch = "wasm32")]
fn no_fs(path: &str) -> EngineError {
    EngineError::Other(format!("{path}: no file system here (pass dataBase64 to open; omit the path to get dataBase64)"))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn read_file(path: &str) -> Result<Vec<u8>> {
    Err(no_fs(path))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn write_file(path: &str, _: &[u8]) -> Result<()> {
    Err(no_fs(path))
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn create_dir(path: &str) -> Result<()> {
    Err(no_fs(path))
}

/// File-name parts for artboards `boards` of `doc`: the artboard's name with unsafe characters as
/// `-`, `Artboard-N` when unnamed, and `-2`, `-3`… after a name already used (compared without
/// case: `Icon` and `icon` are one file on most desktop file systems).
pub fn artboard_file_names(doc: &vectorcraft_doc::Document, boards: &[usize]) -> Vec<String> {
    let mut taken = std::collections::HashSet::new();
    boards
        .iter()
        .map(|&b| {
            let name = doc.artboards.get(b).map_or("", |a| a.name.as_str());
            let mut base: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' }).collect();
            if base.is_empty() {
                base = format!("Artboard-{}", b + 1);
            }
            let mut name = base.clone();
            for i in 2.. {
                if taken.insert(name.to_lowercase()) {
                    break;
                }
                name = format!("{base}-{i}");
            }
            name
        })
        .collect()
}

/// Write an export's files for the destination `path` → `{path, bytes, files?, linked?, …extra}`
/// (`files`: every file when there are several; `linked`: the images it links to); with no path
/// → the same with `dataBase64` and `{name, dataBase64}` rows, named after `name`.
fn write_encoded(path: Option<&str>, name: &str, doc: &vectorcraft_doc::Document, enc: &Encoded, extra: Value) -> Result<Value> {
    let files = enc.named(doc, path.unwrap_or(name));
    let (main, linked) = files.split_at(enc.files.len());
    let row = |(name, bytes): &(String, std::borrow::Cow<[u8]>)| match path {
        Some(_) => json!(name),
        None => json!({ "name": name, "dataBase64": vectorcraft_format::base64_encode(bytes) }),
    };
    if path.is_some() {
        for (p, bytes) in &files {
            write_file(p, bytes)?;
        }
    }
    let Some((first, bytes)) = main.first() else { return Err(EngineError::Other("the export wrote no file".into())) };
    let mut out = match path {
        Some(_) => json!({ "path": first, "bytes": bytes.len() }),
        None => json!({ "dataBase64": vectorcraft_format::base64_encode(bytes), "bytes": bytes.len() }),
    };
    if main.len() > 1 {
        out["files"] = main.iter().map(row).collect();
    }
    if !linked.is_empty() {
        out["linked"] = linked.iter().map(row).collect();
    }
    Ok(merge(out, extra))
}

/// `a` with the fields of `b`.
fn merge(mut a: Value, b: Value) -> Value {
    if let (Some(a), Value::Object(b)) = (a.as_object_mut(), b) {
        a.extend(b);
    }
    a
}

/// A file name for bytes handed back without a path: the document's title with `ext`.
fn default_name(doc: &vectorcraft_doc::Document, ext: &str) -> String {
    let stem = std::path::Path::new(&doc.title).file_stem().map(|s| s.to_string_lossy().into_owned()).filter(|s| !s.is_empty());
    format!("{}.{ext}", stem.as_deref().unwrap_or("Untitled"))
}

/// Write `bytes` to `path` → `{path, bytes, …extra}`; with no path → `{dataBase64, bytes, …extra}`.
fn write_or_return(path: Option<&str>, bytes: &[u8], extra: Value) -> Result<Value> {
    let out = match path {
        Some(path) => {
            write_file(path, bytes)?;
            json!({ "path": path, "bytes": bytes.len() })
        }
        None => json!({ "dataBase64": vectorcraft_format::base64_encode(bytes), "bytes": bytes.len() }),
    };
    Ok(merge(out, extra))
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_pdf;
#[cfg(test)]
mod tests_pdfcolor;
#[cfg(test)]
mod tests_pdfedit;
#[cfg(test)]
mod tests_pdfimport;
#[cfg(test)]
mod tests_svg;
#[cfg(test)]
mod tests_svgedit;

#[cfg(test)]
mod tests_raster;

#[cfg(test)]
mod tests_exportas;

#[cfg(test)]
mod tests_jpeg;
