//! One encoder per writable format. Each format's options are parsed, typed, from the export params
//! (unknown keys are ignored, so one params object can carry the options of several formats).

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use vectorcraft_doc::Document;

use super::super::*;
use super::Format;

const C: &str = "document.export";

/// The params that pick artboards (the fields of [`ArtboardPick`]).
pub const ARTBOARD_PARAMS: [&str; 3] = ["artboard", "artboards", "range"];

/// Which artboards an export covers: `range` (`"1-3, 5"`, 1-based, or `"all"`) wins over
/// `artboards` (0-based), which wins over `artboard`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ArtboardPick {
    pub artboard: Option<usize>,
    pub artboards: Option<Vec<usize>>,
    pub range: Option<String>,
}

impl ArtboardPick {
    /// The named artboards (`None`: none named), checked against the document's `count`.
    pub fn resolve(&self, count: usize) -> std::result::Result<Option<Vec<usize>>, String> {
        let v = match (&self.range, &self.artboards, self.artboard) {
            (Some(r), _, _) if r.trim().eq_ignore_ascii_case("all") => (0..count).collect(),
            (Some(r), _, _) => parse_range(r, count)?,
            (None, Some(a), _) => a.clone(),
            (None, None, Some(a)) => vec![a],
            (None, None, None) => return Ok(None),
        };
        if v.is_empty() {
            return Err("no artboards named".into());
        }
        match v.iter().find(|i| **i >= count) {
            Some(i) => Err(format!("no artboard {i} (0-based; the document has {count})")),
            None => Ok(Some(v)),
        }
    }

    /// The one artboard a single-image format writes (default: the first).
    pub fn one(&self, count: usize) -> std::result::Result<usize, String> {
        match self.resolve(count)?.as_deref() {
            None if count > 0 => Ok(0),
            None => Err("the document has no artboard".into()),
            Some([i]) => Ok(*i),
            Some(_) => Err("this format holds one artboard: export several as PDF or with document.exportForScreens".into()),
        }
    }
}

/// What an export writes: its files (one, or one per artboard for an SVG of several artboards)
/// and the image files an SVG links to.
#[derive(Default)]
pub struct Encoded {
    /// `(artboard, bytes)`; the artboard names the file when there are several.
    pub files: Vec<(Option<usize>, Vec<u8>)>,
    /// Images to write next to the file(s) (SVG `images: "link"`).
    pub linked: Vec<vectorcraft_svg::LinkedImage>,
    /// The encoder's warnings (features approximated or left out, options not applied yet).
    pub warnings: Vec<String>,
}

impl Encoded {
    fn one(bytes: Vec<u8>) -> Self {
        Self { files: vec![(None, bytes)], ..Self::default() }
    }

