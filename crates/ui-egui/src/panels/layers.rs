//! The Layers panel: tree with visibility/lock columns, layer colour bars, thumbnails, target
//! circles and selection squares; bottom bar with new/delete.

use std::collections::HashSet;

use egui::{Color32, Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;
use vectorcraft_doc::{Node, NodeId, NodeKind};

use crate::theme::Tokens;
use crate::{VectorcraftApp, icons, widgets};

const ROW: f32 = 26.0;

fn expanded_id() -> egui::Id {
    egui::Id::new("layers-expanded")
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    let doc = st.doc.clone();
    let sel: HashSet<NodeId> = st.selection.objects.iter().copied().collect();
    let current = st.active_layer;
    let mut expanded: HashSet<u64> = ui.data(|d| d.get_temp(expanded_id())).unwrap_or_else(|| doc.layers.iter().map(|l| l.id.0).collect());
    let mut actions: Vec<(String, serde_json::Value)> = vec![];
    // Search field ("Search All").
    crate::widgets::search_field(ui, egui::Id::new("layers-search"), "Search All");
    ui.add_space(6.0);
    let h = ui.available_height() - 34.0;
    egui::ScrollArea::vertical().max_height(h).auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        for l in doc.layers.iter().rev() {
            row(ui, &doc, l, 0, &sel, current, &mut expanded, &mut actions, &t);
        }
    });
    ui.data_mut(|d| d.insert_temp(expanded_id(), expanded));
    if ui.input(|i| i.pointer.any_released()) {
        ui.data_mut(|d| d.remove::<u64>(egui::Id::new("layers-drag")));
    }
    // Bottom bar.
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::hover());
    ui.painter().line_segment([bar.left_top(), bar.right_top()], Stroke::new(1.0, t.divider));
    let n = doc.layers.len();
    ui.painter().text(
        bar.left_center() + vec2(4.0, 0.0),
        egui::Align2::LEFT_CENTER,
        format!("{n} Layer{}", if n == 1 { "" } else { "s" }),
        egui::FontId::proportional(11.5),
        t.text_dim,
    );
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(bar).layout(egui::Layout::right_to_left(egui::Align::Center)));
    if widgets::icon_button(&mut child, "trash-2", "Delete Selection", false, 24.0).clicked() {
        if app.session.active().is_some_and(|d| !d.selection.is_empty()) {
            actions.push(("edit.clear".into(), json!({})));
        } else {
            actions.push(("layer.delete".into(), json!({})));
        }
    }
    if widgets::icon_button(&mut child, "file-plus", "Create New Layer", false, 24.0).clicked() {
        actions.push(("layer.new".into(), json!({})));
    }
    if widgets::icon_button(&mut child, "plus", "Create New Sublayer", false, 24.0).clicked() {
        actions.push(("layer.newSublayer".into(), json!({})));
    }
    if widgets::icon_button(&mut child, "frame", "Make/Release Clipping Mask", false, 24.0).clicked() {
        actions.push(("object.clippingMask.make".into(), json!({})));
    }
    if widgets::icon_button(&mut child, "search", "Locate Object", false, 24.0).clicked() {
        // Expand ancestors of the selection.
        if let Some(st) = app.session.active() {
            let mut ex: HashSet<u64> = ui.data(|d| d.get_temp(expanded_id())).unwrap_or_default();
            for id in &st.selection.objects {
                for a in st.doc.ancestry(*id).unwrap_or_default() {
                    ex.insert(a.0);
                }
            }
            ui.data_mut(|d| d.insert_temp(expanded_id(), ex));
        }
    }
    for (c, p) in actions {
        app.run(&c, p).ok();
    }
}

