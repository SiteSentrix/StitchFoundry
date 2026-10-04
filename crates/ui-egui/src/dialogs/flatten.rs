//! Object → Flatten Transparency: a preset, the raster/vector balance, the resolutions and the
//! outline and preserve options, previewed live on the canvas (off at first: flattening complex
//! art takes a while); OK keeps the result as one undo step (`object.flattenTransparency`).
//!
//! Fields: `preset` (a preset's name: setting it loads its options), the option keys of
//! [`FlattenOptions`] (`balance`, `lineArtPpi`, `gradientPpi`, `textToOutlines`,
//! `strokesToOutlines`, `clipComplexRegions`, `antiAlias`, `preserveAlpha`, `preserveOverprints`)
//! and `preview`.

use serde_json::{Map, Value, json};
use vectorcraft_engine::cmd::FlattenOptions;

use super::{DialogSpec, form};
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, widgets};

/// The dialog kind of Flatten Transparency.
pub const KIND: &str = "flattenTransparency";

const CMD: &str = "object.flattenTransparency";

/// The preset whose options the fields hold (a `preset` set from outside loads its options).
const APPLIED: &str = "__applied";

/// What the preset dropdown shows once the options differ from the preset's.
const CUSTOM: &str = "[Custom]";

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| "Flatten Transparency".into(), body, confirm, preview: true, min_width: 420.0, ..DialogSpec::FORM };

/// Open Flatten Transparency for the selection with the default preset.
pub fn open(app: &mut VectorcraftApp) {
    let name = FlattenOptions::preset_label(FlattenOptions::PRESETS[1]).unwrap_or_default();
    let mut d = Dialog::new(KIND, json!({ "preset": name, APPLIED: name, "preview": false }));
    put_options(&mut d.fields, &FlattenOptions::default());
    app.ui.dialog = Some(d);
}

/// The presets the dropdown offers: (name, options).
fn presets() -> Vec<(String, FlattenOptions)> {
    FlattenOptions::PRESETS.iter().filter_map(|id| Some((FlattenOptions::preset_label(id)?.to_string(), FlattenOptions::preset(id)?))).collect()
}

/// Write `o` into the option fields.
pub(crate) fn put_options(fields: &mut Map<String, Value>, o: &FlattenOptions) {
    if let Value::Object(m) = serde_json::to_value(o).unwrap_or_default() {
        fields.extend(m);
    }
}

/// The options the fields hold (a `preset` not loaded yet: its options, adjusted by none).
fn options(d: &Dialog) -> Result<FlattenOptions, String> {
    if d.str("preset") != d.str(APPLIED) {
        return FlattenOptions::from_params(&json!({ "preset": d.str("preset") }));
    }
    FlattenOptions::from_params(&Value::Object(d.fields.clone()))
}

/// Load the options of a preset set from outside (`ui.dialog.set`).
fn sync(d: &mut Dialog) {
    let name = d.str("preset");
    if name != d.str(APPLIED) {
        if let Ok(o) = options(d) {
            put_options(&mut d.fields, &o);
        }
        d.fields.insert(APPLIED.into(), json!(name));
    }
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    sync(d);
    let presets = presets();
    let current = options(d);
    let shown = match &current {
        Ok(o) => presets.iter().find(|(n, p)| *n == d.str("preset") && p == o).map_or(CUSTOM, |(n, _)| n.as_str()),
        Err(_) => CUSTOM,
    };
    ui.horizontal(|ui| {
        widgets::dim_label(ui, "Preset:");
        let names: Vec<&str> = presets.iter().map(|(n, _)| n.as_str()).collect();
        if let Some(i) = widgets::dropdown(ui, "flatten-preset", shown, &names, 220.0) {
            let (name, o) = &presets[i];
            put_options(&mut d.fields, o);
            d.fields.insert("preset".into(), json!(name));
            d.fields.insert(APPLIED.into(), json!(name));
        }
    });
    ui.add_space(10.0);
    let mut o = current.clone().unwrap_or_default();
    if options_editor(ui, "flatten-dialog", &mut o, true) {
        put_options(&mut d.fields, &o);
    }
    if let Err(e) = &current {
        ui.label(egui::RichText::new(e).color(Tokens::get(ui.ctx()).text_dim).size(11.5));
    }
    let p = params(d);
    form::preview(app, ui, d, "Flatten Transparency", CMD, p);
    false
}

/// `object.flattenTransparency` parameters: every option.
fn params(d: &Dialog) -> Value {
    let mut fields = Map::new();
    put_options(&mut fields, &options(d).unwrap_or_default());
    Value::Object(fields)
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let o = options(d)?;
    let mut fields = Map::new();
    put_options(&mut fields, &o);
    form::commit_preview(app, CMD, Value::Object(fields))
}

/// The width of the option value fields.
const FIELD: f32 = 56.0;

