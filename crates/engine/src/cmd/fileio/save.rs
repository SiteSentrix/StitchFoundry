//! Saving: `document.save`, `file.saveAs`, `file.saveCopy`, `file.saveAsTemplate`,
//! `file.newFromTemplate`, `file.revert` and `file.formatOptions`.
//!
//! Every frontend saves the same way: [`save_plan`] resolves where and how (path, format, options,
//! a suggested name and folder when there is no path), then [`save_with`] encodes, writes through
//! the caller's writer (the file system here; a save panel or a browser download in the apps) and
//! makes the file the document's own (path, format, options, saved state) for Save and Save As.

use std::borrow::Cow;

use serde_json::{Map, Value, json};
use vectorcraft_doc::Document;

use super::super::*;
use super::{
    Encoded, Format, Loaded, encode_all, file_stem, format, format_for_name, load, read_file, with_compression_pref, write_encoded, write_file,
};
use crate::{DocState, Prefs};

/// The formats Save As offers, in menu order (append-only). The other writable formats are exports:
/// they never become the document's own file.
pub const SAVE_FORMATS: &[&str] = &["vectorcraft", "template", "pdf", "svg", "svgz", "ai"];

/// Save-panel filters `(label, [extension])` for `first` (a format id, or a file name whose
/// extension names one): when Save writes that format, one per [`SAVE_FORMATS`] entry with it
/// leading (the panel's default type), else none (an export picks its own). Only the extension a
/// save writes (never the former native name).
pub fn save_filters(first: &str) -> Vec<(&'static str, &'static [&'static str])> {
    let Some(first) = format_for_name(first).or_else(|| format(first)).filter(|f| SAVE_FORMATS.contains(&f.id)) else { return vec![] };
    let mut v: Vec<&'static Format> = SAVE_FORMATS.iter().filter_map(|id| format(id)).collect();
    v.sort_by_key(|f| f.id != first.id);
    v.into_iter().map(|f| (f.label, &f.extensions[..1])).collect()
}

/// How a save treats the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveMode {
    /// File → Save: the document's own path and format.
    Save,
    /// File → Save As: a new path or format, which the document takes on.
    SaveAs,
    /// File → Save a Copy: the document keeps its path, title and modified state.
    Copy,
    /// File → Save as Template: a template copy; the document keeps its path and state too.
    Template,
}

impl SaveMode {
    /// The engine command of this mode.
    pub fn command(self) -> &'static str {
        match self {
            Self::Save => "document.save",
            Self::SaveAs => "file.saveAs",
            Self::Copy => "file.saveCopy",
            Self::Template => "file.saveAsTemplate",
        }
    }

    /// The mode an engine command id runs.
    pub fn of(command: &str) -> Option<Self> {
        [Self::Save, Self::SaveAs, Self::Copy, Self::Template].into_iter().find(|m| m.command() == command)
    }
}

/// A save resolved against the active document, before anything is encoded or written.
#[derive(Clone, Debug)]
pub struct SavePlan {
    pub mode: SaveMode,
    /// Where to write. `None` when no path is known (never saved, converted from an older version,
    /// or a format other than the document's own): the caller asks for one or takes the bytes.
    pub path: Option<String>,
    pub format: &'static Format,
    /// The format's options (only those its encoder reads).
    pub options: Map<String, Value>,
    /// Suggested file name: `<name>.<ext>`, `<name> copy.<ext>` or `<name> template.vctemplate`.
    pub name: String,
    /// Suggested folder: the document's own, or the Templates folder for a template.
    pub folder: Option<String>,
    /// The `modified` param (as `date_param` reads it): the File Info date a Save or Save As to a file
    /// stamps; `None`: the time it is written.
    pub modified: Option<Option<i64>>,
}

impl SavePlan {
    /// Does writing this make the file the document's own (Save and Save As, except to a
    /// template)?
    pub fn retargets(&self) -> bool {
        matches!(self.mode, SaveMode::Save | SaveMode::SaveAs) && self.format.id != "template"
    }
}