#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut Ui,
    doc: &vectorcraft_doc::Document,
    n: &Node,
    depth: usize,
    sel: &HashSet<NodeId>,
    current: Option<NodeId>,
    expanded: &mut HashSet<u64>,
    actions: &mut Vec<(String, serde_json::Value)>,
    t: &Tokens,
) {
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click_and_drag());
    let is_sel = sel.contains(&n.id);
    let child_sel = !is_sel
        && n.children().is_some_and(|_| {
            let mut any = false;
            n.walk(&mut |c| any |= c.id != n.id && sel.contains(&c.id));
            any
        });
    let color = {
        let c = doc.layer_color(n.id);
        Color32::from_rgb(c[0], c[1], c[2])
    };
    if n.is_layer() && Some(n.id) == current {
        ui.painter().rect_filled(r, 0.0, t.row_selected);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, t.hover.gamma_multiply(0.6));
    }
    ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.input_border));
    if n.is_layer() && Some(n.id) == current {
        // Current-layer marker: small triangle in the top-right corner.
        let c = r.right_top();
        ui.painter().add(egui::Shape::convex_polygon(vec![c, c + vec2(-6.0, 0.0), c + vec2(0.0, 6.0)], Color32::from_gray(0xcc), Stroke::NONE));
    }
    // Eye and lock columns.
    let eye = egui::Rect::from_min_size(r.min, vec2(25.0, ROW));
    let lock = egui::Rect::from_min_size(r.min + vec2(25.0, 0.0), vec2(25.0, ROW));
    ui.painter().line_segment([eye.right_top(), eye.right_bottom()], Stroke::new(1.0, t.input_border));
    ui.painter().line_segment([lock.right_top(), lock.right_bottom()], Stroke::new(1.0, t.input_border));
    let er = ui.interact(eye, ui.id().with(("eye", n.id.0)), Sense::click());
    if n.visible {
        icons::paint(ui, "eye", egui::Rect::from_center_size(eye.center(), vec2(14.0, 14.0)), t.icon);
    }
    if er.clicked() {
        let cmd = if n.is_layer() { "layer.setProps" } else { "object.setProps" };
        let key = if n.is_layer() { "id" } else { "ids" };
        let idv = if n.is_layer() { json!(n.id.0) } else { json!([n.id.0]) };
        actions.push((cmd.into(), json!({key: idv, "visible": !n.visible})));
    }
    let lr = ui.interact(lock, ui.id().with(("lock", n.id.0)), Sense::click());
    if n.locked {
        icons::paint(ui, "lock", egui::Rect::from_center_size(lock.center(), vec2(12.0, 12.0)), t.icon);
    } else if lr.hovered() {
        icons::paint(ui, "lock", egui::Rect::from_center_size(lock.center(), vec2(12.0, 12.0)), t.text_disabled);
    }
    if lr.clicked() {
        let cmd = if n.is_layer() { "layer.setProps" } else { "object.setProps" };
        let key = if n.is_layer() { "id" } else { "ids" };
        let idv = if n.is_layer() { json!(n.id.0) } else { json!([n.id.0]) };
        actions.push((cmd.into(), json!({key: idv, "locked": !n.locked})));
    }
    // Layer colour bar.
    let mut x = lock.right() + 2.0;
    if n.is_layer() {
        ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x - 1.0, r.top()), vec2(4.0, ROW)), 0.0, color);
    }
    x += 6.0 + depth as f32 * 14.0;
    // Disclosure.
    let has_children = n.children().is_some_and(|c| !c.is_empty()) && !matches!(n.kind, NodeKind::Compound { .. });
    if has_children {
        let open = expanded.contains(&n.id.0);
        let dr = egui::Rect::from_min_size(egui::pos2(x, r.top() + 5.0), vec2(14.0, 16.0));
        let dresp = ui.interact(dr, ui.id().with(("disc", n.id.0)), Sense::click());
        icons::paint(ui, if open { "chevron-down" } else { "chevron-right" }, dr, t.text_dim);
        if dresp.clicked() {
            if open {
                expanded.remove(&n.id.0);
            } else {
                expanded.insert(n.id.0);
            }
        }
    }
    x += 16.0;
    // Thumbnail.
    let th = egui::Rect::from_min_size(egui::pos2(x, r.top() + 2.0), vec2(22.0, 22.0));
    ui.painter().rect_filled(th, 0.0, Color32::WHITE);
    ui.painter().rect_stroke(th, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Outside);
    if !real_thumb(ui, doc, n, th) {
        thumb(ui, n, th);
    }
    x += 26.0;
    // Name.
    let name = n.display_name();
    let font = egui::FontId::proportional(13.0);
    let rename_id = egui::Id::new("layers-rename");
    let renaming: Option<(u64, String)> = ui.data(|d| d.get_temp(rename_id));
    let name_rect = egui::Rect::from_min_max(egui::pos2(x - 2.0, r.top() + 3.0), egui::pos2(r.right() - 44.0, r.bottom() - 3.0));
    match renaming {
        Some((rid, mut buf)) if rid == n.id.0 => {
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(name_rect));
            let te = child.add(egui::TextEdit::singleline(&mut buf).desired_width(name_rect.width()).font(font.clone()));
            te.request_focus();
            if te.lost_focus() {
                ui.data_mut(|d| d.remove::<(u64, String)>(rename_id));
                if child.input(|i| !i.key_pressed(egui::Key::Escape)) && buf != name {
                    let cmd = if n.is_layer() { "layer.setProps" } else { "object.setProps" };
                    let key = if n.is_layer() { "id" } else { "ids" };
                    let idv = if n.is_layer() { json!(n.id.0) } else { json!([n.id.0]) };
                    actions.push((cmd.into(), json!({key: idv, "name": buf})));
                }
            } else {
                ui.data_mut(|d| d.insert_temp(rename_id, (n.id.0, buf)));
            }
        }
        _ => {
            ui.painter().with_clip_rect(name_rect).text(egui::pos2(x, r.center().y), egui::Align2::LEFT_CENTER, name.clone(), font, t.text);
        }
    }
    // Target circle and selection square.
    let col_x = r.right() - 43.5;
    ui.painter().line_segment([egui::pos2(col_x, r.top()), egui::pos2(col_x, r.bottom())], Stroke::new(1.0, t.input_border));
    let tc = egui::pos2(r.right() - 28.0, r.center().y);
    let styled = has_styled_target(n);
    ui.painter().circle_stroke(tc, 5.0, Stroke::new(1.0, t.icon));
    if is_sel {
        ui.painter().circle_stroke(tc, 2.8, Stroke::new(1.0, t.icon));
    }
    if styled {
        ui.painter().circle_filled(tc, 3.2, t.icon);
    }
    let sq = egui::pos2(r.right() - 11.0, r.center().y);
    if is_sel {
        let q = egui::Rect::from_center_size(sq, vec2(7.0, 7.0));
        ui.painter().rect_filled(q, 0.0, color);
        ui.painter().rect_stroke(q, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
    } else if child_sel {
        ui.painter().rect_filled(egui::Rect::from_center_size(sq, vec2(4.0, 4.0)), 0.0, color);
    }
    let sq_resp = ui.interact(egui::Rect::from_center_size(sq, vec2(18.0, ROW)), ui.id().with(("selsq", n.id.0)), Sense::click());
    if sq_resp.clicked() {
        if n.is_layer() {
            actions.push(("layer.selectAll".into(), json!({"id": n.id.0})));
        } else if ui.input(|i| i.modifiers.shift) {
            actions.push(("select.toggle".into(), json!({"id": n.id.0})));
        } else {
            actions.push(("select.set".into(), json!({"ids": [n.id.0]})));
        }
    } else if resp.clicked() {
        if n.is_layer() {
            actions.push(("layer.setCurrent".into(), json!({"id": n.id.0})));
        } else {
            actions.push(("select.set".into(), json!({"ids": [n.id.0]})));
        }
    }
    if resp.double_clicked() {
        ui.data_mut(|d| d.insert_temp(egui::Id::new("layers-rename"), (n.id.0, n.display_name())));
    }
    // Drag to reorder: drop onto a row moves the dragged node above it (into its parent).
    let drag_id = egui::Id::new("layers-drag");
    if resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(drag_id, n.id.0));
    }
    let dragging: Option<u64> = ui.data(|d| d.get_temp(drag_id));
    if let Some(src) = dragging
        && src != n.id.0
        && ui.rect_contains_pointer(r)
    {
        let above = ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| p.y < r.center().y);
        let y = if above { r.top() } else { r.bottom() };
        ui.painter().line_segment([egui::pos2(r.left() + 46.0, y), egui::pos2(r.right(), y)], Stroke::new(2.0, t.accent));
        if ui.input(|i| i.pointer.any_released()) {
            let (parent, index) = match doc.position(n.id) {
                Some((par, idx, _)) => (par, if above { idx + 1 } else { idx }),
                None => (None, 0),
            };
            // A layer dropped on a non-layer goes into that row's container; top level only for layers.
            let src_is_layer = doc.node(vectorcraft_doc::NodeId(src)).is_some_and(|x| x.is_layer());
            if parent.is_some() || src_is_layer {
                actions.push(("node.move".into(), json!({"id": src, "parent": parent.map(|p| p.0), "index": index})));
            }
            ui.data_mut(|d| d.remove::<u64>(drag_id));
        }
    }

    if has_children && expanded.contains(&n.id.0) {
        for c in n.children().unwrap().iter().rev() {
            row(ui, doc, c, depth + 1, sel, current, expanded, actions, t);
        }
    }
}

