//! File → Print as a print-ready PDF: each page is a sheet of the chosen paper with the art laid
//! out on it as the [`PrintSettings`] say.
//!
//! - General: copies (collated or not), reverse order, the artboards (all, a range or ignored:
//!   all the art as one page), blank artboards skipped, the paper and its orientation (or turned
//!   to each artboard's), transverse, which layers print (template layers never do), the
//!   placement on the imageable area, and the scale: none, fit, custom, or tiles of the paper or
//!   of its imageable area, overlapping, a range of them.
//! - Marks and bleed as PDF export draws them ([`crate::MarkSettings`], [`crate::BleedSettings`]),
//!   around the artboard on the paper, at the paper's scale.
//! - Output: composite, or separations, one page per ink that prints ([`print_inks`]) in the
//!   grey of its coverage (overprints honoured, spot colours as process if asked), emulsion down
//!   (mirrored), negative (inverted).
//! - Colour management: the rendering intent colours are separated with, and whether CMYK
//!   colours keep their numbers.
//!
//! Halftone screens and flatness are left to the output device (warnings say so). [`preview`]
//! lists the pages, tiles and inks without writing the file.

mod layout;
mod plates;
mod settings;

use std::borrow::Cow;
use std::sync::Arc;

use serde::Serialize;
use vectorcraft_doc::marks::PrinterMarks;
use vectorcraft_doc::{ColorMode, Document, Node, NodeKind};

pub use layout::{MAX_TILES, TileGrid};
pub use plates::print_inks;
pub use settings::*;

use crate::export::{Exporter, Sheet, Writer};
use crate::marks::PageBoxes;
use crate::{CompressionSettings, PdfError, PdfSettings};
use layout::Layout;
use plates::Separator;

/// Most pages of a job, copies and inks included.
pub const MAX_PAGES: usize = 2000;

/// A print job: the settings plus what one run decides.
#[derive(Clone, Debug, Default)]
pub struct PrintOptions {
    pub settings: PrintSettings,
    /// The title in the metadata and the page information; `None` = the document's title.
    pub title: Option<String>,
    /// When the job ran, Unix seconds (UTC); `None` = now (native) / none (wasm).
    pub created: Option<i64>,
    /// Content streams uncompressed (readable operators, for tests and debugging).
    pub uncompressed: bool,
}

/// A printed job.
#[derive(Clone, Debug)]
pub struct PrintReport {
    pub bytes: Vec<u8>,
    pub pages: usize,
    pub warnings: Vec<String>,
}

/// One page of a job, as [`preview`] lists it (one copy).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageSummary {
    /// 0-based artboard (`None`: artboards ignored).
    pub artboard: Option<usize>,
    /// 1-based tile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tile: Option<usize>,
    /// The ink of a separation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ink: Option<String>,
    /// The page size in points.
    pub width: f64,
    pub height: f64,
    pub orientation: Orientation,
    /// Horizontal and vertical scale in percent.
    pub scale: [f64; 2],
}

/// What a job prints, without printing it.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrintPreview {
    /// Pages in all, copies included.
    pub pages: usize,
    /// One copy's pages, in order.
    pub sheets: Vec<PageSummary>,
    /// The tiles of each artboard, when tiling.
    pub tiles: Vec<TileGrid>,
    /// The inks of a separation (empty for composite output).
    pub inks: Vec<PrintInk>,
    pub warnings: Vec<String>,
}

/// A laid-out job.
struct Job<'a> {
    /// The document as it prints: live effects applied, the layers that print.
    doc: Cow<'a, Document>,
    layouts: Vec<Layout>,
    tiles: Vec<TileGrid>,
    /// The inks of a separation (every one, printed or not).
    inks: Vec<PrintInk>,
    /// The inks that print (indices into `inks`): each a plate.
    plates: Vec<usize>,
    /// One copy's pages: (layout, plate).
    sheets: Vec<(usize, Option<usize>)>,
    warnings: Vec<String>,
}

