//! Appearance panel: the full stack like Illustrator — the object row, each stroke/fill row (eye,
//! disclosure, link label, swatch, weight) with "Opacity: Default" sub-rows, fx rows with inline
//! parameter editors (effect.setParams), the object's Opacity row and the bottom bar.

use std::sync::OnceLock;

use egui::{Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};
use vectorcraft_color::{BlendMode, Paint};
use vectorcraft_doc::{AppearanceItem, Effect, Node, NodeId};

use super::{first_selected, live_run, pstate, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, TransparencyEdit, menu_item};
use crate::{VectorcraftApp, icons};

const ROW: f32 = 30.0;
pub(super) const EYE_W: f32 = 26.0;

/// What is selected in the stack (for Duplicate / Delete). A fill/stroke row (also the owner of a
/// selected item effect) is the engine's active appearance item (`appearance.setActiveItem`), so
/// paint edits, the fx button and the Effect menu target it; effect rows are panel state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sel {
    #[default]
    None,
    Item(usize),
    /// An object-level effect.
    Effect(usize),
    /// Effect `k` of fill/stroke item `i`.
    ItemEffect(usize, usize),
}

impl Sel {
    /// The fill/stroke row this selection makes the active item.
    fn item(self) -> Option<usize> {
        match self {
            Sel::Item(i) | Sel::ItemEffect(i, _) => Some(i),
            Sel::None | Sel::Effect(_) => None,
        }
    }
    /// `{item, index}` of a selected effect row (`item: null` for the object's own effects).
    fn effect(self) -> Option<Value> {
        match self {
            Sel::Effect(k) => Some(json!({"item": null, "index": k})),
            Sel::ItemEffect(i, k) => Some(json!({"item": i, "index": k})),
            Sel::None | Sel::Item(_) => None,
        }
    }
}

/// The selected row of `node` (the first selected object): a selected effect row while that
/// effect still exists and agrees with the engine's active item, else the active item's row.
fn current_sel(app: &VectorcraftApp, ctx: &egui::Context, node: Option<&Node>) -> Sel {
    let Some(n) = node else { return Sel::None };
    let (owner, sel): (Option<NodeId>, Sel) = pstate(ctx, "ap-sel");
    let exists = |item: Option<usize>, k: usize| owner == Some(n.id) && n.appearance.effects_at(item).is_some_and(|fx| k < fx.len());
    match (app.session.appearance_item(), sel) {
        (Some(i), Sel::ItemEffect(j, k)) if i == j && exists(Some(i), k) => sel,
        (Some(i), _) => Sel::Item(i),
        (None, Sel::Effect(k)) if exists(None, k) => sel,
        (None, _) => Sel::None,
    }
}

/// Select a row; its fill/stroke becomes the active item that paint and effect edits target.
pub(crate) fn select_row(app: &mut VectorcraftApp, ctx: &egui::Context, sel: Sel) {
    if app.session.appearance_item() != sel.item() {
        app.run("appearance.setActiveItem", json!({ "index": sel.item() })).ok();
    }
    let owner = app.session.active().and_then(|d| d.selection.objects.first().copied());
    set_pstate(ctx, "ap-sel", (owner, sel));
}

fn catalog() -> &'static [(String, String, Vec<String>)] {
    static C: OnceLock<Vec<(String, String, Vec<String>)>> = OnceLock::new();
    C.get_or_init(|| {
        vectorcraft_effects::effect_catalog()
            .into_iter()
            .map(|e| (e.id.to_string(), e.label.trim_end_matches('…').to_string(), e.menu.iter().map(|s| s.to_string()).collect()))
            .collect()
    })
}

/// Display label of an effect id ("stylize.dropShadow" → "Drop Shadow").
pub fn effect_label(id: &str) -> String {
    catalog().iter().find(|c| c.0 == id).map(|c| c.1.clone()).unwrap_or_else(|| id.rsplit('.').next().unwrap_or(id).to_string())
}

/// "Opacity: Default" or "Opacity: 50% Multiply".
pub fn opacity_text(opacity: f32, blend: BlendMode) -> String {
    if (opacity - 1.0).abs() < 1e-4 && blend == BlendMode::Normal {
        "Default".into()
    } else if blend == BlendMode::Normal {
        format!("{:.0}%", opacity * 100.0)
    } else {
        format!("{:.0}% {}", opacity * 100.0, blend.label())
    }
}

