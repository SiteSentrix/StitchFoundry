//! VectorCraft PDF export and import.
//!
//! - [`export`] writes one PDF page per artboard with `krilla`: vector paths (fills, strokes with
//!   caps/joins/miter/dashes, inside/outside alignment as clips, non-zero/even-odd; arrowheads,
//!   width profiles, fitted or dotted dashes as the canvas's filled outlines and brushed strokes
//!   as their brush art), opacity and blend modes (transparency groups),
//!   clip groups, linear/radial gradients (shadings), embedded images and text as outlined glyph
//!   paths. Hidden objects, guides and template layers are skipped. [`PdfSettings`] is the Save PDF
//!   dialog's model (standard, compatibility, General, Compression, Marks and Bleeds, Output,
//!   Advanced, Security); options the writer doesn't apply yet come back as warnings.
//! - [`import`] reads PDF (and PDF-compatible `.ai`) pages with `hayro-interpret` into a
//!   [`Document`]: one artboard and one layer per page, paths with fill/stroke, clip groups,
//!   transparency groups, axial/radial shadings → gradients, images (JPEG passthrough, others
//!   re-encoded as PNG) and text as glyph outlines. [`ImportOptions`] pick the pages, the box each
//!   artboard gets ([`CropTo`]) and the password; [`info`] lists the pages and their boxes.
#![forbid(unsafe_code)]

mod export;
mod import;
mod lab_spot;
mod pages;
mod settings;

pub use export::{export, export_with_report};
pub use import::{import, import_with_report};
pub use pages::{PageInfo, PdfInfo, info};
pub use settings::*;

use vectorcraft_doc::Document;

/// Export options: the Save PDF settings plus what one export run decides (pages, title, date).
#[derive(Clone, Debug, Default)]
pub struct PdfOptions {
    pub settings: PdfSettings,
    /// 0-based artboard indices to export, in page order; `None` = all artboards.
    pub artboards: Option<Vec<usize>>,
    /// Document title for the metadata; `None` = the document's title.
    pub title: Option<String>,
    /// Creation date as Unix seconds (UTC); `None` = now (native) / omitted (wasm). PDF/A needs a date.
    pub created: Option<i64>,
}

impl PdfOptions {
    /// Default options with uncompressed content streams (readable operators, for tests and
    /// debugging).
    pub fn uncompressed() -> Self {
        let compression = CompressionSettings { compress_text: false, ..Default::default() };
        Self { settings: PdfSettings { compression, ..Default::default() }, ..Default::default() }
    }
}

/// Import options.
#[derive(Clone, Debug)]
pub struct ImportOptions {
    /// Import at most this many pages (of those picked); `None` = all of them.
    pub max_pages: Option<usize>,
    /// Horizontal gap in points between the artboards created for consecutive pages.
    pub artboard_gap: f64,
    /// 0-based pages to import, in this order; `None` = every page.
    pub pages: Option<Vec<usize>>,
    /// The page box each artboard gets.
    pub crop: CropTo,
    /// The (user) password of an encrypted PDF.
    pub password: Option<String>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        Self { max_pages: None, artboard_gap: 36.0, pages: None, crop: CropTo::default(), password: None }
    }
}

settings::choice! {
    /// The page box an imported or placed page is cropped to: its artboard (or placed frame).
    CropTo {
        /// The bounds of the page's art.
        Bounding = "bounding", "Bounding Box";
        Art = "art", "Art";
        /// The visible page area (what viewers show).
        Crop = "crop", "Crop";
        Trim = "trim", "Trim";
        Bleed = "bleed", "Bleed";
        /// The whole sheet.
        Media = "media", "Media";
    } default Crop
}

/// Result of an import with non-fatal warnings (unsupported features, skipped content).
#[derive(Clone, Debug)]
pub struct ImportReport {
    pub document: Document,
    pub warnings: Vec<String>,
}

/// Result of an export with non-fatal warnings (features approximated or dropped).
#[derive(Clone, Debug)]
pub struct ExportReport {
    pub bytes: Vec<u8>,
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum PdfError {
    #[error("the document has no artboards")]
    NoArtboards,
    #[error("artboard {0} does not exist")]
    BadArtboard(usize),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("PDF writer error: {0}")]
    Write(String),
    #[error("cannot read PDF: {0}")]
    Parse(String),
    #[error("the PDF has no pages")]
    NoPages,
    #[error("invalid PDF setting: {0}")]
    BadSetting(String),
    #[error("the PDF is password-protected: give its password")]
    NeedsPassword,
    #[error("the PDF password is wrong")]
    WrongPassword,
    #[error("page {0} does not exist (the PDF has {1})")]
    BadPage(usize, usize),
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_blend;
#[cfg(test)]
mod tests_charstroke;
#[cfg(test)]
mod tests_cmykblend;
#[cfg(test)]
mod tests_dashalign;
#[cfg(test)]
mod tests_focal;
#[cfg(test)]
mod tests_fx;
#[cfg(test)]
mod tests_import_options;
#[cfg(test)]
mod tests_settings;
#[cfg(test)]
mod tests_stroke;
#[cfg(test)]
mod tests_strokeout;
