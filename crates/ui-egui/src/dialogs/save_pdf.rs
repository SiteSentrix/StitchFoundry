//! File → Save as PDF: the Save PDF dialog. Preset, Standard and Compatibility on top, then the
//! sections General, Compression, Marks and Bleeds, Output, Advanced, Security and Summary.
//!
//! The fields are the `document.exportPdf` options (sections are objects such as `compression`),
//! plus `preset`, `range`, `path?` and UI-only `__` keys, so agents fill the dialog with
//! `ui.dialog.set`. OK runs `document.exportPdf` through [`io::export_pdf`]; the Summary is
//! `document.pdfSettings`. Save Preset… names the settings as a preset (`pdf.preset.save`).
//!
//! The same dialog edits a preset (kind `pdfPreset`, from Edit → PDF Presets): a `name` and
//! `description` instead of the preset and artboards, and OK saves the preset.

use std::sync::LazyLock;

use serde_json::{Map, Value, json};
use vectorcraft_doc::Unit;
use vectorcraft_engine::cmd::fileio::{SaveMode, pdf};
use vectorcraft_pdf::{
    Changes, Choice, ColorConversion, Compatibility, Downsample, Encryption, ImageCodec, JpegQuality, MarkKind, MonoCodec, Overprint, PdfSettings,
    Printing, ProfileInclusion, Standard,
};

use super::{DialogSpec, form};
use crate::state::Dialog;
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, io, widgets};

pub(super) const KIND: &str = "savePdf";

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |_| "Save PDF".into(), body, confirm, ok: Some("Save PDF"), ..DialogSpec::FORM };

/// The dialog kind of the preset editor (New / Edit in Edit → PDF Presets).
pub(super) const PRESET_KIND: &str = "pdfPreset";

pub(super) const PRESET_SPEC: DialogSpec = DialogSpec {
    heading: |d| if d.str(EDITING).is_empty() { "New PDF Preset" } else { "Edit PDF Preset" }.into(),
    body,
    confirm: confirm_preset,
    ok: Some("Save Preset"),
    ..DialogSpec::FORM
};

/// The preset editor's field holding the name of the saved preset it edits (empty: a new one).
const EDITING: &str = "__editing";
/// Save PDF's field holding the name typed for Save Preset… (present while that row shows).
const SAVE_AS: &str = "__savePresetAs";

pub(super) const SECTIONS: [&str; 7] = ["General", "Compression", "Marks and Bleeds", "Output", "Advanced", "Security", "Summary"];

/// The default settings as JSON: what a field an agent left out reads as.
static DEFAULTS: LazyLock<Value> = LazyLock::new(|| serde_json::to_value(PdfSettings::default()).unwrap_or_default());

/// Trim mark weights offered (pt).
const WEIGHTS: [f64; 3] = [0.125, 0.25, 0.5];
const WEIGHT_LABELS: [&str; 3] = ["0.125 pt", "0.25 pt", "0.5 pt"];

/// The Destination entry for the document's own profile (stored as "").
const DOCUMENT_PROFILE: &str = "Document profile";
/// The Output Intent Profile entry for no output intent (stored as "").
const NO_PROFILE: &str = "None";

const LABEL_WIDTH: f32 = 150.0;
/// Width of the section list (its frame adds 6 px a side).
const LIST_WIDTH: f32 = 150.0;
/// The top rows' labels end where the section content starts, so their dropdowns line up with it.
const TOP_LABEL_WIDTH: f32 = LIST_WIDTH + 26.0;

/// Open the dialog with `params` (`document.exportPdf` options and `path?`) applied over their
/// preset.
pub fn open(app: &mut VectorcraftApp, params: &Value) -> Result<Value, String> {
    let mut fields = settings_fields(app, params)?;
    let s = |k: &str| params.get(k).and_then(Value::as_str);
    fields.insert("preset".into(), json!(s("preset").unwrap_or(pdf::DEFAULT_PRESET)));
    fields.insert("__presets".into(), json!(pdf::presets(&app.session)));
    fields.insert("__section".into(), json!(SECTIONS[0]));
    fields.insert("__allArtboards".into(), json!(s("range").is_none()));
    fields.insert("range".into(), json!(s("range").unwrap_or("")));
    if let Some(path) = s("path") {
        fields.insert("path".into(), json!(path));
    }
    app.ui.dialog = Some(Dialog { kind: KIND.into(), fields });
    Ok(Value::Null)
}

/// The settings `params` ask for (their preset, built-in or saved, with their options applied) as
/// dialog fields. A standard the writer can't produce yet still shows (Save PDF then says so).
fn settings_fields(app: &VectorcraftApp, params: &Value) -> Result<Map<String, Value>, String> {
    let settings = pdf::resolve("ui.savePdfDialog", params, &app.session.prefs.pdf_presets).map_err(|e| e.to_string())?;
    match serde_json::to_value(settings).map_err(|e| e.to_string())? {
        Value::Object(fields) => Ok(fields),
        _ => Err("PDF settings are not an object".into()),
    }
}

/// The `document.exportPdf` params the dialog stands for.
fn params(d: &Dialog) -> Value {
    let mut p = form::params(d);
    if (d.bool("__allArtboards") || d.str("range").trim().is_empty())
        && let Some(o) = p.as_object_mut()
    {
        o.remove("range");
    }
    p
}

