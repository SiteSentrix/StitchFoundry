//! Color Guide panel: the base colour, the harmony-rule dropdown with the harmony strip, and the
//! variation grid: a row per harmony colour with the colour itself in the centre column,
//! shades/cool/muted to the left and tints/warm/vivid to the right (steps and reach from Color
//! Guide Options). The base colour stays put until "Set base color" takes the current colour. A
//! click on a cell applies it to the active proxy and selects it; Shift- or Cmd/Ctrl-click adds
//! cells to the selection that Save Colors as Swatches saves.

use egui::{Rect, Sense, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_color::harmony::{Guide, Harmony, Variation};
use vectorcraft_color::{Color, Paint};

use super::{active_paint, apply_click, c32, color_json, pstate, set_pstate};
use crate::VectorcraftApp;
use crate::widgets::{self, menu_item};

/// The base colour before one is set: an orange.
const DEFAULT_BASE: Color = Color::Rgb { r: 230.0 / 255.0, g: 120.0 / 255.0, b: 40.0 / 255.0 };

/// The guide's base colour: fixed once set; the current colour when the panel first shows.
pub(crate) fn base_color(app: &VectorcraftApp, ctx: &egui::Context) -> Color {
    if let Some(c) = pstate::<Option<Color>>(ctx, "cg-base") {
        return c;
    }
    let c = active_paint(app).color().unwrap_or(DEFAULT_BASE);
    set_pstate(ctx, "cg-base", Some(c));
    c
}

fn rule(ctx: &egui::Context) -> Harmony {
    let i: usize = pstate(ctx, "cg-harmony");
    Harmony::ALL[i.min(Harmony::ALL.len() - 1)]
}

/// Selected grid cells: (row, column).
fn selected(ctx: &egui::Context) -> Vec<(usize, usize)> {
    pstate(ctx, "cg-sel")
}

fn clear_selection(ctx: &egui::Context) {
    set_pstate(ctx, "cg-sel", Vec::<(usize, usize)>::new());
}

fn guide(app: &VectorcraftApp, ctx: &egui::Context) -> Guide {
    Guide::new(base_color(app, ctx), rule(ctx), &app.ui.color_guide)
}

/// Save the selected cells' colours as swatches (one undo step).
fn save_selected(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let g = guide(app, ctx);
    let colors: Vec<Value> = selected(ctx).into_iter().filter_map(|(r, c)| g.grid.get(r)?.get(c)).map(color_json).collect();
    if !colors.is_empty() {
        app.run("swatch.new", json!({ "colors": colors })).ok();
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = crate::theme::Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let h = rule(&ctx);
    let g = guide(app, &ctx);
    let base = g.colors[0];
    let mut chosen: Option<Color> = None;
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(26.0, 26.0), Sense::click());
        widgets::swatch_tile(ui, r, &Paint::solid(base), false, resp.hovered());
        if resp.on_hover_text("Set base color to the current color").clicked()
            && let Some(c) = active_paint(app).color()
        {
            set_pstate(&ctx, "cg-base", Some(c));
            clear_selection(&ctx);
        }
        let labels: Vec<&str> = Harmony::ALL.iter().map(|h| h.label()).collect();
        if let Some(i) = widgets::dropdown(ui, "cg-rule", h.label(), &labels, ui.available_width() - 4.0) {
            set_pstate(&ctx, "cg-harmony", i);
            clear_selection(&ctx);
        }
    });
    // Harmony strip.
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::hover());
    let w = strip.width() / g.colors.len() as f32;
    for (i, c) in g.colors.iter().enumerate() {
        let r = Rect::from_min_size(strip.min + vec2(i as f32 * w, 0.0), vec2(w, strip.height()));
        let resp = ui.interact(r, ui.id().with(("cg-h", i)), Sense::click());
        ui.painter().rect_filled(r, 0.0, c32(c));
        if resp.on_hover_text(c.to_hex()).clicked() {
            chosen = Some(*c);
        }
    }
    ui.add_space(6.0);
    // Variation grid: a row per harmony colour, the colour itself in the centre column.
    let mut sel = selected(&ctx);
    let cols = g.grid.first().map_or(1, Vec::len);
    let centre = g.centre();
    let cell_w = ui.available_width() / cols as f32;
    let gap = if cell_w >= 8.0 { 1.0 } else { 0.0 };
    // Cell edges on whole pixels across the full width.
    let x = |i: usize| (i as f32 * cell_w).round();
    let multi = ui.input(|i| i.modifiers.shift || i.modifiers.command);
    for (ri, row) in g.grid.iter().enumerate() {
        let (rr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
        for (ci, v) in row.iter().enumerate() {
            let r = Rect::from_min_size(rr.min + vec2(x(ci), 0.0), vec2(x(ci + 1) - x(ci) - gap, 17.0));
            let resp = ui.interact(r, ui.id().with(("cg-v", ri, ci)), Sense::click());
            ui.painter().rect_filled(r, 0.0, c32(v));
            if sel.contains(&(ri, ci)) {
                ui.painter().rect_stroke(r.shrink(1.0), 0.0, egui::Stroke::new(1.0, egui::Color32::WHITE), egui::StrokeKind::Inside);
                ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.5, t.accent), egui::StrokeKind::Outside);
            } else if ci == centre {
                ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, t.text_strong), egui::StrokeKind::Inside);
            }
            if resp.on_hover_text(v.to_hex()).clicked() {
                if !multi {
                    sel.clear();
                    chosen = Some(*v);
                }
                match sel.iter().position(|s| *s == (ri, ci)) {
                    Some(i) => {
                        sel.remove(i);
                    }
                    None => sel.push((ri, ci)),
                }
                set_pstate(&ctx, "cg-sel", sel.clone());
            }
        }
    }
    let (left, right) = app.ui.color_guide.variation.sides();
    ui.horizontal(|ui| {
        widgets::dim_label(ui, left);
        ui.add_space((ui.available_width() - 40.0).max(0.0));
        widgets::dim_label(ui, right);
    });
    widgets::bottom_bar(ui, |ui| {
        widgets::icon_button_enabled(ui, "library", "Limit to Swatch Library (on the roadmap)", false, false, 24.0);
        ui.add_space((ui.available_width() - 3.0 * 28.0).max(0.0));
        widgets::icon_button_enabled(ui, "palette", "Edit or Apply Colors (on the roadmap)", false, false, 24.0);
        if widgets::icon_button_enabled(ui, "dc-new-item", "Save selected colors as swatches", false, !sel.is_empty(), 24.0).clicked() {
            save_selected(app, &ctx);
        }
        if widgets::icon_button(ui, "dc-folder", "Save color group to Swatch panel", false, 24.0).clicked() {
            let cs: Vec<_> = g.colors.iter().map(color_json).collect();
            app.run("swatch.newGroup", json!({"name": h.label(), "colors": cs})).ok();
        }
    });
    if let Some(c) = chosen {
        apply_click(app, ui, json!({"color": color_json(&c)}));
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let mode = app.ui.color_guide.variation;
    for (m, l) in
        [(Variation::TintsShades, "Show Tints/Shades"), (Variation::WarmCool, "Show Warm/Cool"), (Variation::VividMuted, "Show Vivid/Muted")]
    {
        if menu_item(ui, l, true, m == mode) {
            app.ui.color_guide.variation = m;
        }
    }
    ui.separator();
    let ctx = ui.ctx().clone();
    if menu_item(ui, "Save Colors as Swatches", !selected(&ctx).is_empty(), false) {
        save_selected(app, &ctx);
    }
    if menu_item(ui, "Color Guide Options…", true, false) {
        app.run("ui.colorGuideOptions", json!({})).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;

    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
        app
    }

    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(app, ui);
            menu(app, ui);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn base_stays_until_set_and_the_grid_follows_the_options() {
        let mut app = app();
        app.run("paint.setFill", json!({"color": "#3366cc"})).unwrap();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
        assert_eq!(base_color(&app, &ctx).to_hex(), "#3366cc", "the first show takes the current colour");
        // Another current colour leaves the base alone.
        app.run("paint.setFill", json!({"color": "#ff0000"})).unwrap();
        frame(&mut app, &ctx);
        assert_eq!(base_color(&app, &ctx).to_hex(), "#3366cc");
        app.ui.color_guide.steps = 6;
        frame(&mut app, &ctx);
        let g = guide(&app, &ctx);
        assert_eq!(g.grid[0].len(), 13);
        assert_eq!(g.grid[0][6].to_hex(), "#3366cc", "the base sits in the centre of the first row");
    }

    #[test]
    fn save_colors_as_swatches_saves_each_selected_cell() {
        let mut app = app();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx);
        let before = app.session.doc().unwrap().doc.swatches.len();
        set_pstate(&ctx, "cg-sel", vec![(0usize, 0usize), (1, 4), (0, 8)]);
        let undo = app.session.doc().unwrap().history.undo.len();
        save_selected(&mut app, &ctx);
        let st = app.session.doc().unwrap();
        assert_eq!(st.doc.swatches.len(), before + 3);
        assert_eq!(st.history.undo.len(), undo + 1, "one undo step");
        let g = guide(&app, &ctx);
        assert_eq!(st.doc.swatches.last().unwrap().paint.color().map(|c| c.to_hex()), Some(g.grid[0][8].to_hex()));
    }
}
