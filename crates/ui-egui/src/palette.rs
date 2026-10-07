//! The command palette: fuzzy search over every command and tool.

use serde_json::json;

use crate::theme::Tokens;
use crate::{VectorcraftApp, menus};

/// Everything the palette searches: (label, command id or `tool:<id>`, shortcut).
pub fn items() -> Vec<(String, String, String)> {
    let mut items = vec![];
    // Aliases kept for older scripts run a command that is listed already.
    for c in vectorcraft_engine::command_specs().iter().filter(|c| !vectorcraft_engine::cmd::is_alias(c.id)) {
        let path = c.menu.join(" › ");
        let label = if path.is_empty() { c.label.to_string() } else { format!("{path} › {}", c.label) };
        items.push((label, c.id.to_string(), menus::shortcut_of(c.id).unwrap_or("").to_string()));
    }
    for c in menus::UI_COMMANDS {
        items.push((c.1.to_string(), c.0.to_string(), menus::shortcut_of(c.0).unwrap_or("").to_string()));
    }
    for tool in vectorcraft_tools::catalog::all_tools() {
        items.push((tool.label.to_string(), format!("tool:{}", tool.id), crate::shortcut_editor::tool_shortcut(tool.id).unwrap_or("").to_string()));
    }
    items
}

/// A palette label in the UI language: each `›`-separated menu segment and the command label
/// translate on their own.
fn shown_label(label: &str) -> String {
    label.split(" › ").map(crate::i18n::t).collect::<Vec<_>>().join(" › ")
}

pub fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !app.ui.palette_open {
        return;
    }
    let t = Tokens::get(ctx);
    let q = app.ui.palette_query.to_lowercase();
    let items = items();
    let matches: Vec<&(String, String, String)> = items
        .iter()
        .filter(|(l, id, _)| {
            q.is_empty() || {
                // Match the English text (what agents document), the shown text and the id.
                let shown = shown_label(l).to_lowercase();
                q.split_whitespace().all(|w| l.to_lowercase().contains(w) || shown.contains(w) || id.to_lowercase().contains(w))
            }
        })
        .take(14)
        .collect();
    let mut run: Option<String> = None;
    egui::Area::new(egui::Id::new("palette")).order(egui::Order::Foreground).anchor(egui::Align2::CENTER_TOP, [0.0, 90.0]).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
            ui.set_width(520.0);
            let r = ui.add(egui::TextEdit::singleline(&mut app.ui.palette_query).hint_text(tl!("Search commands and tools…")).desired_width(500.0));
            r.request_focus();
            ui.add_space(6.0);
            for (i, (label, id, sc)) in matches.iter().enumerate() {
                let resp = ui.add(
                    egui::Button::new(shown_label(label))
                        .shortcut_text(menus::pretty_shortcut(sc))
                        .min_size(egui::vec2(500.0, 24.0))
                        .selected(i == 0),
                );
                if resp.clicked() {
                    run = Some(id.clone());
                }
            }
            if ui.input(|i| i.key_pressed(egui::Key::Enter))
                && let Some(first) = matches.first()
            {
                run = Some(first.1.clone());
            }
        });
    });
    if let Some(id) = run {
        app.ui.palette_open = false;
        if let Some(tool) = id.strip_prefix("tool:") {
            app.select_tool(tool);
        } else {
            menus::invoke(app, &id, json!({}));
        }
    }
}
