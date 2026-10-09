//! Variables panel: the document's variables (what they drive, which objects) and its
//! datasets. Clicking a dataset applies it; clicking a variable selects what it binds; the
//! arrows step to the next or previous one. Everything the reference panel does here runs a
//! `variable.*` / `dataset.*` command, so the panel and an agent drive the document the same
//! way — the commands are also in Window › Variables, the palette and over MCP.
//!
//! A row's name goes in italics when an edit has moved the art away from what the row says:
//! the document then no longer matches the active dataset, and "Update Data Set" writes the
//! art back into it.

use egui::Ui;
use serde_json::json;
use vectorcraft_doc::VariableKind;

use crate::VectorcraftApp;
use crate::widgets;

/// The panel id, as the dock, the Window menu and `window.panel` name it.
pub const ID: &str = "variables";

/// One variable row: its kind's icon, its name, and the object it drives.
struct VarRow {
    name: String,
    kind: VariableKind,
    bound: Vec<u64>,
    /// The name of the object it drives, or "Multiple objects" when it drives more than one.
    object: String,
}

/// One dataset row: its name and how many values it carries.
struct SetRow {
    name: String,
    values: usize,
    active: bool,
    /// False once an edit has moved the art away from what this row says.
    matches: bool,
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
        .map(|v| {
            let bound: Vec<u64> = st.doc.variables.objects_of(&v.name).iter().map(|i| i.0).collect();
            let object = match bound.len() {
                0 => String::new(),
                1 => st.doc.node(NodeId(bound[0])).map(|n| n.display_name()).unwrap_or_default(),
                _ => tl!("Multiple objects").to_string(),
            };
            VarRow { name: v.name.clone(), kind: v.kind, bound, object }
        })
        .collect();
    let sets: Vec<SetRow> = st
        .doc
        .variables
        .datasets
        .iter()
        .map(|d| SetRow {
            name: d.name.clone(),
            values: d.values.len(),
            active: st.doc.variables.active_dataset.as_deref() == Some(d.name.as_str()),
            matches: st.doc.matches_dataset(&d.name),
        })
        .collect();
    let active = sets.iter().find(|s| s.active);
    let selected: Vec<u64> = st.selection.objects.iter().map(|i| i.0).collect();
    let has_selection = !selected.is_empty();

    // The current data set across the top: its name, and the arrows that step through them.
    ui.horizontal(|ui| {
        widgets::field_label(ui, tl!("Data Set"));
        let names: Vec<&str> = sets.iter().map(|s| s.name.as_str()).collect();
        let current = active.map(|s| s.name.as_str()).unwrap_or("");
        let picked = widgets::dropdown(ui, "variables-active", current, &names, 160.0);
        if let Some(i) = picked
            && names.get(i).is_some_and(|n| *n != current)
        {
            app.run("dataset.select", json!({"name": names[i]})).ok();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let has = !sets.is_empty();
            if widgets::icon_button_enabled(ui, "chevron-left", tl!("Previous Data Set"), false, has, 24.0).clicked() {
                app.run("dataset.prev", json!({})).ok();
            }
            if widgets::icon_button_enabled(ui, "chevron-right", tl!("Next Data Set"), false, has, 24.0).clicked() {
                app.run("dataset.next", json!({})).ok();
            }
        });
    });

    widgets::subheader(ui, tl!("Variables"));
    if vars.is_empty() {
        widgets::dim_label(ui, tl!("None"));
    }
    for row in &vars {
        ui.horizontal(|ui| {
            // The kind's own icon, so a text variable and a visibility one read apart.
            let icon = match row.kind {
                VariableKind::Text => "type",
                VariableKind::Visibility => "eye",
            };
            let icon = ui.add(egui::Image::new(crate::icons::source(icon)).fit_to_exact_size(egui::vec2(14.0, 14.0)));
            let _ = icon;
            // The object a variable drives is named beside it, as the reference panel does.
            let text = if row.object.is_empty() { row.name.clone() } else { format!("{} · {}", row.name, row.object) };
            let r = ui.selectable_label(false, text);
            // A click selects what it drives; a double click opens Variable Options.
            if (r.clicked() || r.double_clicked()) && !row.bound.is_empty() {
                app.run("select.set", json!({"ids": row.bound})).ok();
            }
            if r.double_clicked() {
                crate::menus::invoke(app, "variable.rename", json!({"name": row.name}));
            }
        });
    }

    widgets::subheader(ui, tl!("Data Sets"));
    if sets.is_empty() {
        widgets::dim_label(ui, tl!("None"));
    }
    for row in &sets {
        // The active row is ticked and selected; a row the art has drifted from is in
        // italics, so an edit that left the document not matching it is visible.
        let label = format!("{}{} · {}", if row.active { "● " } else { "" }, row.name, row.values);
        let mut text = egui::RichText::new(label);
        if !row.matches {
            text = text.italics();
        }
        if ui.selectable_label(row.active, text).clicked() && !row.active {
            app.run("dataset.select", json!({"name": row.name})).ok();
        }
    }

    // The bottom bar: make the selection dynamic, and define and delete what the lists show.
    ui.separator();
    ui.horizontal(|ui| {
        if widgets::icon_button_enabled(ui, "type", tl!("Make Text Dynamic"), false, has_selection, 24.0).clicked() {
            app.run("variable.makeTextDynamic", json!({})).ok();
        }
        if widgets::icon_button_enabled(ui, "eye", tl!("Make Visibility Dynamic"), false, has_selection, 24.0).clicked() {
            app.run("variable.makeVisibilityDynamic", json!({})).ok();
        }
        if widgets::icon_button(ui, "plus", tl!("New Variable…"), false, 24.0).clicked() {
            crate::menus::invoke(app, "variable.define", json!({}));
        }
        let bound_to_selection = !selected.is_empty();
        if widgets::icon_button_enabled(ui, "link-2-off", tl!("Unbind Variable"), false, bound_to_selection, 24.0).clicked() {
            app.run("variable.unbind", json!({"ids": selected})).ok();
        }
        if widgets::icon_button_enabled(ui, "trash-2", tl!("Delete Variable"), false, !vars.is_empty(), 24.0).clicked() {
            // Everything the document defines goes, in one undo step, as the panel's bin does.
            let names: Vec<String> = vars.iter().map(|v| v.name.clone()).collect();
            app.run("variable.delete", json!({"names": names})).ok();
        }
    });
    ui.horizontal(|ui| {
        // Capture records what the art holds now as a new row (paste: take it in); Update
        // writes the art back into the row that is active (save: write it out).
        if widgets::icon_button(ui, "clipboard-paste", tl!("Capture Data Set"), false, 24.0).clicked() {
            app.run("dataset.capture", json!({})).ok();
        }
        let can_update = active.is_some();
        if widgets::icon_button_enabled(ui, "save", tl!("Update Data Set"), false, can_update, 24.0).clicked() {
            app.run("dataset.update", json!({})).ok();
        }
    });
}

