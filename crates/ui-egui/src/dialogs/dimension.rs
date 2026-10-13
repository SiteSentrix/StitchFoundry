//! Dimension options and compact mode strip. Values are drafts until Save.
use super::DialogSpec;
use crate::{VectorcraftApp, state::Dialog, theme::Tokens, widgets};
use egui::{RichText, Ui};
use serde_json::{Value, json};
use vectorcraft_doc::dimension::DimensionOptions;

pub const KIND: &str = "dimensionOptions";
pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |_| tl!("Dimension Tool Options").into(),
    body,
    confirm,
    ok: Some("Save"),
    min_width: 520.0,
    max_width: Some(560.0),
    ..DialogSpec::FORM
};

pub fn open(app: &mut VectorcraftApp) -> Result<Value, String> {
    if app.session.tool_id() == "dimension" && app.session.tool_busy() {
        app.run("tool.select", json!({"tool":"selection"}))?;
        app.run("tool.select", json!({"tool":"dimension"}))?;
    }
    let selected = app
        .session
        .active()
        .and_then(|st| st.selection.objects.first().and_then(|id| st.doc.node(*id)))
        .and_then(|n| n.extras().dimension.as_deref());
    let style = if let Some(d) = selected {
        serde_json::to_value(&d.options).map_err(|e| e.to_string())?
    } else {
        app.session.tool_options_of("dimension")["style"].clone()
    };
    let o: DimensionOptions = serde_json::from_value(style).unwrap_or_default();
    let mut fields = serde_json::to_value(o).map_err(|e| e.to_string())?.as_object().cloned().unwrap_or_default();
    fields.insert("applySelected".into(), json!(selected.is_some()));
    app.ui.dialog = Some(Dialog { kind: KIND.into(), fields });
    Ok(json!({"dialog":KIND}))
}

fn read(d: &Dialog) -> Result<DimensionOptions, String> {
    let mut fields = d.fields.clone();
    fields.remove("applySelected");
    let o: DimensionOptions = serde_json::from_value(Value::Object(fields)).map_err(|e| e.to_string())?;
    o.validate()?;
    Ok(o)
}

fn choose(ui: &mut Ui, d: &mut Dialog, key: &str, label: &str, choices: &[(&str, &str)]) {
    widgets::label_row(ui, tl!(label), 96.0, |ui| {
        let current = d.str(key);
        let text = choices.iter().find(|(v, _)| *v == current).map_or(current.as_str(), |(_, label)| *label);
        if let Some(index) =
            widgets::dropdown_names(ui, ("dimension", key), tl!(text), &choices.iter().map(|(_, s)| tl!(*s)).collect::<Vec<_>>(), 134.0)
            && let Some((value, _)) = choices.get(index)
        {
            d.fields.insert(key.into(), json!(value));
        }
    });
}
fn number(ui: &mut Ui, d: &mut Dialog, key: &str, label: &str, range: (f64, f64), suffix: &str) {
    widgets::label_row(ui, tl!(label), 96.0, |ui| {
        if let Some(v) = widgets::plain_field(ui, ("dimension", key), d.f64(key, 0.0), suffix, 2, 90.0) {
            d.fields.insert(key.into(), json!(v.clamp(range.0, range.1)));
        }
    });
}
fn check(ui: &mut Ui, d: &mut Dialog, key: &str, label: &str) {
    let mut value = d.bool(key);
    if ui.checkbox(&mut value, tl!(label)).changed() {
        d.fields.insert(key.into(), json!(value));
    }
}
fn color(ui: &mut Ui, d: &mut Dialog, key: &str) {
    let mut value: [u8; 3] = serde_json::from_value(d.fields.get(key).cloned().unwrap_or(Value::Null)).unwrap_or([224, 55, 65]);
    ui.push_id(key, |ui| {
        if ui.color_edit_button_srgb(&mut value).changed() {
            d.fields.insert(key.into(), json!(value));
        }
    });
}
fn section(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.group(|ui| {
        ui.set_min_width(486.0);
        ui.label(RichText::new(tl!(title)).color(t.text_dim).strong());
        ui.add_space(5.0);
        body(ui);
    });
    ui.add_space(7.0);
}
const LINE_TYPES: &[(&str, &str)] = &[("solid", "Solid"), ("dashed", "Dashed"), ("dotted", "Dotted")];

