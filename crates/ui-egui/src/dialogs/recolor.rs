//! Edit → Edit Colors → Recolor Artwork: current → new colour pairs, previewed live.

use serde_json::{Value, json};

use super::DialogSpec;
use crate::VectorcraftApp;
use crate::state::Dialog;
use crate::theme::Tokens;

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |_| "Recolor Artwork".into(), body, confirm, preview: true, ..DialogSpec::FORM };

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    if editor(ui, d) || !app.session.in_interaction() {
        let _ = app.session.begin_interaction("Recolor Artwork");
        let _ = app.session.preview("recolor.apply", &json!({"map": recolor_map(d)}));
    }
    false
}

/// Keep the previewed recolour as one undo step (or apply it when no preview is running).
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let map = recolor_map(d);
    app.ui.dialog = None;
    if app.session.in_interaction() {
        let _ = app.session.preview("recolor.apply", &json!({"map": map}));
        return app.session.commit_interaction().map(|_| Value::Null).map_err(|e| e.to_string());
    }
    app.run("recolor.apply", json!({"map": map}))
}

fn recolor_map(d: &Dialog) -> Value {
    let mut m = serde_json::Map::new();
    for p in d.fields.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default() {
        if let (Some(a), Some(b)) = (p.get(0).and_then(Value::as_str), p.get(1).and_then(Value::as_str)) {
            m.insert(a.to_string(), json!(b));
        }
    }
    Value::Object(m)
}

/// Current → new colour rows, harmony rules, randomize. Returns true when changed.
fn editor(ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    use vectorcraft_color::Color;
    let t = Tokens::get(ui.ctx());
    let mut pairs: Vec<Value> = d.fields.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut changed = false;
    let chip = |ui: &mut egui::Ui, hex: &str| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(34.0, 22.0), egui::Sense::hover());
        let c = Color::from_hex(hex).unwrap_or(Color::BLACK).to_rgba8(1.0);
        ui.painter().rect_filled(r, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
        ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Current Colors").color(t.text_dim));
        ui.add_space(70.0);
        ui.label(egui::RichText::new("New").color(t.text_dim));
    });
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for p in pairs.iter_mut() {
            let from = p[0].as_str().unwrap_or("#000000").to_string();
            let mut to = p[1].as_str().unwrap_or("#000000").to_string();
            ui.horizontal(|ui| {
                chip(ui, &from);
                ui.label(egui::RichText::new("→").color(t.text_dim));
                chip(ui, &to);
                if ui.add(egui::TextEdit::singleline(&mut to).desired_width(80.0)).changed() && Color::from_hex(&to).is_some() {
                    p[1] = json!(to);
                    changed = true;
                }
            });
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Randomize").clicked() {
            // Deterministic scramble of hues (seeded by the colour itself), keeping brightness.
            for (i, p) in pairs.iter_mut().enumerate() {
                let c = Color::from_hex(p[0].as_str().unwrap_or("#000")).unwrap_or(Color::BLACK);
                let [h, s, v] = c.to_hsb();
                let nh = (h + 97.0 * (i as f32 + 1.0) + 41.0) % 360.0;
                p[1] = json!(Color::from_hsb(nh, s.max(0.35), v.max(0.3)).to_hex());
            }
            changed = true;
        }
        for (label, deg) in [("Complement", 180.0), ("Triad", 120.0), ("Analogous", 30.0)] {
            if ui.button(label).clicked() {
                for (i, p) in pairs.iter_mut().enumerate() {
                    let c = Color::from_hex(p[0].as_str().unwrap_or("#000")).unwrap_or(Color::BLACK);
                    let [h, s, v] = c.to_hsb();
                    let shift = if label == "Complement" { deg } else { deg * (i as f32 + 1.0) };
                    p[1] = json!(Color::from_hsb(h + shift, s, v).to_hex());
                }
                changed = true;
            }
        }
        if ui.button("Reset").clicked() {
            for p in pairs.iter_mut() {
                p[1] = p[0].clone();
            }
            changed = true;
        }
    });
    d.fields.insert("pairs".into(), Value::Array(pairs));
    changed
}