/// The panel's (≡) menu: the same commands as its bars and lists, for the ones that act on one
/// row rather than on the selection.
pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Some(st) = app.session.active() else { return };
    let vars: Vec<String> = st.doc.variables.variables.iter().map(|v| v.name.clone()).collect();
    let sets: Vec<String> = st.doc.variables.datasets.iter().map(|d| d.name.clone()).collect();
    let active = st.doc.variables.active_dataset.clone();
    let bound: Vec<u64> = st.doc.variables.bindings.keys().map(|i| i.0).collect();
    let selected: Vec<u64> = st.selection.objects.iter().map(|i| i.0).collect();

    if widgets::menu_item(ui, tl!("New Variable…"), true, false) {
        crate::menus::invoke(app, "variable.define", json!({}));
    }
    // Variable Options renames the variable the panel has highlighted, else the first.
    if widgets::menu_item(ui, tl!("Variable Options…"), !vars.is_empty(), false) {
        let name = vars.first().cloned().unwrap_or_default();
        crate::menus::invoke(app, "variable.rename", json!({"name": name}));
    }
    ui.separator();
    if widgets::menu_item(ui, tl!("Capture Data Set"), true, false) {
        app.run("dataset.capture", json!({})).ok();
    }
    if widgets::menu_item(ui, tl!("Update Data Set"), active.is_some(), false) {
        app.run("dataset.update", json!({})).ok();
    }
    if widgets::menu_item(ui, tl!("New Data Set…"), true, false) {
        crate::menus::invoke(app, "dataset.new", json!({}));
    }
    if widgets::menu_item(ui, tl!("Edit Data Set…"), active.is_some(), false) {
        crate::menus::invoke(app, "dataset.set", json!({}));
    }
    if widgets::menu_item(ui, tl!("Rename Data Set…"), !sets.is_empty(), false) {
        let name = sets.first().cloned().unwrap_or_default();
        crate::menus::invoke(app, "dataset.rename", json!({"name": name}));
    }
    if widgets::menu_item(ui, tl!("Delete Data Set"), !sets.is_empty(), false) {
        app.run("dataset.delete", json!({"names": sets})).ok();
    }
    ui.separator();
    if widgets::menu_item(ui, tl!("Select Bound Object"), !selected.is_empty(), false) {
        // The objects the selection is bound to, when it is bound at all.
        let ids: Vec<u64> = selected.clone();
        app.run("variable.unbind", json!({"ids": ids})).ok();
    }
    if widgets::menu_item(ui, tl!("Select All Bound Objects"), !bound.is_empty(), false) {
        app.run("select.set", json!({"ids": bound})).ok();
    }
}

