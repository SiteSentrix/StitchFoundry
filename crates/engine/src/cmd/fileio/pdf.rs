//! PDF export settings for every PDF path (`document.export {format: pdf}`, Export for Screens,
//! `document.exportPdf` and the Save PDF dialog): presets, the options parsed over them and the
//! warnings that come back.

use serde::Deserialize;
use serde_json::{Value, json};
use vectorcraft_doc::Document;
use vectorcraft_pdf::{Overprint, PdfError, PdfOptions, PdfSettings};

use super::super::*;
use super::{ArtboardPick, FormatOption, write_or_return};
use crate::EngineError;

const C: &str = "document.exportPdf";

/// The built-in preset every PDF export starts from.
pub const DEFAULT_PRESET: &str = "VectorCraft Default";

/// The PDF options `document.formats` lists; `document.exportPdf` documents every field.
pub(super) const OPTIONS: &[FormatOption] = &[
    super::ARTBOARD,
    super::ARTBOARDS,
    super::RANGE,
    FormatOption { name: "preset", ty: "string", default: "\"VectorCraft Default\"", description: "the PDF preset the other options apply over" },
    FormatOption { name: "standard", ty: "string", default: "\"none\"", description: "none | pdfA2b (PDF/X is not supported yet)" },
    FormatOption { name: "compatibility", ty: "string", default: "\"1.7\"", description: "PDF version: 1.4 | 1.5 | 1.6 | 1.7 | 2.0" },
    FormatOption { name: "compression", ty: "object", default: "null", description: "{color, gray, mono, compressText} (see document.exportPdf)" },
    FormatOption { name: "marks", ty: "object", default: "null", description: "printer's marks (see document.exportPdf)" },
    FormatOption { name: "bleed", ty: "object", default: "null", description: "{useDocument, top, bottom, left, right} in points" },
    FormatOption { name: "output", ty: "object", default: "null", description: "colour conversion and output intent (see document.exportPdf)" },
    FormatOption { name: "advanced", ty: "object", default: "null", description: "{fontSubsetPercent, outlineText, overprint}" },
    FormatOption { name: "security", ty: "object", default: "null", description: "passwords (not supported yet) and permissions" },
];

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "document.exportPdf",
        "Export PDF",
        [],
        None,
        "{path?, preset?: \"VectorCraft Default\", artboard? | artboards?: [i…] | range?: \"1-3, 5\" (1-based; default all, one page each), standard?: none|pdfA2b|pdfX1a|pdfX3|pdfX4, compatibility?: 1.4|1.5|1.6|1.7 (default)|2.0, preserveEditing?, thumbnails?, fastWebView?, viewAfterSaving?, createLayers?, compression?: {color?, gray?: {downsample: none|average|subsample|bicubic, ppi: 300, abovePpi: 450, compression: none|zip|jpeg|jpeg2000|auto, quality: minimum|low|medium|high|maximum}, mono?: {downsample, ppi: 1200, abovePpi: 1800, compression: none|ccittG3|ccittG4|zip|runLength}, compressText?: true}, marks?: {trim, registration, colorBars, pageInfo, kind: roman|japanese, weight: 0.25, offset: 6}, bleed?: {useDocument, top, bottom, left, right} (pt), output?: {conversion: none|destination|preserveNumbers, destination, profiles: none|all|destination|taggedSource, outputIntent, outputCondition, outputConditionId, registry, trapped}, advanced?: {fontSubsetPercent: 100, outlineText: true, overprint: preserve|discard}, security?: {openPassword, permissionsPassword, printing: none|low|high, changes: none|pages|forms|comments|any, copy, screenReader, plaintextMetadata}} → {path, bytes, warnings}; no path → {dataBase64, bytes, warnings}. The options apply over the preset (null keeps its value); options accepted but not applied yet come back as warnings; PDF/X, passwords and PDF/A-2b at 2.0 are refused. document.export {format: pdf} takes the same options",
        has_doc,
        export_pdf
    )]
}

/// Every preset name (built-in first).
pub fn presets() -> Vec<String> {
    vec![DEFAULT_PRESET.to_string()]
}

fn preset(cmd: &str, name: &str) -> Result<PdfSettings> {
    if name.eq_ignore_ascii_case(DEFAULT_PRESET) || name.eq_ignore_ascii_case("default") {
        return Ok(PdfSettings::default());
    }
    Err(bad(cmd, format!("unknown PDF preset `{name}` (presets: {})", presets().join(", "))))
}

/// Merge `over` into `base`: objects key by key (recursively), `null` keeps the base (the
/// preset's value), anything else replaces.
fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o.iter().filter(|(_, v)| !v.is_null()) {
                merge(b.entry(k.as_str()).or_insert(Value::Null), v);
            }
        }
        (b, o) => *b = o.clone(),
    }
}

/// A writer error as an engine error: bad settings are bad params of `cmd`.
fn pdf_error(cmd: &str, e: PdfError) -> EngineError {
    match e {
        PdfError::BadSetting(_) | PdfError::Unsupported(_) => bad(cmd, e.to_string()),
        e => EngineError::Other(e.to_string()),
    }
}

/// The settings `p` asks for: its preset (default: [`DEFAULT_PRESET`]) with `p`'s options applied
/// over it, checked. Keys that aren't PDF options (path, format…) are ignored.
pub fn settings(cmd: &str, p: &Value) -> Result<PdfSettings> {
    let base = match str_param(p, "preset") {
        Some(name) => preset(cmd, name)?,
        None => PdfSettings::default(),
    };
    if !p.is_object() {
        return Ok(base);
    }
    let mut v = serde_json::to_value(&base).map_err(|e| EngineError::Other(e.to_string()))?;
    merge(&mut v, p);
    let s = PdfSettings::deserialize(&v).map_err(|e| bad(cmd, format!("PDF options: {e}")))?;
    s.check().map_err(|e| pdf_error(cmd, e))?;
    Ok(s)
}

/// The full export options for `doc`: settings plus the artboards `p` picks (default all).
pub fn options(cmd: &str, doc: &Document, p: &Value) -> Result<PdfOptions> {
    let pick = if p.is_object() { ArtboardPick::deserialize(p).map_err(|e| bad(cmd, format!("PDF options: {e}")))? } else { ArtboardPick::default() };
    let artboards = pick.resolve(doc.artboards.len()).map_err(|e| bad(cmd, e))?;
    Ok(PdfOptions { settings: settings(cmd, p)?, artboards, ..Default::default() })
}

/// Encode `doc` as PDF with the options in `p` → (bytes, warnings). Raster effects are rendered
/// to images at the document's raster effects resolution.
pub fn encode(cmd: &str, doc: &Document, p: &Value) -> Result<(Vec<u8>, Vec<String>)> {
    let opts = options(cmd, doc, p)?;
    let r = super::super::rasterfx::export_pdf_with_report(doc, &opts).map_err(|e| pdf_error(cmd, e))?;
    let mut warnings = r.warnings;
    if opts.settings.advanced.overprint == Overprint::Preserve && doc.layers.iter().any(|l| l.has_overprint()) {
        warnings.push("overprinting objects are written without overprint: overprint is not written to PDF yet".into());
    }
    Ok((r.bytes, warnings))
}

fn export_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    let (bytes, warnings) = encode(C, &s.doc()?.doc, p)?;
    write_or_return(str_param(p, "path"), &bytes, json!({ "warnings": warnings }))
}
