//! The Tools panel. Default: Illustrator 2026's categorized single-column toolbar (Select, Shapes,
//! Draw, Modify, Type, Navigate, Color). Window → Toolbars → Advanced shows every tool group.
//! Bottom: fill/stroke proxy, colour/gradient/none, drawing modes, screen mode, Edit Toolbar.

use egui::{Color32, CornerRadius, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_tools::{TOOL_GROUPS, ToolInfo, tool_info};

use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, icons, widgets};

const PITCH: f32 = 30.0;
const WIDTH: f32 = 48.0;

/// The Basic toolbar: (category, slots); each slot is a flyout group (first = default).
pub const BASIC: &[(&str, &[&[&str]])] = &[
    ("Select", &[&["selection"], &["directSelection", "groupSelection"], &["lasso", "magicWand"]]),
    (
        "Shapes",
        &[
            &["rectangle", "roundedRectangle", "star", "lineSegment", "arc", "spiral", "rectangularGrid", "polarGrid", "flare"],
            &["ellipse"],
            &["polygon"],
            &["shaper"],
        ],
    ),
    (
        "Draw",
        &[
            &["pencil", "smooth", "pathEraser", "join"],
            &["eraser", "scissors", "knife"],
            &["paintbrush", "blobBrush"],
            &["pen", "addAnchor", "deleteAnchor", "anchorPoint"],
            &["curvature"],
        ],
    ),
    (
        "Modify",
        &[
            &["width", "warp", "twirl", "pucker", "bloat", "scallop", "crystallize", "wrinkle"],
            &["rotate", "reflect", "scale", "shear", "reshape", "freeTransform", "puppetWarp"],
            &["shapeBuilder", "livePaintBucket", "livePaintSelection", "blend"],
        ],
    ),
    ("Type", &[&["areaType", "typeOnPath", "verticalType", "verticalAreaType", "verticalTypeOnPath"], &["type", "touchType"]]),
    ("Navigate", &[&["zoom"], &["hand", "printTiling"], &["rotateView"]]),
    ("Color", &[&["gradient", "mesh"], &["eyedropper", "measure"]]),
];

fn tip(t: &ToolInfo) -> String {
    match crate::shortcut_editor::tool_shortcut(t.id) {
        Some(s) => format!("{} ({})", t.label, s),
        None => t.label.to_string(),
    }
}

/// Slots of the current layout: (category label for the first slot of a category, tool ids).
fn slots(app: &VectorcraftApp) -> Vec<(Option<&'static str>, Vec<&'static str>)> {
    if app.ui.toolbar_advanced {
        TOOL_GROUPS.iter().map(|g| (None, g.iter().map(|t| t.id).collect())).collect()
    } else {
        BASIC.iter().flat_map(|(cat, ss)| ss.iter().enumerate().map(move |(i, s)| (if i == 0 { Some(*cat) } else { None }, s.to_vec()))).collect()
    }
}

