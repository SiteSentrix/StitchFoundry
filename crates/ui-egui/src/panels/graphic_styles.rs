//! Graphic Styles panel: thumbnail grid or list of the document's styles. Clicking a style applies
//! it to the selection (Alt-click adds it on top), Shift/Cmd-click selects several, double-click
//! opens Graphic Style Options.

use egui::{Rect, Response, Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;
use vectorcraft_doc::{Appearance, GraphicStyle};

use super::{alt_held, pstate, selection_len, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};

/// What a click on a style does (run after drawing, once the document is no longer borrowed).
enum Click {
    Apply(String),
    Toggle(String),
    Options(String),
}

fn preview(ui: &Ui, r: Rect, ap: &Appearance) {
    ui.painter().rect_filled(r, 0.0, egui::Color32::WHITE);
    let inner = r.shrink(r.width() * 0.18);
    widgets::paint_chip(ui, inner, &ap.fill_paint());
    if let Some(s) = ap.stroke()
        && let Some(c) = s.paint.color()
    {
        ui.painter().rect_stroke(inner, 0.0, Stroke::new((s.width as f32).clamp(1.0, 4.0), super::c32(&c)), StrokeKind::Middle);
    }
    if !ap.effects.is_empty() {
        ui.painter().text(
            r.right_bottom() - vec2(3.0, 2.0),
            egui::Align2::RIGHT_BOTTOM,
            "fx",
            egui::FontId::proportional(9.0),
            egui::Color32::DARK_GRAY,
        );
    }
}

/// The click on a style's row or tile.
fn clicked(ui: &Ui, resp: &Response, name: &str) -> Option<Click> {
    if resp.double_clicked() {
        Some(Click::Options(name.to_string()))
    } else if resp.clicked() {
        let m = ui.input(|i| i.modifiers);
        Some(if m.shift || m.command { Click::Toggle(name.to_string()) } else { Click::Apply(name.to_string()) })
    } else {
        None
    }
}

/// The styles selected in the panel.
fn selected(ctx: &egui::Context) -> Vec<String> {
    pstate(ctx, "gs-sel")
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // The document is shared (an `Arc`): holding it while drawing copies nothing.
    let doc = app.session.active().map(|d| d.doc.clone());
    let styles: &[GraphicStyle] = doc.as_ref().map_or(&[], |d| &d.graphic_styles);
    let sel = selected(ui.ctx());
    let list: bool = pstate(ui.ctx(), "gs-list");
    let mut click = None;
    widgets::list_box(ui, |ui| {
        ui.set_min_height(120.0);
        ui.set_width(ui.available_width());
        if styles.is_empty() {
            super::empty_state(ui, "dc-graphic-styles", "No graphic styles", "Select styled art and click New Graphic Style.");
            return;
        }
        if list {
            for g in styles {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click());
                if sel.contains(&g.name) {
                    ui.painter().rect_filled(r, 0.0, t.row_selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, t.hover);
                }
                preview(ui, Rect::from_min_size(r.left_center() + vec2(4.0, -9.0), vec2(18.0, 18.0)), &g.appearance);
                ui.painter().text(r.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, &g.name, egui::FontId::proportional(12.0), t.text);
                if let Some(c) = clicked(ui, &resp, &g.name) {
                    click = Some(c);
                }
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
                for g in styles {
                    let (r, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
                    preview(ui, r, &g.appearance);
                    let on = sel.contains(&g.name);
                    let color = if on {
                        t.accent
                    } else if resp.hovered() {
                        t.text
                    } else {
                        t.border
                    };
                    ui.painter().rect_stroke(r, 0.0, Stroke::new(if on { 2.0 } else { 1.0 }, color), StrokeKind::Inside);
                    let resp = resp.on_hover_text(&g.name);
                    if let Some(c) = clicked(ui, &resp, &g.name) {
                        click = Some(c);
                    }
                }
            });
        }
    });
    match click {
        Some(Click::Apply(name)) => {
            if selection_len(app) > 0 {
                app.run("graphicStyle.apply", json!({"name": name, "add": alt_held(ui)})).ok();
            }
            set_pstate(ui.ctx(), "gs-sel", vec![name]);
        }
        Some(Click::Toggle(name)) => {
            let mut sel = sel;
            match sel.iter().position(|n| *n == name) {
                Some(i) => {
                    sel.remove(i);
                }
                None => sel.push(name),
            }
            set_pstate(ui.ctx(), "gs-sel", sel);
        }
        Some(Click::Options(name)) => {
            app.run("ui.graphicStyleOptions", json!({ "name": name })).ok();
        }
        None => {}
    }
    let sel = selected(ui.ctx());
    let has_sel = selection_len(app) > 0;
    let linked = app.session.selection_graphic_style().is_some();
    widgets::bottom_bar(ui, |ui| {
        widgets::icon_button_enabled(ui, "library", "Graphic Style Libraries (on the roadmap)", false, false, 24.0);
        if widgets::icon_button_enabled(ui, "link-2-off", "Break Link to Graphic Style", false, linked, 24.0).clicked() {
            app.run("graphicStyle.breakLink", json!({})).ok();
        }
        ui.add_space((ui.available_width() - 2.0 * 28.0).max(0.0));
        if widgets::icon_button_enabled(ui, "dc-new-item", "New Graphic Style (Alt-click to name it)", false, has_sel, 24.0).clicked() {
            new_style(app, alt_held(ui));
        }
        if widgets::icon_button_enabled(ui, "trash-2", "Delete Graphic Style", false, !sel.is_empty(), 24.0).clicked() {
            delete(app, ui.ctx(), &sel);
        }
    });
}