fn row(ui: &mut Ui, selected: bool) -> (Rect, egui::Response) {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW), Sense::click_and_drag());
    if selected {
        ui.painter().rect_filled(r, 0.0, t.row_selected);
    }
    ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.input_border));
    ui.painter().line_segment([pos2(r.left() + EYE_W, r.top()), pos2(r.left() + EYE_W, r.bottom())], Stroke::new(1.0, t.border));
    (r, resp)
}

fn eye(ui: &mut Ui, r: Rect, id: impl std::hash::Hash + std::fmt::Debug, on: bool, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let er = Rect::from_center_size(pos2(r.left() + EYE_W / 2.0, r.center().y), vec2(16.0, 16.0));
    let resp = ui.interact(er, ui.id().with(id), if enabled { Sense::click() } else { Sense::hover() });
    let col = if !enabled {
        t.text_disabled
    } else if resp.hovered() {
        t.text_strong
    } else {
        t.icon
    };
    icons::paint(ui, if on { "eye" } else { "eye-off" }, er, col);
    enabled && resp.on_hover_text("Click to toggle visibility").clicked()
}

fn chevron(ui: &mut Ui, r: Rect, id: impl std::hash::Hash + std::fmt::Debug, open: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let cr = Rect::from_center_size(pos2(r.left() + EYE_W + 12.0, r.center().y), vec2(12.0, 12.0));
    let resp = ui.interact(cr, ui.id().with(id), Sense::click());
    icons::paint(ui, if open { "chevron-down" } else { "chevron-right" }, cr, if resp.hovered() { t.text_strong } else { t.icon });
    resp.clicked()
}

fn text(ui: &Ui, pos: egui::Pos2, s: &str, strong: bool) {
    let t = Tokens::get(ui.ctx());
    ui.painter().text(pos, egui::Align2::LEFT_CENTER, s, egui::FontId::proportional(12.5), if strong { t.text_strong } else { t.text });
}

fn chip(ui: &Ui, r: Rect, p: &Paint) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(r.expand(1.0), 0.0, egui::Color32::BLACK);
    widgets::paint_chip(ui, r, p);
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, egui::Color32::WHITE), StrokeKind::Inside);
    let _ = t;
}

/// A dotted-underline link drawn at `pos`; returns clicked.
fn link(ui: &mut Ui, pos: egui::Pos2, id: impl std::hash::Hash + std::fmt::Debug, s: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(s.to_string(), egui::FontId::proportional(12.5), t.text_strong);
    let r = Rect::from_min_size(pos2(pos.x, pos.y - galley.size().y / 2.0), galley.size());
    let resp = ui.interact(r, ui.id().with(id), Sense::click());
    ui.painter().galley(r.min, galley, t.text_strong);
    let mut x = r.left();
    while x < r.right() {
        ui.painter().line_segment([pos2(x, r.bottom()), pos2((x + 1.0).min(r.right()), r.bottom())], Stroke::new(1.0, t.text_dim));
        x += 2.5;
    }
    resp.clicked()
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let node = first_selected(app);
    let sel = current_sel(app, ui.ctx(), node.as_ref());
    let hide_thumb: bool = pstate(ui.ctx(), "ap-hide-thumb");
    widgets::list_box(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        // Object row (clicking it targets the whole object again).
        let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW + 4.0), Sense::click());
        if resp.clicked() && sel != Sel::None {
            select_row(app, ui.ctx(), Sel::None);
        }
        ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.input_border));
        let mixed = mixed_appearances(app);
        let label = if mixed { "Mixed Appearances" } else { object_label(app) };
        // A linked object names its graphic style ("Rectangle: Sunshine").
        let label = match app.session.selection_graphic_style() {
            Some((g, true)) if !mixed => std::borrow::Cow::Owned(format!("{label}: {}", g.name)),
            _ => std::borrow::Cow::Borrowed(label),
        };
        if !hide_thumb {
            let th = Rect::from_min_size(r.left_center() + vec2(6.0, -12.0), vec2(24.0, 24.0));
            let fill = node.as_ref().map(|n| n.appearance.fill_paint()).unwrap_or_else(|| app.session.paint.fill.clone());
            chip(ui, th, &fill);
        }
        text(ui, r.left_center() + vec2(if hide_thumb { 8.0 } else { 46.0 }, 0.0), &label, true);
        match &node {
            // Objects that differ have no common stack to list.
            Some(_) if mixed => {}
            Some(n) => stack(app, ui, n, sel),
            None => default_stack(app, ui),
        }
        // Spacer row like Illustrator's empty tail.
        ui.allocate_exact_size(vec2(ui.available_width(), 10.0), Sense::hover());
    });
    bottom(app, ui, node.as_ref(), sel);
}