/// Whether an object's target circle is filled: its appearance is not basic or its transparency
/// (opacity, blend mode, isolation, knockout or an opacity mask) is not the default.
fn has_styled_target(n: &Node) -> bool {
    !n.appearance.is_basic() || !n.has_default_transparency()
}

/// A real rendered thumbnail, cached by node identity (unchanged nodes keep their `Arc`
/// allocation, so the address is a free change detector). Only rendered for visible rows.
fn real_thumb(ui: &Ui, doc: &vectorcraft_doc::Document, n: &Node, r: egui::Rect) -> bool {
    use std::cell::RefCell;
    use std::collections::HashMap;
    thread_local! {
        static RENDERER: RefCell<vectorcraft_render::Renderer> = RefCell::new(vectorcraft_render::Renderer::new());
        static CACHE: RefCell<HashMap<(usize, u64), egui::TextureHandle>> = RefCell::new(HashMap::new());
    }
    if !ui.is_rect_visible(r) {
        return true;
    }
    let key = (n as *const Node as usize, n.id.0);
    let ppp = ui.ctx().pixels_per_point();
    let px = (r.width() * ppp).round() as u32;
    let tex = CACHE.with(|c| c.borrow().get(&key).cloned()).or_else(|| {
        let img = RENDERER.with(|rr| rr.borrow_mut().render_thumbnail(doc, n.id, px.max(8)))?;
        let color = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
        let tex = ui.ctx().load_texture(format!("layer-thumb-{}", n.id.0), color, egui::TextureOptions::LINEAR);
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() > 2000 {
                c.clear();
            }
            c.insert(key, tex.clone());
        });
        Some(tex)
    });
    match tex {
        Some(t) => {
            ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            true
        }
        None => false,
    }
}

