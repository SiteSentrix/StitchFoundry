//! File → Export → Export for Screens: the Artboards tab (All, a range or Full Document, Include
//! Bleed, thumbnails with checkboxes) and the Assets tab, the destination (folder picker, Open
//! Location after Export, Create Sub-folders; the web downloads instead), the format rows (scale
//! as `2x`, `100w`, `100h` or `72ppi`, suffix, format), the presets, Format Settings (the gear)
//! and the prefix. It opens on the settings the document last exported with.
//!
//! Fields (`ui.dialog.set`): `tab` (artboards|assets), `select` (all|range|full), `range`,
//! `boards` ([bool] per artboard), `includeBleed`, `folder`, `openLocation`, `subfolders`,
//! `prefix`, `preset` (""|mobile|density: where the rows came from), `formats` (rows of
//! `document.exportForScreens`, `scale` as text), `settings` ({png|png8|jpg|webp|gif|svg|pdf: {…}});
//! `__settings` names the format whose Format Settings show ("" when closed).

use serde::Deserialize;
use serde_json::{Map, Value, json};
use vectorcraft_engine::cmd::fileio::{self, ArtboardPick, SCREEN_PRESETS, ScreenSize, pdf};

use super::{DialogSpec, form, png_options, svg_options};
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, io, widgets};

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |_| "Export for Screens".into(),
    body,
    confirm,
    ok: Some("Export Artboard"),
    ok_label: Some(|app| if io::is_web(app) { "Download" } else { "Export Artboard" }),
    min_width: 660.0,
    ..DialogSpec::FORM
};

/// `Dialog::kind` of this dialog.
pub const KIND: &str = "exportForScreens";

/// The format choices of a row: (label, format, JPEG quality).
const FORMATS: [(&str, &str, Option<u8>); 9] = [
    ("PNG", "png", None),
    ("PNG 8", "png8", None),
    ("JPG 100", "jpg", Some(100)),
    ("JPG 80", "jpg", Some(80)),
    ("JPG 50", "jpg", Some(50)),
    ("WebP", "webp", None),
    ("GIF", "gif", None),
    ("SVG", "svg", None),
    ("PDF", "pdf", None),
];
const FORMAT_LABELS: [&str; 9] = ["PNG", "PNG 8", "JPG 100", "JPG 80", "JPG 50", "WebP", "GIF", "SVG", "PDF"];

/// The formats Format Settings has options for: (format, label).
const SETTINGS: [(&str, &str); 7] =
    [("png", "PNG"), ("png8", "PNG 8"), ("jpg", "JPG"), ("webp", "WebP"), ("gif", "GIF"), ("svg", "SVG"), ("pdf", "PDF")];
const SETTINGS_LABELS: [&str; 7] = ["PNG", "PNG 8", "JPG", "WebP", "GIF", "SVG", "PDF"];

/// The sizes a row's scale list offers (any can be typed).
const SIZES: [&str; 12] = ["0.5x", "0.75x", "1x", "1.5x", "2x", "3x", "4x", "512w", "512h", "72ppi", "150ppi", "300ppi"];

