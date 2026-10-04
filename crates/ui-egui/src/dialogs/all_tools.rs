//! The All Tools drawer: every tool as a button; picking one selects it and closes the drawer.

use super::DialogSpec;
use crate::VectorcraftApp;
use crate::state::Dialog;

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |_| "All Tools".into(), body, ok: None, ..DialogSpec::FORM };

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, _: &mut Dialog) -> bool {
    let mut picked = false;
    for g in vectorcraft_tools::TOOL_GROUPS {
        ui.horizontal_wrapped(|ui| {
            for tool in g.iter() {
                if ui.button(tool.label.trim_end_matches(" Tool")).clicked() {
                    app.select_tool(tool.id);
                    picked = true;
                }
            }
        });
    }
    picked
}