/// The selection as the Control bar and the object row name it: "No Selection", "Mixed Objects"
/// or the object's kind ("Path", "Type", …).
pub(crate) fn object_label(app: &VectorcraftApp) -> &'static str {
    let Some(st) = app.session.active() else { return "No Selection" };
    match st.selection.objects.as_slice() {
        [] => "No Selection",
        [id] => st.doc.node(*id).map_or("No Selection", Node::kind_label),
        _ => "Mixed Objects",
    }
}

/// Do the selected objects differ in appearance (fills, strokes, effects, opacity or blend mode)?
fn mixed_appearances(app: &VectorcraftApp) -> bool {
    let Some(st) = app.session.active() else { return false };
    let mut nodes = st.selection.objects.iter().filter_map(|id| st.doc.node(*id));
    let Some(first) = nodes.next() else { return false };
    nodes.any(|n| n.appearance != first.appearance || n.opacity != first.opacity || n.blend != first.blend)
}

/// The defaults for new art when nothing is selected (read-only rows).
fn default_stack(app: &mut VectorcraftApp, ui: &mut Ui) {
    let p = app.session.paint.clone();
    for (lbl, paint, w) in [("Stroke:", p.stroke, Some(p.stroke_width)), ("Fill:", p.fill, None)] {
        let (r, _) = row(ui, false);
        eye(ui, r, ("ap-def-eye", lbl), true, false);
        text(ui, r.left_center() + vec2(EYE_W + 24.0, 0.0), lbl, false);
        chip(ui, Rect::from_min_size(r.left_center() + vec2(EYE_W + 76.0, -9.0), vec2(18.0, 18.0)), &paint);
        if let Some(w) = w {
            text(ui, r.left_center() + vec2(EYE_W + 106.0, 0.0), &format!("{w} pt"), false);
        }
    }
    let (r, _) = row(ui, false);
    eye(ui, r, "ap-def-op", true, false);
    text(ui, r.left_center() + vec2(EYE_W + 24.0, 0.0), "Opacity: Default", false);
}