fn body(app: &mut VectorcraftApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    section(ui, "Units & Scale", |ui| {
        ui.horizontal(|ui| {
            choose(
                ui,
                d,
                "units",
                "Units:",
                &[
                    ("mm", "Millimeters"),
                    ("cm", "Centimeters"),
                    ("m", "Meters"),
                    ("in", "Inches"),
                    ("ft", "Feet"),
                    ("yd", "Yards"),
                    ("pt", "Points"),
                    ("px", "Pixels"),
                ],
            );
            ui.label(tl!("Precision:"));
            let mut value = d.f64("precision", 1.0) as u32;
            egui::ComboBox::from_id_salt("dimension-precision").selected_text(value.to_string()).width(55.0).show_ui(ui, |ui| {
                for n in 0..=6 {
                    ui.selectable_value(&mut value, n, n.to_string());
                }
            });
            d.fields.insert("precision".into(), json!(value));
        });
        ui.horizontal(|ui| {
            widgets::label_row(ui, tl!("Scale:"), 96.0, |ui| {
                let mut scale = d.str("scale");
                if ui.add(egui::TextEdit::singleline(&mut scale).desired_width(72.0)).changed() {
                    d.fields.insert("scale".into(), json!(scale));
                }
                egui::ComboBox::from_id_salt("dimension-scales").selected_text(tl!("Presets")).width(76.0).show_ui(ui, |ui| {
                    for scale in ["1:1", "1:2", "1:5", "1:10", "1:20", "1:50", "1:100", "1:200", "2:1", "5:1", "10:1"] {
                        if ui.selectable_label(false, scale).clicked() {
                            d.fields.insert("scale".into(), json!(scale));
                        }
                    }
                });
            });
            check(ui, d, "hideUnits", "Hide Units");
        });
        ui.label(RichText::new(tl!("Drawing : real size — 1:10 means 1 cm on the page represents 10 cm.")).small());
    });
    section(ui, "Dimension Lines", |ui| {
        ui.horizontal(|ui| {
            number(ui, d, "lineWeight", "Line Weight:", (0.1, 100.0), "pt");
            choose(ui, d, "lineType", "Line Type:", LINE_TYPES);
            color(ui, d, "lineColor");
        });
        ui.horizontal(|ui| {
            choose(ui, d, "arrowStyle", "Arrow Style:", &[("filled", "Filled"), ("open", "Open"), ("tick", "Tick"), ("none", "None")]);
            number(ui, d, "arrowScale", "Arrow Scale:", (1.0, 1000.0), "%");
        });
    });
    section(ui, "Extension Lines", |ui| {
        ui.horizontal(|ui| {
            number(ui, d, "extensionWeight", "Line Weight:", (0.1, 100.0), "pt");
            choose(ui, d, "extensionType", "Line Type:", LINE_TYPES);
            color(ui, d, "extensionColor");
        });
        ui.horizontal(|ui| {
            number(ui, d, "extensionOffset", "Offset:", (0.0, 10000.0), "pt");
            number(ui, d, "extensionLength", "Extension:", (0.0, 10000.0), "pt");
        });
        check(ui, d, "hideExtensions", "Hide Extension Lines");
    });
    section(ui, "Dimension Text", |ui| {
        widgets::label_row(ui, tl!("Font Family:"), 96.0, |ui| {
            if let Some(crate::font_menu::FontPick::Chosen(family, style)) =
                crate::font_menu::font_menu(ui, "dimension-font", &d.str("fontFamily"), 280.0, Some("123.45 cm"), crate::font_menu::MenuLook::of(app))
            {
                d.fields.insert("fontFamily".into(), json!(family));
                d.fields.insert("fontStyle".into(), json!(style.unwrap_or_else(|| "Regular".into())));
            }
        });
        widgets::label_row(ui, tl!("Font Style:"), 96.0, |ui| {
            let styles = vectorcraft_text::FontDb::global().styles(&d.str("fontFamily"));
            let names: Vec<&str> = styles.iter().map(|s| s.as_str()).collect();
            if let Some(i) = widgets::dropdown_names(ui, "dimension-font-style", &d.str("fontStyle"), &names, 180.0)
                && let Some(style) = styles.get(i)
            {
                d.fields.insert("fontStyle".into(), json!(style));
            }
        });
        ui.horizontal(|ui| {
            number(ui, d, "fontSize", "Font Size:", (1.0, 1000.0), "pt");
            color(ui, d, "textColor");
        });
        ui.horizontal(|ui| {
            choose(ui, d, "textPosition", "Position:", &[("above", "Above"), ("center", "Centered"), ("below", "Below")]);
            number(ui, d, "textOffset", "Offset:", (0.0, 10000.0), "pt");
        });
        choose(ui, d, "textAlignment", "Orientation:", &[("aligned", "Along dimension"), ("horizontal", "Horizontal")]);
    });
    match read(d) {
        Ok(o) => {
            let sample = o.label(72.0 / 2.54, false).unwrap_or_default();
            let t = Tokens::get(ui.ctx());
            ui.label(RichText::new(format!("{}: {sample}", tl!("Preview (1 cm on page)"))).color(t.text));
        }
        Err(e) => {
            ui.colored_label(Tokens::get(ui.ctx()).text, e);
        }
    }
    ui.horizontal(|ui| {
        if ui.button(tl!("Reset")).clicked()
            && let Ok(Value::Object(fields)) = serde_json::to_value(DimensionOptions::default())
        {
            let apply = d.bool("applySelected");
            d.fields = fields;
            d.fields.insert("applySelected".into(), json!(apply));
        }
        check(ui, d, "applySelected", "Apply to selected dimensions");
    });
    false
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let o = read(d)?;
    if d.bool("applySelected") {
        let ids: Vec<_> = app
            .session
            .active()
            .map(|s| s.selection.objects.iter().filter(|id| s.doc.node(**id).is_some_and(|n| n.extras().dimension.is_some())).copied().collect())
            .unwrap_or_default();
        if !ids.is_empty() {
            app.run("dimension.update", json!({"ids":ids,"options":o}))?;
        }
    }
    let r = app.run("tool.setOption", json!({"tool":"dimension","values":{"style":o}}))?;
    app.ui.dialog = None;
    Ok(r)
}

