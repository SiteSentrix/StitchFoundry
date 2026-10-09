//! Variables panel: the document's variables (what they drive, what they bind) and its
//! datasets. Clicking a dataset applies it; clicking a variable selects what it binds; the
//! arrows step to the next or previous one. Definitions are made with the `variable.*` and
//! `dataset.*` commands, from Window › Variables, the command palette or an agent.

use egui::Ui;
use serde_json::json;

use crate::VectorcraftApp;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "variables";

/// One variable row: what it is and how many objects it drives.
struct VarRow {
    name: String,
    kind: &'static str,
    bound: Vec<u64>,
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Some(st) = app.session.active() else {
        widgets::dim_label(ui, tl!("No document"));
        return;
    };
    // Owned snapshots: the rows below run commands, which borrow the app mutably.
    let vars: Vec<VarRow> = st
        .doc
        .variables
        .variables
        .iter()
        .map(|v| VarRow { name: v.name.clone(), kind: v.kind.label(), bound: st.doc.variables.objects_of(&v.name).iter().map(|i| i.0).collect() })
        .collect();
    let sets: Vec<(String, usize, bool)> = st
        .doc
        .variables
        .datasets
        .iter()
        .map(|d| (d.name.clone(), d.values.len(), st.doc.variables.active_dataset.as_deref() == Some(d.name.as_str())))
        .collect();
    let has_doc = app.session.active().is_some();

    widgets::subheader(ui, tl!("Variables"));
    if vars.is_empty() {
        widgets::dim_label(ui, tl!("None"));
    }
    for row in &vars {
        // The binding count is what the row says it drives; clicking selects those objects.
        let text = if row.bound.is_empty() {
            format!("{} · {}", row.name, row.kind)
        } else {
            crate::i18n::fmt(tl!("{name} · {kind} · {count}"), &[("name", &row.name), ("kind", row.kind), ("count", &row.bound.len().to_string())])
        };
        if ui.selectable_label(false, text).clicked() && !row.bound.is_empty() {
            app.run("select.set", json!({"ids": row.bound})).ok();
        }
    }

    ui.horizontal(|ui| {
        widgets::subheader(ui, tl!("Data Sets"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if widgets::icon_button(ui, "chevron-left", tl!("Previous Data Set"), false, 24.0).clicked() {
                app.run("dataset.prev", json!({})).ok();
            }
            if widgets::icon_button(ui, "chevron-right", tl!("Next Data Set"), false, 24.0).clicked() {
                app.run("dataset.next", json!({})).ok();
            }
        });
    });
    if sets.is_empty() {
        widgets::dim_label(ui, tl!("None"));
    }
    for (name, count, active) in &sets {
        // The active row is both ticked and selected, as the reference app's is.
        let row = format!("{}{name} · {count}", if *active { "● " } else { "" });
        if ui.selectable_label(*active, row).clicked() {
            app.run("dataset.select", json!({"name": name})).ok();
        }
    }

    // New and delete, as the reference panel's buttons: the definitions are made here.
    ui.separator();
    ui.horizontal(|ui| {
        if widgets::icon_button(ui, "plus", tl!("New Variable…"), false, 24.0).clicked() {
            crate::menus::invoke(app, "variable.define", json!({}));
        }
        if widgets::icon_button(ui, "plus", tl!("New Data Set…"), false, 24.0).clicked() {
            crate::menus::invoke(app, "dataset.new", json!({}));
        }
        let remove = widgets::icon_button_enabled(ui, "trash-2", tl!("Delete Variable"), false, !vars.is_empty(), 24.0).clicked();
        if remove {
            // Everything the document defines goes, in one undo step, as the panel's bin does.
            let names: Vec<String> = vars.iter().map(|v| v.name.clone()).collect();
            app.run("variable.delete", json!({"names": names})).ok();
        }
    });
    let _ = (has_doc, ui);
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

    #[test]
    fn the_panel_is_registered_with_its_own_icon() {
        let row = crate::state::ICON_PANELS.iter().find(|(id, ..)| *id == ID).expect("in the icon column");
        assert_eq!(row.1, "Variables");
        assert_ne!(row.2, "dc-list-view", "Document Info's icon, which the panel used to share");
        // The icon it does use is one the app ships.
        assert!(crate::icons::exists(row.2), "{} is not a bundled icon", row.2);
    }

    /// Every Variables command is in the Window menu, so the feature is reachable without an
    /// agent: the palette runs these with empty params, which the dialogs turn into a form.
    #[test]
    fn every_variables_command_is_in_the_window_menu() {
        let app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let entries = crate::menus::menu_entries(&app);
        let in_menu = |id: &str| entries.iter().any(|e| e.command.as_deref() == Some(id));
        for id in [
            "variable.define",
            "variable.delete",
            "variable.bind",
            "variable.unbind",
            "dataset.new",
            "dataset.set",
            "dataset.delete",
            "dataset.select",
            "dataset.next",
            "dataset.prev",
        ] {
            assert!(in_menu(id), "{id} is not in the Window menu");
        }
        // The panel is under Window › Variables, and its own entry opens it.
        let panel = entries
            .iter()
            .find(|e| e.command.as_deref() == Some("window.panel") && e.params.get("panel").and_then(serde_json::Value::as_str) == Some(ID))
            .expect("the Variables panel is listed");
        assert_eq!(panel.path, vec!["Window".to_string(), "Variables".to_string()]);
        assert_eq!(panel.label, "Variables Panel");
    }

    /// A menu item that needs a name opens the dialog rather than running the command with
    /// empty params, which would only say "missing `name`".
    #[test]
    fn the_menu_dialogs_open_instead_of_failing() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        for (cmd, field) in [("variable.define", "kind"), ("variable.bind", "variable"), ("dataset.new", "name"), ("dataset.select", "name")] {
            app.ui.dialog = None;
            crate::menus::invoke(&mut app, cmd, json!({}));
            let d = app.ui.dialog.as_ref().unwrap_or_else(|| panic!("{cmd} opened no dialog"));
            assert_eq!(d.kind, crate::dialogs::variables::KIND, "{cmd}");
            assert_eq!(d.str("__command"), cmd);
            assert!(d.fields.contains_key(field), "{cmd}: no `{field}` field");
        }
        // `dataset.set` needs a row to edit, and says so rather than opening an empty form.
        app.ui.dialog = None;
        crate::menus::invoke(&mut app, "dataset.set", json!({}));
        assert!(app.ui.dialog.is_none(), "nothing to edit yet");
        assert!(app.ui.status.contains("data set"), "{}", app.ui.status);
    }
}