fn stack(app: &mut VectorcraftApp, ui: &mut Ui, n: &Node, sel: Sel) {
    let t = Tokens::get(ui.ctx());
    let mut drop: Option<(usize, usize)> = None;
    let mut stopped = false;
    let dragging: Option<usize> = pstate(ui.ctx(), "ap-drag");
    let mut row_rects = vec![];
    for (i, it) in n.appearance.items.iter().enumerate().rev() {
        let open: bool = pstate(ui.ctx(), &format!("ap-open-{i}"));
        let (r, resp) = row(ui, sel == Sel::Item(i));
        row_rects.push((i, r));
        let (visible, paint) = (it.visible(), it.paint());
        let is_stroke = !it.is_fill();
        if eye(ui, r, ("ap-eye", i), visible, true) {
            app.run("appearance.setItem", json!({"index": i, "visible": !visible})).ok();
        }
        if chevron(ui, r, ("ap-chev", i), open) {
            set_pstate(ui.ctx(), &format!("ap-open-{i}"), !open);
        }
        let lx = r.left() + EYE_W + 24.0;
        if is_stroke {
            if link(ui, pos2(lx, r.center().y), ("ap-link", i), "Stroke:") {
                select_row(app, ui.ctx(), Sel::Item(i));
                app.ui.open_panel = Some("stroke".into());
            }
        } else {
            text(ui, pos2(lx, r.center().y), "Fill:", false);
        }
        // Swatch with a swatches popup that sets this item's paint.
        let cr = Rect::from_min_size(pos2(r.left() + EYE_W + 76.0, r.center().y - 9.0), vec2(18.0, 18.0));
        chip(ui, cr, paint);
        let cresp = ui.interact(cr.expand(2.0), ui.id().with(("ap-chip", i)), Sense::click()).on_hover_text("Click to choose a swatch");
        egui::Popup::menu(&cresp).show(|ui| {
            swatch_picker(app, ui, i);
        });
        if let AppearanceItem::Stroke(st) = it {
            let fr = Rect::from_min_size(pos2(r.left() + EYE_W + 104.0, r.center().y - 12.0), vec2(56.0, 24.0));
            let mut child = ui.new_child(egui::UiBuilder::new().max_rect(fr).layout(egui::Layout::left_to_right(egui::Align::Center)));
            if let Some(w) = widgets::num_field(&mut child, ("ap-w", i), Some(st.width), vectorcraft_doc::Unit::Points, 56.0) {
                app.run("appearance.setItem", json!({"index": i, "weight": w})).ok();
            }
        }
        if resp.clicked() {
            select_row(app, ui.ctx(), Sel::Item(i));
        }
        if resp.double_clicked() {
            app.ui.open_panel = Some(if is_stroke { "stroke" } else { "color" }.into());
        }
        if resp.drag_started() {
            set_pstate(ui.ctx(), "ap-drag", Some(i));
        }
        if resp.drag_stopped() {
            stopped = true;
        }
        if open {
            opacity_row(app, ui, Some(i), it.opacity(), it.blend());
            for (k, e) in it.effects().iter().enumerate() {
                effect_row(app, ui, Some(i), k, e, sel == Sel::ItemEffect(i, k));
            }
        }
    }
    if stopped
        && let Some(from) = dragging
        && let Some(p) = ui.ctx().pointer_interact_pos()
    {
        drop = Some((from, target_index(&row_rects, p.y)));
    }
    if dragging.is_some() && !ui.ctx().input(|i| i.pointer.any_down()) {
        set_pstate::<Option<usize>>(ui.ctx(), "ap-drag", None);
    }
    if let Some(p) = ui.ctx().pointer_interact_pos()
        && dragging.is_some()
    {
        // Insertion marker.
        if let Some((_, r)) = row_rects.iter().find(|(_, r)| r.y_range().contains(p.y)) {
            let y = if p.y < r.center().y { r.top() } else { r.bottom() };
            ui.painter().line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(2.0, t.accent));
        }
    }
    // Object-level effects.
    for (k, e) in n.appearance.effects.iter().enumerate() {
        effect_row(app, ui, None, k, e, sel == Sel::Effect(k));
    }
    opacity_row(app, ui, None, n.opacity, n.blend);
    if let Some((from, to_row)) = drop
        && from != to_row
    {
        // The engine keeps the moved row active when it was.
        app.run("appearance.moveItem", json!({"from": from, "to": to_row})).ok();
    }
}

/// Paint-order index a drop at `y` lands on (rows are listed top = last item).
fn target_index(rows: &[(usize, Rect)], y: f32) -> usize {
    rows.iter().find(|(_, r)| r.y_range().contains(y)).map(|(i, _)| *i).unwrap_or_else(|| {
        if rows.first().is_some_and(|(_, r)| y < r.top()) { rows.first().map(|r| r.0).unwrap_or(0) } else { rows.last().map(|r| r.0).unwrap_or(0) }
    })
}

/// The Opacity row of the object (`item: None`) or, indented, of fill/stroke `item`. Clicking it
/// opens a popup with the shared opacity and blend controls (`transparency.set` on that item).
fn opacity_row(app: &mut VectorcraftApp, ui: &mut Ui, item: Option<usize>, opacity: f32, blend: BlendMode) {
    let (r, resp) = row(ui, false);
    eye(ui, r, ("ap-op-eye", item), true, false);
    let lx = r.left() + EYE_W + 24.0 + if item.is_some() { 12.0 } else { 0.0 };
    let clicked = link(ui, pos2(lx, r.center().y), ("ap-op-link", item), "Opacity:") || resp.clicked();
    text(ui, pos2(lx + 56.0, r.center().y), &opacity_text(opacity, blend), false);
    egui::Popup::menu(&resp)
        .open_memory(clicked.then_some(egui::SetOpenCommand::Toggle))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| match widgets::opacity_blend(ui, ("ap-op", item), Some(opacity), Some(blend), true) {
            Some(TransparencyEdit::Blend(b)) => {
                app.run("transparency.set", json!({"item": item, "blend": b.label()})).ok();
            }
            Some(TransparencyEdit::Opacity(o, phase)) => live_run(app, "Opacity", "transparency.set", json!({"item": item, "opacity": o}), phase),
            None => {}
        });
}