/// Open the dialog on the settings the active document last exported with (else one PNG row at
/// 1x, into the Desktop).
pub fn open(app: &mut VectorcraftApp) {
    let (n, saved) = app.session.active().map(|st| (st.doc.artboards.len(), st.doc.export_settings.clone())).unwrap_or_default();
    let get = |k: &str| saved.get(k).filter(|v| !v.is_null());
    let flag = |k: &str, default: bool| get(k).and_then(Value::as_bool).unwrap_or(default);
    // The artboards: Full Document, a range (or a list of them), else all.
    let chosen = ArtboardPick::deserialize(&Value::Object(saved.clone())).ok().and_then(|p| p.resolve(n).ok().flatten());
    let select = match (flag("fullDocument", false), &chosen) {
        (true, _) => "full",
        (false, Some(c)) if c.len() < n => "range",
        _ => "all",
    };
    let boards: Vec<bool> = (0..n).map(|i| chosen.as_ref().is_none_or(|c| c.contains(&i))).collect();
    // The rows: the saved ones (or the saved preset's), sizes shown as text.
    let preset = get("preset").and_then(Value::as_str).filter(|p| fileio::screen_preset_rows(p).is_some());
    let rows = match (get("formats").and_then(Value::as_array), preset) {
        (Some(rows), _) if rows.iter().any(Value::is_object) => rows.iter().filter(|r| r.is_object()).map(editable_row).collect(),
        (_, Some(p)) => fileio::screen_preset_rows(p).unwrap_or_default(),
        _ => vec![json!({"format": "png", "scale": "1x", "suffix": ""})],
    };
    // Every format's settings: its defaults, then the saved ones.
    let mut settings = Map::new();
    for (id, _) in SETTINGS {
        let mut s = defaults(app, id);
        if let Some(o) = get("settings").and_then(|v| v.get(id)).and_then(Value::as_object) {
            s.extend(o.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        settings.insert(id.into(), Value::Object(s));
    }
    let folder = get("folder").and_then(Value::as_str).filter(|f| !f.is_empty()).map(str::to_string).or_else(fileio::export_folder);
    let fields = json!({
        "tab": "artboards",
        "select": select,
        "range": range_text(&boards),
        "boards": boards,
        "includeBleed": flag("includeBleed", false),
        "folder": folder.unwrap_or_default(),
        "openLocation": flag("openLocation", true),
        "subfolders": flag("subfolders", preset.is_some()),
        "prefix": get("prefix").and_then(Value::as_str).unwrap_or_default(),
        "preset": preset.unwrap_or_default(),
        "formats": rows,
        "settings": settings,
        "__settings": "",
        "__presets": pdf::presets(&app.session),
    });
    app.ui.dialog = Some(Dialog::new(KIND, fields));
}

/// The options format `id` starts with in Format Settings: the raster options dialogs' defaults
/// (without quality and image maps: the rows choose quality, a map is another file), the SVG
/// Options' first-use ones (without artboards and images: a linked image is another file), or the
/// default PDF preset.
fn defaults(app: &VectorcraftApp, id: &str) -> Map<String, Value> {
    let mut o = match (id, fileio::format(id)) {
        ("svg", _) => svg_options::first_use(),
        ("pdf", _) => Map::from_iter([("preset".to_string(), json!(pdf::DEFAULT_PRESET))]),
        (_, Some(f)) => png_options::defaults(app, f),
        _ => Map::new(),
    };
    for k in ["quality", "imageMap", "useArtboards", "images"] {
        o.remove(k);
    }
    o
}

/// A row with its size as the text the Scale field shows (`2x`, `100w`, `100h`, `72ppi`).
fn editable_row(row: &Value) -> Value {
    let num = |k: &str| row.get(k).and_then(Value::as_f64);
    let text = match (num("width"), num("height"), num("ppi"), row.get("scale")) {
        (Some(w), ..) => format!("{w}w"),
        (_, Some(h), ..) => format!("{h}h"),
        (_, _, Some(p), _) => format!("{p}ppi"),
        (.., Some(Value::String(s))) => s.clone(),
        (.., Some(v)) => format!("{}x", v.as_f64().unwrap_or(1.0)),
        _ => "1x".into(),
    };
    let mut r = row.clone();
    if let Some(o) = r.as_object_mut() {
        for k in ["width", "height", "ppi"] {
            o.remove(k);
        }
        o.insert("scale".into(), json!(text));
    }
    r
}

/// The checked artboards as a 1-based range (`1-3, 5`).
pub(super) fn range_text(boards: &[bool]) -> String {
    let mut parts = vec![];
    let mut start = None;
    // A false past the end closes the last run.
    for (i, on) in boards.iter().copied().chain([false]).enumerate() {
        match (on, start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                parts.push(if i - 1 > s { format!("{}-{i}", s + 1) } else { i.to_string() });
                start = None;
            }
            _ => {}
        }
    }
    parts.join(", ")
}

/// The `document.exportForScreens` params the dialog describes (the web: no folder, one ZIP for
/// several files).
fn params(app: &VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let n = app.session.active().map_or(0, |st| st.doc.artboards.len());
    let rows = d.fields.get("formats").and_then(Value::as_array).cloned().unwrap_or_default();
    if rows.is_empty() {
        return Err("add a format to export".into());
    }
    let mut p = Map::new();
    let files = match d.str("select").as_str() {
        "full" => {
            p.insert("fullDocument".into(), json!(true));
            rows.len()
        }
        "range" => {
            let range = d.str("range");
            // A bad range keeps the dialog open.
            let chosen = ArtboardPick { range: Some(range.clone()), ..Default::default() }.resolve(n)?.unwrap_or_default();
            p.insert("range".into(), json!(range));
            chosen.len() * rows.len()
        }
        _ => n * rows.len(),
    };
    for k in ["includeBleed", "subfolders"] {
        p.insert(k.into(), json!(d.bool(k)));
    }
    p.insert("prefix".into(), json!(d.str("prefix")));
    p.insert("formats".into(), Value::Array(rows));
    p.insert("settings".into(), d.fields.get("settings").cloned().unwrap_or(json!({})));
    if io::is_web(app) {
        p.insert("zip".into(), json!(files > 1));
    } else {
        let folder = d.str("folder");
        if folder.trim().is_empty() {
            return Err("choose a folder to export to".into());
        }
        p.insert("folder".into(), json!(folder.trim()));
        p.insert("openLocation".into(), json!(d.bool("openLocation")));
    }
    Ok(Value::Object(p))
}

/// Export; the dialog stays open when the export fails.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let p = params(app, d)?;
    let r = io::export_for_screens(app, p);
    if r.is_ok() {
        app.ui.dialog = None;
    }
    r
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let tab = usize::from(d.str("tab") == "assets");
    if let Some(i) = widgets::tab_bar(ui, &["Artboards", "Assets"], tab) {
        d.fields.insert("tab".into(), json!(if i == 1 { "assets" } else { "artboards" }));
    }
    ui.add_space(8.0);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(270.0);
            if tab == 1 {
                ui.add_space(24.0);
                widgets::dim_label(ui, "No assets to export yet: artwork collected for export shows here.");
            } else {
                artboards(app, ui, d);
            }
        });
        let (sep, _) = ui.allocate_exact_size(egui::vec2(17.0, 380.0), egui::Sense::hover());
        ui.painter().line_segment([sep.center_top(), sep.center_bottom()], egui::Stroke::new(1.0, t.divider));
        ui.vertical(|ui| {
            ui.set_width(350.0);
            let shown = d.str("__settings");
            if SETTINGS.iter().any(|(id, _)| *id == shown) {
                format_settings(ui, d, &shown);
            } else {
                destination(app, ui, d);
                ui.add_space(10.0);
                formats(ui, d);
            }
        });
    });
    false
}

