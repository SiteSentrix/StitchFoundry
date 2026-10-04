//! Window chrome: application bar (menus), Control bar, document tabs, status bar.

use egui::{CornerRadius, Sense, Stroke, StrokeKind, Ui, vec2};
use serde_json::json;
use vectorcraft_color::Paint;
use vectorcraft_doc::NodeKind;

use crate::panels::stroke as stroke_panel;
use crate::state::{ZOOM_STOPS, zoom_label};
use crate::theme::{self, Tokens};
use crate::widgets::{self, paint_chip};
use crate::{VectorcraftApp, icons, menus};

pub fn app_bar(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let left = if app.integrated_titlebar { 78 } else { 8 };
    egui::Panel::top("app_bar").exact_size(44.0).frame(egui::Frame::NONE.fill(t.app_bar).inner_margin(egui::Margin { left, right: 14, top: 0, bottom: 0 }).stroke(Stroke::new(1.0, t.border))).show(ui, |ui| {
        ui.horizontal_centered(|ui| {
            // Brand mark (our own artwork, matches the app icon).
            let (r, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
            crate::brand::paint_mark(ui.painter(), r);
            ui.add_space(4.0);
            if widgets::icon_button(ui, "house", "Home", false, 24.0).clicked() {
                app.ui.dialog = Some(crate::state::Dialog::new("newDocument", json!({"preset": "Letter", "width": "612 pt", "height": "792 pt", "units": "Points", "artboards": 1, "colorMode": "RGB", "name": "Untitled-1"})));
            }
            ui.add_space(2.0);
            if app.native_menu {
                let full = ui.max_rect();
                ui.painter().text(full.center(), egui::Align2::CENTER_CENTER, "VectorCraft", egui::FontId::proportional(13.5), t.text);
            } else {
                menus::menu_bar(app, ui);
            }
            let full = ui.max_rect();
            let right = egui::Rect::from_min_max(egui::pos2(full.right() - 440.0, full.top()), full.right_bottom());
            let mut rui = ui.new_child(egui::UiBuilder::new().max_rect(right).layout(egui::Layout::right_to_left(egui::Align::Center)));
            let ui = &mut rui;
            // Workspace switcher: shows the current workspace, opens Window → Workspace.
            let ws = ui.painter().layout_no_wrap(app.ui.workspace.clone(), egui::FontId::proportional(12.0), t.text);
            let (wr, wresp) = ui.allocate_exact_size(vec2((ws.size().x + 36.0).clamp(112.0, 190.0), 24.0), Sense::click());
            ui.painter().rect_filled(wr, CornerRadius::same(4), if wresp.hovered() { t.hover } else { t.panel });
            ui.painter().with_clip_rect(wr.shrink2(vec2(4.0, 0.0))).galley(wr.left_center() + vec2(10.0, -ws.size().y / 2.0), ws, t.text);
            icons::paint(ui, "chevron-down", egui::Rect::from_center_size(wr.right_center() - vec2(12.0, 0.0), vec2(12.0, 12.0)), t.text_dim);
            let wresp = wresp.on_hover_text("Switch workspace");
            egui::Popup::menu(&wresp).show(|ui| crate::workspaces::popup(app, ui));
            ui.add_space(8.0);
            // Search box → command palette.
            let (r, resp) = ui.allocate_exact_size(vec2(200.0, 24.0), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::same(12), t.input);
            ui.painter().rect_stroke(r, CornerRadius::same(12), Stroke::new(1.0, if resp.hovered() { t.input_border } else { t.divider }), StrokeKind::Inside);
            icons::paint(ui, "search", egui::Rect::from_center_size(r.left_center() + vec2(14.0, 0.0), vec2(13.0, 13.0)), t.text_dim);
            ui.painter().text(r.left_center() + vec2(26.0, 0.0), egui::Align2::LEFT_CENTER, "Search commands and tools", egui::FontId::proportional(11.5), t.text_dim);
            if resp.clicked() {
                app.ui.palette_open = true;
                app.ui.palette_query.clear();
            }
            ui.add_space(10.0);
            crate::community::discord_button(app, ui, false);
        });
    });
}

fn chip_button(ui: &mut Ui, paint: &Paint, stroke_style: bool, tip: &str) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(34.0, 22.0), Sense::click());
    let chip = egui::Rect::from_min_size(r.min + vec2(0.0, 2.0), vec2(18.0, 18.0));
    paint_chip(ui, chip, paint);
    if stroke_style {
        ui.painter().rect_filled(chip.shrink(5.0), 0.0, t.panel);
    }
    ui.painter().rect_stroke(chip, 0.0, Stroke::new(1.0, t.input_border), StrokeKind::Outside);
    icons::paint(ui, "chevron-down", egui::Rect::from_min_size(r.min + vec2(21.0, 5.0), vec2(12.0, 12.0)), t.text_dim);
    resp.on_hover_text(tip)
}

