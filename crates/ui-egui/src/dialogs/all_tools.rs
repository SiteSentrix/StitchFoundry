//! The All Tools drawer: every tool with its icon and name, under the headings of the toolbar's
//! groups (Select, Shapes, Draw, Modify, Type, Navigate, Color) and of the tools the Basic toolbar
//! leaves out (#807). The active tool is highlighted; picking one selects it and closes the drawer.

use egui::{Sense, vec2};
use vectorcraft_tools::{TOOL_GROUPS, ToolInfo};

use super::DialogSpec;
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, icons, widgets};

/// A compact drawer two tools wide, however wide the window (#1018); the dialog frame narrows it
/// further in a narrow window.
pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| tl!("All Tools").into(), body, ok: None, min_width: WIDTH, max_width: Some(WIDTH), ..DialogSpec::FORM };

/// A tool's cell: its icon and name.
const CELL: egui::Vec2 = vec2(176.0, 28.0);
const ICON: f32 = 20.0;
/// The drawer's width: two cells, the gap between them and room for the scroll bar.
const WIDTH: f32 = 2.0 * CELL.x + 24.0;

/// The heading a tool is listed under: its Basic toolbar group, else one for the tools only the
/// Advanced toolbar shows.
fn heading(id: &str) -> &'static str {
    if let Some((cat, _)) = crate::toolbar::BASIC.iter().find(|(_, slots)| slots.iter().any(|s| s.contains(&id))) {
        return cat;
    }
    match id {
        _ if id.starts_with("symbol") => "Symbols",
        _ if id.ends_with("Graph") => "Graph",
        _ if id.starts_with("perspective") => "Perspective",
        "artboard" => "Artboards",
        "slice" | "sliceSelection" => "Slices",
        _ => "More",
    }
}

/// The headings in order, each with its tools in the toolbar's order.
fn sections() -> Vec<(&'static str, Vec<&'static ToolInfo>)> {
    let mut out: Vec<(&'static str, Vec<&'static ToolInfo>)> = crate::toolbar::BASIC.iter().map(|(cat, _)| (*cat, vec![])).collect();
    for tool in TOOL_GROUPS.iter().flat_map(|g| g.iter()) {
        let h = heading(tool.id);
        match out.iter_mut().find(|(c, _)| *c == h) {
            Some((_, tools)) => tools.push(tool),
            None => out.push((h, vec![tool])),
        }
    }
    out.retain(|(_, tools)| !tools.is_empty());
    out
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, _: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let active = app.session.tool_id();
    let mut picked = None;
    let room = (ui.ctx().content_rect().height() - 180.0).max(200.0);
    egui::ScrollArea::vertical().max_height(room).auto_shrink([false, true]).show(ui, |ui| {
        for (cat, tools) in sections() {
            widgets::subheader(ui, tl!(cat));
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = vec2(4.0, 2.0);
                for tool in tools {
                    let (rect, resp) = ui.allocate_exact_size(CELL, Sense::click());
                    let on = tool.id == active;
                    if on {
                        ui.painter().rect_filled(rect, 3.0, t.selection);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(rect, 3.0, t.hover);
                    }
                    let icon = egui::Rect::from_center_size(rect.left_center() + vec2(6.0 + ICON / 2.0, 0.0), vec2(ICON, ICON));
                    icons::paint(ui, icons::tool_icon(tool.icon), icon, if on { t.text_strong } else { t.icon });
                    let name = tl!(tool.label);
                    let name = if crate::i18n::current() == crate::i18n::Lang::EN { name.trim_end_matches(" Tool") } else { name };
                    let text_at = egui::pos2(icon.right() + 8.0, rect.center().y);
                    ui.painter().with_clip_rect(rect).text(
                        text_at,
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(12.0),
                        if on { t.text_strong } else { t.text },
                    );
                    if resp.on_hover_text(crate::toolbar::tip(tool)).clicked() {
                        picked = Some(tool.id);
                    }
                }
            });
            ui.add_space(6.0);
        }
    });
    if let Some(id) = picked {
        app.select_tool(id);
    }
    picked.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every tool is listed once, under a heading; the Basic toolbar's groups come first, in its order.
    #[test]
    fn every_tool_is_listed_once_under_a_heading() {
        let s = sections();
        let listed: Vec<&str> = s.iter().flat_map(|(_, tools)| tools.iter().map(|t| t.id)).collect();
        let all: Vec<&str> = TOOL_GROUPS.iter().flat_map(|g| g.iter().map(|t| t.id)).collect();
        assert_eq!(listed.len(), all.len());
        assert!(all.iter().all(|id| listed.contains(id)));
        let heads: Vec<&str> = s.iter().map(|(c, _)| *c).collect();
        assert_eq!(&heads[..3], ["Select", "Shapes", "Draw"]);
        assert!(heads.contains(&"Symbols") && heads.contains(&"Graph"), "{heads:?}");
        assert!(!heads.contains(&"More"), "every tool has a named heading: {heads:?}");
    }

    /// The drawer stays two tools wide in a wide window and fits a narrow one (#1018).
    #[test]
    fn all_tools_dialog_has_a_bounded_width() {
        use egui::{Pos2, RawInput, Rect};

        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.ui.dialog = Some(Dialog::new("allTools", serde_json::json!({})));
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        for width in [1280.0, 430.0] {
            let screen = Rect::from_min_size(Pos2::ZERO, vec2(width, 900.0));
            for _ in 0..6 {
                let mut out = ctx.run_ui(RawInput { screen_rect: Some(screen), ..Default::default() }, |ui| super::super::show(&mut app, ui.ctx()));
                out.textures_delta.clear();
            }
            let rect = ctx.memory(|m| m.area_rect(egui::Id::new(("dialog", "allTools")))).expect("drawer is shown");
            assert!(rect.width() <= width, "drawer exceeds viewport: {rect:?}");
            assert!(rect.width() < 3.0 * CELL.x, "drawer wider than two tools: {rect:?}");
        }
    }
}
