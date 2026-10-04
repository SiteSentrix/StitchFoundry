//! Transparency panel: blend mode, opacity (field + slider popup), object/mask thumbnails with the
//! opacity-mask controls (make/release, link, clip, invert; Shift-click the mask to disable it),
//! Isolate Blending and Knockout Group.

use egui::{Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;
use vectorcraft_color::{BlendMode, Paint};
use vectorcraft_doc::{Node, NodeId, OpacityMask};
use vectorcraft_engine::cmd::TransparencyInfo;

use super::{current_paints, current_transparency, live_run, pstate, selection_len, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, TransparencyEdit, menu_item};
use crate::{VectorcraftApp, icons};

/// The panel's values ([`vectorcraft_engine::Session::transparency_info`]) and its first target
/// (the selection, or the object whose mask is being edited), recomputed only when the document
/// or the selection changes.
fn state(app: &VectorcraftApp, ctx: &egui::Context) -> (TransparencyInfo, Option<Node>) {
    type Key = (u64, u64, usize, Option<NodeId>, Option<NodeId>);
    let Some(st) = app.session.active() else { return Default::default() };
    let sel = &st.selection.objects;
    let key: Key = (st.uid, st.revision, sel.len(), sel.first().copied(), sel.last().copied());
    let id = egui::Id::new("tr-state");
    if let Some((k, v)) = ctx.data(|d| d.get_temp::<(Key, (TransparencyInfo, Option<Node>))>(id))
        && k == key
    {
        return v;
    }
    let info = app.session.transparency_info();
    let first = info.ids.first().and_then(|id| st.doc.node(*id)).cloned();
    let v = (info, first);
    ctx.data_mut(|d| d.insert_temp(id, (key, v.clone())));
    v
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // While editing a mask the panel shows the masked object (the selection is its mask art).
    let editing = app.session.active().and_then(|d| d.doc.mask_edit);
    let (info, n) = state(app, ui.ctx());
    let has = n.is_some();
    let (isolate, knockout) = (info.isolate.unwrap_or(false), info.knockout.unwrap_or(false));
    // Opacity and blend of the Appearance panel's active item (not while a mask is edited: the
    // panel then shows the masked object), else of the targets, blank where they differ; with no
    // target, the defaults for new art.
    let item = editing.is_none().then(|| app.session.appearance_item()).flatten().and_then(|_| current_transparency(app));
    let (op, blend) = match item {
        Some((o, b)) => (Some(o), Some(b)),
        None if has => (info.opacity.map(|o| (o / 100.0) as f32), info.blend),
        None => (Some(1.0), Some(BlendMode::Normal)),
    };
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
    let mask = n.as_ref().and_then(|n| n.mask.as_deref());
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
            match (mask, n.as_ref().map(|n| n.id)) {
                (Some(m), Some(id)) => {
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
                            app.run("transparency.editOpacityMask", json!({"id": id.0})).ok();
                        }
                    }
                }
                _ => {
                    let (m, _) = ui.allocate_exact_size(vec2(50.0, 50.0), Sense::hover());
                    ui.painter().rect_stroke(m, 0.0, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
                    icons::paint(ui, "dc-mask-none", m.shrink(12.0), t.text_disabled);
                }
            }
        }
        ui.vertical(|ui| {
            let label = if mask.is_some() { "Release" } else { "Make Mask" };
            let enabled = mask.is_some() || (has && editing.is_none());
            let r = ui.add_enabled_ui(enabled, |ui| widgets::flat_button(ui, label, 96.0)).inner;
            if r.on_disabled_hover_text("Select the art (and, on top of it, the mask object)").clicked() {
                let id = if mask.is_some() { "transparency.releaseOpacityMask" } else { "transparency.makeOpacityMask" };
                app.run(id, json!({})).ok();
            }
            let (clip, invert) = mask.map(|m| (m.clip, m.invert)).unwrap_or((new_clip, new_invert));
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
    // The masked object's mask, also while editing it (the selection then is the mask art).
    let editing = app.session.active().is_some_and(|d| d.doc.mask_edit.is_some());
    let (_, n) = state(app, ui.ctx());
    let mask = n.as_ref().and_then(|n| n.mask.as_deref());
    for (label, id, enabled) in mask_items(mask, n.is_some() && !editing) {
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

/// The menu's opacity-mask rows (label, command, enabled) for the target's `mask`; Make needs a
/// target without one, outside mask editing (`can_make`).
fn mask_items(mask: Option<&OpacityMask>, can_make: bool) -> [(&'static str, &'static str, bool); 4] {
    match mask {
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
    }
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

    /// Run the panel and its menu for one headless frame: the texts drawn.
    fn frame(app: &mut VectorcraftApp) -> Vec<String> {
        fn texts(s: &egui::Shape, out: &mut Vec<String>) {
            match s {
                egui::Shape::Text(t) => out.push(t.galley.text().to_string()),
                egui::Shape::Vec(v) => v.iter().for_each(|s| texts(s, out)),
                _ => {}
            }
        }
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(app, ui);
            menu(app, ui);
        });
        out.textures_delta.clear();
        let mut v = vec![];
        out.shapes.iter().for_each(|c| texts(&c.shape, &mut v));
        v
    }

    fn run(app: &mut VectorcraftApp, id: &str, p: serde_json::Value) -> serde_json::Value {
        app.session.execute(id, &p).unwrap()
    }

    #[test]
    fn panel_in_mask_editing_mode_shows_and_edits_the_masked_object() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        run(&mut app, "file.new", json!({"width": 100, "height": 100}));
        let a = run(&mut app, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50}))["id"].as_u64().unwrap();
        run(&mut app, "transparency.set", json!({"opacity": 60}));
        // One object: an empty mask, drawn in editing mode.
        run(&mut app, "transparency.makeOpacityMask", json!({}));
        run(&mut app, "shape.ellipse", json!({"x": 0, "y": 0, "width": 30, "height": 30}));
        let texts = frame(&mut app);
        assert!(texts.iter().any(|t| t == "60%"), "the masked object's opacity, not the mask art's: {texts:?}");
        assert!(texts.iter().any(|t| t == "Release"));
        let (info, n) = state(&app, &egui::Context::default());
        assert_eq!((info.ids, n.as_ref().map(|n| n.id.0)), (vec![vectorcraft_doc::NodeId(a)], Some(a)));
        // The menu is enabled for the masked object while editing (Make is not).
        let items = mask_items(n.as_ref().and_then(|n| n.mask.as_deref()), false);
        assert_eq!(items.map(|(_, _, on)| on), [false, true, true, true]);
        // The panel's controls run without ids and reach the masked object.
        run(&mut app, "transparency.setOpacityMask", json!({"clip": false}));
        run(&mut app, "transparency.unlinkOpacityMask", json!({}));
        run(&mut app, "transparency.set", json!({"blend": "Multiply"}));
        frame(&mut app);
        let st = app.session.active().unwrap();
        let obj = st.doc.node(vectorcraft_doc::NodeId(a)).unwrap();
        let m = obj.mask.as_ref().unwrap();
        assert!(!m.clip && !m.linked && obj.blend == BlendMode::Multiply);
        assert!(st.doc.mask_edit.is_some());
        // Release leaves editing and puts the drawn mask art back.
        run(&mut app, "transparency.releaseOpacityMask", json!({}));
        assert!(frame(&mut app).iter().any(|t| t == "Make Mask"));
        assert!(app.session.active().unwrap().doc.mask_edit.is_none());
    }

    #[test]
    fn mixed_values_show_blank() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        run(&mut app, "file.new", json!({"width": 100, "height": 100}));
        let a = run(&mut app, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50}))["id"].clone();
        run(&mut app, "transparency.set", json!({"opacity": 50, "blend": "Screen"}));
        let b = run(&mut app, "shape.rectangle", json!({"x": 60, "y": 0, "width": 30, "height": 30}))["id"].clone();
        run(&mut app, "select.set", json!({"ids": [a, b]}));
        let texts = frame(&mut app);
        assert!(!texts.iter().any(|t| t.ends_with('%') || t == "Screen" || t == "Normal"), "{texts:?}");
        run(&mut app, "select.set", json!({"ids": [a]}));
        let texts = frame(&mut app);
        assert!(texts.iter().any(|t| t == "50%") && texts.iter().any(|t| t == "Screen"), "{texts:?}");
        // Nothing selected: the defaults for new art, disabled.
        run(&mut app, "select.none", json!({}));
        let texts = frame(&mut app);
        assert!(texts.iter().any(|t| t == "100%") && texts.iter().any(|t| t == "No Selection"), "{texts:?}");
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