    /// Every file to write for the destination `path` (a path or a file name): the file itself
    /// when there is one, else `{stem}-{artboard name}.{ext}` beside it, then the linked images
    /// beside it under their own names.
    pub fn named<'a>(&'a self, doc: &Document, path: &str) -> Vec<(String, &'a [u8])> {
        // Siblings keep the path's own separators (this runs on every platform and on the web).
        let dir = &path[..path.rfind(['/', '\\']).map_or(0, |i| i + 1)];
        let sibling = |name: &str| format!("{dir}{name}");
        let mut out: Vec<(String, &[u8])> = match self.files.as_slice() {
            [(_, bytes)] => vec![(path.to_string(), bytes.as_slice())],
            files => {
                let boards: Vec<usize> = files.iter().map(|(b, _)| b.unwrap_or(0)).collect();
                let file = std::path::Path::new(&path[dir.len()..]);
                let stem = file.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let ext = file.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
                super::artboard_file_names(doc, &boards)
                    .into_iter()
                    .zip(files)
                    .map(|(board, (_, bytes))| (sibling(&format!("{stem}-{board}{ext}")), bytes.as_slice()))
                    .collect()
            }
        };
        out.extend(self.linked.iter().map(|l| (sibling(&l.name), l.bytes.as_slice())));
        out
    }

    /// The one file and the warnings, when the export wrote nothing else.
    fn single(self, f: &Format) -> Result<(Vec<u8>, Vec<String>)> {
        if !self.linked.is_empty() {
            return Err(bad(C, format!("{} with linked images writes several files: use document.export or document.save", f.label)));
        }
        match <[_; 1]>::try_from(self.files) {
            Ok([(_, bytes)]) => Ok((bytes, self.warnings)),
            Err(_) => Err(bad(C, format!("{} writes one file per artboard here: use document.export, or name one artboard", f.label))),
        }
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct RasterOptions {
    #[serde(flatten)]
    boards: ArtboardPick,
    scale: Option<f64>,
    quality: Option<u8>,
}

fn boards<T>(r: std::result::Result<T, String>) -> Result<T> {
    r.map_err(|e| bad(C, e))
}

fn options<T: DeserializeOwned + Default>(f: &Format, p: &Value) -> Result<T> {
    if !p.is_object() {
        return Ok(T::default());
    }
    T::deserialize(p).map_err(|e| bad(C, format!("{} options: {e}", f.label)))
}

/// Most pixels a side of a WebP image can have (its sizes are stored in 14 bits).
const WEBP_SIDE: f64 = 16383.0;

/// Refuse a raster export its format can't store (instead of writing an empty file). The
/// renderer's own size limits are [`vectorcraft_render::raster_size`]'s.
fn check_format_size(f: &Format, w: f64, h: f64) -> Result<()> {
    if f.id == "webp" && (w.round() > WEBP_SIDE || h.round() > WEBP_SIDE) {
        return Err(bad(
            C,
            format!("{} × {} pixels is too large for {} (at most {WEBP_SIDE} pixels a side): lower the scale", w.round(), h.round(), f.label),
        ));
    }
    Ok(())
}

/// Encode `doc` as `format` (an id or extension from [`super::FORMATS`]) with that format's
/// options from `p` (see `document.formats`) into one file. Raster formats leave template layers
/// out, and no format writes the opacity-mask editing layer. Exports that write several files (an
/// SVG per artboard, linked images) go through [`encode_all`].
pub fn encode(doc: &Document, format: &str, p: &Value) -> Result<Vec<u8>> {
    encode_with_warnings(doc, format, p).map(|(bytes, _)| bytes)
}

/// Like [`encode`], also returning the encoder's warnings (PDF and SVG: options not applied yet,
/// features approximated or left out; the other formats have none).
pub fn encode_with_warnings(doc: &Document, format: &str, p: &Value) -> Result<(Vec<u8>, Vec<String>)> {
    let f = super::writable(C, Some(format), None)?;
    encode_all(doc, f.id, p)?.single(f)
}

/// [`encode`], with every file the export writes.
pub fn encode_all(doc: &Document, format: &str, p: &Value) -> Result<Encoded> {
    let f = super::writable(C, Some(format), None)?;
    let doc = &*doc.without_edit_modes();
    let n = doc.artboards.len();
    let bytes = match f.id {
        "vectorcraft" => vectorcraft_format::save_file(doc),
        "svg" | "svgz" => return super::svg::encode(doc, p, f.id == "svgz").map_err(|e| bad(C, e)),
        "pdf" => {
            let (bytes, warnings) = super::pdf::encode(C, doc, p)?;
            return Ok(Encoded { warnings, ..Encoded::one(bytes) });
        }
        "png" | "jpg" | "webp" => {
            let o: RasterOptions = options(f, p)?;
            let region = doc.artboards[boards(o.boards.one(n))?].rect;
            let scale = o.scale.unwrap_or(1.0).clamp(0.01, 64.0);
            check_format_size(f, region.width() * scale, region.height() * scale)?;
            vectorcraft_render::raster_size(region, scale).map_err(|e| bad(C, e))?;
            let img = vectorcraft_render::Renderer::new().render_region(doc, region, scale, f.id == "jpg");
            match f.id {
                "png" => img.to_png(),
                "webp" => img.to_webp(),
                _ => img.to_jpeg(o.quality.unwrap_or(90)),
            }
            .map_err(EngineError::Other)?
        }
        _ => return Err(bad(C, format!("no encoder for {} yet", f.label))),
    };
    Ok(Encoded::one(bytes))
}
