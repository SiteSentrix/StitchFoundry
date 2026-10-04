//! One encoder per writable format. Each format's options are parsed, typed, from the export params
//! (unknown keys are ignored, so one params object can carry the options of several formats).

use serde::Deserialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use vectorcraft_doc::Document;

use super::super::*;
use super::Format;
use crate::EngineError;

const C: &str = "document.export";

/// Which artboards an export covers: `range` (`"1-3, 5"`, 1-based) wins over `artboards`
/// (0-based), which wins over `artboard`.
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

#[derive(Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct SvgOptions {
    #[serde(flatten)]
    boards: ArtboardPick,
    /// Text as glyph outlines (SVG Options → Fonts → Convert to Outlines).
    outline_text: Option<bool>,
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

/// Encode `doc` as `format` (an id or extension from [`super::FORMATS`]) with that format's
/// options from `p` (see `document.formats`). Raster formats leave template layers out.
pub fn encode(doc: &Document, format: &str, p: &Value) -> Result<Vec<u8>> {
    let f = super::writable(C, Some(format), None)?;
    let n = doc.artboards.len();
    Ok(match f.id {
        "vectorcraft" => vectorcraft_format::save_file(doc),
        "svg" => {
            let o: SvgOptions = options(f, p)?;
            let artboard = if n == 0 { None } else { Some(boards(o.boards.one(n))?) };
            let opts = vectorcraft_svg::ExportOptions { artboard, outline_text: o.outline_text.unwrap_or(false), ..Default::default() };
            vectorcraft_svg::export(doc, &opts).into_bytes()
        }
        "pdf" => {
            let o: ArtboardPick = options(f, p)?;
            let opts = vectorcraft_pdf::PdfOptions { artboards: boards(o.resolve(n))?, ..Default::default() };
            super::super::rasterfx::export_pdf(doc, &opts).map_err(|e| EngineError::Other(e.to_string()))?
        }
        "png" | "jpg" | "webp" => {
            let o: RasterOptions = options(f, p)?;
            let region = doc.artboards[boards(o.boards.one(n))?].rect;
            let scale = o.scale.unwrap_or(1.0).clamp(0.01, 64.0);
            let img = vectorcraft_render::Renderer::new().render_region(doc, region, scale, f.id == "jpg");
            match f.id {
                "png" => img.to_png(),
                "webp" => img.to_webp(),
                _ => img.to_jpeg(o.quality.unwrap_or(90)),
            }
        }
        _ => return Err(bad(C, format!("no encoder for {} yet", f.label))),
    })
}
