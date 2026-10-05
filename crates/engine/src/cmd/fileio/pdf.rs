//! PDF export settings for every PDF path (`document.export {format: pdf}`, Export for Screens,
//! `document.exportPdf` and the Save PDF dialog): presets (built-in and saved), the options parsed
//! over them, the warnings that come back, and `document.pdfSettings` (the dialog's Summary).

use std::borrow::Cow;

use serde::Deserialize;
use serde_json::{Value, json};
use vectorcraft_doc::Document;
use vectorcraft_pdf::{Overprint, PdfError, PdfOptions, PdfPreset, PdfSettings, Standard};

use super::super::flatten::{FlattenOptions, flatten_document, see_through_images};
use super::super::*;
use super::{ArtboardPick, FormatOption, write_or_return};
use crate::EngineError;

const C: &str = "document.exportPdf";

/// The built-in preset every PDF export starts from.
pub use vectorcraft_pdf::DEFAULT_PRESET;

/// The PDF options `document.formats` lists; `document.exportPdf` documents every field.
pub(super) const OPTIONS: &[FormatOption] = &[
    super::ARTBOARD,
    super::ARTBOARDS,
    super::RANGE,
    super::USE_ARTBOARDS,
    FormatOption {
        name: "preset",
        ty: "string",
        default: "\"VectorCraft Default\"",
        description: "the PDF preset the other options apply over (built-in or saved: pdf.preset.list)",
    },
    FormatOption {
        name: "standard",
        ty: "string",
        default: "\"none\"",
        description: "none | pdfA2b | pdfX1a | pdfX3 | pdfX4 (choosing one sets the compatibility it allows; see document.exportPdf)",
    },
    FormatOption { name: "compatibility", ty: "string", default: "\"1.7\"", description: "PDF version: 1.4 | 1.5 | 1.6 | 1.7 | 2.0" },
    FormatOption {
        name: "preserveEditing",
        ty: "boolean",
        default: "true",
        description: "embed the native document so VectorCraft reopens the PDF editable (the default preset's choice)",
    },
    FormatOption { name: "compression", ty: "object", default: "null", description: "{color, gray, mono, compressText} (see document.exportPdf)" },
    FormatOption { name: "marks", ty: "object", default: "null", description: "printer's marks (see document.exportPdf)" },
    FormatOption {
        name: "bleed",
        ty: "object",
        default: "null",
        description: "{useDocument, top, bottom, left, right} in points: the page grows by the bleed (BleedBox) around its artboard (TrimBox)",
    },
    FormatOption {
        name: "output",
        ty: "object",
        default: "null",
        description: "colour conversion to a destination profile, ICC profiles, output intent and Trapped (see document.exportPdf)",
    },
    FormatOption {
        name: "advanced",
        ty: "object",
        default: "null",
        description: "{fontSubsetPercent, outlineText (false: real text in embedded subset fonts), overprint}",
    },
    FormatOption {
        name: "security",
        ty: "object",
        default: "null",
        description: "passwords and permissions (the file is encrypted: RC4 128-bit at 1.4–1.5, AES-128 at 1.6, AES-256 at 1.7/2.0)",
    },
    FormatOption {
        name: "includeNonPrinting",
        ty: "boolean",
        default: "false",
        description: "keep the layers whose Print option is off (left out otherwise, unless createLayers)",
    },
    FormatOption {
        name: "createLayers",
        ty: "boolean",
        default: "false",
        description: "top-level layers as PDF layers (optional content groups with their visibility, print state and lock; PDF 1.5 or later)",
    },
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "document.exportPdf",
            "Export PDF",
            [],
            None,
            "{path?, preset?: \"VectorCraft Default\" (built-in or saved: pdf.preset.list), artboard? | artboards?: [i…] | range?: \"1-3, 5\" (1-based; default all, one page each), standard?: none|pdfA2b|pdfX1a (CMYK, grey and spot colours only: colours and images converted to output.destination or the CMYK output intent, untagged; transparency flattened with the High Resolution flattener preset; PDF 1.3)|pdfX3 (colours ICC-tagged; transparency flattened; PDF 1.3)|pdfX4 (transparency and layers kept; colours ICC-tagged; PDF 1.6 at most) (PDF/X: a /GTS_PDFX output intent, by default the CMYK profile in effect, embedded; TrimBox, Trapped, GTS_PDFXVersion, title and dates; fonts embedded or outlined), compatibility?: 1.4|1.5|1.6|1.7 (default)|2.0 (choosing a standard sets the latest it allows: 1.7 for pdfA2b, 1.6 for pdfX4, 1.4 for pdfX1a and pdfX3, written as PDF 1.3), preserveEditing? (on in VectorCraft Default: the native document as an embedded file, which document.open restores; choosing a standard turns it off, a standard refuses it), thumbnails?, fastWebView?, viewAfterSaving? (the app opens the written file), createLayers? (each top-level layer but template layers as a PDF layer, an optional content group: hidden layers off, non-printing ones /PrintState /OFF, locked ones locked; needs 1.5 or later; they reopen as layers; choosing pdfX1a or pdfX3 turns it off; they refuse it), includeNonPrinting? (keep layers whose Print option is off; left out by default unless createLayers), compression?: {color?, gray?: {downsample: none|average|subsample|bicubic, ppi: 300, abovePpi: 450, compression: none|zip|jpeg|jpeg2000|auto, quality: minimum|low|medium|high|maximum}, mono?: {downsample, ppi: 1200, abovePpi: 1800, compression: none|ccittG3|ccittG4|zip|runLength} (black-and-white images), compressText?: true} (images above abovePpi are resampled to ppi; auto keeps JPEGs JPEG, the others lossless; JPEG needs opaque images; none, jpeg2000, CCITT and runLength are written as ZIP with a warning), marks?: {trim, registration, colorBars, pageInfo, kind: roman|japanese, weight: 0.25 (trim marks and targets), offset: 6 (pt from the artboard, at least the bleed)} (printer's marks in [Registration], every plate: trim marks, registration targets mid-side, CMYK/spot/black-tint colour bars on top, page information (title, artboard, date UTC) below), bleed?: {useDocument (the document's bleed, document.setup), top, bottom, left, right} (pt; art in the bleed is kept). Each page: TrimBox = artboard, BleedBox = artboard + bleed, MediaBox = that + the marks' room (art clipped to the BleedBox), output?: {conversion: none (default)|destination (every colour into the destination profile's model, CMYK or RGB, also colours of that model in another profile)|preserveNumbers (only colours of the other model and Lab; those in the destination's model keep their numbers) (grey stays grey; images are converted too, printer's marks are not), destination: profile name (edit.colorSettings lists them; default: the document's profile for its colour mode), profiles: none (device colours)|all|destination|taggedSource (documents assigned a profile: edit.assignProfile) (ICC-based colours: CMYK with the destination or document CMYK profile, RGB as sRGB, grey with the sRGB tone curve; PDF/A always), outputIntent: profile name (embedded as the catalog's /GTS_PDFX output intent; a name that isn't a profile is only named, which pdfX1a and pdfX3 allow only with a registry and pdfX4 refuses; pdfX1a needs a CMYK one), outputCondition, outputConditionId (default: outputIntent), registry, trapped (/Trapped /True; /False when an output intent is written)} (PDF/A files keep their own output intent), advanced?: {fontSubsetPercent: 100 (below 100 is accepted with a warning: fonts are always embedded as subsets), outlineText: true (false: type as real, selectable and searchable text in embedded subset fonts with a ToUnicode map; fonts whose licence forbids embedding, or allows only whole fonts, stay outlines with a warning), overprint: preserve|discard}, security?: {openPassword, permissionsPassword, printing: none|low|high, changes: none|pages|forms|comments|any, copy, screenReader, plaintextMetadata}} → {path, bytes, warnings}; no path → {dataBase64, bytes, warnings}. The options apply over the preset (null keeps its value); options accepted but not applied yet come back as warnings; a standard with a PDF version it doesn't allow and a password with a standard are refused, and so is a PDF/X file that would still break its standard (transparency the flattener left, RGB in pdfX1a, a font not embedded). document.export {format: pdf} takes the same options",
            has_doc,
            export_pdf
        ),
        cmd!(
            query "document.pdfSettings",
            "PDF Settings",
            [],
            None,
            "{preset?, includeDocument?: false, …document.exportPdf options} → {settings (the preset with the options applied), presets: [name…], changed: [{option: \"compression.compressText\", value}] (what differs from the preset), warnings}; includeDocument also exports the active document in memory and adds its warnings (knockout groups approximated, effects left out…)",
            always,
            pdf_settings
        ),
    ]
}