fn set_select(d: &mut Dialog, select: &str) {
    d.fields.insert("select".into(), json!(select));
}

/// Select (All, Range, Full Document), Include Bleed, and the artboards' thumbnails and checkboxes.
fn artboards(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) {
    let n = app.session.active().map_or(0, |st| st.doc.artboards.len());
    let mut boards: Vec<bool> =
        d.fields.get("boards").and_then(Value::as_array).map(|a| a.iter().map(|b| b.as_bool().unwrap_or(false)).collect()).unwrap_or_default();
    boards.resize(n, true);
    let before = boards.clone();
    let select = d.str("select");
    widgets::dim_label(ui, "Select:");
    ui.horizontal(|ui| {
        if widgets::radio(ui, "All", select == "all", true) {
            set_select(d, "all");
            boards.iter_mut().for_each(|b| *b = true);
        }
        if widgets::radio(ui, "Range:", select == "range", true) {
            set_select(d, "range");
        }
        ui.add_enabled_ui(select == "range", |ui| {
            if form::text(ui, d, "range", 90.0)
                && let Ok(Some(chosen)) = (ArtboardPick { range: Some(d.str("range")), ..Default::default() }).resolve(n)
            {
                boards = (0..n).map(|i| chosen.contains(&i)).collect();
            }
        });
    });
    if widgets::radio(ui, "Full Document", select == "full", true) {
        set_select(d, "full");
    }
    ui.add_space(4.0);
    form::check(ui, d, "includeBleed", "Include Bleed");
    ui.add_space(6.0);
    let typed = boards != before;
    let checked = boards.clone();
    ui.add_enabled_ui(d.str("select") != "full", |ui| {
        ui.horizontal(|ui| {
            if ui.small_button("Select All").clicked() {
                boards.iter_mut().for_each(|b| *b = true);
            }
            if ui.small_button("Clear").clicked() {
                boards.iter_mut().for_each(|b| *b = false);
            }
        });
        let names: Vec<String> = app.session.active().map(|s| s.doc.artboards.iter().map(|a| a.name.clone()).collect()).unwrap_or_default();
        egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
            for (i, name) in names.iter().enumerate() {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), egui::Sense::hover());
                    match artboard_thumb(app, ui.ctx(), i) {
                        Some(tex) => {
                            let sz = tex.size_vec2();
                            let s = (46.0 / sz.x.max(sz.y)).min(1.0);
                            let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                            ui.painter().image(tex.id(), egui::Rect::from_center_size(r.center(), sz * s), uv, egui::Color32::WHITE);
                        }
                        None => {
                            ui.painter().rect_filled(r, 2.0, egui::Color32::WHITE);
                        }
                    }
                    if let Some(b) = boards.get_mut(i) {
                        ui.checkbox(b, name);
                    }
                });
            }
        });
    });
    // Checking artboards picks them as the range (all of them: All); a typed range keeps its text.
    if boards != checked {
        set_select(d, if boards.iter().all(|b| *b) { "all" } else { "range" });
        d.fields.insert("range".into(), json!(range_text(&boards)));
    }
    if typed || boards != checked {
        d.fields.insert("boards".into(), json!(boards));
    }
}

