//! Gradient panel: type buttons, angle / aspect ratio / reverse, the gradient slider with draggable
//! stops and midpoint diamonds (click below the ramp adds a stop, drag a stop off to remove it),
//! and stop Opacity / Location fields. Stop colours are edited with the Color panel.

use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};
use vectorcraft_color::gradient::{MIN_STOPS, insert_stop, midpoint_from_pos, midpoint_pos, move_stop, remove_stop, set_midpoint};
use vectorcraft_color::{Gradient, GradientKind, GradientPaint, GradientStop, Paint};

use super::{active_paint, live_run, pstate, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, Live, menu_item};
use crate::{VectorcraftApp, icons};

/// Dragging a stop this far below the ramp removes it.
pub const REMOVE_DISTANCE: f32 = 28.0;

/// Select stop `i` (the session's selected stop, shared with the Gradient tool's annotator, the
/// Color panel and agents). Run after the edit that creates it.
fn select_stop(app: &mut VectorcraftApp, i: usize) {
    app.run("gradient.selectStop", json!({ "index": i })).ok();
}

// ---------- pure ramp math (unit-tested) ----------

/// Offset (0..1) of an x position on a ramp spanning `left..left + width`.
pub fn x_to_offset(x: f32, left: f32, width: f32) -> f32 {
    ((x - left) / width.max(1.0)).clamp(0.0, 1.0)
}

/// Stops as `paint.editGradient` JSON.
pub use vectorcraft_tools::params::stops_json;

// ---------- UI ----------

fn current(app: &VectorcraftApp) -> Option<GradientPaint> {
    match active_paint(app) {
        Paint::Gradient(g) => Some(*g),
        _ => None,
    }
}

fn edit(app: &mut VectorcraftApp, params: Value, phase: Live) {
    let mut p = params;
    p["stroke"] = json!(!app.session.fill_active);
    live_run(app, "Gradient", "paint.editGradient", p, phase);
}

