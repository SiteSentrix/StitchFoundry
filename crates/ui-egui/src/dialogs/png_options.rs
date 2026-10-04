//! The raster export options: PNG Options (resolution, background, anti-aliasing, interlaced),
//! JPEG Options (quality instead of interlacing; no transparency) and WebP Options. Fields are
//! `document.export` params (format, path, artboard choice…); `__`-prefixed ones are the dialog's.

use serde::Deserialize;
use serde_json::{Value, json};
use vectorcraft_engine::cmd::fileio::{self, ArtboardPick, Format};
use vectorcraft_render::AntiAlias;

use super::{DialogSpec, form};
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, io, widgets};

pub(super) const SPEC: DialogSpec = DialogSpec { heading, body, confirm, ok: Some("Export"), min_width: 360.0, ..DialogSpec::FORM };

/// Resolution presets (pixels per inch).
const RESOLUTIONS: [f64; 3] = [72.0, 150.0, 300.0];
/// Their labels, then `Other` (any resolution).
const RESOLUTION_LABELS: [&str; 4] = ["Screen (72 ppi)", "Medium (150 ppi)", "High (300 ppi)", "Other"];
/// Background param values.
const BACKGROUNDS: [&str; 3] = ["transparent", "white", "black"];
/// Their labels, then `Other` (a colour).
const BACKGROUND_LABELS: [&str; 4] = ["Transparent", "White", "Black", "Other"];

/// The dialog kind of a raster format (`pngOptions`, `jpgOptions`, `webpOptions`).
fn kind(f: &Format) -> String {
    format!("{}Options", f.id)
}

fn format_of(d: &Dialog) -> Option<&'static Format> {
    fileio::format(d.kind.strip_suffix("Options")?)
}

/// Open the options of raster format `f` for an export whose params (format, path, artboard
/// choice) are `params`.
pub fn open(app: &mut VectorcraftApp, f: &Format, mut params: Value) {
    let size = app.session.active().and_then(|st| export_size(&st.doc, &params));
    // The document's background (New Document → Background Contents) is the default; JPEG has no alpha.
    let white = f.id == "jpg" || app.session.active().is_some_and(|st| st.doc.setup.background == vectorcraft_doc::Background::White);
    if let Some(o) = params.as_object_mut() {
        o.insert("ppi".into(), json!(72));
        o.insert("background".into(), json!(if white { "white" } else { "transparent" }));
        o.insert("antiAlias".into(), json!(AntiAlias::default().id()));
        match f.id {
            "png" => o.insert("interlaced".into(), json!(false)),
            "jpg" => o.insert("quality".into(), json!(90)),
            _ => None,
        };
        if let Some((w, h)) = size {
            o.insert("__size".into(), json!([w, h]));
        }
    }
    app.ui.dialog = Some(Dialog::new(&kind(f), params));
}

/// The size in points of the region an export covers: the chosen artboard.
fn export_size(doc: &vectorcraft_doc::Document, p: &Value) -> Option<(f64, f64)> {
    let r = doc.artboards.get(ArtboardPick::deserialize(p).ok()?.one(doc.artboards.len()).ok()?)?.rect;
    Some((r.width(), r.height()))
}

fn heading(d: &Dialog) -> String {
    format!("{} Options", format_of(d).map_or("Export", |f| f.label))
}

fn body(_: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let id = format_of(d).map_or("png", |f| f.id);
    let label = |ui: &mut egui::Ui, text: &str| ui.label(egui::RichText::new(text).color(t.text_dim));
    egui::Grid::new("raster-options").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
        label(ui, "Resolution:");
        let ppi = d.f64("ppi", 72.0);
        let preset = RESOLUTIONS.iter().position(|p| *p == ppi).filter(|_| !d.bool("__otherPpi"));
        ui.horizontal(|ui| {
            if let Some(i) = widgets::dropdown(ui, "ro-ppi", RESOLUTION_LABELS[preset.unwrap_or(3)], &RESOLUTION_LABELS, 150.0) {
                if let Some(p) = RESOLUTIONS.get(i) {
                    d.fields.insert("ppi".into(), json!(p));
                }
                d.fields.insert("__otherPpi".into(), json!(i == 3));
            }
            if preset.is_none()
                && let Some(v) = widgets::plain_field(ui, "ro-ppi-other", ppi, " ppi", 0, 80.0)
            {
                d.fields.insert("ppi".into(), json!(v.round().clamp(1.0, 4608.0)));
            }
        });
        ui.end_row();

        label(ui, "Background Color:");
        let bg = d.str("background");
        let choice = BACKGROUNDS.iter().position(|v| bg.eq_ignore_ascii_case(v)).unwrap_or(3);
        // JPEG has no transparency: its list starts at White.
        let first = usize::from(id == "jpg");
        ui.horizontal(|ui| {
            if let Some(i) = widgets::dropdown(ui, "ro-bg", BACKGROUND_LABELS[choice], &BACKGROUND_LABELS[first..], 150.0) {
                let value = BACKGROUNDS.get(i + first).copied().unwrap_or("#808080");
                d.fields.insert("background".into(), json!(value));
            }
            if choice == 3 {
                let c = vectorcraft_color::Color::from_hex(&bg).map_or([128, 128, 128, 255], |c| c.to_rgba8(1.0));
                let mut rgb = [c[0], c[1], c[2]];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    d.fields.insert("background".into(), json!(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])));
                }
            }
        });
        ui.end_row();

        label(ui, "Anti-aliasing:");
        let aa = AntiAlias::from_id(&d.str("antiAlias")).unwrap_or_default();
        if let Some(i) = widgets::dropdown(ui, "ro-aa", aa.label(), &AntiAlias::ALL.map(AntiAlias::label), 150.0) {
            d.fields.insert("antiAlias".into(), json!(AntiAlias::ALL[i].id()));
        }
        ui.end_row();

        match id {
            "png" => {
                ui.label("");
                form::check(ui, d, "interlaced", "Interlaced");
                ui.end_row();
            }
            "jpg" => {
                label(ui, "Quality:");
                let mut q = d.f64("quality", 90.0).round() as u8;
                if ui.add(egui::Slider::new(&mut q, 1..=100)).changed() {
                    d.fields.insert("quality".into(), json!(q));
                }
                ui.end_row();
            }
            _ => {}
        }

        if let Some([w, h]) =
            d.fields.get("__size").and_then(Value::as_array).map(|a| [0, 1].map(|i| a.get(i).and_then(Value::as_f64).unwrap_or(0.0)))
        {
            label(ui, "Size:");
            let (pw, ph) = vectorcraft_render::region_pixels(vectorcraft_geom::Rect::new(0.0, 0.0, w, h), ppi / 72.0);
            ui.label(egui::RichText::new(format!("{pw} × {ph} px")).color(t.text));
            ui.end_row();
        }
    });
    false
}

/// Write the file(s) with the chosen options.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let (format, path) = (d.str("format"), d.str("path"));
    app.ui.dialog = None;
    io::export(app, Some(&format), Some(path), &form::params(d)).map(|path| json!({"path": path}))
}
