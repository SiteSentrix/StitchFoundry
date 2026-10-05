//! File → Print as a print-ready PDF (`file.print`), what it would print (`print.preview`) and the
//! print settings saved with the document (`print.setup`, [`vectorcraft_doc::Document::print_setup`]).

use serde::Deserialize;
use serde_json::{Value, json};
use vectorcraft_doc::Document;
use vectorcraft_pdf::{PrintOptions, PrintSettings};

use super::fileio::pdf::{merge, pdf_error};
use super::fileio::write_or_return;
use super::*;

const SETUP: &str = "print.setup";

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "print.setup",
            "Print Setup",
            [],
            None,
            "{settings?: {copies: 1 (1–999), collate: true, reverse, artboards: all|range|ignore (all the art as one page), range: \"1-3, 5\" (1-based, with artboards: range), skipBlank (leave out artboards with no art that prints), media: letter|legal|tabloid|a3|a4|a5|b4|b5|custom, width, height (custom paper, 72–14400 pt), orientation: portrait|landscape|portraitFlipped|landscapeFlipped, autoRotate: true (turn the paper to each artboard; orientation ignored), transverse (the page a quarter turn on the paper), printLayers: visiblePrintable|visible|all (template layers never print), placement: {origin: topLeft|top|topRight|left|center|right|bottomLeft|bottom|bottomRight (the point of the printed area, the artboard with its bleed and marks, on the same point of the imageable area), x, y (pt, right and down)}, scaling: none|fit|custom|tileFull|tileImageable, scale: {width: 100, height: 100} (% with custom and tiling), overlap: 0 (pt between tiles), tileRange: \"\" (1-based tiles across then down; empty: all), margin: 0 (pt the device can't print around the paper: the imageable area is inside it), marks: {trim, registration, colorBars, pageInfo, kind: roman|japanese, weight: 0.25, offset: 6} (as document.exportPdf, at the paper's scale; page information adds the tile and the ink), bleed: {useDocument: true (the document's bleed, document.setup), top, bottom, left, right} (pt), output: {mode: composite|separations (one grey page per ink that prints), emulsion: up|down (down mirrors), image: positive|negative, spotsToProcess, inks: [{name: \"Cyan\"|…|a spot swatch, print: true, frequency (lpi, default 60), angle (default C 15, M 75, Y 0, K 45, spots 45)}]}, graphics: {autoFlatness: true, flatness: 1 (0.2–100), fonts: none|subset|complete}, color: {intent: perceptual|relativeColorimetric|saturation|absoluteColorimetric, preserveNumbers: true (CMYK colours keep their values in separations)}}} store the print settings with the document, over the ones it has (null keeps a value), as one undo step; no settings → {settings} (the current ones, defaults if never set up)",
            has_doc,
            setup
        ),
        cmd!(
            query "print.preview",
            "Print Preview",
            [],
            None,
            "{settings?: {…print.setup settings} (over the document's)} → {pages (copies included), sheets: [{artboard (0-based; null with artboards ignored), tile? (1-based), ink? (separations), width, height (pt, the page), orientation, scale: [width %, height %]}] (one copy, in order), tiles: [{artboard, columns, rows, printed: [1-based tiles], tiles: [[x0, y0, x1, y1]…] (document space)}], inks: [{name, spot, print, frequency, angle}] (separations), warnings (art larger than the imageable area, options not applied yet…), settings} without printing",
            has_doc,
            preview
        ),
        cmd!(
            "file.print",
            "Print",
            [],
            None,
            "{settings?: {…print.setup settings} (over the document's), path?} print the document as a print-ready PDF, one page per sheet of paper (per tile, per ink, per copy) → {pages, path, bytes, warnings}; no path → {dataBase64, bytes, pages, warnings}. Raster effects print at the document's raster effects resolution; halftone screens and flatness are left to the output device",
            has_doc,
            print
        ),
    ]
}

/// The print settings saved with `doc` (defaults if never set up) as JSON.
fn saved(doc: &Document) -> Value {
    doc.print_setup.clone().unwrap_or_else(|| serde_json::to_value(PrintSettings::default()).unwrap_or_default())
}

/// The settings of `doc` with `p.settings` over them, checked.
fn settings(cmd: &str, doc: &Document, p: &Value) -> Result<PrintSettings> {
    let mut v = saved(doc);
    match p.get("settings") {
        None | Some(Value::Null) => {}
        Some(over @ Value::Object(_)) => merge(&mut v, over),
        Some(_) => return Err(bad(cmd, "settings must be an object")),
    }
    let set = PrintSettings::deserialize(&v).map_err(|e| bad(cmd, format!("print settings: {e}")))?;
    set.check().map_err(|e| pdf_error(cmd, e))?;
    Ok(set)
}

fn to_json(set: &PrintSettings) -> Result<Value> {
    serde_json::to_value(set).map_err(|e| EngineError::Other(e.to_string()))
}

fn setup(s: &mut Session, p: &Value) -> Result<Value> {
    let doc = &s.doc()?.doc;
    let set = to_json(&settings(SETUP, doc, p)?)?;
    if p.get("settings").is_some_and(|v| !v.is_null()) && doc.print_setup.as_ref() != Some(&set) {
        let stored = set.clone();
        s.edit("Print Setup", |d, _| {
            d.print_setup = Some(stored);
            Ok(())
        })?;
    }
    Ok(json!({ "settings": set }))
}

fn preview(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "print.preview";
    let doc = &s.doc()?.doc;
    let set = settings(C, doc, p)?;
    let pv = vectorcraft_pdf::preview(doc, &set).map_err(|e| pdf_error(C, e))?;
    let mut v = serde_json::to_value(pv).map_err(|e| EngineError::Other(e.to_string()))?;
    v["settings"] = to_json(&set)?;
    Ok(v)
}

fn print(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "file.print";
    let doc = &s.doc()?.doc;
    let set = settings(C, doc, p)?;
    // Raster effects print as images at the document's raster effects resolution.
    let flat = super::rasterfx::flatten_raster_effects(doc);
    let r =
        vectorcraft_pdf::print(flat.as_ref().unwrap_or(doc), &PrintOptions { settings: set, ..Default::default() }).map_err(|e| pdf_error(C, e))?;
    write_or_return(str_param(p, "path"), &r.bytes, json!({ "pages": r.pages, "warnings": r.warnings }))
}