/// Keep the layers of `nodes` that print as `which` says, shown (hidden ones print with All),
/// down through sublayers.
fn keep_layers(nodes: &mut Vec<Arc<Node>>, which: PrintLayers) {
    nodes.retain_mut(|n| {
        let NodeKind::Layer { printable, .. } = n.kind else { return true };
        let keep = match which {
            PrintLayers::VisiblePrintable => n.visible && printable,
            PrintLayers::Visible => n.visible,
            PrintLayers::All => true,
        };
        if keep {
            let layer = Arc::make_mut(n);
            layer.visible = true;
            if let Some(children) = layer.children_mut() {
                keep_layers(children, which);
            }
        }
        keep
    });
}

impl<'a> Job<'a> {
    fn new(doc: &'a Document, set: &PrintSettings) -> Result<Self, PdfError> {
        set.check()?;
        // Live effects print as their result (raster effects are rendered by the app first).
        let mut doc = vectorcraft_effects::bake_document(doc).map_or(Cow::Borrowed(doc), Cow::Owned);
        keep_layers(&mut doc.to_mut().layers, set.print_layers);
        let mut warnings = vec![];
        let (layouts, tiles) = layout::layout(&doc, set, &mut warnings)?;
        let separations = set.output.mode == OutputMode::Separations;
        let (inks, more) = if separations { print_inks(&doc, set) } else { Default::default() };
        warnings.extend(more);
        let plates: Vec<usize> = (0..inks.len()).filter(|i| inks[*i].print).collect();
        let printed: Vec<Option<usize>> = if separations { (0..plates.len()).map(Some).collect() } else { vec![None] };
        if printed.is_empty() {
            return Err(PdfError::BadSetting("output.inks: no ink is set to print".into()));
        }
        let sheets: Vec<(usize, Option<usize>)> = (0..layouts.len()).flat_map(|l| printed.iter().map(move |i| (l, *i))).collect();
        if sheets.len().saturating_mul(set.copies as usize) > MAX_PAGES {
            return Err(PdfError::BadSetting(format!("the job would print more than {MAX_PAGES} pages")));
        }
        if separations && doc.color_mode == ColorMode::Rgb {
            warnings.push("the document is RGB: separations convert its colours to CMYK with the colour settings".into());
        }
        if separations && set.output.inks.iter().any(|i| i.frequency.is_some() || i.angle.is_some()) {
            warnings.push("ink frequencies and angles are not written as halftone screens yet: the output device's screens apply".into());
        }
        if !separations && doc.layers.iter().any(|l| l.has_overprint()) {
            warnings.push("overprinting fills and strokes print as knockouts in composite output (separations honour them)".into());
        }
        if !set.graphics.auto_flatness {
            warnings.push("a fixed flatness is not written yet: the output device flattens curves".into());
        }
        Ok(Self { doc, layouts, tiles, inks, plates, sheets, warnings })
    }

    /// The ink of plate `plate` (`None`: composite).
    fn ink(&self, plate: Option<usize>) -> Option<&PrintInk> {
        self.inks.get(*self.plates.get(plate?)?)
    }

    /// Every page, copies included, in print order: (layout, ink).
    fn pages(&self, set: &PrintSettings) -> Vec<(usize, Option<usize>)> {
        let copies = set.copies.max(1) as usize;
        let mut v: Vec<_> = if set.collate {
            (0..copies).flat_map(|_| self.sheets.iter().copied()).collect()
        } else {
            self.sheets.iter().flat_map(|s| std::iter::repeat_n(*s, copies)).collect()
        };
        if set.reverse {
            v.reverse();
        }
        v
    }
}

