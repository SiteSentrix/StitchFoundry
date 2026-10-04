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
use vectorcraft_color::{BlendMode, Color, Paint};
use vectorcraft_doc::{Node, NodeKind, StrokeLayer};

use crate::theme::Tokens;
use crate::widgets::{Live, dim_label};
use crate::{VectorcraftApp, icons};

/// The first selected node (cloned), if any.
pub fn first_selected(app: &VectorcraftApp) -> Option<Node> {
    first_node(app).cloned()
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

/// The paint command for the active proxy (Fill or Stroke).
pub(crate) fn paint_target(app: &VectorcraftApp) -> &'static str {
    if app.session.fill_active { "paint.setFill" } else { "paint.setStroke" }
}

/// The first selected node, borrowed (for per-frame reads that need no copy of it).
fn first_node(app: &VectorcraftApp) -> Option<&Node> {
    let st = app.session.active()?;
    st.selection.objects.first().and_then(|id| st.doc.node(*id))
}

/// Fill and stroke as the proxies show them: the first selected object's, where the Appearance
/// panel's active item stands in for the top fill or stroke of its kind (text without one shows
/// its first run's style); else, and for a group without its own fill or stroke, the defaults
/// for new art.
pub(crate) fn current_paints(app: &VectorcraftApp) -> (Paint, Paint) {
    let defaults = || (app.session.paint.fill.clone(), app.session.paint.stroke.clone());
    let Some(n) = first_node(app) else { return defaults() };
    let item = app.session.appearance_item();
    if item.is_none() {
        if let NodeKind::Text(t) = &n.kind {
            let s = t.first_style();
            return (s.fill, s.stroke);
        }
        if n.is_container() && n.appearance.items.is_empty() {
            return defaults();
        }
    }
    let ap = &n.appearance;
    (ap.fill_for(item).map_or(Paint::None, |f| f.paint.clone()), ap.stroke_for(item).map_or(Paint::None, |s| s.paint.clone()))
}

/// The stroke the Stroke panel, Control bar and Properties show: the Appearance panel's active
/// item when it is a stroke, else the first selected object's top stroke.
pub(crate) fn current_stroke(app: &VectorcraftApp) -> Option<StrokeLayer> {
    first_node(app)?.appearance.stroke_for(app.session.appearance_item()).cloned()
}

/// Opacity and blend mode as the Transparency panel and Control bar show them: the Appearance
/// panel's active item's, else the first selected object's.
pub(crate) fn current_transparency(app: &VectorcraftApp) -> Option<(f32, BlendMode)> {
    let n = first_node(app)?;
    Some(match app.session.appearance_item().and_then(|i| n.appearance.items.get(i)) {
        Some(it) => (it.opacity(), it.blend()),
        None => (n.opacity, n.blend),
    })
}

/// The paint behind the active proxy.
pub(crate) fn active_paint(app: &VectorcraftApp) -> Paint {
    let (f, s) = current_paints(app);
    if app.session.fill_active { f } else { s }
}

/// Draw the Fill/Stroke proxy and handle its clicks through commands.
pub(crate) fn proxy(app: &mut VectorcraftApp, ui: &mut Ui, size: f32) {
    let (f, s) = current_paints(app);
    let (a, b, swap, def) = crate::widgets::fill_stroke_proxy(ui, &f, &s, app.session.fill_active, size);
    if (a && !app.session.fill_active) || (b && app.session.fill_active) {
        app.run("paint.toggleActive", json!({})).ok();
    }
    if swap {
        app.run("paint.swap", json!({})).ok();
    }
    if def {
        app.run("paint.default", json!({})).ok();
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

/// Recently applied colours (Swatches / Color panels "Recent Colors" row), newest first.
pub(crate) fn recent_colors(ctx: &egui::Context) -> Vec<Color> {
    pstate::<Vec<Color>>(ctx, "recent-colors")
}
pub(crate) fn push_recent(ctx: &egui::Context, c: Color) {
    let mut v = recent_colors(ctx);
    v.retain(|x| x.to_hex() != c.to_hex());
    v.insert(0, c);
    v.truncate(10);
    set_pstate(ctx, "recent-colors", v);
}

/// "Recent Colors" header + a row of chips; clicking one applies it to the active proxy.
pub(crate) fn recent_colors_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    crate::widgets::subheader(ui, "Recent Colors");
    let recent = recent_colors(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    let mut chosen = None;
    for (i, c) in recent.iter().enumerate() {
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
        app.run(paint_target(app), json!({"color": color_json(&c)})).ok();
    }
}

/// A colour as command JSON, keeping its model.
pub(crate) use vectorcraft_tools::params::color_json;

/// Paint as command params (`{color}`, `{swatch}`, `{none}`, `{gradient}` (lossless) or `{pattern}`).
pub(crate) fn paint_params(p: &Paint) -> Value {
    match p {
        Paint::None => json!({"none": true}),
        Paint::Solid { swatch: Some(n), .. } => json!({"swatch": n}),
        Paint::Solid { color, .. } => json!({"color": color_json(color)}),
        Paint::Gradient(g) => json!({"gradient": vectorcraft_tools::params::gradient_params(g)}),
        Paint::Pattern { pattern, .. } => json!({"pattern": pattern}),
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
        assert_eq!(paint_params(&vectorcraft_doc::pattern::pattern_paint("Dots")), json!({"pattern": "Dots"}));
        assert_eq!(p["gradient"]["stops"][0]["midpoint"], json!(0.5));
    }
}
#[cfg(test)]
mod tests_appearance;