/// An effect row: of the object (`item: None`) or, indented under it, of fill/stroke `item`.
fn effect_row(app: &mut VectorcraftApp, ui: &mut Ui, item: Option<usize>, k: usize, e: &Effect, selected: bool) {
    let t = Tokens::get(ui.ctx());
    let open_key = format!("ap-fx-open-{item:?}-{k}");
    let open: bool = pstate(ui.ctx(), &open_key);
    let this = item.map_or(Sel::Effect(k), |i| Sel::ItemEffect(i, k));
    let (r, resp) = row(ui, selected);
    if eye(ui, r, ("ap-fx-eye", item, k), e.visible, true) {
        app.run("effect.setParams", json!({"item": item, "index": k, "visible": !e.visible})).ok();
    }
    let indent = if item.is_some() { 12.0 } else { 0.0 };
    let r = Rect::from_min_max(pos2(r.left() + indent, r.top()), r.max);
    if chevron(ui, r, ("ap-fx-chev", item, k), open) {
        set_pstate(ui.ctx(), &open_key, !open);
    }
    let lx = r.left() + EYE_W + 24.0;
    if link(ui, pos2(lx, r.center().y), ("ap-fx-link", item, k), &effect_label(&e.id)) {
        set_pstate(ui.ctx(), &open_key, !open);
        select_row(app, ui.ctx(), this);
    }
    icons::paint(ui, "dc-fx", Rect::from_center_size(r.right_center() - vec2(14.0, 0.0), vec2(16.0, 16.0)), t.icon);
    if resp.clicked() {
        select_row(app, ui.ctx(), this);
    }
    if resp.double_clicked() {
        set_pstate(ui.ctx(), &open_key, !open);
    }
    if open {
        effect_editor(app, ui, item, k, e);
    }
}

/// Inline editor for an applied effect's parameters (numbers, booleans, strings, colours);
/// edits go through `effect.setParams`.
fn effect_editor(app: &mut VectorcraftApp, ui: &mut Ui, item: Option<usize>, k: usize, e: &Effect) {
    let t = Tokens::get(ui.ctx());
    let defaults = vectorcraft_effects::effect_catalog().into_iter().find(|c| c.id == e.id).map(|c| c.defaults).unwrap_or(Value::Null);
    let mut params = defaults.as_object().cloned().unwrap_or_default();
    if let Some(cur) = e.params.as_object() {
        for (key, v) in cur {
            params.insert(key.clone(), v.clone());
        }
    }
    let mut change: Option<(String, Value)> = None;
    egui::Frame::NONE.fill(t.panel_darker).inner_margin(egui::Margin { left: (EYE_W + 12.0) as i8, right: 6, top: 4, bottom: 4 }).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        if params.is_empty() {
            widgets::dim_label(ui, "No options");
        }
        for (key, v) in &params {
            ui.horizontal(|ui| {
                let label = humanize(key);
                ui.add_sized(vec2(84.0, 22.0), egui::Label::new(egui::RichText::new(label).size(11.5).color(t.text)).truncate());
                if let Some(cur) = widgets::blend_param(key, v) {
                    if let Some(m) = widgets::blend_param_dropdown(ui, ("fx-blend", item, k, key.as_str()), cur) {
                        change = Some((key.clone(), m));
                    }
                    return;
                }
                match v {
                    Value::Bool(b) => {
                        if widgets::check(ui, "", *b, true) {
                            change = Some((key.clone(), json!(!b)));
                        }
                    }
                    Value::Number(n) => {
                        let x = n.as_f64().unwrap_or(0.0);
                        if let Some(nx) = widgets::plain_field(ui, ("fx", item, k, key.as_str()), x, "", 2, 70.0) {
                            change = Some((key.clone(), json!(nx)));
                        }
                    }
                    Value::String(s) => {
                        let mut buf = s.clone();
                        let r = ui.add(egui::TextEdit::singleline(&mut buf).desired_width(90.0));
                        if r.lost_focus() && buf != *s {
                            change = Some((key.clone(), json!(buf)));
                        }
                    }
                    other => {
                        widgets::dim_label(ui, &other.to_string());
                    }
                }
            });
        }
    });
    if let Some((key, v)) = change {
        app.run("effect.setParams", json!({"item": item, "index": k, "params": {key: v}})).ok();
    }
}