/// Save PDF: write the file (a PDF copy, or the PDF a Save As / Save a Copy asked for, `__save`);
/// the dialog stays open when that fails (bad range, cancelled save…). While Save Preset… asks for
/// a name, OK (and Enter) saves the preset instead.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    if d.fields.contains_key(SAVE_AS) {
        let mut d = d.clone();
        let r = save_preset(app, &mut d);
        app.ui.dialog = Some(d);
        return r;
    }
    let r = match SaveMode::of(&d.str("__save")) {
        Some(mode) => io::save_pdf(app, mode, params(d)),
        None => io::export_pdf(app, params(d)),
    };
    if r.is_ok() {
        app.ui.dialog = None;
    }
    r
}

/// The settings as `pdf.preset.save` params (without the artboards and path).
fn preset_params(d: &Dialog) -> Value {
    let mut p = form::params(d);
    if let Some(o) = p.as_object_mut() {
        for k in ["range", "path"] {
            o.remove(k);
        }
    }
    p
}

/// Save Preset…: save the settings under the typed name and pick that preset.
fn save_preset(app: &mut VectorcraftApp, d: &mut Dialog) -> Result<Value, String> {
    let mut p = preset_params(d);
    p["name"] = json!(d.str(SAVE_AS));
    let r = app.run("pdf.preset.save", p)?;
    d.fields.remove(SAVE_AS);
    d.fields.insert("preset".into(), r["name"].clone());
    d.fields.insert("__presets".into(), json!(pdf::presets(&app.session)));
    Ok(r)
}

/// Open the preset editor on the saved preset `name`, or on a new preset starting from `preset`
/// (default: the app default).
pub fn open_preset(app: &mut VectorcraftApp, params: &Value) -> Result<Value, String> {
    let s = |k: &str| params.get(k).and_then(Value::as_str);
    let (base, name, description, editing) = match s("name") {
        Some(name) => {
            let p = app.session.prefs.pdf_presets.iter().find(|p| p.name.eq_ignore_ascii_case(name.trim()));
            let p = p.ok_or_else(|| format!("no saved PDF preset named `{name}` (built-in presets are read-only: start a new one from them)"))?;
            (p.name.clone(), p.name.clone(), p.description.clone(), p.name.clone())
        }
        None => (s("preset").unwrap_or(pdf::DEFAULT_PRESET).to_string(), app.session.new_pdf_preset_name(), String::new(), String::new()),
    };
    let mut fields = settings_fields(app, &json!({ "preset": base }))?;
    fields.insert("name".into(), json!(name));
    fields.insert("description".into(), json!(description));
    fields.insert(EDITING.into(), json!(editing));
    fields.insert("__section".into(), json!(SECTIONS[0]));
    app.ui.dialog = Some(Dialog { kind: PRESET_KIND.into(), fields });
    Ok(Value::Null)
}

/// Save Preset (the preset editor's OK): `pdf.preset.save`, then back to Edit → PDF Presets on it.
/// A new preset may not take the name of another one.
fn confirm_preset(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let mut p = preset_params(d);
    let (name, editing) = (d.str("name"), d.str(EDITING));
    if editing.is_empty() {
        if app.session.pdf_presets().iter().any(|q| q.name.eq_ignore_ascii_case(name.trim())) {
            return Err(format!("a preset named `{}` exists", name.trim()));
        }
    } else {
        p["name"] = json!(editing);
        p["newName"] = json!(name);
    }
    let r = app.run("pdf.preset.save", p)?;
    super::pdf_presets::open(app, r["name"].as_str());
    Ok(r)
}

// ---------- fields by path (`compression.color.ppi`) ----------

fn lookup<'a>(m: &'a Map<String, Value>, path: &str) -> Option<&'a Value> {
    let mut keys = path.split('.');
    let first = m.get(keys.next()?)?;
    keys.try_fold(first, |v, k| v.get(k))
}

/// The value at `path`: the dialog's, else the default.
fn get<'a>(d: &'a Dialog, path: &str) -> &'a Value {
    lookup(&d.fields, path).or_else(|| DEFAULTS.as_object().and_then(|m| lookup(m, path))).unwrap_or(&Value::Null)
}

fn set(d: &mut Dialog, path: &str, value: Value) {
    let (parents, key) = path.rsplit_once('.').unwrap_or(("", path));
    let mut map = &mut d.fields;
    for k in parents.split('.').filter(|k| !k.is_empty()) {
        let e = map.entry(k).or_insert(Value::Null);
        if !e.is_object() {
            *e = Value::Object(Map::new());
        }
        let Value::Object(m) = e else { return };
        map = m;
    }
    map.insert(key.into(), value);
}

// ---------- widgets bound to a path ----------

fn heading(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    ui.label(egui::RichText::new(text).font(theme::semibold(12.5)).color(t.text_strong));
    ui.add_space(2.0);
}

fn note(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).size(11.5).color(t.text_dim));
}

/// A labelled row.
fn row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    row_with(ui, label, LABEL_WIDTH, add);
}

fn row_with(ui: &mut egui::Ui, label: &str, width: f32, add: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(width, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(width);
            ui.label(egui::RichText::new(label).color(t.text));
        });
        add(ui);
    });
}

fn flag(ui: &mut egui::Ui, d: &mut Dialog, path: &str, label: &str, enabled: bool) {
    let on = get(d, path).as_bool().unwrap_or(false);
    if widgets::check(ui, label, on, enabled) {
        set(d, path, json!(!on));
    }
}