/// Drag state of the ramp: which handle is being dragged.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Drag {
    #[default]
    None,
    Stop(usize),
    Mid(usize),
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let gp = current(app);
    let fallback = GradientPaint::new(Gradient::default());
    let g = gp.clone().unwrap_or(fallback);
    let is_grad = gp.is_some();
    let kind = g.gradient.kind;
    // Header: gradient swatch + type buttons + Edit Gradient.
    ui.horizontal(|ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(40.0, 40.0), Sense::click());
        widgets::paint_chip(ui, r, &Paint::Gradient(Box::new(g.clone())));
        ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
        if resp.on_hover_text("Gradient Fill — click to apply").clicked() && !is_grad {
            edit(app, json!({}), Live::Released);
        }
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                widgets::dim_label(ui, "Type:");
                for (k, icon, tip) in [
                    (GradientKind::Linear, "dc-grad-linear", "Linear Gradient"),
                    (GradientKind::Radial, "dc-grad-radial", "Radial Gradient"),
                    (GradientKind::Freeform, "dc-grad-freeform", "Freeform Gradient"),
                ] {
                    if widgets::icon_button(ui, icon, tip, is_grad && kind == k, 24.0).clicked() {
                        edit(app, json!({"kind": k.label().to_lowercase()}), Live::Released);
                    }
                }
            });
            ui.horizontal(|ui| {
                if widgets::flat_button(ui, "Edit Gradient", 96.0).clicked() {
                    app.select_tool("gradient");
                }
            });
        });
    });
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        widgets::dim_label(ui, "Stroke:");
        for (icon, tip) in [
            ("dc-stroke-center", "Gradient inside stroke"),
            ("dc-stroke-outside", "Gradient along stroke (on the roadmap)"),
            ("dc-stroke-inside", "Gradient across stroke (on the roadmap)"),
        ] {
            widgets::icon_button_enabled(ui, icon, tip, icon == "dc-stroke-center" && !app.session.fill_active, false, 22.0);
        }
    });
    ui.horizontal(|ui| {
        icons::icon(ui, "rotate-ccw", 15.0, t.icon).on_hover_text("Angle");
        if let Some(a) = widgets::plain_field(ui, "grad-angle", g.geom.map(|x| x.angle_deg()).unwrap_or(g.angle), "°", 1, 64.0) {
            edit(app, json!({"angle": a}), Live::Released);
        }
        ui.add_space(4.0);
        let radial = kind == GradientKind::Radial;
        icons::icon(ui, "scaling", 15.0, if radial { t.icon } else { t.text_disabled }).on_hover_text("Aspect Ratio");
        let asp = g.geom.map(|x| x.aspect * 100.0).unwrap_or(100.0);
        if radial {
            if let Some(a) = widgets::plain_field(ui, "grad-aspect", asp, "%", 1, 60.0) {
                edit(app, json!({"aspect": a}), Live::Released);
            }
        } else {
            ui.add_enabled(false, egui::Label::new(egui::RichText::new(format!("{asp:.0}%")).color(t.text_disabled)));
        }
        if widgets::icon_button_enabled(ui, "dc-reverse", "Reverse Gradient", false, is_grad, 22.0).clicked() {
            edit(app, json!({"reverse": true}), Live::Released);
        }
    });
    ui.add_space(6.0);
    ramp(app, ui, &g.gradient, is_grad);
    ui.add_space(4.0);
    // Stop fields.
    let sel = app.session.gradient_stop.filter(|i| *i < g.gradient.stops.len());
    ui.horizontal(|ui| {
        let enabled = is_grad && sel.is_some();
        let stop = sel.and_then(|i| g.gradient.stops.get(i)).copied();
        widgets::dim_label(ui, "Opacity:");
        let op = stop.map(|s| s.opacity as f64 * 100.0).unwrap_or(100.0);
        if let Some(v) = widgets::plain_field(ui, "grad-op", op, "%", 0, 54.0)
            && enabled
            && let Some(i) = sel
        {
            let mut stops = g.gradient.stops.clone();
            stops[i].opacity = (v / 100.0).clamp(0.0, 1.0) as f32;
            edit(app, json!({"stops": stops_json(&stops)}), Live::Released);
        }
        widgets::dim_label(ui, "Location:");
        let loc = stop.map(|s| s.offset as f64 * 100.0).unwrap_or(0.0);
        if let Some(v) = widgets::plain_field(ui, "grad-loc", loc, "%", 1, 54.0)
            && enabled
            && let Some(i) = sel
        {
            let (stops, ni) = move_stop(&g.gradient.stops, i, (v / 100.0) as f32);
            edit(app, json!({"stops": stops_json(&stops)}), Live::Released);
            select_stop(app, ni);
        }
        let can_del = enabled && g.gradient.stops.len() > MIN_STOPS;
        if widgets::icon_button_enabled(ui, "trash-2", "Delete Stop", false, can_del, 22.0).clicked()
            && let Some(i) = sel
            && let Some(stops) = remove_stop(&g.gradient.stops, i)
        {
            edit(app, json!({"stops": stops_json(&stops)}), Live::Released);
            select_stop(app, i.min(stops.len() - 1));
        }
    });
    if let Some(i) = sel
        && is_grad
    {
        let c = g.gradient.stops[i].color;
        ui.horizontal(|ui| {
            widgets::dim_label(ui, "Stop color:");
            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            widgets::swatch_tile(ui, r, &Paint::solid(c), false, resp.hovered());
            widgets::dim_label(ui, &c.to_hex().to_uppercase());
            if resp.on_hover_text("Edit the stop in the Color panel").clicked() {
                app.ui.open_panel = Some("color".into());
            }
        });
    }
    if !is_grad {
        widgets::dim_label(ui, "Click the ramp or a type button to apply a gradient.");
    }
}

