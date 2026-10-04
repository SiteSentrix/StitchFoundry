//! Panels. Each panel reads engine state and acts only through `app.run(...)` (slider drags
//! preview through the session's interaction so one drag is one undo step).
//!
//! Every icon panel has a body (`show_icon_panel`) and a panel (≡) menu (`panel_menu`).

pub mod actions;
pub mod align;
pub mod appearance;
pub mod artboards;
pub mod brushes;
pub mod character;
pub mod color;
pub mod color_guide;
pub mod doc_info;
pub mod glyphs;
pub mod gradient;
pub mod graphic_styles;
pub mod history;
pub mod image_trace;
pub mod info;
pub mod layers;
pub mod magic_wand;
pub mod navigator;
pub mod opentype;
pub mod paragraph;
pub mod pathfinder;
pub mod pattern_options;
pub mod properties;
pub mod separations;
pub mod stroke;
pub mod swatches;
pub mod symbols;
pub mod tabs;
pub mod text_styles;
pub mod transform;
pub mod transparency;

use egui::{Rect, Sense, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::Node;

use crate::theme::Tokens;
use crate::widgets::{Live, dim_label};
use crate::{VectorcraftApp, icons};

/// The first selected node (cloned), if any.
pub fn first_selected(app: &VectorcraftApp) -> Option<Node> {
    let st = app.session.active()?;
    st.selection.objects.first().and_then(|id| st.doc.node(*id)).cloned()
}

/// Number of selected objects.
pub(crate) fn selection_len(app: &VectorcraftApp) -> usize {
    app.session.active().map(|d| d.selection.len()).unwrap_or(0)
}

pub fn show_icon_panel(app: &mut VectorcraftApp, ui: &mut Ui, id: &str) {
    match id {
        "swatches" => swatches::show(app, ui),
        "color" => color::show(app, ui),
        "colorGuide" => color_guide::show(app, ui),
        "stroke" => stroke::show(app, ui),
        "transparency" => transparency::show(app, ui),
        "appearance" => appearance::show(app, ui),
        "graphicStyles" => graphic_styles::show(app, ui),
        "align" => align::show(app, ui),
        "pathfinder" => pathfinder::show(app, ui),
        "transform" => transform::show(app, ui),
        "history" => history::show(app, ui),
        "actions" => actions::show(app, ui),
        "info" => info::show(app, ui),
        "separations" => separations::show(app, ui),
        "artboards" => artboards::show(app, ui),
        "gradient" => gradient::show(app, ui),
        "character" => character::show(app, ui),
        "paragraph" => paragraph::show(app, ui),
        "glyphs" => glyphs::show(app, ui),
        "navigator" => navigator::show(app, ui),
        "brushes" => brushes::show(app, ui),
        "symbols" => symbols::show(app, ui),
        "patternOptions" => pattern_options::show(app, ui),
        "imageTrace" => image_trace::show(app, ui),
        "openType" => opentype::show(app, ui),
        "docInfo" => doc_info::show(app, ui),
        "charStyles" => text_styles::show(app, ui, text_styles::Kind::Char),
        "paraStyles" => text_styles::show(app, ui, text_styles::Kind::Para),
        "magicWand" => magic_wand::show(app, ui),
        "tabs" => tabs::show(app, ui),
        _ => {
            dim_label(ui, "This panel is on the roadmap (see the parity plan).");
        }
    }
}

/// Items of a panel's (≡) menu. Unimplemented Illustrator items are listed disabled.
pub fn panel_menu_items(app: &mut VectorcraftApp, ui: &mut Ui, id: &str) {
    match id {
        "swatches" => swatches::menu(app, ui),
        "color" => color::menu(app, ui),
        "colorGuide" => color_guide::menu(app, ui),
        "stroke" => stroke::menu(app, ui),
        "transparency" => transparency::menu(app, ui),
        "appearance" => appearance::menu(app, ui),
        "graphicStyles" => graphic_styles::menu(app, ui),
        "align" => align::menu(app, ui),
        "pathfinder" => pathfinder::menu(app, ui),
        "transform" => transform::menu(app, ui),
        "history" => history::menu(app, ui),
        "info" => info::menu(app, ui),
        "separations" => separations::menu(app, ui),
        "artboards" => artboards::menu(app, ui),
        "gradient" => gradient::menu(app, ui),
        "character" => character::menu(app, ui),
        "paragraph" => paragraph::menu(app, ui),
        "glyphs" => glyphs::menu(app, ui),
        "navigator" => navigator::menu(app, ui),
        "brushes" => brushes::menu(app, ui),
        "symbols" => symbols::menu(app, ui),
        "patternOptions" => pattern_options::menu(app, ui),
        "imageTrace" => image_trace::menu(app, ui),
        "openType" => opentype::menu(app, ui),
        "docInfo" => doc_info::menu(app, ui),
        "charStyles" => text_styles::menu(app, ui, text_styles::Kind::Char),
        "paraStyles" => text_styles::menu(app, ui, text_styles::Kind::Para),
        "magicWand" => magic_wand::menu(app, ui),
        "tabs" => tabs::menu(app, ui),
        _ => {
            ui.add_enabled(false, egui::Button::new("No options").frame(false));
        }
    }
}

/// The ≡ panel-menu button drawn into `rect` (the right end of a panel's title/tab strip).
pub fn panel_menu(app: &mut VectorcraftApp, ui: &mut Ui, id: &str, rect: Rect) {
    let t = Tokens::get(ui.ctx());
    let resp = ui.interact(rect, ui.id().with(("panel-menu", id)), Sense::click());
    icons::paint(ui, "menu", rect.shrink(1.0), if resp.hovered() { t.text_strong } else { t.text_dim });
    let resp = resp.on_hover_text("Panel menu");
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(220.0);
        panel_menu_items(app, ui, id);
    });
}