/// The index in `T`'s choices of the value at `path`.
fn choice_index<T: Choice>(d: &Dialog, path: &str) -> Option<usize> {
    get(d, path).as_str().and_then(|s| T::IDS.iter().position(|i| *i == s))
}

/// The choice at `path`.
fn choice<T: Choice>(d: &Dialog, path: &str) -> Option<T> {
    choice_index::<T>(d, path).and_then(|i| T::ALL.get(i).copied())
}

/// A dropdown of the choices `T` (options for which `enabled` is false are greyed).
fn pick<T: Choice>(ui: &mut egui::Ui, d: &mut Dialog, path: &str, width: f32, enabled: impl Fn(T) -> bool) -> bool {
    let current = choice_index::<T>(d, path);
    let label = current.and_then(|i| T::LABELS.get(i)).copied().unwrap_or_default();
    let chosen = widgets::dropdown_with(ui, path, label, T::LABELS, width, |i| T::ALL.get(i).is_some_and(|c| enabled(*c)));
    let Some(id) = chosen.and_then(|i| T::IDS.get(i)) else { return false };
    set(d, path, json!(id));
    true
}

fn number(ui: &mut egui::Ui, d: &mut Dialog, path: &str, suffix: &str, enabled: bool) {
    let v = get(d, path).as_f64().unwrap_or(0.0);
    ui.add_enabled_ui(enabled, |ui| {
        if let Some(x) = widgets::plain_field(ui, path, v, suffix, 3, 64.0) {
            set(d, path, json!(x));
        }
    });
}

/// A length in points, shown in the document's units.
fn length(ui: &mut egui::Ui, d: &mut Dialog, path: &str, unit: Unit, enabled: bool) {
    let v = get(d, path).as_f64();
    ui.add_enabled_ui(enabled, |ui| {
        if let Some(x) = widgets::num_field(ui, path, v, unit, 80.0) {
            set(d, path, json!(x));
        }
    });
}

fn text(ui: &mut egui::Ui, d: &mut Dialog, path: &str, enabled: bool) {
    let mut s = get(d, path).as_str().unwrap_or_default().to_string();
    if ui.add_enabled(enabled, egui::TextEdit::singleline(&mut s).desired_width(240.0)).changed() {
        set(d, path, json!(s));
    }
}

// ---------- the dialog ----------

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let editor = d.kind == PRESET_KIND;
    if editor {
        row_with(ui, "Preset name:", TOP_LABEL_WIDTH, |ui| {
            form::text_edit(ui, d, "name", 288.0);
        });
    } else {
        preset_rows(app, ui, d);
    }
    row_with(ui, "Standard:", TOP_LABEL_WIDTH, |ui| {
        let picked = pick::<Standard>(ui, d, "standard", 170.0, Standard::supported);
        let standard = choice::<Standard>(d, "standard").unwrap_or_default();
        if picked && choice::<Compatibility>(d, "compatibility").is_some_and(|c| !standard.allows(c)) {
            // The standard's own version (PDF/A-2b is a PDF 1.7 standard).
            set(d, "compatibility", json!(Compatibility::Pdf17.id()));
        }
        if picked && standard != Standard::None {
            // Files of a standard don't carry the editing data, nor passwords.
            set(d, "preserveEditing", json!(false));
            for p in [OPEN_PASSWORD, PERMISSIONS_PASSWORD] {
                set(d, p, json!(""));
            }
        }
        ui.add_space(16.0);
        ui.label(egui::RichText::new("Compatibility:").color(t.text));
        pick::<Compatibility>(ui, d, "compatibility", 110.0, |c| standard.allows(c));
    });
    ui.add_space(10.0);
    let section = SECTIONS.iter().copied().find(|s| *s == d.str("__section")).unwrap_or(SECTIONS[0]);
    ui.horizontal_top(|ui| {
        egui::Frame::NONE.fill(t.panel_darker).corner_radius(egui::CornerRadius::same(4)).inner_margin(egui::Margin::same(6)).show(ui, |ui| {
            ui.set_width(LIST_WIDTH);
            ui.set_min_height(380.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                for s in SECTIONS {
                    let sel = s == section;
                    let label = egui::RichText::new(s).size(12.5).color(if sel { t.text_strong } else { t.text });
                    if ui.add(egui::Button::selectable(sel, label).frame_when_inactive(false).min_size(egui::vec2(LIST_WIDTH - 12.0, 24.0))).clicked()
                    {
                        d.fields.insert("__section".into(), json!(s));
                    }
                }
            });
        });
        ui.add_space(14.0);
        ui.vertical(|ui| {
            ui.set_width(500.0);
            ui.label(egui::RichText::new(section).font(theme::semibold(14.0)).color(t.text_strong));
            egui::ScrollArea::vertical().id_salt(("save-pdf", section)).max_height(360.0).auto_shrink([false, false]).show(ui, |ui| match section {
                "Compression" => compression(ui, d),
                "Marks and Bleeds" => marks_and_bleeds(app, ui, d),
                "Output" => output(ui, d),
                "Advanced" => advanced(ui, d),
                "Security" => security(ui, d),
                "Summary" => summary(app, ui, d),
                _ => general(ui, d, editor),
            });
        });
    });
    false
}