/// Every preset name: the built-in ones, then the saved ones ([`crate::Prefs::pdf_presets`]).
pub fn presets(s: &Session) -> Vec<String> {
    names(&s.prefs.pdf_presets)
}

/// The built-in preset names, then those of `saved`.
fn names(saved: &[PdfPreset]) -> Vec<String> {
    vectorcraft_pdf::builtin_presets().into_iter().map(|p| p.name).chain(saved.iter().map(|p| p.name.clone())).collect()
}

/// The preset `name` names: a built-in one (any case; `default` is the app default) or one of
/// `saved`.
pub fn find_preset(name: &str, saved: &[PdfPreset]) -> Option<PdfPreset> {
    vectorcraft_pdf::builtin_preset(name).or_else(|| saved.iter().find(|p| p.name.eq_ignore_ascii_case(name.trim())).cloned())
}

fn preset(cmd: &str, name: &str, saved: &[PdfPreset]) -> Result<PdfSettings> {
    find_preset(name, saved)
        .map(|p| p.settings)
        .ok_or_else(|| bad(cmd, format!("unknown PDF preset `{name}` (presets: {})", names(saved).join(", "))))
}

/// The settings of the preset `p` names (default: [`DEFAULT_PRESET`]), before `p`'s options.
pub fn preset_settings(cmd: &str, p: &Value, saved: &[PdfPreset]) -> Result<PdfSettings> {
    preset(cmd, str_param(p, "preset").unwrap_or(DEFAULT_PRESET), saved)
}

