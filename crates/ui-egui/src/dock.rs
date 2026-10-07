//! The right dock: Properties | Layers | Libraries tabs, plus the collapsed icon-panel column
//! whose panels pop out to the left.

use egui::{CornerRadius, Sense, Stroke, Ui, vec2};

use crate::state::{DockTab, ICON_PANEL_GROUPS, ICON_PANELS};
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, icons, panels, widgets};

const ICON_COL: f32 = 38.0;

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    // Main tabbed group.
    egui::Panel::right("dock")
        .resizable(true)
        .default_size(300.0)
        .size_range(230.0..=520.0)
        .frame(egui::Frame::NONE.fill(t.panel).stroke(Stroke::new(1.5, t.border)))
        .show(ui, |ui| {
            // Tab strip.
            let (hdr, _) = ui.allocate_exact_size(vec2(ui.available_width(), 14.0), Sense::hover());
            ui.painter().rect_filled(hdr, 0.0, t.tab_strip);
            icons::paint(ui, "chevrons-right", egui::Rect::from_min_size(hdr.right_top() + vec2(-14.0, 2.0), vec2(10.0, 10.0)), t.text);
            let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 33.0), Sense::hover());
            ui.painter().rect_filled(strip, 0.0, t.tab_strip);
            ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, t.border));
            let mut x = strip.left();
            for (tab, label) in [(DockTab::Properties, "Properties"), (DockTab::Layers, "Layers"), (DockTab::Libraries, "Libraries")] {
                let active = app.ui.dock_tab == tab;
                let galley =
                    ui.painter().layout_no_wrap(tl!(label).to_string(), theme::semibold(12.5), if active { t.text_strong } else { t.text_dim });
                let r = egui::Rect::from_min_size(egui::pos2(x, strip.top()), vec2(galley.size().x + 24.0, strip.height() - 1.0));
                let resp = ui.interact(r, ui.id().with(("docktab", label)), Sense::click());
                if active {
                    ui.painter().rect_filled(r, 0.0, t.panel);
                }
                ui.painter().galley(egui::pos2(r.left() + 12.0, r.center().y - galley.size().y / 2.0), galley, t.text);
                ui.painter().line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, t.border));
                if resp.clicked() {
                    app.ui.dock_tab = tab;
                }
                x = r.right();
            }
            let menu_id = match app.ui.dock_tab {
                DockTab::Properties => "properties",
                DockTab::Layers => "layers",
                DockTab::Libraries => "libraries",
            };
            panels::panel_menu(app, ui, menu_id, egui::Rect::from_center_size(strip.right_center() - vec2(14.0, 0.0), vec2(16.0, 16.0)));
            egui::Frame::NONE.inner_margin(egui::Margin { left: 12, right: 10, top: 10, bottom: 8 }).show(ui, |ui| match app.ui.dock_tab {
                DockTab::Properties => {
                    egui::ScrollArea::vertical().id_salt("props").auto_shrink([false, false]).show(ui, |ui| panels::properties::show(app, ui));
                }
                DockTab::Layers => panels::layers::show(app, ui),
                DockTab::Libraries => panels::libraries(app, ui),
            });
        });
    // Collapsed icon-panel strip, left of the expanded panel group (like Illustrator's dock).
    egui::Panel::right("icon_column")
        .resizable(false)
        .exact_size(ICON_COL)
        .frame(egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::symmetric(4, 6)).stroke(Stroke::new(1.5, t.border)))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            // The icons scroll in a window too short for them.
            widgets::strip_scroll(ui, "icon_column", |ui| {
                for (gi, group) in ICON_PANEL_GROUPS.iter().enumerate() {
                    if gi > 0 {
                        let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::hover());
                        ui.painter().line_segment([r.left_center() + vec2(4.0, 0.0), r.right_center() - vec2(4.0, 0.0)], Stroke::new(1.0, t.divider));
                    }
                    for id in group.iter() {
                        let Some((_, label, icon)) = ICON_PANELS.iter().find(|p| p.0 == *id) else { continue };
                        let open = app.ui.open_panel.as_deref() == Some(*id);
                        if widgets::icon_button(ui, icon, label, open, 30.0).clicked() {
                            app.ui.open_panel = if open { None } else { Some(id.to_string()) };
                        }
                    }
                }
            });
        });
}

