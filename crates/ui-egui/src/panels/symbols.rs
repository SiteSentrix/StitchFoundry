//! Symbols panel: the document's symbols as rendered thumbnails or a list. Click selects a symbol
//! (it becomes the Symbol Sprayer's current symbol); double-click places an instance.

use std::cell::RefCell;
use std::collections::HashMap;

use egui::{Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node, NodeKind};

use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};

/// A thumbnail of symbol `name` (an instance at its natural size), cached by the art's identity.
fn thumb(ui: &Ui, doc: &Document, name: &str, size: f32) -> Option<egui::TextureHandle> {
    thread_local! {
        static RENDERER: RefCell<vectorcraft_render::Renderer> = RefCell::new(vectorcraft_render::Renderer::new());
        static CACHE: RefCell<HashMap<(usize, String, u32), egui::TextureHandle>> = RefCell::new(HashMap::new());
    }
    let sym = doc.symbols.iter().find(|s| s.name == name)?;
    let px = (size * ui.ctx().pixels_per_point()).round().max(8.0) as u32;
    let key = (std::sync::Arc::as_ptr(&sym.art) as usize, name.to_string(), px);
    if let Some(t) = CACHE.with(|c| c.borrow().get(&key).cloned()) {
        return Some(t);
    }
    let (w, h) = doc
        .unknown
        .get("symbolSizes")
        .and_then(|m| m.get(name))
        .and_then(|v| Some((v.get(0)?.as_f64()?, v.get(1)?.as_f64()?)))
        .unwrap_or((20.0, 20.0));
    let mut d = Document::new(w.max(1.0), h.max(1.0));
    d.symbols.push(sym.clone());
    let id = d.alloc_id();
    let xf = vectorcraft_geom::Affine::scale_non_uniform(w / 20.0, h / 20.0);
    let l = d.layers[0].id;
    d.insert(Some(l), 0, Node::new(id, NodeKind::SymbolInstance { symbol: name.into(), xf })).ok()?;
    let img = RENDERER.with(|r| r.borrow_mut().render_thumbnail(&d, id, px))?;
    let color = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
    let tex = ui.ctx().load_texture(format!("sym-{name}-{px}"), color, egui::TextureOptions::LINEAR);
    CACHE.with(|c| {
        let mut c = c.borrow_mut();
        if c.len() > 512 {
            c.clear();
        }
        c.insert(key, tex.clone());
    });
    Some(tex)
}

fn current(app: &mut VectorcraftApp) -> Option<String> {
    app.run("symbol.list", json!({})).ok().and_then(|v| v["current"].as_str().map(str::to_string))
}