/// File Info's dates for a save to a file: the modified date (and the created date, when it has
/// none) become `at`. Not an undo step.
pub fn stamp_save_dates(st: &mut DocState, at: i64) {
    let d = std::sync::Arc::make_mut(&mut st.doc);
    d.metadata.created.get_or_insert(at);
    d.metadata.modified = Some(at);
}

/// The Templates folder: the `templatesFolder` preference, else `Documents/VectorCraft Templates`
/// in the user's home (none where there is no home folder, as on the web).
pub fn templates_folder(prefs: &Prefs) -> Option<String> {
    if !prefs.templates_folder.is_empty() {
        return Some(prefs.templates_folder.clone());
    }
    let home = std::env::var("HOME").or_else(|_| std::env::var("USERPROFILE")).ok().filter(|h| !h.is_empty())?;
    Some(std::path::Path::new(&home).join("Documents").join("VectorCraft Templates").to_string_lossy().to_string())
}

/// The folder of a path (`None` for a bare file name).
fn parent_folder(path: &str) -> Option<String> {
    std::path::Path::new(path).parent().map(|p| p.to_string_lossy().to_string()).filter(|p| !p.is_empty())
}

/// A save format by id or extension, or why it isn't one.
fn save_format_of(f: &str) -> std::result::Result<&'static Format, String> {
    match format(f) {
        Some(f) if SAVE_FORMATS.contains(&f.id) => Ok(f),
        Some(f) => Err(format!("Save writes {}, not {}: use document.export for other formats", SAVE_FORMATS.join(", "), f.label)),
        None => Err(format!("unknown format `{f}` (Save writes {})", SAVE_FORMATS.join(", "))),
    }
}

/// The format Save writes for `format` (an id or extension), else the format `path`'s extension
/// names, else native (see [`SAVE_FORMATS`]); other formats are exports.
pub fn save_format(format: Option<&str>, path: Option<&str>) -> std::result::Result<&'static Format, String> {
    save_format_of(format.or_else(|| path.and_then(format_for_name).map(|f| f.id)).unwrap_or("vectorcraft"))
}

/// [`save_format_of`] for command `cmd`.
fn checked(cmd: &str, f: &str) -> Result<&'static Format> {
    save_format_of(f).map_err(|e| bad(cmd, e))
}

/// Resolve a save of the active document from `{path?, format?, options?}` (see the commands'
/// params docs).
pub fn save_plan(s: &Session, mode: SaveMode, p: &Value) -> Result<SavePlan> {
    let cmd = mode.command();
    let st = s.doc()?;
    let path = str_param(p, "path").map(str::to_string);
    let format = match (mode, str_param(p, "format"), path.as_deref().and_then(format_for_name)) {
        (SaveMode::Template, ..) => checked(cmd, "template")?,
        (_, Some(f), _) => checked(cmd, f)?,
        (_, None, Some(f)) => checked(cmd, f.id)?,
        (_, None, None) => checked(cmd, st.format)?,
    };
    let mut given: Map<String, Value> = match p.get("options") {
        Some(Value::Object(o)) => o.iter().filter(|(k, _)| reads_option(format, k)).map(|(k, v)| (k.clone(), v.clone())).collect(),
        None | Some(Value::Null) => Map::new(),
        Some(_) => return Err(bad(cmd, "options must be an object (see file.formatOptions)")),
    };
    if is_svg(format) {
        // SVG options also come flat or as `svg: {…}` beside the path; kept flat.
        let mut svg = super::svg_options(&Value::Object(given)).map_err(|e| bad(cmd, e))?;
        svg.extend(super::svg_options(p).map_err(|e| bad(cmd, e))?);
        given = svg;
    } else {
        // So do PDF options (as the Save PDF dialog and document.exportPdf name them) and the
        // native ones (compress, version, preview).
        if let Some(o) = p.as_object() {
            given.extend(
                o.iter().filter(|(k, v)| !v.is_null() && k.as_str() != "path" && reads_option(format, k)).map(|(k, v)| (k.clone(), v.clone())),
            );
        }
    }
    // None given: the ones the document was last saved with in this format.
    let options = if given.is_empty() && format.id == st.format { st.save_options.clone() } else { given };
    // Save writes the document's own file; the other modes only where they are told to.
    let path = path.or_else(|| (mode == SaveMode::Save && !st.converted && format.id == st.format).then(|| st.path.clone()).flatten());
    let stem = file_stem(st.path.as_deref().unwrap_or(&st.doc.title));
    let ext = format.extensions[0];
    let name = match mode {
        SaveMode::Copy => format!("{stem} copy.{ext}"),
        SaveMode::Template => format!("{stem} template.{ext}"),
        SaveMode::Save | SaveMode::SaveAs => format!("{stem}.{ext}"),
    };
    let folder = match mode {
        SaveMode::Template => templates_folder(&s.prefs),
        _ => st.path.as_deref().and_then(parent_folder),
    };
    let modified = date_param(p, "modified", cmd)?;
    Ok(SavePlan { mode, path, format, options, name, folder, modified })
}