/// Merge `over` into `base`: objects key by key (recursively), `null` keeps the base (the
/// preset's value), anything else replaces.
pub(crate) fn merge(base: &mut Value, over: &Value) {
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
pub(crate) fn pdf_error(cmd: &str, e: PdfError) -> EngineError {
    match e {
        PdfError::BadSetting(_) | PdfError::Unsupported(_) => bad(cmd, e.to_string()),
        e => EngineError::Other(e.to_string()),
    }
}

/// The settings `p` asks for: its preset (built-in or one of `saved`; default
/// [`DEFAULT_PRESET`]) with `p`'s options applied over it. Choosing another standard turns
/// Preserve Editing off, the layers off where it has none, and sets the latest compatibility it
/// allows, unless `p` asks for those. Checked as a preset is
/// ([`PdfSettings::check_values`]): profiles that aren't there pass. Keys that aren't PDF options
/// (path, format…) are ignored.
pub fn resolve(cmd: &str, p: &Value, saved: &[PdfPreset]) -> Result<PdfSettings> {
    let base = preset_settings(cmd, p, saved)?;
    if !p.is_object() {
        return Ok(base);
    }
    let mut v = serde_json::to_value(&base).map_err(|e| EngineError::Other(e.to_string()))?;
    merge(&mut v, p);
    let mut s = PdfSettings::deserialize(&v).map_err(|e| bad(cmd, format!("PDF options: {e}")))?;
    let unasked = |key: &str| p.get(key).is_none_or(Value::is_null);
    if s.standard != base.standard {
        if s.standard != Standard::None && unasked("preserveEditing") {
            s.preserve_editing = false;
        }
        if !s.standard.allows_layers() && unasked("createLayers") {
            s.create_layers = false;
        }
        if !s.standard.allows(s.compatibility) && unasked("compatibility") {
            s.compatibility = s.standard.version();
        }
    }
    s.check_values().map_err(|e| pdf_error(cmd, e))?;
    Ok(s)
}

/// [`resolve`], refusing what the writer can't honour ([`PdfSettings::check`]).
pub fn settings_with(cmd: &str, p: &Value, saved: &[PdfPreset]) -> Result<PdfSettings> {
    let s = resolve(cmd, p, saved)?;
    s.check().map_err(|e| pdf_error(cmd, e))?;
    Ok(s)
}

/// [`settings_with`] the built-in presets only (what the encoders know: see [`expand_preset`]).
pub fn settings(cmd: &str, p: &Value) -> Result<PdfSettings> {
    settings_with(cmd, p, &[])
}

/// `p` with a saved preset it names written out as options, for the encoders, which know only the
/// built-in presets. Params naming no preset, or a built-in one, come back as they are.
pub fn expand_preset<'a>(s: &Session, cmd: &str, p: &'a Value) -> Result<Cow<'a, Value>> {
    match str_param(p, "preset") {
        Some(name) if vectorcraft_pdf::builtin_preset(name).is_none() => {
            let set = settings_with(cmd, p, &s.prefs.pdf_presets)?;
            let mut q = p.clone();
            merge(&mut q, &serde_json::to_value(set).map_err(|e| EngineError::Other(e.to_string()))?);
            if let Some(o) = q.as_object_mut() {
                o.remove("preset");
            }
            Ok(Cow::Owned(q))
        }
        _ => Ok(Cow::Borrowed(p)),
    }
}

