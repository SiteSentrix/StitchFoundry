//! The lettering kiosk's screen (KI-01): while the kiosk is on, the app shows only this panel and
//! the design: a template, the name, a font, a thread, then the design made, shown on the product
//! and sent to the machine (or saved). Leaving it takes the PIN when the shop set one.

use egui::{Color32, Ui};
use serde_json::json;

use super::panel::{button, pick_names};
use crate::VectorcraftApp;
use crate::theme::Tokens;

/// The threads offered when the shop gave none.
const THREADS: [&str; 12] =
    ["#1d3557", "#d62828", "#ffffff", "#000000", "#f4a261", "#2a9d8f", "#e9c46a", "#6d597a", "#c0c0c0", "#d4af37", "#e76f51", "#457b9d"];
const PRODUCTS: [&str; 6] = ["tshirt", "polo", "hoodie", "cap", "tote", "towel"];

/// What the counter has typed and picked.
#[derive(Default)]
pub struct KioskView {
    pub text: String,
    pub template: usize,
    pub font: usize,
    pub color: usize,
    /// The capitals' height (mm); 0: as large as fits.
    pub size: f64,
    pub product: usize,
    /// The design made and its mock-up (document indices).
    pub design: Option<usize>,
    pub mockup: Option<usize>,
    pub pin: String,
    pub status: String,
}

/// The kiosk's screen in place of the app's chrome.
pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::left("kiosk")
        .exact_size(340.0)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(16.0))
        .show(ui, |ui| egui::ScrollArea::vertical().show(ui, |ui| body(app, ui)));
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.pasteboard)).show(ui, |ui| {
        if app.session.active().is_some() {
            crate::canvas::show(app, ui);
        } else {
            ui.centered_and_justified(|ui| ui.heading(tl!("Type a name and make the design")));
        }
    });
}

fn hex_color(hex: &str) -> Color32 {
    let c = vectorcraft_embroidery::sf_core::threads::Rgb::from_hex(hex).unwrap_or(vectorcraft_embroidery::sf_core::threads::Rgb::new(0, 0, 0));
    Color32::from_rgb(c.r, c.g, c.b)
}

