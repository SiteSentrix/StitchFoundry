//! File → New: presets and the document details (name, size, artboards, colour mode).

use serde_json::{Value, json};

use super::{DialogSpec, form};
use crate::VectorcraftApp;
use crate::state::Dialog;
use crate::theme::{self, Tokens};

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| "New Document".into(), body, confirm, ok: Some("Create"), min_width: 560.0, ..DialogSpec::FORM };

fn body(_: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let presets: [(&str, &str, &str, &str); 8] = [
        ("Letter", "612 pt", "792 pt", "Points"),
        ("Legal", "612 pt", "1008 pt", "Points"),
        ("Tabloid", "792 pt", "1224 pt", "Points"),
        ("A4", "595.28 pt", "841.89 pt", "Points"),
        ("A3", "841.89 pt", "1190.55 pt", "Points"),
        ("Web 1920", "1920 px", "1080 px", "Pixels"),
        ("Phone 390×844", "390 px", "844 px", "Pixels"),
        ("Square Post", "1080 px", "1080 px", "Pixels"),
    ];
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            ui.label(egui::RichText::new("Presets").color(t.text_dim));
            ui.horizontal_wrapped(|ui| {
                for (name, w, h, u) in presets {
                    let sel = d.str("preset") == name;
                    if ui.selectable_label(sel, name).clicked() {
                        d.fields.insert("preset".into(), json!(name));
                        d.fields.insert("width".into(), json!(w));
                        d.fields.insert("height".into(), json!(h));
                        d.fields.insert("units".into(), json!(u));
                    }
                }
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Preset Details").font(theme::semibold(12.5)));
            egui::Grid::new("newdoc").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                form::field(ui, d, "name", "Name:");
                form::field(ui, d, "width", "Width:");
                form::field(ui, d, "height", "Height:");
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