/// Remember the tool shown in the slot that contains `id`.
pub fn remember(app: &mut VectorcraftApp, id: &str) {
    for (_, s) in slots(app) {
        if s.contains(&id) {
            app.ui.slot_tool.insert(s[0].to_string(), id.to_string());
        }
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let all = slots(app);
    let avail = ui.available_height();
    let labels = all.iter().filter(|s| s.0.is_some()).count() as f32 * 20.0;
    let need = all.len() as f32 * PITCH + labels + 190.0;
    let cols = if app.ui.toolbar_double || avail < need { 2 } else { 1 };
    let w = if cols == 2 { 76.0 } else { WIDTH };
    egui::Panel::left("toolbar")
        .resizable(false)
        .exact_size(w)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin { left: 0, right: 0, top: 0, bottom: 4 }).stroke(Stroke::new(1.5, t.border)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            // Dock header with » and a grip strip.
            let (hdr, hresp) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::click());
            ui.painter().rect_filled(hdr, 0.0, t.tab_strip);
            icons::paint(
                ui,
                if cols == 2 { "chevrons-left" } else { "chevrons-right" },
                egui::Rect::from_min_size(hdr.left_top() + vec2(3.0, 2.0), vec2(10.0, 10.0)),
                if hresp.hovered() { t.text_strong } else { t.text },
            );
            if hresp.on_hover_text("Toggle single/double column").clicked() {
                app.ui.toolbar_double = !app.ui.toolbar_double;
            }
            let (grip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
            for k in 0..6 {
                ui.painter().line_segment(
                    [pos2(grip.center().x - 9.0, grip.top() + 1.5 + k as f32 * 0.6), pos2(grip.center().x + 9.0, grip.top() + 1.5 + k as f32 * 0.6)],
                    Stroke::new(0.4, t.text_disabled),
                );
            }
            let active = app.session.tool_id();
            let mut open_flyout: Option<(Vec<&'static str>, egui::Rect)> = None;
            let mut i = 0;
            while i < all.len() {
                if let Some(cat) = all[i].0 {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
                    let label = if cols == 1 && cat.len() > 6 { format!("{}...", &cat[..4]) } else { cat.to_string() };
                    ui.painter().text(r.center() + vec2(0.0, 2.0), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), t.text);
                }
                // One row = `cols` slots (a category label always starts a new row).
                let mut row = vec![i];
                while row.len() < cols && i + row.len() < all.len() && all[i + row.len()].0.is_none() {
                    row.push(i + row.len());
                }
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    if cols == 1 {
                        ui.add_space((WIDTH - 36.0) / 2.0);
                    } else {
                        ui.add_space(2.0);
                    }
                    for &k in &row {
                        let slot = &all[k].1;
                        let shown_id = if slot.contains(&active) {
                            active.to_string()
                        } else {
                            app.ui.slot_tool.get(slot[0]).cloned().unwrap_or_else(|| slot[0].to_string())
                        };
                        let Some(shown) = tool_info(&shown_id).or_else(|| tool_info(slot[0])) else { continue };
                        let is_active = slot.contains(&active);
                        let (rect, resp) = ui.allocate_exact_size(vec2(36.0, PITCH - 1.0), Sense::click_and_drag());
                        let well = egui::Rect::from_center_size(rect.center(), vec2(35.5, 27.5));
                        if is_active {
                            ui.painter().rect_filled(well, CornerRadius::same(1), t.tool_active);
                        } else if resp.hovered() {
                            ui.painter().rect_filled(well, CornerRadius::same(1), t.hover);
                        }
                        let ir = egui::Rect::from_center_size(rect.center(), vec2(18.0, 18.0));
                        icons::paint(ui, icons::tool_icon(shown.icon), ir, if is_active { t.text_strong } else { t.icon });
                        if slot.len() > 1 {
                            let c = rect.center() + vec2(12.5, 10.0);
                            ui.painter().add(egui::Shape::convex_polygon(vec![c, c + vec2(-3.5, 0.0), c + vec2(0.0, -3.5)], t.icon, Stroke::NONE));
                        }
                        let long_press =
                            resp.is_pointer_button_down_on() && ui.input(|inp| inp.pointer.press_start_time().is_some_and(|s| inp.time - s > 0.35));
                        let alt = ui.input(|inp| inp.modifiers.alt);
                        if (resp.secondary_clicked() || long_press) && slot.len() > 1 {
                            open_flyout = Some((slot.clone(), rect));
                        } else if resp.clicked() && alt && slot.len() > 1 {
                            let idx = slot.iter().position(|x| *x == shown.id).unwrap_or(0);
                            app.select_tool(slot[(idx + 1) % slot.len()]);
                        } else if resp.clicked() {
                            app.select_tool(shown.id);
                        }
                        resp.on_hover_text(tip(shown));
                    }
                });
                i += row.len();
            }
            if let Some((slot, rect)) = open_flyout {
                app.ui.flyout = Some(0);
                ui.data_mut(|d| {
                    d.insert_temp(egui::Id::new("flyout-anchor"), rect);
                    d.insert_temp(egui::Id::new("flyout-tools"), slot.iter().map(|s| s.to_string()).collect::<Vec<String>>());
                });
            }
            ui.add_space(8.0);
            bottom_controls(app, ui, &t);
        });
    flyout(app, ui.ctx());
}