/// A new style from the selection, first asking for its name (`ask`) in Graphic Style Options.
fn new_style(app: &mut VectorcraftApp, ask: bool) {
    if ask {
        app.run("ui.graphicStyleOptions", json!({})).ok();
    } else {
        app.run("graphicStyle.new", json!({})).ok();
    }
}

fn delete(app: &mut VectorcraftApp, ctx: &egui::Context, sel: &[String]) {
    if app.run("graphicStyle.delete", json!({ "names": sel })).is_ok() {
        set_pstate(ctx, "gs-sel", Vec::<String>::new());
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let sel = selected(ui.ctx());
    let one = (sel.len() == 1).then(|| sel[0].clone());
    let has_doc = app.session.active().is_some();
    let list: bool = pstate(ui.ctx(), "gs-list");
    if menu_item(ui, "New Graphic Style…", selection_len(app) > 0, false) {
        new_style(app, true);
    }
    if menu_item(ui, "Duplicate Graphic Style", one.is_some(), false)
        && let Some(n) = &one
    {
        app.run("graphicStyle.duplicate", json!({ "name": n })).ok();
    }
    menu_item(ui, "Merge Graphic Styles", false, false);
    if menu_item(ui, "Delete Graphic Style", !sel.is_empty(), false) {
        delete(app, ui.ctx(), &sel);
    }
    if menu_item(ui, "Break Link to Graphic Style", app.session.selection_graphic_style().is_some(), false) {
        app.run("graphicStyle.breakLink", json!({})).ok();
    }
    ui.separator();
    if menu_item(ui, "Select All Unused", has_doc, false)
        && let Ok(r) = app.run("graphicStyle.unused", json!({}))
    {
        let names: Vec<String> = serde_json::from_value(r["names"].clone()).unwrap_or_default();
        set_pstate(ui.ctx(), "gs-sel", names);
    }
    if menu_item(ui, "Sort by Name", has_doc, false) {
        app.run("graphicStyle.sortByName", json!({})).ok();
    }
    ui.separator();
    if menu_item(ui, "Thumbnail View", true, !list) {
        set_pstate(ui.ctx(), "gs-list", false);
    }
    if menu_item(ui, "Large List View", true, list) {
        set_pstate(ui.ctx(), "gs-list", true);
    }
    ui.separator();
    if menu_item(ui, "Graphic Style Options…", one.is_some(), false)
        && let Some(n) = &one
    {
        app.run("ui.graphicStyleOptions", json!({ "name": n })).ok();
    }
    menu_item(ui, "Open Graphic Style Library", false, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;

    /// The texts `draw` paints in one headless frame.
    fn frame(ctx: &egui::Context, draw: impl FnMut(&mut Ui)) -> Vec<String> {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let mut out = ctx.run_ui(egui::RawInput::default(), draw);
        out.textures_delta.clear();
        let mut v = vec![];
        out.shapes.iter().for_each(|c| texts(&c.shape, &mut v));
        v
    }

    #[test]
    fn list_view_names_the_styles_and_deletes_the_selected_ones() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 50, "height": 50})).unwrap();
        app.session.execute("graphicStyle.new", &json!({"name": "Mine"})).unwrap();
        let ctx = egui::Context::default();
        set_pstate(&ctx, "gs-list", true);
        let texts = frame(&ctx, |ui| {
            show(&mut app, ui);
            menu(&mut app, ui);
        });
        assert!(texts.iter().any(|t| t == "Mine") && texts.iter().any(|t| t == "Sunshine"), "{texts:?}");
        // Select All Unused picks every style but the one the rectangle is linked to.
        let unused: Vec<String> = serde_json::from_value(app.session.execute("graphicStyle.unused", &json!({})).unwrap()["names"].clone()).unwrap();
        assert_eq!(unused.len(), 4);
        set_pstate(&ctx, "gs-sel", unused.clone());
        frame(&ctx, |ui| show(&mut app, ui));
        delete(&mut app, &ctx, &unused);
        assert!(selected(&ctx).is_empty());
        let names: Vec<&str> = app.session.active().unwrap().doc.graphic_styles.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Mine"]);
    }
}