/// The Preset row (the presets, built-in and saved, and Save Preset…) and, while Save Preset…
/// asks for a name, the row taking it.
fn preset_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) {
    row_with(ui, "Preset:", TOP_LABEL_WIDTH, |ui| {
        let presets = pdf::presets(&app.session);
        let names: Vec<&str> = presets.iter().map(String::as_str).collect();
        let chosen = widgets::dropdown(ui, "pdf-preset", &d.str("preset"), &names, 300.0).and_then(|i| names.get(i)).map(|p| p.to_string());
        if let Some(name) = chosen {
            apply_preset(app, d, &name);
        }
        let asking = d.fields.contains_key(SAVE_AS);
        if ui
            .add_enabled_ui(!asking, |ui| widgets::flat_button(ui, "Save Preset…", 96.0))
            .inner
            .on_hover_text("Save these settings as a preset")
            .clicked()
        {
            d.fields.insert(SAVE_AS.into(), json!(app.session.new_pdf_preset_name()));
        }
    });
    if d.fields.contains_key(SAVE_AS) {
        row_with(ui, "Save as preset:", TOP_LABEL_WIDTH, |ui| {
            form::text_edit(ui, d, SAVE_AS, 200.0);
            if widgets::flat_button(ui, "Save", 52.0).clicked()
                && let Err(e) = save_preset(app, d)
            {
                app.status(e);
            }
            if widgets::flat_button(ui, "Cancel", 60.0).clicked() {
                d.fields.remove(SAVE_AS);
            }
        });
    }
}

/// Replace the settings with preset `name`'s (the artboard choice and path stay).
fn apply_preset(app: &mut VectorcraftApp, d: &mut Dialog, name: &str) {
    match settings_fields(app, &json!({ "preset": name })) {
        Ok(settings) => {
            d.fields.extend(settings);
            d.fields.insert("preset".into(), json!(name));
        }
        Err(e) => app.status(e),
    }
}

/// General: the options, then the artboards (Save PDF) or the description (preset editor).
fn general(ui: &mut egui::Ui, d: &mut Dialog, editor: bool) {
    heading(ui, "Options");
    let plain = choice::<Standard>(d, "standard").unwrap_or_default() == Standard::None;
    flag(ui, d, "preserveEditing", "Preserve editing capabilities", plain);
    flag(ui, d, "thumbnails", "Embed page thumbnails", true);
    flag(ui, d, "fastWebView", "Optimize for fast web view", true);
    flag(ui, d, "viewAfterSaving", "View PDF after saving", true);
    flag(ui, d, "createLayers", "Create PDF layers from top-level layers", true);
    flag(ui, d, "includeNonPrinting", "Include non-printing layers", true);
    if editor {
        heading(ui, "Description");
        if let Some(text) = widgets::text_field(ui, "pdf-preset-description", Some(&d.str("description")), 460.0, 3) {
            d.fields.insert("description".into(), json!(text));
        }
        return;
    }
    heading(ui, "Artboards");
    let mut all = d.bool("__allArtboards");
    ui.horizontal(|ui| {
        let changed = ui.radio_value(&mut all, true, "All").changed() | ui.radio_value(&mut all, false, "Range:").changed();
        if changed {
            d.fields.insert("__allArtboards".into(), json!(all));
        }
        let mut range = d.str("range");
        if ui.add_enabled(!all, egui::TextEdit::singleline(&mut range).desired_width(140.0).hint_text("1-3, 5")).changed() {
            d.fields.insert("range".into(), json!(range));
        }
    });
}

fn image_rows(ui: &mut egui::Ui, d: &mut Dialog, key: &str, title: &str) {
    heading(ui, title);
    let base = format!("compression.{key}");
    let on = get(d, &format!("{base}.downsample")) != Downsample::None.id();
    ui.horizontal(|ui| {
        pick::<Downsample>(ui, d, &format!("{base}.downsample"), 150.0, |_| true);
        ui.label("to");
        number(ui, d, &format!("{base}.ppi"), " ppi", on);
        ui.label("above");
        number(ui, d, &format!("{base}.abovePpi"), " ppi", on);
    });
    ui.horizontal(|ui| {
        ui.label("Compression:");
        if key == "mono" {
            pick::<MonoCodec>(ui, d, &format!("{base}.compression"), 150.0, |_| true);
        } else {
            pick::<ImageCodec>(ui, d, &format!("{base}.compression"), 120.0, |_| true);
            let lossy = get(d, &format!("{base}.compression")).as_str().is_some_and(|c| c != ImageCodec::None.id() && c != ImageCodec::Zip.id());
            ui.label("Quality:");
            ui.add_enabled_ui(lossy, |ui| {
                pick::<JpegQuality>(ui, d, &format!("{base}.quality"), 110.0, |_| true);
            });
        }
    });
}

fn compression(ui: &mut egui::Ui, d: &mut Dialog) {
    image_rows(ui, d, "color", "Color images");
    image_rows(ui, d, "gray", "Grayscale images");
    image_rows(ui, d, "mono", "Monochrome images");
    ui.add_space(8.0);
    flag(ui, d, "compression.compressText", "Compress text and line art", true);
}