/// "offsetX" → "Offset X".
pub fn humanize(key: &str) -> String {
    let mut out = String::new();
    for (i, ch) in key.chars().enumerate() {
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else if ch.is_uppercase() {
            out.push(' ');
            out.push(ch);
        } else {
            out.push(ch);
        }
    }
    out
}

/// Small swatch grid inside the appearance chip popup.
fn swatch_picker(app: &mut VectorcraftApp, ui: &mut Ui, index: usize) {
    let Some(st) = app.session.active() else { return };
    let mut all: Vec<(String, Paint)> = st.doc.swatches.iter().map(|s| (s.name.clone(), s.paint.clone())).collect();
    for g in &st.doc.swatch_groups {
        all.extend(g.swatches.iter().map(|s| (s.name.clone(), s.paint.clone())));
    }
    ui.set_max_width(12.0 * 17.0 + 8.0);
    let mut chosen = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.5, 1.5);
        for (name, p) in &all {
            let (r, resp) = ui.allocate_exact_size(vec2(15.5, 15.5), Sense::click());
            widgets::swatch_tile(ui, r, p, false, resp.hovered());
            if resp.on_hover_text(name).clicked() {
                chosen = Some((name.clone(), p.is_none()));
            }
        }
    });
    if let Some((name, none)) = chosen {
        let params = if none { json!({"index": index, "none": true}) } else { json!({"index": index, "swatch": name}) };
        app.run("appearance.setItem", params).ok();
        ui.close();
    }
}

fn bottom(app: &mut VectorcraftApp, ui: &mut Ui, node: Option<&Node>, sel: Sel) {
    let has = node.is_some();
    widgets::bottom_bar(ui, |ui| {
        if widgets::icon_button_enabled(ui, "dc-new-stroke", "Add New Stroke", false, has, 24.0).clicked() {
            app.run("appearance.addStroke", json!({})).ok();
        }
        if widgets::icon_button_enabled(ui, "dc-new-fill", "Add New Fill", false, has, 24.0).clicked() {
            app.run("appearance.addFill", json!({})).ok();
        }
        let fx = widgets::icon_button_enabled(ui, "dc-fx", "Add New Effect", false, has, 24.0);
        egui::Popup::menu(&fx).show(|ui| {
            fx_menu(app, ui);
        });
        ui.add_space((ui.available_width() - 3.0 * 28.0).max(0.0));
        let basic = node.is_none_or(|n| n.appearance.is_basic());
        if widgets::icon_button_enabled(ui, "dc-clear", "Clear Appearance", false, has, 24.0).clicked() {
            app.run("appearance.clear", json!({})).ok();
        }
        let can_dup = has && sel != Sel::None;
        if widgets::icon_button_enabled(ui, "dc-new-item", "Duplicate Selected Item", false, can_dup, 24.0).clicked() {
            duplicate_selected(app, sel);
        }
        let can_del = has && sel != Sel::None;
        if widgets::icon_button_enabled(ui, "trash-2", "Delete Selected Item", false, can_del, 24.0).clicked() {
            delete_selected(app, ui, sel);
        }
        let _ = basic;
    });
}

fn duplicate_selected(app: &mut VectorcraftApp, sel: Sel) {
    match (sel, sel.effect()) {
        (Sel::Item(i), _) => app.run("appearance.duplicateItem", json!({"index": i})).ok(),
        (_, Some(fx)) => app.run("effect.duplicate", fx).ok(),
        _ => None,
    };
}