fn bottom_controls(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens) {
    let (fill, stroke) = crate::panels::current_paints(app);
    ui.vertical_centered(|ui| {
        let (f, s, swap, def) = widgets::fill_stroke_proxy(ui, &fill, &stroke, app.session.fill_active, 36.0);
        if f {
            app.session.fill_active = true;
        }
        if s {
            app.session.fill_active = false;
        }
        if swap {
            app.run("paint.swap", json!({})).ok();
        }
        if def {
            app.run("paint.default", json!({})).ok();
        }
    });
    ui.add_space(5.0);
    let target = if app.session.fill_active { "paint.setFill" } else { "paint.setStroke" };
    ui.horizontal(|ui| {
        ui.add_space((ui.available_width() - 27.0) / 2.0);
        ui.spacing_mut().item_spacing.x = 2.0;
        let s = 7.5;
        let last = app.session.paint.fill.color().unwrap_or(vectorcraft_color::Color::BLACK);
        let (r, resp) = ui.allocate_exact_size(vec2(s, s), Sense::click());
        widgets::paint_chip(ui, r, &vectorcraft_color::Paint::solid(last));
        if resp.on_hover_text("Color (,)").clicked() {
            app.run(target, json!({"color": last.to_hex()})).ok();
        }
        let (r, resp) = ui.allocate_exact_size(vec2(s, s), Sense::click());
        widgets::paint_chip(ui, r, &vectorcraft_color::Paint::Gradient(Box::new(vectorcraft_color::GradientPaint::new(Default::default()))));
        if resp.on_hover_text("Gradient (.)").clicked() {
            app.run(target, json!({"gradient": {"kind": "linear"}})).ok();
        }
        let (r, resp) = ui.allocate_exact_size(vec2(s, s), Sense::click());
        widgets::paint_chip(ui, r, &vectorcraft_color::Paint::None);
        if resp.on_hover_text("None (/)").clicked() {
            app.run(target, json!({"none": true})).ok();
        }
    });
    ui.add_space(6.0);
    ui.vertical_centered(|ui| {
        let modes = ["dc-draw-normal", "dc-draw-behind", "dc-draw-inside"];
        let names = ["Draw Normal", "Draw Behind", "Draw Inside"];
        let m = match app.session.draw_mode {
            vectorcraft_engine::DrawMode::Normal => 0,
            vectorcraft_engine::DrawMode::Behind => 1,
            vectorcraft_engine::DrawMode::Inside => 2,
        };
        if widgets::icon_button(ui, modes[m], &format!("{} (Shift+D)", names[m]), m != 0, 26.0).clicked() {
            app.run("view.drawMode", json!({})).ok();
        }
        if widgets::icon_button(ui, "dc-screen-mode", "Change Screen Mode (F)", false, 26.0).clicked() {
            app.run("view.screenMode", json!({})).ok();
        }
        if widgets::icon_button(ui, "ellipsis", "Edit Toolbar", false, 26.0).clicked() {
            app.ui.dialog = Some(crate::state::Dialog::new("allTools", json!({})));
        }
    });
    let _ = t;
}

fn flyout(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if app.ui.flyout.is_none() {
        return;
    }
    let tools: Vec<String> = ctx.data(|d| d.get_temp(egui::Id::new("flyout-tools"))).unwrap_or_default();
    if tools.is_empty() {
        app.ui.flyout = None;
        return;
    }
    let anchor: egui::Rect =
        ctx.data(|d| d.get_temp(egui::Id::new("flyout-anchor"))).unwrap_or(egui::Rect::from_min_size(pos2(40.0, 100.0), vec2(32.0, 32.0)));
    let t = Tokens::get(ctx);
    let mut chosen = None;
    let resp = egui::Area::new(egui::Id::new("tool-flyout")).order(egui::Order::Foreground).fixed_pos(anchor.right_top() + vec2(8.0, -1.0)).show(
        ctx,
        |ui| {
            egui::Frame::NONE
                .fill(t.panel)
                .stroke(Stroke::new(1.0, t.input_border))
                .shadow(egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(90) })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
                    let w = 250.0;
                    for id in &tools {
                        let Some(tool) = tool_info(id) else { continue };
                        let active = app.session.tool_id() == tool.id;
                        let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, 0.0, t.hover);
                        }
                        if active {
                            ui.painter().rect_filled(egui::Rect::from_center_size(r.left_center() + vec2(12.0, 0.0), vec2(5.0, 5.0)), 0.0, t.text);
                        }
                        let ir = egui::Rect::from_min_size(r.min + vec2(24.0, 6.0), vec2(18.0, 18.0));
                        icons::paint(ui, icons::tool_icon(tool.icon), ir, t.icon);
                        let color = if active { t.flyout_active } else { t.text_strong };
                        ui.painter().text(
                            r.left_center() + vec2(52.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            tool.label,
                            egui::FontId::proportional(13.0),
                            color,
                        );
                        if let Some(sc) = crate::shortcut_editor::tool_shortcut(tool.id) {
                            ui.painter().text(
                                r.right_center() - vec2(18.0, 0.0),
                                egui::Align2::RIGHT_CENTER,
                                format!("({sc})"),
                                egui::FontId::proportional(13.0),
                                color,
                            );
                        }
                        if resp.clicked() {
                            chosen = Some(tool.id);
                        }
                    }
                });
        },
    );
    if let Some(id) = chosen {
        app.select_tool(id);
    } else if resp.response.clicked_elsewhere() {
        app.ui.flyout = None;
    }
    let _ = theme::semibold;
}