/// Export to: the folder (typed or picked), Open Location after Export and Create Sub-folders; the
/// web downloads instead.
fn destination(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) {
    widgets::dim_label(ui, "Export to:");
    if io::is_web(app) {
        widgets::dim_label(ui, "Your browser downloads the files (several as one .zip).");
    } else {
        let pick = app.services.pick_folder.is_some();
        ui.horizontal(|ui| {
            form::text(ui, d, "folder", if pick { 300.0 } else { 334.0 });
            if pick
                && widgets::icon_button(ui, "folder-open", "Choose a folder", false, 26.0).clicked()
                && let Some(f) = app.services.pick_folder.as_mut().and_then(|pick| pick())
            {
                d.fields.insert("folder".into(), json!(f));
            }
        });
        form::check(ui, d, "openLocation", "Open Location after Export");
    }
    form::check(ui, d, "subfolders", "Create Sub-folders");
}

/// The preset, the Format Settings gear, the format rows (scale, suffix, format), Add Scale and the
/// prefix.
fn formats(ui: &mut egui::Ui, d: &mut Dialog) {
    let mut rows: Vec<Value> = d.fields.get("formats").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut edited = false;
    ui.horizontal(|ui| {
        widgets::dim_label(ui, "Formats:");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, "settings", "Format Settings", false, 24.0).clicked() {
                let first = rows.first().and_then(|r| r["format"].as_str()).filter(|f| SETTINGS.iter().any(|(id, _)| id == f));
                d.fields.insert("__settings".into(), json!(first.unwrap_or("png")));
            }
            let preset = d.str("preset");
            let labels: Vec<&str> = std::iter::once("Custom").chain(SCREEN_PRESETS.iter().map(|(_, l)| *l)).collect();
            let at = SCREEN_PRESETS.iter().position(|(id, _)| *id == preset).map_or(0, |i| i + 1);
            if let Some(i) = widgets::dropdown(ui, "efs-preset", labels[at], &labels, 180.0) {
                let id = i.checked_sub(1).and_then(|i| SCREEN_PRESETS.get(i)).map_or("", |(id, _)| *id);
                if let Some(preset_rows) = fileio::screen_preset_rows(id) {
                    rows = preset_rows;
                    d.fields.insert("subfolders".into(), json!(true));
                    d.fields.insert("formats".into(), json!(rows));
                }
                d.fields.insert("preset".into(), json!(id));
            }
        });
    });
    let mut remove = None;
    let one = rows.len() == 1;
    egui::Grid::new("efs-formats").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
        for head in ["Scale", "Suffix", "Format", ""] {
            widgets::dim_label(ui, head);
        }
        ui.end_row();
        for (i, row) in rows.iter_mut().enumerate() {
            let raster = row["format"].as_str().and_then(fileio::format).is_none_or(|f| f.raster);
            let scale = row["scale"].as_str().unwrap_or("1x").to_string();
            ui.add_enabled_ui(raster, |ui| {
                if let Some(s) = widgets::text_presets(ui, ("efs-scale", i), &scale, &SIZES, 96.0) {
                    // The suffix follows the size while it is the automatic one.
                    let auto = |text: &str| ScreenSize::parse(text).map(ScreenSize::suffix);
                    if row["suffix"].as_str().is_none_or(|x| Some(x.to_string()) == auto(&scale))
                        && let Some(suffix) = auto(&s)
                    {
                        row["suffix"] = json!(suffix);
                    }
                    row["scale"] = json!(s);
                    edited = true;
                }
            });
            if let Some(s) = widgets::text_field(ui, ("efs-suffix", i), row["suffix"].as_str(), 70.0, 1) {
                row["suffix"] = json!(s);
                edited = true;
            }
            let (format, quality) = (row["format"].as_str().unwrap_or("png"), row["quality"].as_u64());
            let current = FORMATS.iter().position(|(_, f, q)| *f == format && q.is_none_or(|q| Some(u64::from(q)) == quality));
            let label = current.map_or_else(|| format.to_uppercase(), |i| FORMAT_LABELS[i].to_string());
            if let Some((_, f, q)) = widgets::dropdown(ui, ("efs-fmt", i), &label, &FORMAT_LABELS, 96.0).and_then(|i| FORMATS.get(i)) {
                row["format"] = json!(f);
                if let Some(o) = row.as_object_mut() {
                    match q {
                        Some(q) => _ = o.insert("quality".into(), json!(q)),
                        None => _ = o.remove("quality"),
                    }
                }
                edited = true;
            }
            ui.add_enabled_ui(!one, |ui| {
                if widgets::icon_button(ui, "x", "Remove", false, 22.0).clicked() {
                    remove = Some(i);
                }
            });
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        rows.remove(i);
        edited = true;
    }
    if widgets::secondary_button(ui, "+ Add Scale").clicked() {
        // One more than the largest factor so far.
        let largest = rows
            .iter()
            .filter_map(|r| match r["scale"].as_str().and_then(ScreenSize::parse) {
                Some(ScreenSize::Scale(f)) => Some(f),
                _ => None,
            })
            .fold(0.0, f64::max);
        let size = ScreenSize::Scale(largest.floor() + 1.0);
        rows.push(json!({"format": "png", "scale": size.label(), "suffix": size.suffix()}));
        edited = true;
    }
    // Rows changed by hand are no longer a preset's.
    if edited {
        d.fields.insert("formats".into(), json!(rows));
        d.fields.insert("preset".into(), json!(""));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        widgets::dim_label(ui, "Prefix:");
        form::text(ui, d, "prefix", 140.0);
    });
}