/// The gradient slider: ramp, stops below, midpoint diamonds above.
fn ramp(app: &mut VectorcraftApp, ui: &mut Ui, g: &Gradient, is_grad: bool) {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (area, _) = ui.allocate_exact_size(vec2(w, 50.0), Sense::hover());
    let bar = Rect::from_min_size(area.min + vec2(8.0, 10.0), vec2(w - 16.0, 18.0));
    // Checkerboard under the ramp (opacity).
    let cell = 6.0;
    let mut y = bar.top();
    let mut row = 0;
    while y < bar.bottom() {
        let mut x = bar.left();
        let mut col = row % 2;
        while x < bar.right() {
            let r = Rect::from_min_max(pos2(x, y), pos2((x + cell).min(bar.right()), (y + cell).min(bar.bottom())));
            ui.painter().rect_filled(r, 0.0, if col % 2 == 0 { Color32::WHITE } else { Color32::from_gray(204) });
            x += cell;
            col += 1;
        }
        y += cell;
        row += 1;
    }
    let n = (bar.width() / 2.0) as usize;
    for i in 0..n {
        let tt = (i as f32 + 0.5) / n as f32;
        let (c, o) = g.sample(tt);
        let [r, gg, b, a] = c.to_rgba8(o);
        let rr =
            Rect::from_min_size(pos2(bar.left() + i as f32 * bar.width() / n as f32, bar.top()), vec2(bar.width() / n as f32 + 0.5, bar.height()));
        ui.painter().rect_filled(rr, 0.0, Color32::from_rgba_unmultiplied(r, gg, b, a));
    }
    ui.painter().rect_stroke(bar, 0.0, Stroke::new(1.0, t.border), StrokeKind::Outside);
    let x_of = |o: f32| bar.left() + o * bar.width();
    let sel = app.session.gradient_stop;
    let mut drag: Drag = pstate(ui.ctx(), "grad-drag");
    let stops = &g.stops;
    // While dragging, edits are computed against the stops as they were when the drag began (the
    // document shows the preview).
    let origin: Vec<GradientStop> = if drag == Drag::None { stops.clone() } else { pstate::<Vec<GradientStop>>(ui.ctx(), "grad-origin") };
    let origin = if origin.len() == stops.len() { origin } else { stops.clone() };
    let mut changed: Option<(Vec<GradientStop>, Live)> = None;
    // The stop to select once `changed` is applied.
    let mut select: Option<usize> = None;
    // Midpoint diamonds.
    for i in 0..stops.len().saturating_sub(1) {
        let Some(p) = midpoint_pos(stops, i) else { continue };
        let c = pos2(x_of(p), bar.top() - 5.0);
        let rect = Rect::from_center_size(c, vec2(10.0, 10.0));
        let resp = ui.interact(rect, ui.id().with(("grad-mid", i)), Sense::click_and_drag());
        let active = drag == Drag::Mid(i) || resp.hovered();
        let pts = vec![c + vec2(0.0, -4.0), c + vec2(4.0, 0.0), c + vec2(0.0, 4.0), c + vec2(-4.0, 0.0)];
        ui.painter().add(egui::Shape::convex_polygon(pts, if active { Color32::WHITE } else { t.icon }, Stroke::new(1.0, t.border)));
        if !is_grad {
            continue;
        }
        if resp.drag_started() {
            drag = Drag::Mid(i);
            set_pstate(ui.ctx(), "grad-origin", stops.clone());
        }
        if (resp.dragged() || resp.drag_stopped())
            && let Some(pp) = resp.interact_pointer_pos()
            && let Some(m) = midpoint_from_pos(&origin, i, x_to_offset(pp.x, bar.left(), bar.width()))
        {
            let phase = if resp.drag_stopped() { Live::Released } else { Live::Dragging };
            changed = Some((set_midpoint(&origin, i, m), phase));
            if resp.drag_stopped() {
                drag = Drag::None;
            }
        }
    }
    // Stops (house-shaped markers under the ramp).
    let mut hit_stop = false;
    for (i, s) in stops.iter().enumerate() {
        let x = x_of(s.offset);
        let top = bar.bottom() + 2.0;
        let marker = Rect::from_min_size(pos2(x - 6.0, top), vec2(12.0, 16.0));
        let resp = ui.interact(marker, ui.id().with(("grad-stop", i)), Sense::click_and_drag());
        hit_stop |= resp.hovered() || resp.dragged();
        let dragging_this = drag == Drag::Stop(i);
        let off = dragging_this && resp.interact_pointer_pos().is_some_and(|p| p.y > bar.bottom() + REMOVE_DISTANCE) && origin.len() > MIN_STOPS;
        let outline = if sel == Some(i) { t.accent } else { t.border };
        let body = vec![pos2(x, top), pos2(x + 6.0, top + 5.0), pos2(x + 6.0, top + 15.0), pos2(x - 6.0, top + 15.0), pos2(x - 6.0, top + 5.0)];
        if !off {
            ui.painter().add(egui::Shape::convex_polygon(body, if sel == Some(i) { t.text_strong } else { t.icon }, Stroke::new(1.0, outline)));
            let chip = Rect::from_min_size(pos2(x - 4.0, top + 6.0), vec2(8.0, 7.0));
            ui.painter().rect_filled(chip, 0.0, super::c32(&s.color));
        }
        if !is_grad {
            continue;
        }
        if resp.clicked() || resp.drag_started() {
            select = Some(i);
        }
        if resp.drag_started() {
            drag = Drag::Stop(i);
            set_pstate(ui.ctx(), "grad-origin", stops.clone());
        }
        if (resp.dragged() || resp.drag_stopped())
            && dragging_this
            && let Some(pp) = resp.interact_pointer_pos()
        {
            let removing = pp.y > bar.bottom() + REMOVE_DISTANCE && origin.len() > MIN_STOPS;
            if resp.drag_stopped() {
                drag = Drag::None;
                if removing {
                    if let Some(v) = remove_stop(&origin, i) {
                        select = Some(i.min(v.len() - 1));
                        changed = Some((v, Live::Released));
                    }
                } else {
                    let (v, ni) = move_stop(&origin, i, x_to_offset(pp.x, bar.left(), bar.width()));
                    select = Some(ni);
                    changed = Some((v, Live::Released));
                }
            } else if removing {
                // Dragged off: show the gradient without moving the stop until release.
                changed = Some((origin.clone(), Live::Dragging));
            } else {
                let (v, _) = move_stop(&origin, i, x_to_offset(pp.x, bar.left(), bar.width()));
                changed = Some((v, Live::Dragging));
            }
        }
    }
    // Click below the ramp (not on a stop) adds a stop; clicking the ramp applies a gradient.
    let below = Rect::from_min_max(pos2(bar.left(), bar.bottom()), pos2(bar.right(), area.bottom()));
    let resp = ui.interact(below, ui.id().with("grad-add"), Sense::click());
    if resp.hovered() && !hit_stop && is_grad {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Copy);
    }
    if resp.clicked()
        && !hit_stop
        && let Some(p) = resp.interact_pointer_pos()
    {
        if is_grad {
            let (v, i) = insert_stop(g, x_to_offset(p.x, bar.left(), bar.width()));
            select = Some(i);
            changed = Some((v, Live::Released));
        } else {
            edit(app, json!({}), Live::Released);
        }
    }
    let bar_resp = ui.interact(bar, ui.id().with("grad-bar"), Sense::click());
    if bar_resp.clicked() && !is_grad {
        edit(app, json!({}), Live::Released);
    }
    set_pstate(ui.ctx(), "grad-drag", drag);
    if let Some((v, phase)) = changed {
        edit(app, json!({"stops": stops_json(&v)}), phase);
    }
    if let Some(i) = select {
        select_stop(app, i);
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let g = current(app);
    menu_item(ui, "Hide Options", false, false);
    if menu_item(ui, "Add to Swatches", g.is_some(), false)
        && let Some(g) = g
    {
        app.run("swatch.new", super::paint_params(&Paint::Gradient(Box::new(g)))).ok();
    }
    if menu_item(ui, "Reverse Gradient", current(app).is_some(), false) {
        edit(app, json!({"reverse": true}), Live::Released);
    }
    if menu_item(ui, "Reset to White, Black", true, false) {
        let d = Gradient::default();
        edit(app, json!({"stops": stops_json(&d.stops)}), Live::Released);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_and_json() {
        assert_eq!(x_to_offset(50.0, 0.0, 200.0), 0.25);
        assert_eq!(x_to_offset(-5.0, 0.0, 200.0), 0.0);
        assert_eq!(x_to_offset(500.0, 0.0, 200.0), 1.0);
        let j = stops_json(&Gradient::default().stops);
        assert_eq!(j.as_array().unwrap().len(), 2);
        assert_eq!(j[1]["midpoint"], json!(0.5));
    }
}