fn has_instances_selected(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| {
        st.selection.objects.iter().any(|id| {
            let mut any = false;
            if let Some(n) = st.doc.node(*id) {
                n.walk(&mut |c| any |= matches!(c.kind, NodeKind::SymbolInstance { .. }));
            }
            any
        })
    })
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(doc) = app.session.active().map(|d| d.doc.clone()) else { return };
    let names: Vec<String> = doc.symbols.iter().map(|s| s.name.clone()).collect();
    let list: bool = pstate(ui.ctx(), "sym-list");
    let sel = current(app);
    let mut clicked: Option<(String, bool)> = None;
    widgets::list_box(ui, |ui| {
        ui.set_min_height(110.0);
        ui.set_width(ui.available_width());
        if names.is_empty() {
            super::empty_state(ui, "spray-can", tl!("No symbols in this document"), tl!("Select art and click New Symbol to make one."));
            return;
        }
        let paint_thumb = |ui: &Ui, r: egui::Rect, n: &str| {
            ui.painter().rect_filled(r, 0.0, egui::Color32::WHITE);
            if let Some(tex) = thumb(ui, &doc, n, r.width()) {
                ui.painter().image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
        };
        if list {
            for n in &names {
                let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
                if sel.as_deref() == Some(n.as_str()) {
                    ui.painter().rect_filled(r, 0.0, t.row_selected);
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 0.0, t.hover);
                }
                paint_thumb(ui, egui::Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(22.0, 22.0)), n);
                ui.painter().text(r.left_center() + vec2(32.0, 0.0), egui::Align2::LEFT_CENTER, n, egui::FontId::proportional(12.5), t.text);
                if resp.double_clicked() {
                    clicked = Some((n.clone(), true));
                } else if resp.clicked() {
                    clicked = Some((n.clone(), false));
                }
            }
        } else {
            ui.horizontal_wrapped(|ui| {
                for n in &names {
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
                    paint_thumb(ui, r.shrink(2.0), n);
                    let on = sel.as_deref() == Some(n.as_str());
                    ui.painter().rect_stroke(
                        r,
                        0.0,
                        Stroke::new(if on { 2.0 } else { 1.0 }, if on { t.accent } else { t.border }),
                        StrokeKind::Inside,
                    );
                    let resp = resp.on_hover_text(n);
                    if resp.double_clicked() {
                        clicked = Some((n.clone(), true));
                    } else if resp.clicked() {
                        clicked = Some((n.clone(), false));
                    }
                }
            });
        }
    });
    if let Some((n, place)) = clicked {
        app.run("symbol.setCurrent", json!({"name": n})).ok();
        if place {
            app.run("symbol.place", json!({"name": n})).ok();
        }
    }
    let has_sel = app.session.active().is_some_and(|d| !d.selection.is_empty());
    let inst = has_instances_selected(app);
    widgets::bottom_bar(ui, |ui| {
        widgets::icon_button_enabled(ui, "library", tl!("Symbol Libraries (on the roadmap)"), false, false, 24.0);
        if widgets::icon_button_enabled(ui, "dc-place-symbol", tl!("Place Symbol Instance"), false, sel.is_some(), 24.0).clicked() {
            app.run("symbol.place", json!({})).ok();
        }
        if widgets::icon_button_enabled(ui, "link-2-off", tl!("Break Link to Symbol"), false, inst, 24.0).clicked() {
            app.run("symbol.breakLink", json!({})).ok();
        }
        ui.add_space((ui.available_width() - 2.0 * 28.0).max(0.0));
        if widgets::icon_button_enabled(ui, "dc-new-item", tl!("New Symbol"), false, has_sel, 24.0).clicked() {
            app.run("symbol.new", json!({})).ok();
        }
        if widgets::icon_button_enabled(ui, "trash-2", tl!("Delete Symbol"), false, sel.is_some(), 24.0).clicked()
            && let Some(n) = &sel
        {
            app.run("symbol.delete", json!({"name": n})).ok();
        }
    });
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let sel = current(app);
    let has_sel = app.session.active().is_some_and(|d| !d.selection.is_empty());
    let inst = has_instances_selected(app);
    let name = json!({"name": sel});
    let items: [(&str, bool, &str, Value); 9] = [
        (tl!("New Symbol…"), has_sel, "symbol.new", json!({})),
        (tl!("Redefine Symbol"), has_sel && sel.is_some(), "symbol.update", name.clone()),
        (tl!("Duplicate Symbol"), sel.is_some(), "symbol.duplicate", name.clone()),
        (tl!("Delete Symbol"), sel.is_some(), "symbol.delete", name.clone()),
        (tl!("Edit Symbol"), inst, "symbol.edit", json!({})),
        (tl!("Place Symbol Instance"), sel.is_some(), "symbol.place", name.clone()),
        (tl!("Replace Symbol"), inst && sel.is_some(), "symbol.replace", name.clone()),
        (tl!("Break Link to Symbol"), inst, "symbol.breakLink", json!({})),
        (tl!("Select All Instances"), sel.is_some(), "symbol.selectInstances", name),
    ];
    for (label, enabled, cmd, params) in items {
        if menu_item(ui, label, enabled, false) {
            app.run(cmd, params).ok();
        }
    }
    ui.separator();
    let list: bool = pstate(ui.ctx(), "sym-list");
    if menu_item(ui, tl!("Thumbnail View"), true, !list) {
        set_pstate(ui.ctx(), "sym-list", false);
    }
    if menu_item(ui, tl!("List View"), true, list) {
        set_pstate(ui.ctx(), "sym-list", true);
    }
}