/// What printing `doc` with `set` makes: pages, tiles, inks and warnings.
pub fn preview(doc: &Document, set: &PrintSettings) -> Result<PrintPreview, PdfError> {
    let job = Job::new(doc, set)?;
    let sheets = job
        .sheets
        .iter()
        .filter_map(|(l, plate)| {
            let l = job.layouts.get(*l)?;
            Some(PageSummary {
                artboard: l.artboard,
                tile: l.tile.map(|t| t.0 + 1),
                ink: job.ink(*plate).map(|i| i.name.clone()),
                width: l.size.0,
                height: l.size.1,
                orientation: l.orientation,
                scale: [l.scale.0 * 100.0, l.scale.1 * 100.0],
            })
        })
        .collect();
    Ok(PrintPreview { pages: job.sheets.len() * set.copies.max(1) as usize, sheets, tiles: job.tiles, inks: job.inks, warnings: job.warnings })
}

/// Print `doc` to a PDF as `opts` say.
pub fn print(doc: &Document, opts: &PrintOptions) -> Result<PrintReport, PdfError> {
    let set = &opts.settings;
    let job = Job::new(doc, set)?;
    let doc = &*job.doc;
    let pdf = PdfSettings { compression: CompressionSettings { compress_text: !opts.uncompressed, ..Default::default() }, ..Default::default() };
    let title = opts.title.clone().unwrap_or_else(|| doc.title.clone());
    let created = opts.created.or_else(vectorcraft_doc::metadata::now_unix);
    let separations = set.output.mode == OutputMode::Separations;
    let mut w = Writer::new(doc, &pdf, &title, created, !separations && doc.color_mode == ColorMode::Cmyk)?;
    // One exporter per ink (one for composite), each drawing its own document.
    let sep = Separator::new(doc, set);
    let plates: Vec<Document> = (0..job.plates.len()).filter_map(|p| job.ink(Some(p))).map(|i| sep.plate(&i.name)).collect();
    let mut exporters: Vec<Exporter> =
        if separations { plates.iter().map(|d| Exporter::new(d, &pdf)).collect() } else { vec![Exporter::new(doc, &pdf)] };
    for ex in &mut exporters {
        ex.non_printing = true;
        ex.intent = set.color.intent;
    }
    let marks = set.marks.printer_marks();
    let pages = job.pages(set);
    for (l, plate) in &pages {
        let (Some(l), Some(ex)) = (job.layouts.get(*l), exporters.get_mut(plate.unwrap_or(0))) else { continue };
        let ink = job.ink(*plate);
        let art = marks_art(doc, &marks, l, ink, &title, created).map(|mut art| {
            if let Some(ink) = ink {
                sep.node(&ink.name, &mut art);
            }
            art
        });
        let sheet = Sheet {
            size: l.size,
            trim: l.page_trim,
            bleed: l.page_bleed,
            view: l.view,
            window: l.window,
            place: l.place,
            area: l.area,
            clip: true,
            marks: art.as_ref(),
            negative: set.output.image == PrintImage::Negative,
        };
        w.page(ex, &sheet)?;
    }
    for ex in exporters {
        w.absorb(ex);
    }
    let (bytes, more) = w.finish()?;
    let mut warnings = job.warnings;
    for m in more {
        if !warnings.contains(&m) {
            warnings.push(m);
        }
    }
    Ok(PrintReport { bytes, pages: pages.len(), warnings })
}

/// The printer's marks of page `l` (`None` without marks), with its page information: the
/// title, artboard and date, the tile, and the ink with its screen.
fn marks_art(doc: &Document, marks: &PrinterMarks, l: &Layout, ink: Option<&PrintInk>, title: &str, created: Option<i64>) -> Option<Node> {
    if !marks.any() {
        return None;
    }
    let mut info = String::new();
    if marks.page_info {
        info = crate::marks::page_info_of(doc, title, l.artboard, created);
        if let Some((t, n)) = l.tile {
            info.push_str(&format!("  ·  tile {} of {n}", t + 1));
        }
        if let Some(ink) = ink {
            info.push_str(&format!("  ·  {} {} lpi {}°", ink.name, ink.frequency, ink.angle));
        }
    }
    let boxes = PageBoxes::new(l.trim, l.bleed, marks);
    crate::marks::art(doc, marks, &boxes, l.bleed, &info)
}
