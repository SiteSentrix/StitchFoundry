//! Color Guide panel: base colour, harmony-rule dropdown with the harmony strip, and a variation
//! grid (tints/shades, warm/cool or vivid/muted) whose cells apply to the active proxy.

use egui::{Rect, Sense, Ui, vec2};
use serde_json::json;
use vectorcraft_color::harmony::Harmony;
use vectorcraft_color::{Color, Paint};

use super::{active_paint, apply_click, color_json, pstate, set_pstate};
use crate::VectorcraftApp;
use crate::widgets::{self, menu_item};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Variation {
    #[default]
    TintsShades,
    WarmCool,
    VividMuted,
}

/// Rows above/below the base row in the variation grid.
pub const STEPS: i32 = 3;

/// Variation of `c` at `step` (−STEPS..=STEPS; 0 = the colour itself).
pub fn variation(c: &Color, mode: Variation, step: i32) -> Color {
    if step == 0 {
        return *c;
    }
    let f = step.unsigned_abs() as f32 / (STEPS as f32 + 1.0);
    match mode {
        Variation::TintsShades => {
            if step < 0 {
                c.lerp(&Color::BLACK, f)
            } else {
                c.lerp(&Color::WHITE, f)
            }
        }
        Variation::WarmCool => {
            let target = if step < 0 { Color::rgb(0.2, 0.45, 1.0) } else { Color::rgb(1.0, 0.55, 0.1) };
            c.lerp(&target, f * 0.6)
        }
        Variation::VividMuted => {
            let [h, s, b] = c.to_hsb();
            let s2 = if step < 0 { s * (1.0 - f) } else { s + (1.0 - s) * f };
            Color::from_hsb(h, s2.clamp(0.0, 1.0), b)
        }
    }
}

fn harmony(ui: &Ui) -> Harmony {
    let i: usize = pstate(ui.ctx(), "cg-harmony");
    Harmony::ALL[i.min(Harmony::ALL.len() - 1)]
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let base = match active_paint(app) {
        Paint::Solid { color, .. } => color,
        _ => pstate::<Option<Color>>(ui.ctx(), "cg-base").unwrap_or(Color::rgb8(230, 120, 40)),
    };
    let h = harmony(ui);
    let colors = h.apply(base);
    let mode: Variation = pstate(ui.ctx(), "cg-mode");
    let mut chosen: Option<Color> = None;
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
        widgets::swatch_tile(ui, r, &Paint::solid(base), false, resp.hovered());
        if resp.on_hover_text("Set base color to the current color").clicked() {
            set_pstate(ui.ctx(), "cg-base", Some(base));
        }
        let labels: Vec<&str> = Harmony::ALL.iter().map(|h| h.label()).collect();
        if let Some(i) = widgets::dropdown(ui, "cg-rule", h.label(), &labels, ui.available_width() - 4.0) {
            set_pstate(ui.ctx(), "cg-harmony", i);
        }
    });
    // Harmony strip.
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let w = strip.width() / colors.len() as f32;
    for (i, c) in colors.iter().enumerate() {
        let r = Rect::from_min_size(strip.min + vec2(i as f32 * w, 0.0), vec2(w, strip.height()));
        let resp = ui.interact(r, ui.id().with(("cg-h", i)), Sense::click());
        ui.painter().rect_filled(r, 0.0, super::c32(c));
        if resp.on_hover_text(c.to_hex()).clicked() {
            chosen = Some(*c);
        }
    }
    ui.add_space(6.0);
    // Variation grid: columns = harmony colours, rows = variations.
    let cols = colors.len().max(1) as f32;
    let cell_w = (ui.available_width() / cols).floor();
    for step in -STEPS..=STEPS {
        let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
        for (i, c) in colors.iter().enumerate() {
            let v = variation(c, mode, step);
            let r = Rect::from_min_size(row.min + vec2(i as f32 * cell_w, 0.0), vec2(cell_w - 1.0, 17.0));
            let resp = ui.interact(r, ui.id().with(("cg-v", step, i)), Sense::click());
            ui.painter().rect_filled(r, 0.0, super::c32(&v));
            if step == 0 {
                ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, t.text_strong), egui::StrokeKind::Inside);
            }
            if resp.on_hover_text(v.to_hex()).clicked() {
                chosen = Some(v);
            }
        }
    }
    let label = match mode {
        Variation::TintsShades => ("Shades", "Tints"),
        Variation::WarmCool => ("Cool", "Warm"),
        Variation::VividMuted => ("Muted", "Vivid"),
    };
    ui.horizontal(|ui| {
        widgets::dim_label(ui, label.0);
        ui.add_space((ui.available_width() - 40.0).max(0.0));
        widgets::dim_label(ui, label.1);
    });
    widgets::bottom_bar(ui, |ui| {
        widgets::icon_button_enabled(ui, "library", "Limit to Swatch Library (on the roadmap)", false, false, 24.0);
        ui.add_space((ui.available_width() - 2.0 * 28.0).max(0.0));
        widgets::icon_button_enabled(ui, "palette", "Edit or Apply Colors (Recolor Artwork is on the roadmap)", false, false, 24.0);
        if widgets::icon_button(ui, "dc-folder", "Save color group to Swatch panel", false, 24.0).clicked() {
            let cs: Vec<_> = colors.iter().map(color_json).collect();
            app.run("swatch.newGroup", json!({"name": h.label(), "colors": cs})).ok();
        }
    });
    if let Some(c) = chosen {
        apply_click(app, ui, json!({"color": color_json(&c)}));
    }
}

pub fn menu(_app: &mut VectorcraftApp, ui: &mut Ui) {
    let mode: Variation = pstate(ui.ctx(), "cg-mode");
    for (m, l) in
        [(Variation::TintsShades, "Show Tints/Shades"), (Variation::WarmCool, "Show Warm/Cool"), (Variation::VividMuted, "Show Vivid/Muted")]
    {
        if menu_item(ui, l, true, m == mode) {
            set_pstate(ui.ctx(), "cg-mode", m);
        }
    }
    ui.separator();
    menu_item(ui, "Color Guide Options…", false, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variations() {
        let c = Color::rgb(0.5, 0.5, 0.5);
        assert_eq!(variation(&c, Variation::TintsShades, 0), c);
        assert!(variation(&c, Variation::TintsShades, STEPS).to_rgb()[0] > 0.5);
        assert!(variation(&c, Variation::TintsShades, -STEPS).to_rgb()[0] < 0.5);
        let red = Color::rgb(1.0, 0.2, 0.2);
        assert!(variation(&red, Variation::VividMuted, -2).to_hsb()[1] < red.to_hsb()[1]);
        let warm = variation(&c, Variation::WarmCool, 2).to_rgb();
        assert!(warm[0] > warm[2]);
    }
}
