//! File → New: presets and the document details (name, size, units, artboards, colour mode).
//!
//! Fields: `preset`, `name`, `width` and `height` (points, or strings with a unit), `units` (a unit
//! label; print presets start in Preferences ▸ Units ▸ General), `artboards`, `colorMode`.

use serde_json::{Value, json};
use vectorcraft_doc::Unit;

use super::{DialogSpec, form};
use crate::state::Dialog;
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, widgets};

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| "New Document".into(), body, confirm, ok: Some("Create"), min_width: 560.0, ..DialogSpec::FORM };

/// The presets: (name, width, height in points, in pixels).
const PRESETS: [(&str, f64, f64, bool); 8] = [
    ("Letter", 612.0, 792.0, false),
    ("Legal", 612.0, 1008.0, false),
    ("Tabloid", 792.0, 1224.0, false),
    ("A4", 595.28, 841.89, false),
    ("A3", 841.89, 1190.55, false),
    ("Web 1920", 1920.0, 1080.0, true),
    ("Phone 390×844", 390.0, 844.0, true),
    ("Square Post", 1080.0, 1080.0, true),
];

/// The units a preset starts in: pixels for screen presets, else the General preference.
fn preset_units(app: &VectorcraftApp, pixels: bool) -> Unit {
    if pixels { Unit::Pixels } else { app.session.default_units() }
}

/// Open the dialog on the Letter preset.
pub fn open(app: &mut VectorcraftApp) {
    let units = preset_units(app, false).label();
    let fields = json!({"preset": "Letter", "width": 612, "height": 792, "units": units, "artboards": 1, "colorMode": "RGB", "name": "Untitled-1"});
    app.ui.dialog = Some(Dialog::new("newDocument", fields));
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            ui.label(egui::RichText::new("Presets").color(t.text_dim));
            ui.horizontal_wrapped(|ui| {
                for (name, w, h, pixels) in PRESETS {
                    let sel = d.str("preset") == name;
                    if ui.selectable_label(sel, name).clicked() {
                        d.fields.insert("preset".into(), json!(name));
                        d.fields.insert("width".into(), json!(w));
                        d.fields.insert("height".into(), json!(h));
                        d.fields.insert("units".into(), json!(preset_units(app, pixels).label()));
                    }
                }
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Preset Details").font(theme::semibold(12.5)));
            let unit = Unit::named(&d.str("units")).unwrap_or_default();
            egui::Grid::new("newdoc").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                form::field(ui, d, "name", "Name:");
                form::length_field(ui, d, "width", "Width:", unit);
                form::length_field(ui, d, "height", "Height:", unit);
                ui.label(egui::RichText::new("Units:").color(t.text_dim));
                let labels: Vec<&str> = Unit::ALL.iter().map(|u| u.label()).collect();
                if let Some(i) = widgets::dropdown(ui, "newdoc-units", unit.label(), &labels, 132.0) {
                    d.fields.insert("units".into(), json!(labels[i]));
                }
                ui.end_row();
                form::field(ui, d, "artboards", "Artboards:");
                form::field(ui, d, "colorMode", "Color Mode:");
            });
        });
    });
    false
}

/// Create the document; the dialog stays open when that fails (bad size, …).
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let r = app.run(
        "file.new",
        json!({"width": d.f64("width", 612.0), "height": d.f64("height", 792.0), "units": d.str("units"), "title": d.str("name"), "artboards": d.f64("artboards", 1.0), "colorMode": d.str("colorMode").to_lowercase()}),
    );
    if r.is_ok() {
        app.ui.dialog = None;
    }
    r
}