/// Format Settings: the options of format `id` that apply to every row of it, then Done.
fn format_settings(ui: &mut egui::Ui, d: &mut Dialog, id: &str) {
    let t = Tokens::get(ui.ctx());
    widgets::subheader(ui, "Format Settings");
    ui.add_space(6.0);
    let at = SETTINGS.iter().position(|(f, _)| *f == id).unwrap_or(0);
    if let Some((f, _)) = widgets::dropdown(ui, "efs-settings-format", SETTINGS_LABELS[at], &SETTINGS_LABELS, 150.0).and_then(|i| SETTINGS.get(i)) {
        d.fields.insert("__settings".into(), json!(f));
    }
    ui.add_space(8.0);
    // The format's options as a dialog of their own, for the options dialogs' rows.
    let fields = d.fields.get("settings").and_then(|all| all.get(id)).and_then(Value::as_object).cloned().unwrap_or_default();
    let mut s = Dialog { kind: KIND.into(), fields };
    match id {
        "svg" => svg_options::option_fields(ui, &mut s, true),
        "pdf" => {
            let labels: Vec<&str> =
                d.fields.get("__presets").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("PDF Preset:").color(t.text_dim));
                if let Some(p) = widgets::dropdown(ui, "efs-pdf-preset", &s.str("preset"), &labels, 220.0).and_then(|i| labels.get(i)) {
                    s.fields.insert("preset".into(), json!(p));
                }
            });
        }
        _ => {
            egui::Grid::new("efs-settings").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| png_options::option_rows(ui, &mut s, id, true));
        }
    }
    if let Some(all) = d.fields.get_mut("settings").and_then(Value::as_object_mut) {
        all.insert(id.into(), Value::Object(s.fields));
    }
    ui.add_space(10.0);
    if widgets::secondary_button(ui, "Done").clicked() {
        d.fields.insert("__settings".into(), json!(""));
    }
}

/// A small cached rendering of artboard `i` (keyed by document revision).
fn artboard_thumb(app: &mut VectorcraftApp, ctx: &egui::Context, i: usize) -> Option<egui::TextureHandle> {
    let st = app.session.active()?;
    let key = egui::Id::new(("ab-thumb", i, st.revision));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return Some(t);
    }
    let doc = st.doc.clone();
    let r = doc.artboards.get(i)?.rect;
    let tex = crate::widgets::region_texture(ctx, &mut app.canvas.renderer, &format!("ab-thumb-{i}"), &doc, r, 92.0);
    ctx.data_mut(|d| d.insert_temp(key, tex.clone()));
    Some(tex)
}