/// Does a save in `f` read option `key`? PDF (and a .ai file, a PDF) also reads its General
/// settings (`createLayers`…) and presets.
fn reads_option(f: &Format, key: &str) -> bool {
    match is_pdf(f) {
        true => super::pdf::OPTIONS.iter().any(|o| o.name == key) || super::pdf::is_setting(key),
        false => f.options.iter().any(|o| o.name == key),
    }
}

/// Formats that carry the whole native document: they lose nothing, and record links relative to
/// where they are written. A `.ai` file is a PDF carrying it.
fn is_native(f: &Format) -> bool {
    matches!(f.id, "vectorcraft" | "template" | "ai")
}

fn is_pdf(f: &Format) -> bool {
    matches!(f.id, "pdf" | "ai")
}

fn is_svg(f: &Format) -> bool {
    matches!(f.id, "svg" | "svgz")
}

/// The document as written: native files carry the view to reopen at.
fn doc_to_save<'a>(st: &'a DocState, f: &Format) -> Cow<'a, Document> {
    if is_native(f) && st.doc.last_view != st.view {
        let mut d = (*st.doc).clone();
        d.last_view = st.view.clone();
        Cow::Owned(d)
    } else {
        Cow::Borrowed(&st.doc)
    }
}

/// What a format loses against a native file (reported whenever a save writes it).
fn fidelity_warning(f: &Format) -> Option<String> {
    (!is_native(f)).then(|| {
        format!(
            "{} keeps the artwork but not everything a VectorCraft document holds (editable effects, symbols, swatches, styles): save as VectorCraft to keep it all editable",
            f.label
        )
    })
}