/// The flattener options as Flatten Transparency, the preset manager and the Flattener Preview
/// panel edit them: the raster/vector balance, both resolutions and the six switches. Returns true
/// when one changed.
pub(crate) fn options_editor(ui: &mut egui::Ui, id: &str, o: &mut FlattenOptions, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let before = o.clone();
    let dim = |ui: &mut egui::Ui, s: &str| {
        ui.label(egui::RichText::new(s).color(t.text_dim).size(11.5));
    };
    ui.add_enabled_ui(enabled, |ui| {
        widgets::dim_label(ui, "Raster/Vector Balance:");
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            dim(ui, "Rasters");
            let rail = (ui.available_width() - FIELD - 56.0).clamp(80.0, 200.0);
            if let (Some(x), _) = widgets::color_slider(ui, (id, "balance-slider"), (o.balance / 100.0) as f32, rail, &|_| t.input_border) {
                o.balance = (x as f64 * 100.0).round();
            }
            dim(ui, "Vectors");
            if let Some(v) = widgets::plain_field(ui, (id, "balance"), o.balance, "", 0, FIELD - 12.0) {
                o.balance = v.round().clamp(0.0, 100.0);
            }
        });
        ui.add_space(4.0);
        egui::Grid::new((id, "resolutions")).num_columns(2).spacing([8.0, 4.0]).show(ui, |ui| {
            for (label, key) in [("Line Art and Text Resolution:", 0), ("Gradient and Mesh Resolution:", 1)] {
                widgets::dim_label(ui, label);
                ui.horizontal(|ui| {
                    let v = if key == 0 { &mut o.line_art_ppi } else { &mut o.gradient_ppi };
                    if let Some(n) = widgets::plain_field(ui, (id, key), *v, "", 0, FIELD) {
                        *v = n.round().clamp(1.0, 2400.0);
                    }
                    dim(ui, "ppi");
                });
                ui.end_row();
            }
        });
        ui.add_space(4.0);
        let switches: [(&str, &mut bool); 6] = [
            ("Convert All Text to Outlines", &mut o.text_to_outlines),
            ("Convert All Strokes to Outlines", &mut o.strokes_to_outlines),
            ("Clip Complex Regions", &mut o.clip_complex_regions),
            ("Anti-alias Rasters", &mut o.anti_alias),
            ("Preserve Alpha Transparency", &mut o.preserve_alpha),
            ("Preserve Overprints and Spot Colors", &mut o.preserve_overprints),
        ];
        for (label, v) in switches {
            if widgets::check(ui, label, *v, enabled) {
                *v = !*v;
            }
        }
    });
    *o != before
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;

    fn frame(app: &mut VectorcraftApp) {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| super::super::show(app, ui.ctx()));
        out.textures_delta.clear();
    }

    /// A document with a half-transparent red square over a blue one, both selected.
    fn scene() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        for (x, color) in [(10, "#0000ff"), (60, "#ff0000")] {
            let id = app.run("shape.rectangle", json!({"x": x, "y": x, "width": 100, "height": 100})).unwrap()["id"].clone();
            app.run("paint.setFill", json!({"color": color, "ids": [id]})).unwrap();
            app.run("paint.setStroke", json!({"none": true, "ids": [id]})).unwrap();
        }
        app.run("transparency.set", json!({"opacity": 50})).unwrap();
        app.run("select.all", json!({})).unwrap();
        app
    }

    fn transparent_left(app: &VectorcraftApp) -> bool {
        let mut any = false;
        for l in &app.session.doc().unwrap().doc.layers {
            l.walk(&mut |n| any |= n.opacity < 1.0 && !n.is_container());
        }
        any
    }

    #[test]
    fn cancel_rolls_the_preview_back_and_ok_flattens_as_one_step() {
        let mut app = scene();
        let before = app.session.doc().unwrap().doc.clone();
        let undo = app.session.doc().unwrap().history.undo.len();
        app.run("ui.flattenTransparencyDialog", json!({})).unwrap();
        assert_eq!(app.ui.dialog.as_ref().map(|d| d.kind.as_str()), Some(KIND));
        frame(&mut app);
        assert!(!app.session.in_interaction(), "no preview until it is turned on");
        app.ui.dialog.as_mut().unwrap().fields.insert("preview".into(), json!(true));
        frame(&mut app);
        assert!(app.session.in_interaction() && !transparent_left(&app), "previewed on the canvas");
        super::super::cancel(&mut app);
        assert!(app.ui.dialog.is_none() && !app.session.in_interaction());
        assert_eq!(app.session.doc().unwrap().doc, before, "Cancel rolls back");
        // OK without a preview flattens with the dialog's options as one undo step.
        app.run("ui.flattenTransparencyDialog", json!({})).unwrap();
        let d = app.ui.dialog.as_mut().unwrap();
        d.fields.insert("preset".into(), json!("High Resolution"));
        frame(&mut app);
        assert_eq!(app.ui.dialog.as_ref().unwrap().f64("lineArtPpi", 0.0), 1200.0, "a preset set from outside loads");
        app.ui.dialog.as_mut().unwrap().fields.insert("balance".into(), json!(0));
        super::super::confirm(&mut app).unwrap();
        assert!(app.ui.dialog.is_none());
        let st = app.session.doc().unwrap();
        assert_eq!(st.history.undo.len(), undo + 1);
        let mut images = 0;
        st.doc.layers[0].walk(&mut |n| images += matches!(n.kind, vectorcraft_doc::NodeKind::Image(_)) as usize);
        assert_eq!(images, 1, "balance 0 rasterizes");
        assert!(!transparent_left(&app));
    }

    #[test]
    fn bad_values_report_and_keep_the_dialog_open() {
        let mut app = scene();
        app.run("ui.flattenTransparencyDialog", json!({})).unwrap();
        app.ui.dialog.as_mut().unwrap().fields.insert("gradientPpi".into(), json!(9000));
        frame(&mut app);
        assert!(super::super::confirm(&mut app).unwrap_err().contains("resolutions"));
        assert!(app.ui.dialog.is_some());
    }
}