/// The full export options for `doc`: settings plus the artboards `p` picks (default all).
pub fn options(cmd: &str, doc: &Document, p: &Value) -> Result<PdfOptions> {
    let pick = if p.is_object() { ArtboardPick::deserialize(p).map_err(|e| bad(cmd, format!("PDF options: {e}")))? } else { ArtboardPick::default() };
    let artboards = pick.resolve(doc.artboards.len()).map_err(|e| bad(cmd, e))?;
    Ok(PdfOptions { settings: settings(cmd, p)?, artboards, ..Default::default() })
}

/// Why a PDF of some artboards carries no editing data.
pub const EDITING_NEEDS_EVERY_ARTBOARD: &str =
    "Preserve editing was left out: it needs a PDF of every artboard, in order (reopened, this one would show the others too)";

/// Encode `doc` as PDF with the options in `p` → (bytes, warnings). Raster effects are rendered
/// to images at the document's raster effects resolution. Preserve Editing embeds `doc` itself
/// when the PDF has every artboard (a PDF of some reopens as just those).
pub fn encode(cmd: &str, doc: &Document, p: &Value) -> Result<(Vec<u8>, Vec<String>)> {
    encode_carrying(cmd, doc, p, || Ok(vectorcraft_format::save(doc, false)))
}

/// [`encode`] the pages of `doc`, carrying `native()` as the editing data (a `.ai` file: the
/// native document with its save options, whose pages may be left blank).
pub(super) fn encode_carrying(cmd: &str, doc: &Document, p: &Value, native: impl FnOnce() -> Result<Vec<u8>>) -> Result<(Vec<u8>, Vec<String>)> {
    let mut opts = options(cmd, doc, p)?;
    let mut warnings = vec![];
    if opts.settings.preserve_editing {
        if opts.artboards.as_ref().is_none_or(|v| v.iter().copied().eq(0..doc.artboards.len())) {
            opts.native = Some(native()?);
        } else {
            opts.settings.preserve_editing = false;
            warnings.push(EDITING_NEEDS_EVERY_ARTBOARD.to_string());
        }
    }
    let flat = flatten_for(doc, &opts.settings)?;
    if flat.is_some() {
        warnings.push(format!(
            "transparency was flattened with the High Resolution flattener preset: {} files have none",
            opts.settings.standard.label()
        ));
    }
    let r = super::super::rasterfx::export_pdf_with_report(flat.as_ref().unwrap_or(doc), &opts).map_err(|e| pdf_error(cmd, e))?;
    warnings.extend(r.warnings);
    if opts.settings.advanced.overprint == Overprint::Preserve && doc.layers.iter().any(|l| l.has_overprint()) {
        warnings.push("overprinting objects are written without overprint: overprint is not written to PDF yet".into());
    }
    Ok((r.bytes, warnings))
}

