//! Transparency panel: blend mode, opacity (field + slider popup), object/mask thumbnails with the
//! opacity-mask controls (make/release, link, clip, invert; Shift-click the mask to disable it),
//! Isolate Blending and Knockout Group.

use egui::{Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;
use vectorcraft_color::{BlendMode, Paint};

use super::{current_paints, current_transparency, first_selected, live_run, pstate, selection_len, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, TransparencyEdit, menu_item};
use crate::{VectorcraftApp, icons};

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // While editing a mask the panel shows the masked object (the selection is its mask art).
    let editing = app.session.active().and_then(|d| d.doc.mask_edit);
    let n = match editing {
        Some(me) => app.session.active().and_then(|d| d.doc.node(me.object).cloned()),
        None => first_selected(app),
    };
    let has = n.is_some();
    let (isolate, knockout) = n.as_ref().map_or((false, false), |n| (n.isolate, n.knockout));
    // Opacity and blend of the Appearance panel's active item, else of the object.
    let (op, blend) = match editing {
        None => current_transparency(app),
        Some(_) => n.as_ref().map(|n| (n.opacity, n.blend)),
    }
    .unwrap_or((1.0, BlendMode::Normal));
    match widgets::opacity_blend(ui, "tr", op, blend, has) {
        Some(TransparencyEdit::Blend(b)) => {
            app.run("transparency.set", json!({"blend": b.label()})).ok();
        }
        Some(TransparencyEdit::Opacity(o, phase)) => live_run(app, "Opacity", "transparency.set", json!({ "opacity": o }), phase),
        None => {}
    }
    widgets::divider(ui);
    // Thumbnails and the opacity-mask controls.
    let hide_thumbs: bool = pstate(ui.ctx(), "tr-hide-thumbs");
    let mask = n.as_ref().and_then(|n| n.mask.as_deref().cloned());
    let (new_clip, new_invert) = app.session.new_mask_defaults();
    ui.horizontal(|ui| {
        if !hide_thumbs {
            let (r, oresp) = ui.allocate_exact_size(vec2(60.0, 50.0), Sense::click());
            ui.painter().rect_filled(r, 0.0, egui::Color32::WHITE);
            // The thumbnail being edited is outlined (object normally, the mask while editing it).
            let (obj_w, mask_w) = if editing.is_some() { (0.5, 1.5) } else { (1.5, 1.0) };
            ui.painter().rect_stroke(r, 0.0, Stroke::new(obj_w, t.border), StrokeKind::Outside);
            if editing.is_some() && oresp.on_hover_text("Stop editing the opacity mask").clicked() {
                app.run("transparency.stopEditingOpacityMask", json!({})).ok();
            }
            if let Some(n) = &n {
                let mut bare = n.clone();
                bare.mask = None;
                if !node_thumb(app, ui, "tr-obj", &bare, r.shrink(3.0), None) {
                    let (f, s) = current_paints(app);
                    let inner = r.shrink2(vec2(6.0, 10.0));
                    widgets::paint_chip(ui, inner, &f);
                    if let Some(c) = s.color()
                        && !matches!(s, Paint::None)
                    {
                        ui.painter().rect_stroke(inner, 0.0, Stroke::new(1.0, super::c32(&c)), StrokeKind::Middle);
                    }
                }
            }
            match &mask {
                Some(m) => {
                    // Link toggle between the object and its mask.
                    let (lr, lresp) = ui.allocate_exact_size(vec2(14.0, 50.0), Sense::click());
                    icons::paint(
                        ui,
                        if m.linked { "link" } else { "link-2-off" },
                        egui::Rect::from_center_size(lr.center(), vec2(12.0, 12.0)),
                        t.icon,
                    );
                    if lresp.on_hover_text(if m.linked { "Unlink the mask" } else { "Link the mask" }).clicked() {
                        app.run("transparency.setOpacityMask", json!({"linked": !m.linked})).ok();
                    }
                    let (mr, mresp) = ui.allocate_exact_size(vec2(50.0, 50.0), Sense::click());
                    let bg = if m.clip != m.invert { [0, 0, 0, 255] } else { [255, 255, 255, 255] };
                    ui.painter().rect_filled(mr, 0.0, egui::Color32::from_rgb(bg[0], bg[1], bg[2]));
                    node_thumb(app, ui, "tr-mask", &m.art, mr.shrink(2.0), Some(bg));
                    ui.painter().rect_stroke(
                        mr,
                        0.0,
                        Stroke::new(mask_w, if editing.is_some() { t.border } else { t.input_border }),
                        StrokeKind::Inside,
                    );
                    if m.disabled {
                        let red = Stroke::new(2.0, egui::Color32::from_rgb(220, 40, 40));
                        ui.painter().line_segment([mr.left_top(), mr.right_bottom()], red);
                        ui.painter().line_segment([mr.right_top(), mr.left_bottom()], red);
                    }
                    let shift = ui.input(|i| i.modifiers.shift);
                    if mresp.on_hover_text("Click to edit the mask; Shift-click to disable or enable it").clicked() {
                        if shift {
                            app.run(if m.disabled { "transparency.enableOpacityMask" } else { "transparency.disableOpacityMask" }, json!({})).ok();
                        } else if editing.is_none() {
                            app.run("transparency.editOpacityMask", json!({})).ok();
                        }
                    }
                }
                None => {
                    let (m, _) = ui.allocate_exact_size(vec2(50.0, 50.0), Sense::hover());
                    ui.painter().rect_stroke(m, 0.0, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
                    icons::paint(ui, "dc-mask-none", m.shrink(12.0), t.text_disabled);
                }
            }
        }
        ui.vertical(|ui| {
            let can_make = selection_len(app) >= 2;
            let label = if mask.is_some() { "Release" } else { "Make Mask" };
            let enabled = mask.is_some() || can_make;
            let r = ui.add_enabled_ui(enabled, |ui| widgets::flat_button(ui, label, 96.0)).inner;
            if r.on_disabled_hover_text("Select the art and, on top of it, the mask object").clicked() {
                let id = if mask.is_some() { "transparency.releaseOpacityMask" } else { "transparency.makeOpacityMask" };
                app.run(id, json!({})).ok();
            }
            let (clip, invert) = mask.as_ref().map(|m| (m.clip, m.invert)).unwrap_or((new_clip, new_invert));
            if widgets::check(ui, "Clip", clip, mask.is_some()) {
                app.run("transparency.setOpacityMask", json!({"clip": !clip})).ok();
            }
            if widgets::check(ui, "Invert Mask", invert, mask.is_some()) {
                app.run("transparency.setOpacityMask", json!({"invert": !invert})).ok();
            }
        });
    });
    if !pstate::<bool>(ui.ctx(), "tr-hide-options") {
        widgets::divider(ui);
        if widgets::check(ui, "Isolate Blending", isolate, has) {
            app.run("transparency.set", json!({"isolate": !isolate})).ok();
        }
        if widgets::check(ui, "Knockout Group", knockout, has) {
            app.run("transparency.set", json!({"knockout": !knockout})).ok();
        }
        widgets::check(ui, "Opacity & Mask Define Knockout Shape", false, false);
    }
    if !has {
        widgets::dim_label(ui, if selection_len(app) == 0 { "No Selection" } else { "" });
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let hide_thumbs: bool = pstate(ui.ctx(), "tr-hide-thumbs");
    let hide_opts: bool = pstate(ui.ctx(), "tr-hide-options");
    if menu_item(ui, if hide_thumbs { "Show Thumbnails" } else { "Hide Thumbnails" }, true, false) {
        set_pstate(ui.ctx(), "tr-hide-thumbs", !hide_thumbs);
    }
    if menu_item(ui, if hide_opts { "Show Options" } else { "Hide Options" }, true, false) {
        set_pstate(ui.ctx(), "tr-hide-options", !hide_opts);
    }
    ui.separator();
    let mask = first_selected(app).and_then(|n| n.mask.as_deref().cloned());
    let can_make = selection_len(app) >= 2 && mask.is_none();
    let items: [(&str, &str, bool); 4] = match &mask {
        Some(m) => [
            ("Make Opacity Mask", "transparency.makeOpacityMask", false),
            ("Release Opacity Mask", "transparency.releaseOpacityMask", true),
            if m.disabled {
                ("Enable Opacity Mask", "transparency.enableOpacityMask", true)
            } else {
                ("Disable Opacity Mask", "transparency.disableOpacityMask", true)
            },
            if m.linked {
                ("Unlink Opacity Mask", "transparency.unlinkOpacityMask", true)
            } else {
                ("Link Opacity Mask", "transparency.linkOpacityMask", true)
            },
        ],
        None => [
            ("Make Opacity Mask", "transparency.makeOpacityMask", can_make),
            ("Release Opacity Mask", "", false),
            ("Disable Opacity Mask", "", false),
            ("Unlink Opacity Mask", "", false),
        ],
    };
    for (label, id, enabled) in items {
        if menu_item(ui, label, enabled, false) {
            app.run(id, json!({})).ok();
        }
    }
    ui.separator();
    let (clip, invert) = app.session.new_mask_defaults();
    if menu_item(ui, "New Opacity Masks Are Clipping", true, clip) {
        app.run("transparency.toggleNewMasksClipping", json!({})).ok();
    }
    if menu_item(ui, "New Opacity Masks Are Inverted", true, invert) {
        app.run("transparency.toggleNewMasksInverted", json!({})).ok();
    }
    ui.separator();
    menu_item(ui, "Page Isolated Blending", false, false);
    menu_item(ui, "Page Knockout Group", false, false);
}

/// A rendered thumbnail of `n` (which may live outside the tree, like mask art). One texture per
/// slot, re-rendered only when the document revision, object or size changes.
fn node_thumb(app: &VectorcraftApp, ui: &Ui, slot: &str, n: &vectorcraft_doc::Node, r: egui::Rect, bg: Option<[u8; 4]>) -> bool {
    use std::cell::RefCell;
    thread_local! {
        static RENDERER: RefCell<vectorcraft_render::Renderer> = RefCell::new(vectorcraft_render::Renderer::new());
    }
    let Some(st) = app.session.active() else { return false };
    let px = (r.width().min(r.height()) * ui.ctx().pixels_per_point()).round().max(8.0) as u32;
    let (slot_id, key) = (egui::Id::new(slot), egui::Id::new((st.uid, st.revision, n.id, bg, px)));
    let cached: Option<(egui::Id, egui::TextureHandle)> = ui.ctx().data(|d| d.get_temp(slot_id));
    let tex = match cached {
        Some((k, tex)) if k == key => Some(tex),
        _ => RENDERER.with(|rr| rr.borrow_mut().render_node_thumbnail(&st.doc, n, px, bg)).map(|img| {
            let color = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
            let tex = ui.ctx().load_texture(slot, color, egui::TextureOptions::LINEAR);
            ui.ctx().data_mut(|d| d.insert_temp(slot_id, (key, tex.clone())));
            tex
        }),
    };
    let Some(tex) = tex else { return false };
    let side = r.width().min(r.height());
    let dst = egui::Rect::from_center_size(r.center(), vec2(side, side));
    ui.painter().image(tex.id(), dst, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;

    /// Run the panel and its ≡ menu for one headless frame.
    fn frame(app: &mut VectorcraftApp) {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(app, ui);
            menu(app, ui);
        });
        out.textures_delta.clear();
    }

    #[test]
    fn panel_draws_with_and_without_a_mask() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let r = |app: &mut VectorcraftApp, id: &str, p: serde_json::Value| app.session.execute(id, &p).unwrap();
        r(&mut app, "file.new", json!({"width": 100, "height": 100}));
        frame(&mut app);
        let a = r(&mut app, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50}))["id"].clone();
        let b = r(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 20, "height": 20}))["id"].clone();
        r(&mut app, "select.set", json!({"ids": [a, b]}));
        frame(&mut app);
        r(&mut app, "transparency.makeOpacityMask", json!({}));
        frame(&mut app);
        r(&mut app, "transparency.disableOpacityMask", json!({}));
        r(&mut app, "transparency.unlinkOpacityMask", json!({}));
        frame(&mut app);
        assert_eq!(app.session.execute("transparency.opacityMaskInfo", &json!({})).unwrap()[0]["disabled"], true);
    }
}