/// Tiny vector thumbnail painted with egui (fallback when rendering isn't possible).
fn thumb(ui: &Ui, n: &Node, r: egui::Rect) {
    let Some(b) = n.visual_bounds() else { return };
    let s = ((r.width() - 3.0) as f64 / b.width().max(b.height()).max(1e-6)) as f32;
    let off = r.center() - vec2(b.center().x as f32 * s, b.center().y as f32 * s);
    let p = ui.painter().with_clip_rect(r);
    let mut count = 0;
    n.walk(&mut |c| {
        if count > 40 {
            return;
        }
        if let Some(pd) = c.path_data() {
            count += 1;
            let col = c.appearance.fill_paint().color().or(c.appearance.stroke_paint().color()).map(|cc| {
                let [a, b2, d, _] = cc.to_rgba8(1.0);
                Color32::from_rgb(a, b2, d)
            });
            if let Some(bb) = pd.bounds() {
                let rr = egui::Rect::from_min_max(off + vec2(bb.x0 as f32 * s, bb.y0 as f32 * s), off + vec2(bb.x1 as f32 * s, bb.y1 as f32 * s));
                p.rect_filled(rr, 0.0, col.unwrap_or(Color32::from_gray(120)));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;

    /// Filled target circles drawn by one headless frame of the panel.
    fn filled_targets(app: &mut VectorcraftApp, ctx: &egui::Context) -> usize {
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(app, ui));
        out.textures_delta.clear();
        out.shapes.iter().filter(|c| matches!(&c.shape, egui::Shape::Circle(cs) if cs.radius == 3.2 && cs.fill != Color32::TRANSPARENT)).count()
    }

    #[test]
    fn target_circle_fills_for_non_default_transparency() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: serde_json::Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 100, "height": 100}));
        let id = run(&mut app, "shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50}))["id"].clone();
        run(&mut app, "select.set", json!({ "ids": [id] }));
        let ctx = egui::Context::default();
        assert_eq!(filled_targets(&mut app, &ctx), 0);
        run(&mut app, "transparency.set", json!({"blend": "multiply"}));
        assert_eq!(filled_targets(&mut app, &ctx), 1, "a Multiply object");
        run(&mut app, "transparency.set", json!({"blend": "normal", "knockout": true}));
        assert_eq!(filled_targets(&mut app, &ctx), 1, "a knockout group");
        run(&mut app, "transparency.set", json!({"knockout": false}));
        run(&mut app, "appearance.addStroke", json!({}));
        assert_eq!(filled_targets(&mut app, &ctx), 1, "two strokes");
    }
}