/// The Control bar (Window → Control), context-sensitive like Illustrator's.
pub fn control_bar(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("control_bar")
        .exact_size(34.0)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(10, 0)).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                // Gripper
                let (g, _) = ui.allocate_exact_size(vec2(6.0, 20.0), Sense::hover());
                for i in 0..5 {
                    ui.painter().circle_filled(g.center_top() + vec2(0.0, 2.0 + i as f32 * 4.0), 0.9, t.text_disabled);
                }
                let Some(st) = app.session.active() else {
                    ui.label(egui::RichText::new("No Document").color(t.text_dim));
                    return;
                };
                let sel = st.selection.objects.clone();
                let units = st.doc.units;
                let first = sel.first().and_then(|id| st.doc.node(*id)).cloned();
                let anchor_mode = !st.selection.anchors.is_empty();
                let label = match &first {
                    Some(_) if sel.len() == 1 && anchor_mode => "Anchor Point",
                    Some(n) if sel.len() == 1 && matches!(n.kind, NodeKind::Image(_)) => "Embedded",
                    _ => crate::panels::appearance::object_label(app),
                };
                ui.label(egui::RichText::new(label).font(theme::semibold(12.0)).color(t.text));
                ui.add_space(6.0);
                let (fill, stroke) = crate::panels::current_paints(app);
                let shown_stroke = crate::panels::current_stroke(app);
                let mixed = crate::panels::stroke_mixed(app, ui.ctx());
                let weight = stroke_panel::shown_weight(app, shown_stroke.as_ref(), &mixed);
                let opacity = if first.is_some() { crate::panels::current_transparency(app).map_or(1.0, |t| t.0) } else { 1.0 };
                if chip_button(ui, &fill, false, "Fill").clicked() {
                    app.run("paint.toggleActive", json!({})).ok();
                    app.session.fill_active = true;
                    app.ui.open_panel = Some("swatches".into());
                }
                if chip_button(ui, &stroke, true, "Stroke").clicked() {
                    app.session.fill_active = false;
                    app.ui.open_panel = Some("swatches".into());
                }
                // The link opens the Stroke panel as a popover under it; then the weight spinner
                // (with presets) and the width profile.
                let link = ui.link(egui::RichText::new("Stroke:").size(12.0).color(t.text).underline()).on_hover_text("Stroke options");
                stroke_panel::popover(app, &link);
                stroke_panel::weight_field(app, ui, "cb-stroke", weight, 100.0);
                let profile = vectorcraft_doc::WidthProfile::id_of(shown_stroke.as_ref().and_then(|s| s.profile.as_ref()));
                if let Some(id) = stroke_panel::profile_dropdown(ui, profile) {
                    app.run("stroke.set", json!({"profile": id})).ok();
                }
                ui.add_space(4.0);
                ui.separator();
                if ui.link(egui::RichText::new("Opacity:").size(12.0).color(t.text).underline()).clicked() {
                    app.ui.open_panel = Some("transparency".into());
                }
                if let Some(o) = widgets::plain_field(ui, "cb-opacity", opacity as f64 * 100.0, "%", 0, 56.0)
                    && !sel.is_empty()
                {
                    app.run("transparency.set", json!({"opacity": o.clamp(0.0, 100.0)})).ok();
                }
                ui.separator();
                if sel.is_empty() {
                    if widgets::flat_button(ui, "Document Setup", 112.0).clicked() {
                        app.run("file.documentSetup", json!({})).ok();
                    }
                    if widgets::flat_button(ui, "Preferences", 90.0).clicked() {
                        app.run("edit.preferences", json!({})).ok();
                    }
                    return;
                }
                // Align buttons.
                for (icon, tip, p) in [
                    ("align-start-vertical", "Horizontal Align Left", json!({"horizontal": "left"})),
                    ("align-center-vertical", "Horizontal Align Center", json!({"horizontal": "center"})),
                    ("align-end-vertical", "Horizontal Align Right", json!({"horizontal": "right"})),
                    ("align-start-horizontal", "Vertical Align Top", json!({"vertical": "top"})),
                    ("align-center-horizontal", "Vertical Align Center", json!({"vertical": "center"})),
                    ("align-end-horizontal", "Vertical Align Bottom", json!({"vertical": "bottom"})),
                ] {
                    if widgets::icon_button(ui, icon, tip, false, 24.0).clicked() {
                        let mut p = p;
                        if sel.len() == 1 {
                            p["to"] = json!("artboard");
                        }
                        app.run("object.align", p).ok();
                    }
                }
                ui.separator();
                // Transform fields.
                let b = app.session.active().and_then(|s| s.doc.bounds_of(&s.selection.objects, false));
                if let Some(b) = b {
                    for (k, lbl, v) in
                        [("x", "X:", b.center().x), ("y", "Y:", b.center().y), ("width", "W:", b.width()), ("height", "H:", b.height())]
                    {
                        ui.label(egui::RichText::new(lbl).size(12.0).color(t.text_dim));
                        if let Some(nv) = widgets::num_field(ui, ("cb", k), Some(v), units, 74.0) {
                            app.run("object.setBounds", json!({k: nv, "reference": 4})).ok();
                        }
                    }
                }
            });
        });
}

