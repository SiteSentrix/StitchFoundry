//! Document Info panel: the document's setup and what it contains (objects by kind, fonts,
//! images, symbols, spot colours, graphic styles…), for the whole document or the selection only.

use egui::Ui;
use serde_json::{Value, json};

use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};

const OBJECT_LABELS: [(&str, &str); 16] = [
    ("paths", "Paths"),
    ("compoundPaths", "Compound Paths"),
    ("groups", "Groups"),
    ("clipGroups", "Clipping Masks"),
    ("textObjects", "Text Objects"),
    ("images", "Images"),
    ("symbolInstances", "Symbol Instances"),
    ("gradients", "Gradient Objects"),
    ("patterns", "Pattern Objects"),
    ("meshes", "Gradient Meshes"),
    ("blends", "Blends"),
    ("envelopes", "Envelopes"),
    ("repeats", "Repeats"),
    ("opacityMasks", "Opacity Masks"),
    ("liveEffects", "Objects with Effects"),
    ("guides", "Guides"),
];

fn row(ui: &mut Ui, label: &str, value: String) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(label).size(12.0).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(value).size(12.0).color(t.text));
        });
    });
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let sel_only: bool = pstate(ui.ctx(), "docinfo-sel");
    let Ok(i) = app.session.execute("document.info", &json!({ "selectionOnly": sel_only })) else {
        widgets::dim_label(ui, "No document");
        return;
    };
    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
        widgets::subheader(ui, "Document");
        let d = &i["document"];
        row(ui, "Color Mode", d["colorMode"].as_str().unwrap_or("").into());
        row(ui, "Units", d["units"].as_str().unwrap_or("").into());
        row(ui, "Raster Effects", format!("{} ppi", d["rasterEffectsPpi"]));
        for a in d["artboards"].as_array().into_iter().flatten() {
            row(
                ui,
                a["name"].as_str().unwrap_or("Artboard"),
                format!("{:.2} × {:.2}", a["width"].as_f64().unwrap_or(0.0), a["height"].as_f64().unwrap_or(0.0)),
            );
        }
        widgets::divider(ui);
        widgets::subheader(ui, if sel_only { "Objects (selection)" } else { "Objects" });
        let objs = &i["objects"];
        let mut any = false;
        for (k, label) in OBJECT_LABELS {
            if let Some(n) = objs[k].as_u64() {
                row(ui, label, n.to_string());
                any = true;
            }
        }
        if !any {
            widgets::dim_label(ui, "None");
        }
        widgets::divider(ui);
        for (key, title) in [("fonts", "Fonts"), ("symbols", "Symbols"), ("spotColors", "Spot Colors"), ("graphicStyleNames", "Graphic Styles")] {
            let list: Vec<&str> = i[key].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
            widgets::subheader(ui, &format!("{title} ({})", list.len()));
            for f in list {
                widgets::dim_label(ui, f);
            }
        }
        let imgs = i["images"].as_object().cloned().unwrap_or_default();
        widgets::subheader(ui, &format!("Images ({})", imgs.len()));
        for (_, im) in imgs {
            let kind = if im["linked"].as_bool() == Some(true) { im["link"].as_str().unwrap_or("Linked").to_string() } else { "Embedded".into() };
            widgets::dim_label(ui, &format!("{} × {} px — {kind}", im["width"], im["height"]));
        }
        widgets::divider(ui);
        for (key, label) in [
            ("swatches", "Swatches"),
            ("characterStyles", "Character Styles"),
            ("paragraphStyles", "Paragraph Styles"),
            ("patterns", "Pattern Swatches"),
        ] {
            row(ui, label, i[key].to_string());
        }
    });
}

pub fn menu(_app: &mut VectorcraftApp, ui: &mut Ui) {
    let sel: bool = pstate(ui.ctx(), "docinfo-sel");
    if menu_item(ui, "Selection Only", true, sel) {
        set_pstate(ui.ctx(), "docinfo-sel", !sel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_headless() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("text.create", &json!({"x": 10, "y": 40, "text": "Hi"})).unwrap();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(&mut app, ui);
            menu(&mut app, ui);
        });
        out.textures_delta.clear();
    }
}
