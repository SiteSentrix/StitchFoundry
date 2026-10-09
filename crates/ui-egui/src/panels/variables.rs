//! Variables panel: the document's variables (what they drive, what they bind) and
//! its datasets. Clicking a dataset applies it; clicking a variable selects what it
//! binds; the arrows step to the next or previous dataset. Definitions are made with
//! the `variable.*` and `dataset.*` commands (command palette, actions, MCP).

use egui::Ui;
use serde_json::json;

use crate::VectorcraftApp;
use crate::widgets;

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Some(st) = app.session.active() else {
        widgets::dim_label(ui, tl!("No document"));
        return;
    };
    // Owned snapshots: the rows below run commands, which borrow the app mutably.
    let vars: Vec<(String, String, Vec<u64>)> = st
        .doc
        .variables
        .variables
        .iter()
        .map(|v| {
            let bound: Vec<u64> = st.doc.variables.bindings.iter().filter(|(_, name)| *name == &v.name).map(|(id, _)| id.0).collect();
            (v.name.clone(), v.kind.label().to_string(), bound)
        })
        .collect();
    let sets: Vec<(String, usize, bool)> = st
        .doc
        .variables
        .datasets
        .iter()
        .map(|d| (d.name.clone(), d.values.len(), st.doc.variables.active_dataset.as_deref() == Some(d.name.as_str())))
        .collect();
    widgets::subheader(ui, tl!("Variables"));
    if vars.is_empty() {
        widgets::dim_label(ui, tl!("None"));
    }
    for (name, kind, bound) in &vars {
        let row = format!("{name} · {kind} · {}", bound.len());
        if ui.selectable_label(false, row).clicked() && !bound.is_empty() {
            app.run("select.set", json!({"ids": bound})).ok();
        }
    }
    ui.horizontal(|ui| {
        widgets::subheader(ui, tl!("Data Sets"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("◀").clicked() {
                app.run("dataset.prev", json!({})).ok();
            }
            if ui.small_button("▶").clicked() {
                app.run("dataset.next", json!({})).ok();
            }
        });
    });
    if sets.is_empty() {
        widgets::dim_label(ui, tl!("None"));
    }
    for (name, count, active) in &sets {
        let row = format!("{}{name} · {count}", if *active { "● " } else { "" });
        if ui.selectable_label(*active, row).clicked() {
            app.run("dataset.select", json!({"name": name})).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draws_headless() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
        app.session.execute("text.create", &json!({"x": 10, "y": 40, "text": "Hi"})).unwrap();
        app.session.execute("variable.define", &json!({"name": "Title", "kind": "text"})).unwrap();
        app.session.execute("dataset.new", &json!({"name": "A", "values": {"Title": "Yo"}})).unwrap();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            show(&mut app, ui);
        });
        out.textures_delta.clear();
        let text = crate::tests_labels::painted_text(&mut app, show);
        assert!(text.contains("Title") && text.contains("Data Sets") && text.contains("A · 1"), "{text}");
        // Clicking the dataset applies it.
        app.run("dataset.select", json!({"name": "A"})).unwrap();
        let text = crate::tests_labels::painted_text(&mut app, show);
        assert!(text.contains("● A"), "{text}");
    }
}