/// Document tab strip: "Name* @ 66.67% (RGB/Preview)".
pub fn doc_tabs(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 35.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, t.tab_strip);
    ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, t.border));
    let mut x = strip.left();
    let mut activate = None;
    let mut close = None;
    let active = app.session.active_index();
    for (i, d) in app.session.documents().iter().enumerate() {
        let zoom = app.views.get(i).map(|v| v.zoom).unwrap_or(1.0);
        let mode = if d.doc.color_mode == vectorcraft_doc::ColorMode::Cmyk { "CMYK" } else { "RGB" };
        let vm = if app.ui.view.outline { "Outline" } else { "Preview" };
        let title = format!("{}{} @ {} ({mode}/{vm})", d.title(), if d.is_dirty() { "*" } else { "" }, zoom_label(zoom).replace('%', " %"));
        let is_active = Some(i) == active;
        let galley = ui.painter().layout_no_wrap(title, theme::semibold(12.5), if is_active { t.text_strong } else { t.text_dim });
        let w = galley.size().x + 50.0;
        let r = egui::Rect::from_min_size(egui::pos2(x, strip.top()), vec2(w, strip.height() - 1.0));
        let resp = ui.interact(r, ui.id().with(("tab", i)), Sense::click());
        if is_active {
            ui.painter().rect_filled(r, 0.0, t.panel);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, 0.0, t.hover.gamma_multiply(0.4));
        }
        ui.painter().line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.5, t.border));
        // × at the left like Illustrator.
        let xr = egui::Rect::from_center_size(egui::pos2(r.left() + 16.0, r.center().y), vec2(12.0, 12.0));
        let xresp = ui.interact(xr.expand(3.0), ui.id().with(("tabx", i)), Sense::click());
        icons::paint(ui, "x", xr, if xresp.hovered() { t.text_strong } else { t.text });
        ui.painter().galley(egui::pos2(r.left() + 32.0, r.center().y - galley.size().y / 2.0), galley, t.text);
        if xresp.clicked() {
            close = Some(i);
        } else if resp.clicked() {
            activate = Some(i);
        }
        x += w;
    }
    if let Some(i) = close {
        if let Err(e) = crate::unsaved::close(app, i) {
            app.status(e);
        }
    } else if let Some(i) = activate {
        app.session.set_active(i);
    }
    // Isolation mode breadcrumb bar.
    if let Some(st) = app.session.active()
        && let Some(iso) = st.isolation
    {
        let mut crumbs: Vec<String> =
            st.doc.ancestry(iso).unwrap_or_default().iter().filter_map(|id| st.doc.node(*id)).map(|n| n.display_name()).collect();
        // Pattern editing mode: the pattern's name, and Save a Copy / Done / Cancel at the right.
        let pattern_edit = st.doc.pattern_edit.as_ref().map(|e| e.pattern.clone());
        if let Some(pn) = &pattern_edit {
            crumbs.push(pn.clone());
        }
        let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::hover());
        ui.painter().rect_filled(bar, 0.0, t.panel);
        let back = egui::Rect::from_min_size(bar.min + vec2(6.0, 3.0), vec2(18.0, 18.0));
        let bresp = ui.interact(back, ui.id().with("iso-back"), Sense::click());
        icons::paint(ui, "chevron-left", back, if bresp.hovered() { t.text } else { t.icon });
        ui.painter().text(
            bar.left_center() + vec2(30.0, 0.0),
            egui::Align2::LEFT_CENTER,
            crumbs.join("  ›  "),
            egui::FontId::proportional(12.0),
            t.text,
        );
        let mut pattern_cmd = None;
        if pattern_edit.is_some() {
            let mut x = bar.right() - 6.0;
            for (label, cmd) in [("Cancel", "object.pattern.cancel"), ("Done", "object.pattern.done"), ("Save a Copy", "object.pattern.saveCopy")] {
                let w = 12.0 + 7.0 * label.len() as f32;
                let r = egui::Rect::from_min_max(egui::pos2(x - w, bar.top() + 3.0), egui::pos2(x, bar.bottom() - 3.0));
                let resp = ui.interact(r, ui.id().with(("pat-bar", cmd)), Sense::click());
                ui.painter().rect_filled(r, 3.0, if resp.hovered() { t.hover } else { t.panel });
                ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0, t.button_border), egui::StrokeKind::Inside);
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(12.0), t.text);
                if resp.clicked() {
                    pattern_cmd = Some(cmd);
                }
                x -= w + 6.0;
            }
        }
        if bresp.clicked() {
            pattern_cmd = pattern_cmd.or(pattern_edit.as_ref().map(|_| "object.pattern.done"));
            if pattern_cmd.is_none() {
                app.run("object.exitIsolation", json!({})).ok();
            }
        }
        if let Some(cmd) = pattern_cmd {
            app.run(cmd, json!({})).ok();
        }
    }
}