/// Encode `plan` and write it with `write` (the file, then the images an SVG links to, beside it).
/// Without a path → `{dataBase64, bytes, format, name, folder?, warnings, linked?: [{name,
/// dataBase64}]}` and the document is unchanged; with one → `{path, format, bytes, warnings,
/// linked?: [path…]}`, and Save or Save As (except to a template) make the file the document's
/// own: path, title, format, options, saved state.
pub fn save_with(s: &mut Session, plan: SavePlan, mut write: impl FnMut(&str, &[u8]) -> Result<()>) -> Result<Value> {
    let cmd = plan.mode.command();
    let retargets = plan.retargets();
    // The web build, without a clock, leaves the dates unless given one.
    if plan.path.is_some()
        && retargets
        && let Some(at) = clock_date(s, "modified", plan.modified)
    {
        stamp_save_dates(s.doc_mut()?, at);
    }
    let own = doc_to_save(s.doc()?, plan.format);
    // A native file records its links' paths relative to where it is written.
    let relative = plan.path.as_deref().filter(|_| is_native(plan.format)).and_then(|p| crate::cmd::links::with_relative_paths(&own, p));
    let doc: &Document = relative.as_ref().unwrap_or(&own);
    let mut params = plan.options.clone();
    if is_svg(plan.format) {
        // A save keeps hidden layers (not displayed) unless told otherwise; exports leave them out.
        params.entry("hiddenLayers").or_insert(Value::Bool(true));
    }
    let mut params = Value::Object(params);
    if matches!(plan.format.id, "vectorcraft" | "template") {
        // Not remembered with the options: the preference decides each time it isn't given.
        params = with_compression_pref(&s.prefs, &params);
    }
    let params = super::pdf::expand_preset(s, cmd, &params)?;
    let mut enc = if plan.format.id == "ai" {
        // A PDF of every artboard with the PDF options, always carrying the native document.
        let mut q = params.into_owned();
        if let Some(o) = q.as_object_mut() {
            o.extend([("preserveEditing".into(), json!(true)), ("range".into(), json!("all"))]);
        }
        let (bytes, warnings) = super::pdf::encode(cmd, &doc.without_edit_modes(), &q)?;
        Encoded { warnings, ..Encoded::one(bytes) }
    } else {
        encode_all(doc, plan.format.id, &params)?
    };
    if enc.files.len() != 1 {
        return Err(bad(cmd, "Save writes one artboard: name one, or export several with document.export"));
    }
    // What the format loses first, then the encoder's own notes (PDF options not applied yet…).
    let mut warnings = std::mem::take(&mut enc.warnings);
    warnings.splice(0..0, fidelity_warning(plan.format));
    let extra = json!({ "format": plan.format.id, "warnings": warnings });
    let Some(path) = plan.path else {
        let mut out = write_encoded(None, &plan.name, doc, &enc, extra)?;
        out["name"] = json!(plan.name);
        if let Some(folder) = plan.folder {
            out["folder"] = json!(folder);
        }
        return Ok(out);
    };
    let files = enc.named(doc, &path);
    for (p, bytes) in &files {
        write(p, bytes)?;
    }
    let bytes = files.first().map_or(0, |f| f.1.len());
    let mut out = super::merge(json!({ "path": path, "bytes": bytes }), extra);
    if let Some(linked) = files.get(1..).filter(|l| !l.is_empty()) {
        out["linked"] = linked.iter().map(|(p, _)| json!(p)).collect();
    }
    if retargets {
        let st = s.doc_mut()?;
        st.path = Some(path.clone());
        st.format = plan.format.id;
        st.save_options = plan.options;
        st.converted = false;
        st.mark_saved();
    }
    Ok(out)
}

/// A save command: plan, then write with the file system.
fn save(s: &mut Session, mode: SaveMode, p: &Value) -> Result<Value> {
    let plan = save_plan(s, mode, p)?;
    save_with(s, plan, write_file)
}

fn can_revert(s: &Session) -> std::result::Result<(), String> {
    let st = s.active().ok_or("no document open")?;
    match (&st.path, st.is_dirty()) {
        (None, _) => Err("the document has never been saved".into()),
        (_, false) => Err("no changes since the last save".into()),
        _ => Ok(()),
    }
}

/// File → Revert: read and decode the saved file first (on any failure the document is left as it
/// is), then replace the document in its tab.
fn revert(s: &mut Session, _: &Value) -> Result<Value> {
    let index = s.active_index().ok_or(crate::EngineError::NoDocument)?;
    let path = s.doc()?.path.clone().ok_or_else(|| bad("file.revert", "the document has never been saved"))?;
    let Loaded { mut doc, .. } = load(&path, &read_file(&path)?)?;
    doc.template = false;
    s.replace_document(index, doc);
    Ok(json!({ "path": path }))
}

/// `file.formatOptions`: a writable format's options with the values Save would use.
fn format_options(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "file.formatOptions";
    let st = s.active();
    let id = str_param(p, "format").or(st.map(|d| d.format)).unwrap_or("vectorcraft");
    let f = format(id).filter(|f| f.write).ok_or_else(|| bad(C, format!("`{id}` is no writable format (see document.formats)")))?;
    let saved = st.filter(|d| d.format == f.id).map(|d| &d.save_options);
    // A native save compresses as Use Compression says, unless told otherwise.
    let prefs = with_compression_pref(&s.prefs, &json!({}));
    let mut v = f.to_json();
    if let Some(options) = v["options"].as_object_mut() {
        for (name, o) in options.iter_mut() {
            let default = prefs.get(name).filter(|_| matches!(f.id, "vectorcraft" | "template")).unwrap_or(&o["default"]);
            o["value"] = saved.and_then(|m| m.get(name)).unwrap_or(default).clone();
        }
    }
    v["saveFormats"] =
        SAVE_FORMATS.iter().filter_map(|id| format(id)).map(|f| json!({"id": f.id, "label": f.label, "extensions": f.extensions})).collect();
    Ok(v)
}