fn marks_and_bleeds(app: &VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) {
    let unit = app.session.general_unit();
    const MARKS: [(&str, &str); 4] = [
        ("marks.trim", "Trim marks"),
        ("marks.registration", "Registration marks"),
        ("marks.colorBars", "Color bars"),
        ("marks.pageInfo", "Page information"),
    ];
    heading(ui, "Marks");
    let all = MARKS.iter().all(|(p, _)| get(d, p).as_bool() == Some(true));
    if widgets::check(ui, "All printer's marks", all, true) {
        for (p, _) in MARKS {
            set(d, p, json!(!all));
        }
    }
    egui::Grid::new("pdf-marks").num_columns(2).spacing([24.0, 4.0]).show(ui, |ui| {
        for pair in MARKS.chunks(2) {
            for (p, label) in pair {
                flag(ui, d, p, label, true);
            }
            ui.end_row();
        }
    });
    row(ui, "Printer mark type:", |ui| {
        pick::<MarkKind>(ui, d, "marks.kind", 130.0, |_| true);
    });
    row(ui, "Trim mark weight:", |ui| {
        let w = get(d, "marks.weight").as_f64().unwrap_or(0.25);
        let current =
            WEIGHTS.iter().zip(WEIGHT_LABELS).find(|(x, _)| (*x - w).abs() < 1e-9).map_or_else(|| format!("{w} pt"), |(_, l)| l.to_string());
        if let Some(x) = widgets::dropdown(ui, "marks.weight", &current, &WEIGHT_LABELS, 130.0).and_then(|i| WEIGHTS.get(i)) {
            set(d, "marks.weight", json!(x));
        }
    });
    row(ui, "Offset:", |ui| length(ui, d, "marks.offset", unit, true));
    heading(ui, "Bleeds");
    flag(ui, d, "bleed.useDocument", "Use document bleed settings", true);
    // With the document's bleed the fields show it (Document Setup's `[top, bottom, left, right]`).
    let document = (get(d, "bleed.useDocument").as_bool() == Some(true)).then(|| app.session.active().map_or([0.0; 4], |s| s.doc.setup.bleed));
    egui::Grid::new("pdf-bleed").num_columns(4).spacing([10.0, 6.0]).show(ui, |ui| {
        for pair in [[(0, "bleed.top", "Top:"), (1, "bleed.bottom", "Bottom:")], [(2, "bleed.left", "Left:"), (3, "bleed.right", "Right:")]] {
            for (i, p, label) in pair {
                ui.label(label);
                match document {
                    Some(b) => {
                        ui.add_enabled_ui(false, |ui| widgets::num_field(ui, p, b.get(i).copied(), unit, 80.0));
                    }
                    None => length(ui, d, p, unit, true),
                }
            }
            ui.end_row();
        }
    });
}

/// A dropdown of the colour profiles (`names`, the first one standing for "" at `path`), showing
/// the profile at `path` (a name that isn't one of them too).
fn profile_pick(ui: &mut egui::Ui, d: &mut Dialog, path: &str, names: &[&str], enabled: bool) {
    let blank = names.first().copied().unwrap_or_default();
    let current = get(d, path).as_str().filter(|s| !s.is_empty()).unwrap_or(blank).to_string();
    ui.add_enabled_ui(enabled, |ui| {
        if let Some(i) = widgets::dropdown(ui, path, &current, names, 300.0) {
            set(d, path, json!(if i == 0 { "" } else { names.get(i).copied().unwrap_or_default() }));
        }
    });
}

fn output(ui: &mut egui::Ui, d: &mut Dialog) {
    let profiles = vectorcraft_color::cms::profiles();
    let names = |blank: &'static str| -> Vec<&str> { std::iter::once(blank).chain(profiles.iter().map(|p| p.name.as_str())).collect() };
    heading(ui, "Color");
    row(ui, "Color conversion:", |ui| {
        pick::<ColorConversion>(ui, d, "output.conversion", 300.0, |_| true);
    });
    let converting = get(d, "output.conversion") != ColorConversion::None.id();
    row(ui, "Destination:", |ui| profile_pick(ui, d, "output.destination", &names(DOCUMENT_PROFILE), converting));
    row(ui, "Profile inclusion:", |ui| {
        pick::<ProfileInclusion>(ui, d, "output.profiles", 300.0, |_| true);
    });
    heading(ui, "Output Intent");
    // PDF/A files carry their own output intent.
    let own = choice::<Standard>(d, "standard") != Some(Standard::PdfA2b);
    row(ui, "Output intent profile:", |ui| profile_pick(ui, d, "output.outputIntent", &names(NO_PROFILE), own));
    for (p, label) in [
        ("output.outputCondition", "Output condition:"),
        ("output.outputConditionId", "Condition identifier:"),
        ("output.registry", "Registry name:"),
    ] {
        row(ui, label, |ui| text(ui, d, p, own));
    }
    flag(ui, d, "output.trapped", "Mark as trapped", own);
    if !own {
        note(ui, "PDF/A files carry their own output intent.");
    }
}

fn advanced(ui: &mut egui::Ui, d: &mut Dialog) {
    heading(ui, "Fonts");
    let outline = get(d, "advanced.outlineText").as_bool() == Some(true);
    row(ui, "Subset fonts below:", |ui| {
        number(ui, d, "advanced.fontSubsetPercent", "%", !outline);
        ui.label("of characters used");
    });
    flag(ui, d, "advanced.outlineText", "Convert text to outlines", true);
    heading(ui, "Overprint");
    row(ui, "Overprint:", |ui| {
        pick::<Overprint>(ui, d, "advanced.overprint", 130.0, |_| true);
    });
}

/// The password fields (never stored in presets).
const OPEN_PASSWORD: &str = "security.openPassword";
const PERMISSIONS_PASSWORD: &str = "security.permissionsPassword";