pub fn controls(app: &mut VectorcraftApp, ui: &mut Ui) {
    let mode = app.session.tool_options()["mode"].as_str().unwrap_or("linear").to_owned();
    for (id, label) in [("linear", "Length"), ("angular", "Angle"), ("radius", "Radius"), ("diameter", "Diameter")] {
        if ui.selectable_label(mode == id, tl!(label)).clicked() {
            // Switching away cancels previews and unfinished point selections.
            if let Err(e) = app
                .run("tool.select", json!({"tool":"selection"}))
                .and_then(|_| app.run("tool.select", json!({"tool":"dimension"})))
                .and_then(|_| app.run("tool.setOption", json!({"tool":"dimension","values":{"mode":id}})))
            {
                app.ui.status = e;
            }
        }
    }
    if ui.button(tl!("Options…")).clicked()
        && let Err(e) = open(app)
    {
        app.ui.status = e;
    }
    if ui.button(tl!("All selected edges")).clicked()
        && let Err(e) = app.run("dimension.selection", json!({}))
    {
        app.ui.status = e;
    }
}

/// Always available while measuring, including workspaces with the main Control bar hidden.
pub fn floating(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if app.session.tool_id() != "dimension" || app.ui.dialog.is_some() {
        return;
    }
    egui::Window::new(tl!("Dimensions"))
        .id(egui::Id::new("dimension-strip"))
        .default_pos(egui::pos2(90.0, 120.0))
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.horizontal(|ui| controls(app, ui));
            let mode = app.session.tool_options()["mode"].as_str().unwrap_or("linear").to_owned();
            let hint = match mode.as_str() {
                "angular" => "Click two straight edges, then place the angle.",
                "radius" | "diameter" => "Click a circle, then click to choose the label direction.",
                _ => "Click an edge, then place the dimension. Alt-click picks individual anchors.",
            };
            ui.label(RichText::new(tl!(hint)).small().color(Tokens::get(ctx).text_dim));
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dimension_dialog_fields_save_and_cancel() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("tool.options", json!({"tool":"dimension"})).unwrap();
        let text = crate::tests_labels::painted_text(&mut app, |app, ui| super::super::show(app, ui.ctx()));
        for label in ["Dimension Tool Options", "Units & Scale", "Dimension Lines", "Extension Lines", "Dimension Text", "Save"] {
            assert!(text.contains(label), "{label}: {text}");
        }
        let d = app.ui.dialog.as_mut().unwrap();
        d.fields.insert("scale".into(), json!("1:10"));
        super::super::confirm(&mut app).unwrap();
        assert_eq!(app.session.tool_options_of("dimension")["style"]["scale"], "1:10");
        open(&mut app).unwrap();
        app.ui.dialog.as_mut().unwrap().fields.insert("scale".into(), json!("1:0"));
        assert!(super::super::confirm(&mut app).is_err());
        app.ui.dialog = None;
        assert_eq!(app.session.tool_options_of("dimension")["style"]["scale"], "1:10");
    }
}