/// An icon panel popped out next to the icon column.
pub fn floating_panel(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let Some(id) = app.ui.open_panel.clone() else { return };
    let Some((_, label, _)) = ICON_PANELS.iter().find(|p| p.0 == id) else { return };
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    // Pinned by its right edge, 6 points left of the icon column: a panel wider than its 256
    // points grows towards the canvas rather than over the panel icons.
    let right = screen.right() - ICON_COL - 300.0 - 6.0;
    let mut open = true;
    let area = egui::Area::new(egui::Id::new("icon-panel")).order(egui::Order::Foreground).pivot(egui::Align2::RIGHT_TOP);
    area.fixed_pos(egui::pos2(right, 110.0)).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).corner_radius(CornerRadius::same(4)).inner_margin(egui::Margin::ZERO).show(ui, |ui| {
            ui.set_width(256.0);
            let (strip, _) = ui.allocate_exact_size(vec2(256.0, 26.0), Sense::hover());
            ui.painter().rect_filled(strip, CornerRadius { nw: 4, ne: 4, sw: 0, se: 0 }, t.panel_darker);
            let tab = egui::Rect::from_min_size(
                strip.min,
                vec2(ui.painter().layout_no_wrap(tl!(label).to_string(), theme::semibold(12.0), t.text).size().x + 24.0, 26.0),
            );
            ui.painter().rect_filled(tab, CornerRadius { nw: 4, ne: 0, sw: 0, se: 0 }, t.panel);
            ui.painter().text(tab.left_center() + vec2(12.0, 0.0), egui::Align2::LEFT_CENTER, tl!(label), theme::semibold(12.0), t.text);
            let close = egui::Rect::from_center_size(strip.right_center() - vec2(13.0, 0.0), vec2(14.0, 14.0));
            let cr = ui.interact(close, ui.id().with("close-panel"), Sense::click());
            icons::paint(ui, "chevrons-right", close, if cr.hovered() { t.text } else { t.text_dim });
            if cr.clicked() {
                open = false;
            }
            let menu_r = egui::Rect::from_center_size(strip.right_center() - vec2(34.0, 0.0), vec2(16.0, 16.0));
            panels::panel_menu(app, ui, &id, menu_r);
            egui::Frame::NONE.inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(236.0);
                panels::show_icon_panel(app, ui, &id);
            });
        });
    });
    if !open {
        app.ui.open_panel = None;
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_engine::Session;

    use super::*;
    use crate::toolbar::tests::{wheel, widget_rects};

    #[test]
    fn every_flyout_stays_left_of_the_icon_column() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let id = app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap()["id"].clone();
        app.run("select.set", json!({ "ids": [id] })).unwrap();
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(1600.0, 1200.0));
        // Where the icon column begins: the flyout must stay left of it.
        let column = screen.right() - ICON_COL - 300.0;
        let mut too_wide = vec![];
        for (panel, _, _) in crate::state::ICON_PANELS {
            app.ui.open_panel = Some(panel.to_string());
            // Two frames: the first lays the panel out, the second places the settled area.
            for _ in 0..2 {
                let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
                ctx.run_ui(input, |ui| floating_panel(&mut app, ui.ctx())).textures_delta.clear();
            }
            let rect = ctx.memory(|m| m.area_rect(egui::Id::new("icon-panel"))).unwrap();
            if rect.right() > column {
                too_wide.push(format!("{panel}: {:.0} past the column", rect.right() - column));
            }
        }
        assert!(too_wide.is_empty(), "flyouts reaching over the icon column: {too_wide:?}");
    }

    #[test]
    fn the_icon_column_scrolls_in_a_short_window() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        let mut frame = |time: f64, events| {
            let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, vec2(900.0, 400.0));
            let input = egui::RawInput { time: Some(time), events, screen_rect: Some(screen), ..Default::default() };
            ctx.run_ui(input, |ui| show(&mut app, ui)).textures_delta.clear();
            widget_rects(&ctx, vec2(30.0, 30.0))
        };
        let icons = frame(0.0, vec![]);
        assert!(icons.last().unwrap().bottom() > 400.0, "the last panel icon starts below the window");
        frame(0.1, wheel(icons[0].center(), 2000.0));
        let mut icons = vec![];
        for k in 2..40 {
            icons = frame(f64::from(k) * 0.1, vec![]);
        }
        assert!(icons.last().unwrap().bottom() <= 400.0, "scrolled into view: {:?}", icons.last());
    }
}