/// A checkbox turning a password on (UI-only `flag`) or off (clearing it); checked while the
/// password is set. Returns whether the password applies.
fn password_check(ui: &mut egui::Ui, d: &mut Dialog, path: &str, flag: &str, label: &str, enabled: bool) -> bool {
    let on = d.bool(flag) || get(d, path).as_str().is_some_and(|s| !s.is_empty());
    if widgets::check(ui, label, on, enabled) {
        d.fields.insert(flag.into(), json!(!on));
        if on {
            set(d, path, json!(""));
        }
    }
    on && enabled
}

fn password_field(ui: &mut egui::Ui, d: &mut Dialog, path: &str, enabled: bool) {
    let mut s = get(d, path).as_str().unwrap_or_default().to_string();
    if ui.add_enabled(enabled, egui::TextEdit::singleline(&mut s).password(true).desired_width(200.0)).changed() {
        set(d, path, json!(s));
    }
}

fn security(ui: &mut egui::Ui, d: &mut Dialog) {
    let plain = choice::<Standard>(d, "standard").unwrap_or_default() == Standard::None;
    if !plain {
        note(ui, "Files of a PDF standard can't be password-protected.");
    }
    heading(ui, "Document open password");
    let open = password_check(ui, d, OPEN_PASSWORD, "__requireOpenPassword", "Require a password to open the document", plain);
    row(ui, "Open password:", |ui| password_field(ui, d, OPEN_PASSWORD, open));
    heading(ui, "Permissions");
    let restrict = password_check(ui, d, PERMISSIONS_PASSWORD, "__restrictPermissions", "Restrict printing, editing and other tasks", plain);
    row(ui, "Permissions password:", |ui| password_field(ui, d, PERMISSIONS_PASSWORD, restrict));
    ui.add_enabled_ui(restrict, |ui| {
        row(ui, "Printing allowed:", |ui| {
            pick::<Printing>(ui, d, "security.printing", 260.0, |_| true);
        });
        row(ui, "Changes allowed:", |ui| {
            pick::<Changes>(ui, d, "security.changes", 260.0, |_| true);
        });
    });
    flag(ui, d, "security.copy", "Enable copying of text, images and other content", restrict);
    let reader = "Enable text access for screen readers";
    if get(d, "security.copy").as_bool() == Some(true) {
        // What can be copied can be read out.
        widgets::check(ui, reader, true, false);
    } else {
        flag(ui, d, "security.screenReader", reader, restrict);
    }
    let compatibility = choice::<Compatibility>(d, "compatibility").unwrap_or_default();
    flag(ui, d, "security.plaintextMetadata", "Enable plaintext metadata", (open || restrict) && compatibility != Compatibility::Pdf14);
    ui.add_space(6.0);
    note(ui, &format!("Encryption: {} (set by Compatibility)", Encryption::for_compatibility(compatibility).label()));
}

/// The section a changed option belongs to (for the Summary's order).
pub(super) fn section_of(option: &str) -> usize {
    match option.split('.').next().unwrap_or_default() {
        "compression" => 1,
        "marks" | "bleed" => 2,
        "output" => 3,
        "advanced" => 4,
        "security" => 5,
        _ => 0,
    }
}

/// `compression.color.abovePpi` → "Compression › Color › Above Ppi": the section, then the keys
/// (without the section's own object), `thumbnails` → "General › Thumbnails".
pub(super) fn option_label(option: &str) -> String {
    let section = SECTIONS.get(section_of(option)).copied().unwrap_or(SECTIONS[0]);
    let mut keys = option.split('.').peekable();
    keys.next_if(|k| k.eq_ignore_ascii_case(section));
    std::iter::once(section.to_string()).chain(keys.map(|k| form::humanize(k).trim_end_matches(':').to_string())).collect::<Vec<_>>().join(" › ")
}

/// `document.pdfSettings` for the dialog with the document's warnings (an export in memory),
/// cached until a field or the document changes.
fn summary_of(app: &mut VectorcraftApp, ctx: &egui::Context, p: &Value) -> Result<Value, String> {
    let revision = app.session.active().map_or(0, |s| s.revision);
    let stamp = egui::Id::new((p.to_string(), revision));
    let key = egui::Id::new("save-pdf-summary");
    if let Some((s, v)) = ctx.data(|m| m.get_temp::<(egui::Id, Result<Value, String>)>(key))
        && s == stamp
    {
        return v;
    }
    let mut q = p.clone();
    q["includeDocument"] = json!(true);
    let v = app.session.execute("document.pdfSettings", &q).map_err(|e| e.to_string());
    ctx.data_mut(|m| m.insert_temp(key, (stamp, v.clone())));
    v
}