pub fn status_bar(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("status_bar")
        .exact_size(24.0)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(8, 0)).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let zoom = app.view().map(|v| v.zoom).unwrap_or(1.0);
                egui::ComboBox::from_id_salt("zoom-combo").selected_text(egui::RichText::new(zoom_label(zoom)).size(11.5)).width(76.0).show_ui(
                    ui,
                    |ui| {
                        for z in ZOOM_STOPS.iter().rev() {
                            if ui.selectable_label((z / 100.0 - zoom).abs() < 1e-4, zoom_label(z / 100.0)).clicked() {
                                app.run("view.setZoom", json!({"zoom": z})).ok();
                            }
                        }
                        ui.separator();
                        if ui.selectable_label(false, "Fit on Screen").clicked() {
                            app.run("view.fitArtboard", json!({})).ok();
                        }
                        if ui.selectable_label(false, "Fit All").clicked() {
                            app.run("view.fitAll", json!({})).ok();
                        }
                    },
                );
                let rot = app.view().map(|v| v.rotation).unwrap_or(0.0);
                egui::ComboBox::from_id_salt("rot-combo").selected_text(egui::RichText::new(format!("{rot:.0}°")).size(11.5)).width(52.0).show_ui(
                    ui,
                    |ui| {
                        for a in [0.0, 15.0, 30.0, 45.0, 60.0, 90.0, 180.0, -15.0, -30.0, -45.0, -60.0, -90.0] {
                            if ui.selectable_label(rot == a, format!("{a:.0}°")).clicked()
                                && let Some(v) = app.view_mut()
                            {
                                v.rotation = a;
                            }
                        }
                    },
                );
                ui.separator();
                let nab = app.session.active().map(|d| d.doc.artboards.len()).unwrap_or(0);
                for icon in ["chevrons-left", "chevron-left"] {
                    widgets::icon_button(ui, icon, "", false, 18.0);
                }
                ui.label(egui::RichText::new(if nab > 0 { "1" } else { "–" }).size(11.5));
                for icon in ["chevron-right", "chevrons-right"] {
                    widgets::icon_button(ui, icon, "", false, 18.0);
                }
                ui.separator();
                let tool = vectorcraft_tools::tool_info(app.session.tool_id()).map(|t| t.label.trim_end_matches(" Tool")).unwrap_or("");
                ui.label(egui::RichText::new(tool).size(11.5).color(t.text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "render {:.1} ms · ui {:.1} ms · {:.0} fps",
                            app.perf.render_ms, app.perf.frame_ms, app.perf.fps
                        ))
                        .size(10.5)
                        .color(t.text_disabled),
                    );
                    if let Some(p) = app.hover_doc {
                        ui.label(egui::RichText::new(format!("X: {:.2}  Y: {:.2}", p.x, p.y)).font(theme::mono(10.5)).color(t.text_dim));
                    }
                    if !app.ui.status.is_empty() {
                        ui.label(egui::RichText::new(&app.ui.status).size(11.0).color(t.text));
                    }
                });
            });
        });
}