use vectorcraft_doc::NodeId;

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with_data() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 300, "height": 200})).unwrap();
        let t = app.session.execute("text.create", &json!({"x": 10, "y": 40, "text": "Alice"})).unwrap()["id"].as_u64().unwrap();
        app.session.execute("variable.define", &json!({"name": "Name", "kind": "text"})).unwrap();
        app.session.execute("variable.bind", &json!({"variable": "Name", "ids": [t]})).unwrap();
        app.session.execute("dataset.capture", &json!({})).unwrap();
        app
    }

    #[test]
    fn draws_headless() {
        let mut app = app_with_data();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut app, ui));
        out.textures_delta.clear();
        let text = crate::tests_labels::painted_text(&mut app, show);
        assert!(text.contains("Name") && text.contains("Data Sets") && text.contains("Data Set 1"), "{text}");
    }

    #[test]
    fn a_row_the_art_left_behind_is_in_italics() {
        let mut app = app_with_data();
        let t = app.session.doc().unwrap().doc.variables.objects_of("Name")[0];
        // The document no longer matches the row once the art is edited.
        app.session.execute("text.editRange", &json!({"id": t.0, "insert": "Bob"})).unwrap();
        let text = crate::tests_labels::painted_text(&mut app, show);
        assert!(text.contains("Data Set 1"), "{text}");
        // …and the row's drift is what `dataset.list` reports, so an agent sees it too.
        let sets = app.run("dataset.list", json!({})).unwrap();
        assert_eq!(sets["datasets"][0]["matches"], json!(false));
        // Updating writes the art back into the row.
        app.run("dataset.update", json!({})).unwrap();
        assert_eq!(app.run("dataset.list", json!({})).unwrap()["datasets"][0]["matches"], json!(true));
    }

    #[test]
    fn the_panel_menu_lists_the_row_commands() {
        let mut app = app_with_data();
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| menu(&mut app, ui));
        out.textures_delta.clear();
        let text = crate::tests_labels::painted_text(&mut app, menu);
        for item in [
            "New Variable…",
            "Variable Options…",
            "Capture Data Set",
            "Update Data Set",
            "Rename Data Set…",
            "Delete Data Set",
            "Select Bound Object",
            "Select All Bound Objects",
        ] {
            assert!(text.contains(item), "{item} missing from the panel menu:\n{text}");
        }
    }

    #[test]
    fn the_panel_is_registered_with_its_own_icon() {
        let row = crate::state::ICON_PANELS.iter().find(|(id, ..)| *id == ID).expect("in the icon column");
        assert_eq!(row.1, "Variables");
        assert_ne!(row.2, "dc-list-view", "Document Info's icon, which the panel used to share");
        assert!(crate::icons::exists(row.2), "{} is not a bundled icon", row.2);
    }

    /// The collapsed column draws `ICON_PANEL_GROUPS`, not `ICON_PANELS`: a panel registered
    /// but in no group opens from the Window menu and then has nowhere to collapse to. Both
    /// lists need an entry, and this is the one that was missing.
    #[test]
    fn the_panel_reaches_the_collapsed_icon_column() {
        let in_column = crate::state::ICON_PANEL_GROUPS.iter().any(|g| g.contains(&ID));
        assert!(in_column, "{ID} is in ICON_PANELS but in no ICON_PANEL_GROUPS row, so the collapsed column never draws it");
        // Every id in a group names a panel that is registered with a label and an icon.
        for group in crate::state::ICON_PANEL_GROUPS {
            for id in *group {
                let row = crate::state::ICON_PANELS.iter().find(|p| p.0 == *id).unwrap_or_else(|| panic!("{id} is in a group but not registered"));
                assert!(!row.1.is_empty() && crate::icons::exists(row.2), "{id}: {} / {}", row.1, row.2);
            }
        }
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
            "variable.rename",
            "variable.delete",
            "variable.bind",
            "variable.unbind",
            "variable.makeTextDynamic",
            "variable.makeVisibilityDynamic",
            "dataset.new",
            "dataset.set",
            "dataset.capture",
            "dataset.update",
            "dataset.rename",
            "dataset.delete",
            "dataset.select",
            "dataset.next",
            "dataset.prev",
        ] {
            assert!(in_menu(id), "{id} is not in the Window menu");
        }
        let panel = entries
            .iter()
            .find(|e| e.command.as_deref() == Some("window.panel") && e.params.get("panel").and_then(serde_json::Value::as_str) == Some(ID))
            .expect("the Variables panel is listed");
        assert_eq!(panel.path, vec!["Window".to_string(), "Variables".to_string()]);
    }

    /// A menu item that needs a name opens the dialog rather than running the command with
    /// empty params, which would only say "missing `name`".
    #[test]
    fn the_menu_dialogs_open_instead_of_failing() {
        let mut app = app_with_data();
        for (cmd, field) in [
            ("variable.define", "kind"),
            ("variable.rename", "newName"),
            ("variable.bind", "variable"),
            ("dataset.new", "name"),
            ("dataset.rename", "newName"),
            ("dataset.select", "name"),
        ] {
            app.ui.dialog = None;
            crate::menus::invoke(&mut app, cmd, json!({}));
            let d = app.ui.dialog.as_ref().unwrap_or_else(|| panic!("{cmd} opened no dialog"));
            assert_eq!(d.kind, crate::dialogs::variables::KIND, "{cmd}");
            assert_eq!(d.str("__command"), cmd);
            assert!(d.fields.contains_key(field), "{cmd}: no `{field}` field");
        }
    }
}