/// `doc` with its transparency flattened for a standard whose files have none (PDF/X-1a, PDF/X-3),
/// with the High Resolution flattener preset; images with see-through pixels count as
/// transparency. `None` when nothing changes (or the standard keeps transparency).
fn flatten_for(doc: &Document, set: &PdfSettings) -> Result<Option<Document>> {
    if !set.standard.flattens() {
        return Ok(None);
    }
    let mut o = FlattenOptions::preset("high").unwrap_or_default();
    o.no_transparency = Some(see_through_images(doc));
    flatten_document(doc, &o)
}

fn export_pdf(s: &mut Session, p: &Value) -> Result<Value> {
    let expanded = expand_preset(s, C, p)?;
    let p = &*expanded;
    let (bytes, warnings) = encode(C, &s.doc()?.doc, p)?;
    write_or_return(str_param(p, "path"), &bytes, json!({ "warnings": warnings }))
}

/// `(path, value)` for every leaf of `v` that differs from `default` (`compression.color.ppi`).
pub fn changed(prefix: &str, v: &Value, default: &Value, out: &mut Vec<Value>) {
    match (v, default) {
        (Value::Object(o), Value::Object(d)) => {
            for (k, x) in o {
                let path = if prefix.is_empty() { k.clone() } else { format!("{prefix}.{k}") };
                changed(&path, x, d.get(k).unwrap_or(&Value::Null), out);
            }
        }
        _ if v != default => out.push(json!({ "option": prefix, "value": v })),
        _ => {}
    }
}

/// `[{option, value}]` for every setting of `set` that differs from `base`
/// (`compression.color.ppi`).
pub fn changes(set: &PdfSettings, base: &PdfSettings) -> Vec<Value> {
    let (Ok(v), Ok(base)) = (serde_json::to_value(set), serde_json::to_value(base)) else { return vec![] };
    let mut diff = vec![];
    changed("", &v, &base, &mut diff);
    diff
}

fn pdf_settings(s: &mut Session, p: &Value) -> Result<Value> {
    const Q: &str = "document.pdfSettings";
    let saved = &s.prefs.pdf_presets;
    let set = settings_with(Q, p, saved)?;
    let diff = changes(&set, &preset_settings(Q, p, saved)?);
    let warnings = match s.active() {
        Some(st) if bool_or(p, "includeDocument", false) => encode(Q, &st.doc, &*expand_preset(s, Q, p)?)?.1,
        _ => set.warnings(),
    };
    let v = serde_json::to_value(&set).map_err(|e| EngineError::Other(e.to_string()))?;
    Ok(json!({ "settings": v, "presets": presets(s), "changed": diff, "warnings": warnings }))
}

/// Is `key` a top-level PDF setting (a [`PdfSettings`] field as `document.exportPdf` names it)?
pub fn is_setting(key: &str) -> bool {
    static DEFAULTS: std::sync::LazyLock<Value> = std::sync::LazyLock::new(|| serde_json::to_value(PdfSettings::default()).unwrap_or_default());
    DEFAULTS.get(key).is_some()
}