/// Contextual hint for the active tool: segments of (text, bold).
fn hint_for(tool: &str) -> &'static [(&'static str, bool)] {
    match tool {
        "selection" => &[
            ("Click", true),
            (" the object to select  |  ", false),
            ("Shift+Click", true),
            (" to select multiple objects  |  ", false),
            ("Option+Drag", true),
            (" the object to duplicate", false),
        ],
        "directSelection" => &[
            ("Click", true),
            (" an anchor or path segment to select it  |  ", false),
            ("Drag", true),
            (" to move  |  ", false),
            ("Shift+Click", true),
            (" to add", false),
        ],
        "pen" => &[
            ("Click", true),
            (" to add a corner point  |  ", false),
            ("Drag", true),
            (" to add a smooth point  |  ", false),
            ("Click the first point", true),
            (" to close  |  ", false),
            ("Enter", true),
            (" to finish", false),
        ],
        "curvature" => &[
            ("Click", true),
            (" to add smooth points  |  ", false),
            ("Double-click", true),
            (" to toggle corner  |  ", false),
            ("Esc", true),
            (" to finish", false),
        ],
        "type" => &[
            ("Click", true),
            (" to create point type  |  ", false),
            ("Drag", true),
            (" to create area type  |  ", false),
            ("Esc", true),
            (" to exit editing", false),
        ],
        "rectangle" | "roundedRectangle" | "ellipse" | "polygon" | "star" => &[
            ("Drag", true),
            (" to draw  |  ", false),
            ("Shift+Drag", true),
            (" to constrain proportions  |  ", false),
            ("Option+Drag", true),
            (" from center  |  ", false),
            ("Click", true),
            (" for exact size", false),
        ],
        "rotate" | "reflect" | "scale" | "shear" => &[
            ("Click", true),
            (" to set the reference point  |  ", false),
            ("Drag", true),
            (" to transform  |  ", false),
            ("Option+Click", true),
            (" for exact values", false),
        ],
        "hand" => &[("Drag", true), (" to pan the view", false)],
        "zoom" => &[
            ("Click", true),
            (" to zoom in  |  ", false),
            ("Option+Click", true),
            (" to zoom out  |  ", false),
            ("Drag", true),
            (" to zoom into an area", false),
        ],
        "eyedropper" => &[("Click", true), (" an object to copy its appearance  |  ", false), ("Shift+Click", true), (" to sample a color", false)],
        "gradient" => &[("Drag", true), (" across a selected object to set the gradient direction", false)],
        "artboard" => &[("Click", true), (" to select an artboard  |  ", false), ("Drag", true), (" on the canvas to create one", false)],
        _ => &[("Press ", false), ("Cmd+Shift+/", true), (" to search every command", false)],
    }
}

pub fn hint_bar(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::bottom("hint_bar")
        .exact_size(28.0)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(10, 0)).stroke(Stroke::new(1.0, t.border)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                ui.painter().circle_stroke(r.center(), 7.0, Stroke::new(1.2, t.text));
                ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, "?", theme::semibold(11.0), t.text);
                ui.add_space(6.0);
                let mut job = egui::text::LayoutJob::default();
                for (txt, bold) in hint_for(app.session.tool_id()) {
                    let font = if *bold { theme::semibold(12.5) } else { egui::FontId::proportional(12.5) };
                    job.append(txt, 0.0, egui::TextFormat { font_id: font, color: if *bold { t.text_strong } else { t.text }, ..Default::default() });
                }
                ui.label(job);
            });
        });
}