fn body(app: &mut VectorcraftApp, ui: &mut Ui) {
    let k = app.session.prefs.embroidery_kiosk.clone();
    ui.heading(tl!("Lettering Kiosk"));
    ui.add_space(8.0);
    // the fonts remembered from earlier sessions, loaded the first time
    if !std::mem::replace(&mut app.embroidery.fonts_listed, true) {
        app.run("embroidery.fonts", json!({})).ok();
    }
    let templates: Vec<String> = app
        .run("embroidery.templates", json!({}))
        .ok()
        .and_then(|r| r["templates"].as_array().map(|a| a.iter().filter_map(|t| t["name"].as_str().map(str::to_string)).collect::<Vec<_>>()))
        .unwrap_or_default()
        .into_iter()
        .filter(|n| k.templates.is_empty() || k.templates.iter().any(|o| o.eq_ignore_ascii_case(n)))
        .collect();
    let fonts: Vec<String> = app
        .session
        .embroidery_fonts
        .iter()
        .map(|f| f.info.name.clone())
        .filter(|n| k.fonts.is_empty() || k.fonts.iter().any(|o| o.eq_ignore_ascii_case(n)))
        .collect();
    let colors: Vec<String> = if k.colors.is_empty() { THREADS.iter().map(ToString::to_string).collect() } else { k.colors.clone() };
    let v = &mut app.embroidery.kiosk;
    let names: Vec<&str> = templates.iter().map(String::as_str).collect();
    if let Some(i) = pick_names(ui, tl!("Template"), "kiosk-template", v.template.min(names.len().saturating_sub(1)), &names) {
        v.template = i;
    }
    ui.add_space(6.0);
    ui.label(tl!("Name"));
    ui.add(egui::TextEdit::multiline(&mut v.text).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Heading));
    ui.add_space(6.0);
    if fonts.is_empty() {
        ui.label(tl!("No embroidery font is loaded: ask the shop to load one."));
    } else {
        let names: Vec<&str> = fonts.iter().map(String::as_str).collect();
        if let Some(i) = pick_names(ui, tl!("Font"), "kiosk-font", v.font.min(names.len().saturating_sub(1)), &names) {
            v.font = i;
        }
    }
    ui.add_space(6.0);
    ui.label(tl!("Thread"));
    ui.horizontal_wrapped(|ui| {
        for (i, c) in colors.iter().enumerate() {
            let (rect, r) = ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::click());
            let chosen = v.color == i;
            ui.painter().rect_filled(rect, egui::CornerRadius::same(4), hex_color(c));
            let t = Tokens::get(ui.ctx());
            ui.painter().rect_stroke(
                rect,
                egui::CornerRadius::same(4),
                egui::Stroke::new(if chosen { 3.0 } else { 1.0 }, if chosen { t.accent } else { t.border }),
                egui::StrokeKind::Inside,
            );
            if r.on_hover_text(c.as_str()).clicked() {
                v.color = i;
            }
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let mut auto = v.size <= 0.0;
        if ui.checkbox(&mut auto, tl!("As large as fits")).changed() {
            v.size = if auto { 0.0 } else { 10.0 };
        }
        if !auto {
            ui.add(egui::DragValue::new(&mut v.size).range(3.0..=200.0).suffix(" mm"));
        }
    });
    ui.add_space(10.0);
    let text = v.text.trim().to_string();
    let mut order = json!({ "text": text });
    if let Some(t) = templates.get(v.template) {
        order["template"] = json!(t);
    }
    if let Some(f) = fonts.get(v.font) {
        order["font"] = json!(f);
    }
    if let Some(c) = colors.get(v.color) {
        order["color"] = json!(c);
    }
    if v.size > 0.0 {
        order["size"] = json!(v.size);
    }
    if ui.add_enabled(!text.is_empty() && !fonts.is_empty(), egui::Button::new(tl!("Make Design")).min_size(egui::vec2(200.0, 36.0))).clicked() {
        let r = app.run("embroidery.kiosk.order", order);
        let v = &mut app.embroidery.kiosk;
        match r {
            Ok(r) => {
                v.design = app.session.active_index();
                v.mockup = None;
                v.status = format!("{} × {} mm", r["widthMm"], r["heightMm"]);
            }
            Err(e) => v.status = e.to_string(),
        }
    }
    let made = app.embroidery.kiosk.design.filter(|i| *i < app.session.documents().len());
    if let Some(design) = made {
        ui.add_space(10.0);
        let labels = [tl!("T-shirt"), tl!("Polo shirt"), tl!("Hoodie"), tl!("Cap"), tl!("Tote bag"), tl!("Towel")];
        let cur = app.embroidery.kiosk.product.min(labels.len() - 1);
        if let Some(i) = pick_names(ui, tl!("Product"), "kiosk-product", cur, &labels) {
            app.embroidery.kiosk.product = i;
        }
        ui.horizontal(|ui| {
            if button(ui, tl!("Show on Product")).clicked() {
                app.session.set_active(design);
                let product = PRODUCTS.get(app.embroidery.kiosk.product).copied().unwrap_or("tshirt");
                match app.run("embroidery.visualize", json!({ "product": product })) {
                    Ok(_) => app.embroidery.kiosk.mockup = app.session.active_index(),
                    Err(e) => app.embroidery.kiosk.status = e.to_string(),
                }
            }
            if app.session.active_index() != Some(design) && button(ui, tl!("Back to the Design")).clicked() {
                app.session.set_active(design);
            }
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if !k.folder.is_empty() && button(ui, tl!("Send to Machine")).clicked() {
                app.session.set_active(design);
                let mut q = json!({ "folder": k.folder });
                if !k.format.is_empty() {
                    q["format"] = json!(k.format);
                }
                app.embroidery.kiosk.status = match app.run("embroidery.queue.add", q) {
                    Ok(r) => match r["path"].as_str() {
                        Some(p) => format!("{} {p}", tl!("Sent:")),
                        None => tl!("Waiting for the folder").to_string(),
                    },
                    Err(e) => e.to_string(),
                };
            }
            if button(ui, tl!("Save Machine File…")).clicked() {
                app.session.set_active(design);
                let format = if k.format.is_empty() { "dst" } else { k.format.as_str() };
                if let Err(e) = app.run("file.export.embroidery", json!({ "format": format })) {
                    app.embroidery.kiosk.status = e.to_string();
                }
            }
        });
        // waiting designs go once their folder is there
        if app.session.prefs.embroidery_queue.iter().any(|i| i.state == "waiting") {
            let now = ui.ctx().input(|i| i.time);
            if now - app.embroidery.queue_checked > 5.0 {
                app.embroidery.queue_checked = now;
                app.run("embroidery.queue.run", json!({})).ok();
            }
            ui.ctx().request_repaint_after(std::time::Duration::from_secs(5));
        }
        if button(ui, tl!("Next Name")).clicked() {
            let v = &mut app.embroidery.kiosk;
            v.text.clear();
            v.design = None;
            v.mockup = None;
            v.status.clear();
        }
    }
    if !app.embroidery.kiosk.status.is_empty() {
        ui.add_space(6.0);
        ui.label(egui::RichText::new(app.embroidery.kiosk.status.clone()).small());
    }
    // leaving: the PIN when it is locked
    ui.add_space(24.0);
    ui.separator();
    ui.horizontal(|ui| {
        let locked = !k.pin.is_empty();
        ui.add(egui::TextEdit::singleline(&mut app.embroidery.kiosk.pin).password(true).desired_width(90.0).hint_text(tl!("PIN")));
        let pin = app.embroidery.kiosk.pin.trim().to_string();
        if button(ui, tl!("Leave Kiosk")).clicked() {
            let mut q = json!({ "on": false });
            if locked {
                q["pin"] = json!(pin);
            }
            match app.run("embroidery.kiosk", q) {
                Ok(_) => app.embroidery.kiosk.pin.clear(),
                Err(e) => app.embroidery.kiosk.status = e.to_string(),
            }
        }
        if !locked && !pin.is_empty() && button(ui, tl!("Lock")).on_hover_text(tl!("Leaving the kiosk will take this PIN")).clicked() {
            match app.run("embroidery.kiosk", json!({ "on": true, "pin": pin })) {
                Ok(_) => app.embroidery.kiosk.pin.clear(),
                Err(e) => app.embroidery.kiosk.status = e.to_string(),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_kiosk_shows_its_panel_and_leaves_with_the_pin() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("embroidery.kiosk", &json!({ "on": true, "pin": "2468" })).unwrap();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut app, ui));
        out.textures_delta.clear();
        let mut texts = Vec::new();
        fn walk(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        out.shapes.iter().for_each(|c| walk(&c.shape, &mut texts));
        assert!(texts.iter().any(|t| t == "Lettering Kiosk"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Leave Kiosk"), "{texts:?}");
        assert!(app.session.execute("embroidery.kiosk", &json!({ "on": false, "pin": "1111" })).is_err());
        app.session.execute("embroidery.kiosk", &json!({ "on": false, "pin": "2468" })).unwrap();
        assert!(!app.session.prefs.embroidery_kiosk.on);
    }
}