pub fn libraries(_app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(20.0);
    ui.vertical_centered(|ui| {
        icons::icon(ui, "library", 40.0, t.text_dim);
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Local Libraries").size(14.0).color(t.text));
        dim_label(
            ui,
            "Drag art, colors and text styles here to reuse them across documents. Libraries are stored on this machine — no account required.",
        );
    });
}

// ---------- shared helpers ----------

/// The paint command of the active proxy (Fill or Stroke), or of the inactive one.
pub(crate) fn proxy_cmd(app: &VectorcraftApp, inactive: bool) -> &'static str {
    if app.session.fill_active != inactive { "paint.setFill" } else { "paint.setStroke" }
}

/// Is Alt held? A click on a colour then paints the inactive proxy.
pub(crate) fn alt_held(ui: &Ui) -> bool {
    ui.input(|i| i.modifiers.alt)
}

/// Apply a clicked colour, swatch or None (`params`: `{color}`, `{swatch}` or `{none}`) to the
/// active proxy, or with Alt held to the inactive one, which stays behind.
pub(crate) fn apply_click(app: &mut VectorcraftApp, ui: &Ui, mut params: Value) {
    let alt = alt_held(ui);
    params["focus"] = json!(!alt);
    app.run(proxy_cmd(app, alt), params).ok();
}

/// Fill and stroke as the proxies show them ([`vectorcraft_engine::Session::proxy_paints`]: the
/// first selected object's, a group's first painted object's, else the defaults for new art).
pub(crate) fn current_paints(app: &VectorcraftApp) -> (Paint, Paint) {
    app.session.proxy_paints()
}

/// Whether the selected objects' fills and strokes differ (the proxies show "?"), cached per
/// document revision.
pub(crate) fn mixed_paints(app: &VectorcraftApp, ctx: &egui::Context) -> (bool, bool) {
    let Some(st) = app.session.active() else { return (false, false) };
    let key = (st.uid, st.revision);
    match pstate::<Option<((u64, u64), (bool, bool))>>(ctx, "proxy-mixed") {
        Some((k, mixed)) if k == key => mixed,
        _ => {
            let mixed = st.proxy_mixed();
            set_pstate(ctx, "proxy-mixed", Some((key, mixed)));
            mixed
        }
    }
}

/// Is the active proxy "?" (the selected objects' paints differ)?
pub(crate) fn active_mixed(app: &VectorcraftApp, ctx: &egui::Context) -> bool {
    let (f, s) = mixed_paints(app, ctx);
    if app.session.fill_active { f } else { s }
}

/// The paint behind the active proxy.
pub(crate) fn active_paint(app: &VectorcraftApp) -> Paint {
    let (f, s) = current_paints(app);
    if app.session.fill_active { f } else { s }
}

/// Draw the Fill/Stroke proxy and handle its clicks through commands.
pub(crate) fn proxy(app: &mut VectorcraftApp, ui: &mut Ui, size: f32) {
    let (f, s) = current_paints(app);
    let mixed = mixed_paints(app, ui.ctx());
    let c = crate::widgets::fill_stroke_proxy(ui, &f, &s, mixed, app.session.fill_active, size);
    if (c.fill && !app.session.fill_active) || (c.stroke && app.session.fill_active) {
        app.run("paint.toggleActive", json!({})).ok();
    }
    if c.swap {
        app.run("paint.swap", json!({})).ok();
    }
    if c.default {
        app.run("paint.default", json!({})).ok();
    }
    if let Some(stroke) = c.pick {
        app.run("ui.colorPicker", json!({ "stroke": stroke })).ok();
    }
}