pub(super) fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "document.save",
            "Save Document",
            [],
            None,
            "{path?, format?: vectorcraft|template|pdf|svg|svgz|ai (default: the path's extension, else the document's own format), options?: {…the format's options, see file.formatOptions; default: as last saved}, svg?: {…SVG options} (SVG options may also be given flat; an SVG save keeps hidden layers, display:none, unless hiddenLayers is false), native (also flat): compress?: bool (gzip; default: the useCompression preference), version?: 3 (2 or 1: for older VectorCraft versions, never compressed), preview?: false (embed a PNG of the first artboard, at most 256 px), modified?: Unix seconds|null (the File Info modified date, and created date when there is none, a save to a file stamps; default now, recorded in the journal so a replay matches; null: leave the dates)} → {path, format, bytes, warnings, linked?: [path…] (images an SVG links to)}. Save writes one artboard, except a .ai file: a PDF-compatible file of every artboard carrying the native document (preserveEditing always on; PDF options flat or in options), which document.open restores exactly. Without a path it writes the document's own file in its own format: a document opened from or saved as SVG/PDF saves as that again (warnings name what the format loses). No path known (never saved, converted from an older version, or another format) → {dataBase64, format, name, folder?, warnings} and the document stays modified",
            has_doc,
            |s, p| save(s, SaveMode::Save, p)
        ),
        cmd!(
            "file.saveAs",
            "Save As…",
            ["File"],
            Some("Cmd+Shift+S"),
            "{path?, format?: vectorcraft|template|pdf|svg|svgz|ai (default: the path's extension, else the document's own), options?, svg?, modified? (as document.save)} the document takes on the new path, name and format (except a template, which is always a copy) → {path, format, bytes, warnings}; no path → {dataBase64, format, name, folder?, warnings}",
            has_doc,
            |s, p| save(s, SaveMode::SaveAs, p)
        ),
        cmd!(
            "file.saveCopy",
            "Save a Copy…",
            ["File"],
            Some("Cmd+Alt+S"),
            "{path?, format?, options?} (as file.saveAs) write a copy; the document keeps its path, title and modified state → {path, format, bytes, warnings}; no path → {dataBase64, format, name: \"<name> copy.<ext>\", folder?, warnings}",
            has_doc,
            |s, p| save(s, SaveMode::Copy, p)
        ),
        cmd!(
            "file.saveAsTemplate",
            "Save as Template…",
            ["File"],
            None,
            "{path?, compress?, version?, preview? (as document.save)} a native template (.vctemplate) that opens as a new untitled document; the document is unchanged → {path, format, bytes, warnings}; no path → {dataBase64, format, name: \"<name> template.vctemplate\", folder: the Templates folder (preference templatesFolder), warnings}",
            has_doc,
            |s, p| save(s, SaveMode::Template, p)
        ),
        cmd!(
            "file.newFromTemplate",
            "New from Template…",
            ["File"],
            Some("Cmd+Shift+N"),
            "{path} or {name, dataBase64}: open a template (or any readable file) as a new untitled document → {index, title, format, warnings}",
            always,
            super::load::new_from_template
        ),
        cmd!(
            "file.revert",
            "Revert",
            ["File"],
            Some("F12"),
            "{} discard the changes: reload the saved file into the same tab (history cleared; the tab keeps its place and view). Saved, modified documents only; if the file can't be read the document is left as it is → {path}. The app asks first (a confirm dialog) unless confirmed: true",
            can_revert,
            revert
        ),
        cmd!(
            query "file.formatOptions",
            "Format Options",
            [],
            None,
            "{format?: a writable format id (default: the document's own)} → {id, label, extensions, mime, read, write, raster, options: {name: {type, default, description, value}}, saveFormats: [{id, label, extensions}]}; value = the document's option as last saved in that format, else the default",
            always,
            format_options
        ),
    ]
}