fn summary(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) {
    let t = Tokens::get(ui.ctx());
    let v = match summary_of(app, ui.ctx(), &params(d)) {
        Ok(v) => v,
        Err(e) => {
            heading(ui, "Error");
            ui.label(egui::RichText::new(format!("⚠ {e}")).color(t.text));
            return;
        }
    };
    heading(ui, "Options");
    let mut changed: Vec<&Value> = v["changed"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    changed.sort_by_key(|c| section_of(c["option"].as_str().unwrap_or_default()));
    if changed.is_empty() {
        let base = if d.kind == PRESET_KIND { pdf::DEFAULT_PRESET } else { "the preset" };
        note(ui, &format!("Every option matches {base}."));
    }
    for c in changed {
        let value = match &c["value"] {
            Value::Bool(b) => if *b { "On" } else { "Off" }.to_string(),
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        ui.label(egui::RichText::new(format!("{}: {value}", option_label(c["option"].as_str().unwrap_or_default()))).color(t.text));
    }
    heading(ui, "Warnings");
    let warnings = v["warnings"].as_array().map(Vec::as_slice).unwrap_or_default();
    if warnings.is_empty() {
        note(ui, "None.");
    }
    for w in warnings {
        ui.label(egui::RichText::new(format!("⚠ {}", w.as_str().unwrap_or_default())).color(t.text));
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use vectorcraft_engine::Session;

    use super::*;
    use crate::Services;

    type Log = Rc<RefCell<Vec<(String, Vec<u8>)>>>;

    /// An app with two artboards whose writer and URL opener record what they get.
    fn app() -> (VectorcraftApp, Log, Log) {
        let (written, opened) = (Log::default(), Log::default());
        let (w, o) = (written.clone(), opened.clone());
        let services = Services {
            write: Some(Box::new(move |p: &str, b: &[u8]| {
                w.borrow_mut().push((p.to_string(), b.to_vec()));
                Ok(())
            })),
            open_url: Some(Box::new(move |u: &str| o.borrow_mut().push((u.to_string(), vec![])))),
            ..Default::default()
        };
        let mut app = VectorcraftApp::new(Session::new(), services);
        app.run("file.new", json!({"width": 120, "height": 90, "artboards": 3})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
        (app, written, opened)
    }

    /// One headless frame of the dialog layer.
    fn frame(app: &mut VectorcraftApp) {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| super::super::show(app, ui.ctx()));
        out.textures_delta.clear();
    }

    fn set_field(app: &mut VectorcraftApp, path: &str, v: Value) {
        set(app.ui.dialog.as_mut().expect("dialog open"), path, v);
    }

    #[test]
    fn every_section_draws() {
        let (mut app, _, _) = app();
        app.run("file.export.pdf", json!({})).unwrap();
        assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some(KIND));
        assert_eq!(super::super::DialogKind::of(KIND), Some(super::super::DialogKind::SavePdf));
        for s in SECTIONS {
            set_field(&mut app, "__section", json!(s));
            frame(&mut app);
            assert!(app.ui.dialog.is_some(), "{s} closed the dialog");
        }
        // The Summary lists what changed and the warnings for options not applied yet.
        set_field(&mut app, "thumbnails", json!(true));
        let p = params(app.ui.dialog.as_ref().unwrap());
        let v = summary_of(&mut app, &egui::Context::default(), &p).unwrap();
        assert_eq!(v["changed"][0]["option"], "thumbnails");
        assert!(v["warnings"][0].as_str().unwrap().contains("thumbnails"));
    }

    #[test]
    fn confirm_runs_export_pdf_with_the_dialog_options() {
        let (mut app, written, opened) = app();
        app.run("ui.savePdfDialog", json!({"path": "/tmp/out.pdf", "compatibility": "1.5"})).unwrap();
        set_field(&mut app, "__allArtboards", json!(false));
        set_field(&mut app, "range", json!("1,3"));
        set_field(&mut app, "compression.compressText", json!(false));
        set_field(&mut app, "viewAfterSaving", json!(true));
        let r = super::super::confirm(&mut app).unwrap();
        assert!(app.ui.dialog.is_none(), "closes after saving");
        assert_eq!(r["path"], "/tmp/out.pdf");
        assert!(r.get("dataBase64").is_none());
        let w = written.borrow();
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].0, "/tmp/out.pdf");
        assert!(w[0].1.starts_with(b"%PDF-1.5"), "compatibility reaches the writer");
        assert_eq!(vectorcraft_pdf::import(&w[0].1).unwrap().artboards.len(), 2, "range 1,3");
        assert!(!String::from_utf8_lossy(&w[0].1).contains("/FlateDecode"), "uncompressed content");
        assert_eq!(opened.borrow().len(), 1, "View PDF after Saving opens the file once");
        let url = &opened.borrow()[0].0;
        assert!(url.starts_with("file:///") && url.ends_with("/tmp/out.pdf"), "the written file as a URL: {url}");
        assert!(app.ui.status.starts_with("Saved /tmp/out.pdf"), "{}", app.ui.status);
    }

    #[test]
    fn warnings_reach_the_status_and_errors_keep_the_dialog_open() {
        let (mut app, written, opened) = app();
        app.run("ui.savePdfDialog", json!({"path": "/tmp/w.pdf"})).unwrap();
        set_field(&mut app, "thumbnails", json!(true));
        super::super::confirm(&mut app).unwrap();
        assert!(app.ui.status.contains("1 note(s)") && app.ui.status.contains("thumbnails"), "{}", app.ui.status);
        assert!(opened.borrow().is_empty(), "not opened unless asked");
        app.run("ui.savePdfDialog", json!({"path": "/tmp/bad.pdf"})).unwrap();
        set_field(&mut app, "__allArtboards", json!(false));
        set_field(&mut app, "range", json!("7"));
        assert!(super::super::confirm(&mut app).is_err());
        assert!(app.ui.dialog.is_some(), "a bad range keeps the dialog open");
        assert_eq!(written.borrow().len(), 1);
        // Without a path, bad options are refused before a save dialog asks for one.
        let asked = Rc::new(RefCell::new(0));
        let a = asked.clone();
        app.services.pick_save = Some(Box::new(move |pick: &crate::FilePick| {
            *a.borrow_mut() += 1;
            Some(format!("/tmp/picked-{}", pick.name))
        }));
        app.run("ui.savePdfDialog", json!({"range": "7"})).unwrap();
        assert!(super::super::confirm(&mut app).is_err());
        assert_eq!(*asked.borrow(), 0, "no save dialog for a bad range");
        set_field(&mut app, "range", json!("2"));
        let r = super::super::confirm(&mut app).unwrap();
        assert_eq!(*asked.borrow(), 1);
        assert!(r["path"].as_str().is_some_and(|p| p.starts_with("/tmp/picked-") && p.ends_with(".pdf")), "{r}");
        // Agents can export without the dialog.
        let r = app.run("file.export.pdf", json!({"path": "/tmp/direct.pdf", "range": "2"})).unwrap();
        assert_eq!(r["path"], "/tmp/direct.pdf");
        assert!(r["warnings"].is_array());
    }

    #[test]
    fn presets_reset_the_settings_and_fields_fall_back_to_defaults() {
        let (mut app, _, _) = app();
        app.run("ui.savePdfDialog", json!({"compatibility": "1.4", "marks": {"trim": true}})).unwrap();
        let d = app.ui.dialog.as_mut().unwrap();
        assert_eq!(get(d, "compatibility"), "1.4");
        assert_eq!(get(d, "marks.weight"), 0.25, "the rest of a section keeps its defaults");
        d.fields.remove("compression");
        assert_eq!(get(d, "compression.color.ppi"), 300.0, "a field an agent dropped reads as its default");
        let mut d = d.clone();
        apply_preset(&mut app, &mut d, pdf::DEFAULT_PRESET);
        assert_eq!(get(&d, "compatibility"), "1.7");
        assert_eq!(get(&d, "marks.trim"), false);
        assert!(app.run("ui.savePdfDialog", json!({"compatibility": "1.0"})).is_err(), "bad options are refused");
    }

    #[test]
    fn marks_and_bleeds_reach_the_pdf() {
        let (mut app, written, _) = app();
        app.run("document.setup", json!({"bleed": 9})).unwrap();
        let notes = app.run("layer.new", json!({})).unwrap()["id"].clone();
        app.run("layer.setProps", json!({"id": notes, "printable": false})).unwrap();
        app.run("shape.ellipse", json!({"x": 60, "y": 40, "width": 20, "height": 20})).unwrap();
        app.run("ui.savePdfDialog", json!({"path": "/tmp/marks.pdf"})).unwrap();
        set_field(&mut app, "__section", json!("Marks and Bleeds"));
        set_field(&mut app, "bleed.useDocument", json!(true));
        set_field(&mut app, "marks.trim", json!(true));
        frame(&mut app);
        set_field(&mut app, "__section", json!("General"));
        set_field(&mut app, "includeNonPrinting", json!(true));
        frame(&mut app);
        let r = super::super::confirm(&mut app).unwrap();
        assert_eq!(r["warnings"], json!([]), "marks and bleed are applied");
        let bytes = &written.borrow()[0].1;
        let page = &vectorcraft_pdf::info(bytes, None).unwrap().pages[0];
        let trim = page.boxes.iter().find(|(b, _)| *b == vectorcraft_pdf::CropTo::Trim).unwrap().1;
        assert_eq!((trim.width(), trim.x0), (120.0, 9.0 + 18.0 + 0.25), "the trim box is the artboard, inside the bleed and marks");
        let mut ellipse = false;
        vectorcraft_pdf::import(bytes).unwrap().walk(|n| {
            let b = n.path_data().and_then(|p| p.bounds()).unwrap_or_default();
            ellipse |= (b.width() - 20.0).abs() < 0.1 && (b.height() - 20.0).abs() < 0.1;
        });
        assert!(ellipse, "the non-printing layer is included");
    }

    #[test]
    fn option_labels_read_well() {
        assert_eq!(option_label("compression.color.abovePpi"), "Compression › Color › Above Ppi");
        assert_eq!(option_label("compression.compressText"), "Compression › Compress Text");
        assert_eq!(option_label("thumbnails"), "General › Thumbnails");
        assert_eq!(option_label("bleed.top"), "Marks and Bleeds › Bleed › Top");
        assert_eq!(section_of("bleed.top"), 2);
        assert_eq!(SECTIONS[section_of("standard")], "General");
    }

    #[test]
    fn the_security_section_takes_passwords_that_presets_drop() {
        let (mut app, _, _) = app();
        app.run("ui.savePdfDialog", json!({})).unwrap();
        set_field(&mut app, "__section", json!("Security"));
        set_field(&mut app, OPEN_PASSWORD, json!("open"));
        set_field(&mut app, PERMISSIONS_PASSWORD, json!("own"));
        set_field(&mut app, "security.copy", json!(false));
        frame(&mut app);
        let p = params(app.ui.dialog.as_ref().unwrap());
        assert_eq!((p["security"]["openPassword"].as_str(), p["security"]["permissionsPassword"].as_str()), (Some("open"), Some("own")));
        assert_eq!(p["security"]["copy"], false);
        // With a standard the section still draws (greyed); a preset brings its own security.
        set_field(&mut app, "standard", json!(Standard::PdfA2b.id()));
        frame(&mut app);
        let mut d = app.ui.dialog.clone().unwrap();
        apply_preset(&mut app, &mut d, pdf::DEFAULT_PRESET);
        assert_eq!(get(&d, OPEN_PASSWORD), &Value::Null, "presets carry no passwords");
        assert_eq!(get(&d, "security.copy"), true);
    }
}