/// Run `cmd` live: while dragging, preview on top of an interaction snapshot; on release, commit
/// it as one undo step. Falls back to a plain run when no document is open.
pub(crate) fn live_run(app: &mut VectorcraftApp, label: &str, cmd: &str, params: Value, phase: Live) {
    match phase {
        Live::Idle => {}
        Live::Dragging | Live::Released => {
            if app.session.begin_interaction(label).is_ok() {
                if let Err(e) = app.session.preview(cmd, &params) {
                    app.ui.status = e.to_string();
                }
                if phase == Live::Released {
                    app.session.commit_interaction().ok();
                }
                app.sync_views();
            } else if phase == Live::Released {
                app.run(cmd, params).ok();
            }
        }
    }
}

/// Per-panel UI state kept in egui memory (not document state).
pub(crate) fn pstate<T: Clone + Default + Send + Sync + 'static>(ctx: &egui::Context, key: &str) -> T {
    ctx.data(|d| d.get_temp::<T>(egui::Id::new(("panel-state", key)))).unwrap_or_default()
}
pub(crate) fn set_pstate<T: Clone + Send + Sync + 'static>(ctx: &egui::Context, key: &str, v: T) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(("panel-state", key)), v));
}

/// "Recent Colors" header + a row of chips (the Session's recent colours, which every paint
/// command feeds); clicking one applies it to the active proxy (Alt: the inactive one).
pub(crate) fn recent_colors_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    crate::widgets::subheader(ui, "Recent Colors");
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let mut chosen = None;
    for (i, c) in app.session.recent_colors.iter().enumerate() {
        let cell = Rect::from_min_size(r.min + vec2(3.0 + i as f32 * 19.0, 3.0), vec2(16.0, 16.0));
        if cell.right() > r.right() - 2.0 {
            break;
        }
        let resp = ui.interact(cell, ui.id().with(("recent", i)), Sense::click());
        crate::widgets::swatch_tile(ui, cell, &Paint::solid(*c), false, resp.hovered());
        if resp.on_hover_text(c.to_hex()).clicked() {
            chosen = Some(*c);
        }
    }
    if let Some(c) = chosen {
        apply_click(app, ui, json!({"color": color_json(&c)}));
    }
}

/// A colour as command JSON, keeping its model.
pub(crate) fn color_json(c: &Color) -> Value {
    match *c {
        Color::Rgb { r, g, b } => json!([r, g, b]),
        Color::Cmyk { c, m, y, k } => json!({"c": c, "m": m, "y": y, "k": k}),
        Color::Gray { k } => json!({"gray": k}),
    }
}

/// Paint as command params (`{color}`, `{none}` or `{gradient}`).
pub(crate) fn paint_params(p: &Paint) -> Value {
    match p {
        Paint::None => json!({"none": true}),
        Paint::Solid { swatch: Some(n), .. } => json!({"swatch": n}),
        Paint::Solid { color, .. } => json!({"color": color_json(color)}),
        Paint::Gradient(g) => json!({"gradient": {
            "kind": g.gradient.kind.label().to_lowercase(),
            "angle": g.angle,
            "stops": g.gradient.stops.iter().map(|s| json!({"offset": s.offset, "color": color_json(&s.color), "opacity": s.opacity})).collect::<Vec<_>>()
        }}),
        Paint::Pattern { .. } => json!({"none": true}),
    }
}

/// Egui colour of a document colour.
pub(crate) fn c32(c: &Color) -> egui::Color32 {
    let [r, g, b, _] = c.to_rgba8(1.0);
    egui::Color32::from_rgb(r, g, b)
}

/// A labelled empty-state message used by list panels.
pub(crate) fn empty_state(ui: &mut Ui, icon: &str, title: &str, body: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        icons::icon(ui, icon, 28.0, t.text_disabled);
        ui.add_space(4.0);
        ui.label(egui::RichText::new(title).size(12.5).color(t.text));
        ui.label(egui::RichText::new(body).size(11.5).color(t.text_dim));
    });
    ui.add_space(12.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_params_roundtrip_shapes() {
        assert_eq!(paint_params(&Paint::None), json!({"none": true}));
        let p = paint_params(&Paint::solid(Color::cmyk(0.1, 0.2, 0.3, 0.4)));
        assert!((p["color"]["k"].as_f64().unwrap() - 0.4).abs() < 1e-6);
        let g = Paint::Gradient(Box::new(vectorcraft_color::GradientPaint::new(Default::default())));
        let p = paint_params(&g);
        assert_eq!(p["gradient"]["kind"], "linear");
        assert_eq!(p["gradient"]["stops"].as_array().unwrap().len(), 2);
    }
}