fn delete_selected(app: &mut VectorcraftApp, ui: &Ui, sel: Sel) {
    let ok = match (sel, sel.effect()) {
        (Sel::Item(i), _) => app.run("appearance.removeItem", json!({"index": i})).is_ok(),
        (_, Some(fx)) => app.run("effect.remove", fx).is_ok(),
        _ => false,
    };
    // Removing the active item clears it in the engine; a removed item effect leaves its item
    // selected.
    if ok {
        select_row(app, ui.ctx(), if let Sel::ItemEffect(i, _) = sel { Sel::Item(i) } else { Sel::None });
    }
}

/// The fx menu: effects grouped by their Effect-menu submenu; opens the effect dialog.
fn fx_menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let mut groups: Vec<(String, Vec<(String, String)>)> = vec![];
    for (id, label, menu) in catalog() {
        let g = menu.get(1).cloned().unwrap_or_else(|| "Other".into());
        match groups.iter_mut().find(|(n, _)| *n == g) {
            Some((_, v)) => v.push((id.clone(), label.clone())),
            None => groups.push((g, vec![(id.clone(), label.clone())])),
        }
    }
    for (g, items) in groups {
        ui.menu_button(g, |ui| {
            for (id, label) in items {
                if ui.button(format!("{label}…")).clicked() {
                    app.run("effect.dialog", json!({"effect": id})).ok();
                    ui.close();
                }
            }
        });
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let node = first_selected(app);
    let has = node.is_some();
    let sel = current_sel(app, ui.ctx(), node.as_ref());
    if menu_item(ui, "Add New Fill", has, false) {
        app.run("appearance.addFill", json!({})).ok();
    }
    if menu_item(ui, "Add New Stroke", has, false) {
        app.run("appearance.addStroke", json!({})).ok();
    }
    ui.separator();
    if menu_item(ui, "Duplicate Item", has && sel != Sel::None, false) {
        duplicate_selected(app, sel);
    }
    if menu_item(ui, "Remove Item", has && sel != Sel::None, false) {
        delete_selected(app, ui, sel);
    }
    if menu_item(ui, "Clear Appearance", has, false) {
        app.run("appearance.clear", json!({})).ok();
    }
    if menu_item(ui, "Reduce to Basic Appearance", has && !node.as_ref().is_some_and(|n| n.appearance.is_basic()), false) {
        app.run("appearance.reduceToBasic", json!({})).ok();
    }
    ui.separator();
    menu_item(ui, "New Art Has Basic Appearance", false, true);
    let hide: bool = pstate(ui.ctx(), "ap-hide-thumb");
    if menu_item(ui, if hide { "Show Thumbnail" } else { "Hide Thumbnail" }, true, false) {
        set_pstate(ui.ctx(), "ap-hide-thumb", !hide);
    }
    ui.separator();
    let style = app.session.selection_graphic_style().map(|(g, _)| g.name.clone());
    let redefine = style.as_ref().map_or_else(|| "Redefine Graphic Style".into(), |n| format!("Redefine Graphic Style \u{201c}{n}\u{201d}"));
    if menu_item(ui, &redefine, style.is_some(), false) {
        app.run("graphicStyle.redefine", json!({})).ok();
    }
    menu_item(ui, "Show All Hidden Attributes", false, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_labels() {
        assert_eq!(opacity_text(1.0, BlendMode::Normal), "Default");
        assert_eq!(opacity_text(0.5, BlendMode::Normal), "50%");
        assert_eq!(opacity_text(0.5, BlendMode::Multiply), "50% Multiply");
    }

    #[test]
    fn labels() {
        assert_eq!(humanize("offsetX"), "Offset X");
        assert_eq!(humanize("blur"), "Blur");
        assert!(!effect_label("stylize.dropShadow").contains('…'));
        assert_eq!(effect_label("x.unknownThing"), "unknownThing");
    }

    #[test]
    fn drop_targets() {
        let r = |y: f32| Rect::from_min_size(pos2(0.0, y), vec2(100.0, ROW));
        let rows = vec![(2, r(0.0)), (1, r(30.0)), (0, r(60.0))];
        assert_eq!(target_index(&rows, 45.0), 1);
        assert_eq!(target_index(&rows, -10.0), 2);
        assert_eq!(target_index(&rows, 500.0), 0);
    }
}
