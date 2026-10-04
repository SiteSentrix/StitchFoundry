//! The menu bar (Illustrator's menu tree) and UI-level commands.
//!
//! Items bound to a command id run through [`VectorcraftApp::run`]. Items not implemented yet are
//! listed (disabled, with their shortcut) so the full surface is visible and discoverable; the
//! parity tracker drives them to "done".

use serde::Serialize;
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::io;
use crate::state::{DockTab, ICON_PANELS, next_zoom};
use crate::theme::{self, Brightness, Tokens};

#[derive(Clone, Debug)]
pub enum Item {
    /// (label, command id, params)
    Cmd(&'static str, &'static str, Value),
    /// Not implemented yet: (label, shortcut)
    Todo(&'static str, &'static str),
    Sub(&'static str, Vec<Item>),
    Sep,
    /// Section header (disabled label, e.g. "Vector Effects").
    Header(&'static str),
}

fn c(label: &'static str, id: &'static str) -> Item {
    Item::Cmd(label, id, Value::Null)
}
fn cp(label: &'static str, id: &'static str, p: Value) -> Item {
    Item::Cmd(label, id, p)
}
fn todo(label: &'static str) -> Item {
    Item::Todo(label, "")
}
fn todos(label: &'static str, sc: &'static str) -> Item {
    Item::Todo(label, sc)
}
fn sub(label: &'static str, items: Vec<Item>) -> Item {
    Item::Sub(label, items)
}
/// Window → … Libraries until the code-generated libraries land (disabled entries).
fn library_placeholders() -> Vec<Item> {
    vec![todo("Built-in Libraries"), todo("User Defined"), Sep, todo("Other Library…")]
}
use Item::Sep;

/// UI-level commands: (id, label, shortcut, params doc).
pub const UI_COMMANDS: &[(&str, &str, &str, &str)] = &[
    ("file.open", "Open…", "Cmd+O", "{path?}"),
    ("file.save", "Save", "Cmd+S", "{path?}"),
    ("file.saveAs", "Save As…", "Cmd+Shift+S", "{path?}"),
    ("file.saveCopy", "Save a Copy…", "Cmd+Alt+S", "{path?}"),
    ("file.newFromTemplate", "New from Template…", "Cmd+Shift+N", "{path?} open a template as a new untitled document"),
    ("file.revert", "Revert", "F12", "{}"),
    ("file.place", "Place…", "Cmd+Shift+P", "{path?}"),
    ("file.openRecent1", "Open Recent File 1", "", "{}"),
    ("file.openRecent2", "Open Recent File 2", "", "{}"),
    ("file.openRecent3", "Open Recent File 3", "", "{}"),
    ("file.openRecent4", "Open Recent File 4", "", "{}"),
    ("file.openRecent5", "Open Recent File 5", "", "{}"),
    ("file.openRecent6", "Open Recent File 6", "", "{}"),
    ("file.openRecent7", "Open Recent File 7", "", "{}"),
    ("file.openRecent8", "Open Recent File 8", "", "{}"),
    ("file.openRecent9", "Open Recent File 9", "", "{}"),
    ("file.openRecent10", "Open Recent File 10", "", "{}"),
    ("view.goto1", "Saved View 1", "", "{} go to the 1. saved view"),
    ("view.goto2", "Saved View 2", "", "{} go to the 2. saved view"),
    ("view.goto3", "Saved View 3", "", "{} go to the 3. saved view"),
    ("view.goto4", "Saved View 4", "", "{} go to the 4. saved view"),
    ("view.goto5", "Saved View 5", "", "{} go to the 5. saved view"),
    ("view.goto6", "Saved View 6", "", "{} go to the 6. saved view"),
    ("view.goto7", "Saved View 7", "", "{} go to the 7. saved view"),
    ("view.goto8", "Saved View 8", "", "{} go to the 8. saved view"),
    ("view.goto9", "Saved View 9", "", "{} go to the 9. saved view"),
    ("view.goto10", "Saved View 10", "", "{} go to the 10. saved view"),
    ("type.recentFont1", "Recent Font 1", "", "{} apply the 1. most recently used font"),
    ("type.recentFont2", "Recent Font 2", "", "{} apply the 2. most recently used font"),
    ("type.recentFont3", "Recent Font 3", "", "{} apply the 3. most recently used font"),
    ("type.recentFont4", "Recent Font 4", "", "{} apply the 4. most recently used font"),
    ("type.recentFont5", "Recent Font 5", "", "{} apply the 5. most recently used font"),
    ("type.recentFont6", "Recent Font 6", "", "{} apply the 6. most recently used font"),
    ("type.recentFont7", "Recent Font 7", "", "{} apply the 7. most recently used font"),
    ("type.recentFont8", "Recent Font 8", "", "{} apply the 8. most recently used font"),
    ("type.recentFont9", "Recent Font 9", "", "{} apply the 9. most recently used font"),
    ("type.recentFont10", "Recent Font 10", "", "{} apply the 10. most recently used font"),
    ("file.clearRecent", "Clear Recent Files", "", "{}"),
    ("type.findFont", "Find Font…", "", "{} open the Find Font dialog (engine: text.fonts / text.replaceFont / select.font)"),
    ("file.recentFiles", "Recent Files", "", "{} → [path…] most recent first"),
    ("file.export.svg", "Export As SVG…", "", "{path?, artboard?, outlineText?} (document.export options)"),
    ("file.export.png", "Export As PNG…", "", "{path?, scale?: 1, artboard?} (document.export options)"),
    ("file.exportForScreens", "Export for Screens…", "Cmd+Alt+E", "{} opens the dialog; with params = document.exportForScreens"),
    ("file.documentSetup", "Document Setup…", "Cmd+Alt+P", "{}"),
    ("file.newDialog", "New…", "Cmd+N", "{} opens the New Document dialog"),
    ("edit.preferences", "Preferences…", "Cmd+K", "{category?} open Preferences (engine: prefs.get / prefs.set / prefs.list)"),
    ("edit.keyboardShortcuts", "Keyboard Shortcuts…", "Cmd+Alt+Shift+K", "{}"),
    ("shortcuts.set", "Set Keyboard Shortcut", "", "{id: command id or tool:<id>, shortcut: \"Cmd+Shift+K\" | \"\" (none) | null (default), force?}"),
    ("shortcuts.list", "List Keyboard Shortcuts", "", "{query?} → [{id, label, group, shortcut, default, overridden}]"),
    ("shortcuts.conflicts", "Keyboard Shortcut Conflicts", "", "{} → [{shortcut, ids}]"),
    ("shortcuts.reset", "Reset Keyboard Shortcuts", "", "{}"),
    (
        "shortcuts.preset",
        "Keyboard Shortcut Set",
        "",
        "{name: \"VectorCraft Defaults\" | \"Classic Defaults\"} (names of earlier versions are accepted)",
    ),
    ("shortcuts.export", "Export Keyboard Shortcuts…", "", "{path?}"),
    ("shortcuts.import", "Import Keyboard Shortcuts…", "", "{path? | data?}"),
    ("view.outline", "Outline", "Cmd+Y", "{} toggle Outline/Preview"),
    ("view.pixelPreview", "Pixel Preview", "Cmd+Alt+Y", "{}"),
    ("view.trimView", "Trim View", "", "{} toggle: hide everything outside the artboards"),
    ("view.cornerWidget", "Hide Corner Widget", "", "{} toggle the live corner widgets"),
    ("view.snapToPixel", "Snap to Pixel", "", "{} toggle: drawing and moving land on whole pixels"),
    ("view.textThreads", "Hide Text Threads", "Cmd+Shift+Y", "{} toggle the thread lines between threaded text frames"),
    ("view.gradientAnnotator", "Hide Gradient Annotator", "Cmd+Alt+G", "{} toggle the Gradient tool's annotator"),
    ("type.hiddenCharacters", "Show Hidden Characters", "Cmd+Alt+I", "{} toggle markers for spaces, paragraph ends and story ends"),
    ("effect.last", "Last Effect…", "Cmd+Alt+Shift+E", "{} open the dialog of the last effect applied"),
    ("view.zoomIn", "Zoom In", "Cmd+=", "{}"),
    ("view.zoomOut", "Zoom Out", "Cmd+-", "{}"),
    ("view.fitArtboard", "Fit Artboard in Window", "Cmd+0", "{}"),
    ("view.fitAll", "Fit All in Window", "Cmd+Alt+0", "{}"),
    ("view.actualSize", "Actual Size", "Cmd+1", "{}"),
    ("view.setZoom", "Set Zoom", "", "{zoom: percent, center?: [x,y]}"),
    ("view.edges", "Hide Edges", "Cmd+H", "{}"),
    ("view.artboards", "Hide Artboards", "Cmd+Shift+H", "{}"),
    ("view.rulers", "Show Rulers", "Cmd+R", "{}"),
    ("view.boundingBox", "Hide Bounding Box", "Cmd+Shift+B", "{}"),
    ("view.transparencyGrid", "Show Transparency Grid", "Cmd+Shift+D", "{}"),
    ("view.guides", "Hide Guides", "Cmd+;", "{}"),
    ("view.smartGuides", "Smart Guides", "Cmd+U", "{}"),
    ("view.grid", "Show Grid", "Cmd+'", "{}"),
    ("view.snapToGrid", "Snap to Grid", "Cmd+Shift+'", "{}"),
    ("view.snapToPoint", "Snap to Point", "Cmd+Alt+'", "{}"),
    ("view.presentation", "Presentation Mode", "Shift+F", "{}"),
    ("view.screenMode", "Screen Mode", "F", "{mode?: 0..2} (no param cycles)"),
    ("view.rotateReset", "Reset Rotate View", "Cmd+Shift+1", "{}"),
    ("window.control", "Control", "", "{}"),
    ("window.toolbar", "Tools", "", "{}"),
    ("window.toolbarColumns", "Toolbar: Single/Double Column", "", "{}"),
    ("window.toolbarAdvanced", "Toolbar: Advanced / Basic", "", "{}"),
    ("window.taskBar", "Contextual Task Bar", "", "{}"),
    ("window.dock", "Panels", "Tab", "{} show/hide all panels"),
    ("window.panel", "Show Panel", "", "{panel: id} e.g. layers, swatches, stroke"),
    ("window.brightness", "UI Brightness", "", "{brightness: dark|mediumDark|mediumLight|light}"),
    ("window.workspace", "Workspace", "", "{name} switch workspace (Essentials, Essentials Classic, Painting, …)"),
    ("window.workspace.reset", "Reset Essentials", "", "{} reset the current workspace"),
    ("window.workspace.new", "New Workspace…", "", "{name?} save the current layout"),
    ("window.workspace.manage", "Manage Workspaces…", "", "{}"),
    ("window.workspace.delete", "Delete Workspace", "", "{name}"),
    ("window.workspace.rename", "Rename Workspace", "", "{name, to}"),
    ("window.workspace.list", "List Workspaces", "", "{}"),
    ("window.newWindow", "New Window", "", "{}"),
    ("tool.select", "Select Tool", "", "{tool: id} (see tools)"),
    ("tool.setOption", "Tool Option", "", "{key, value}"),
    (
        "effect.dialog",
        "Effect…",
        "",
        "{effect: id, index?: int (edit that applied effect in place, prefilled; OK runs effect.setParams), item?: appearance item index|null (the fill/stroke whose effects, null the object's; default: the Appearance panel's active item)} open the effect's dialog with live preview; without `index`, an effect already in that list first opens `effectExists` (confirm edits it, discard adds another) → {pending: \"effectExists\"}",
    ),
    ("ui.paramDialog", "Command Dialog", "", "{command, label?, params} open a parameter dialog for any command"),
    ("ui.recolorDialog", "Recolor Artwork…", "", "{} open Recolor Artwork (engine: recolor.colors / recolor.apply)"),
    ("effect.applyLast", "Apply Last Effect", "Cmd+Shift+E", "{}"),
    ("file.export.pdf", "Save as PDF…", "", "{path?, artboard? | artboards? | range?: \"1-3, 5\"} (document.export options)"),
    ("help.about", "About VectorCraft", "", "{}"),
    ("help.commandPalette", "Search Commands…", "Cmd+Shift+/", "{}"),
    ("app.quit", "Quit VectorCraft", "Cmd+Q", "{}"),
    (
        "ui.swatchOptions",
        "Swatch Options…",
        "",
        "{name} edit a swatch: Swatch Options for a colour (dialog `swatchOptions`, engine: swatch.edit) or a gradient (the same dialog with `name` only; it shows the gradient), pattern editing for a pattern",
    ),
    (
        "ui.newSwatch",
        "New Swatch…",
        "",
        "{spot?, group?: colour group name} open New Swatch for the active fill or stroke (dialog `newSwatch`, engine: swatch.new)",
    ),
    (
        "ui.newColorGroup",
        "New Color Group…",
        "",
        "{swatches?: [names]} open New Color Group, from those swatches or the selected artwork (dialog `newColorGroup`, engine: swatch.newGroup)",
    ),
    (
        "ui.colorPicker",
        "Color Picker…",
        "",
        "{stroke?: bool (default: the active proxy), color?: \"#rrggbb\"|[r,g,b]|{c,m,y,k}|{gray} (default: the proxy's colour)} open the Color Picker (fields: hex or color, channel, webOnly, swatches); OK runs paint.setFill / paint.setStroke",
    ),
    (
        "ui.graphicStyleOptions",
        "Graphic Style Options…",
        "",
        "{name?} open Graphic Style Options (dialog `graphicStyleOptions`, field `name`): for style `name` OK renames it (graphicStyle.rename); without, OK makes a new style of that name from the selection (graphicStyle.new)",
    ),
    (
        "tool.options",
        "Tool Options…",
        "",
        "{tool: id} what double-clicking a tool button opens: gradient → the Gradient panel (window.panel), eyedropper → Eyedropper Options (dialog `command` running eyedropper.setOptions, fields appearance, transparency)",
    ),
    (
        "ui.colorGuideOptions",
        "Color Guide Options…",
        "",
        "{} open Color Guide Options (dialog `colorGuideOptions`, fields `steps` 1–20 and `amount` 0–100): OK sets the Color Guide panel's variation grid (read back with `ui.inspect`: ui.color_guide; engine: color.harmony)",
    ),
    (
        "ui.colorBalanceDialog",
        "Adjust Color Balance…",
        "",
        "{} open Adjust Colors for the selection (dialog `colorBalance`, fields `mode` gray|rgb|cmyk|global, channels r g b / c m y k / gray −100..100, `convert`, `fill`, `stroke`, `preview`): previews live, OK runs edit.colors.adjustBalance as one undo step",
    ),
    (
        "ui.saturateDialog",
        "Saturate…",
        "",
        "{} open Saturate for the selection (dialog `saturate`, fields `intensity` −100..100, `preview`): previews live, OK runs edit.colors.saturate as one undo step",
    ),
    (
        "window.swatchLibrary",
        "Swatch Library",
        "",
        "{library: id or name (see swatch.library.list) | null (close)} open the read-only library panel on a swatch library (UI state `library_panel`); clicking a swatch there runs swatch.library.add with apply → {open, name, count}",
    ),
    (
        "window.swatchLibrary.other",
        "Other Library…",
        "",
        "{path?} (default: pick a file) load a .vcswatches or .gpl library, or another document's swatches (engine: swatch.library.load), and open it in the library panel",
    ),
    (
        "ui.saveSwatchLibrary",
        "Save Swatch Library…",
        "",
        "{names?: [the swatches selected in the Swatches panel]} open Save Swatch Library (dialog `saveSwatchLibrary`: name, format: vcswatches|gpl|css, user: save to the user library folder, selectedOnly); OK runs swatch.library.save",
    ),
    ("window.userSwatchLibrary1", "User Swatch Library 1", "", "{} open the 1. User Defined swatch library (swatch.library.list, category user)"),
    ("window.userSwatchLibrary2", "User Swatch Library 2", "", "{} open the 2. User Defined swatch library"),
    ("window.userSwatchLibrary3", "User Swatch Library 3", "", "{} open the 3. User Defined swatch library"),
    ("window.userSwatchLibrary4", "User Swatch Library 4", "", "{} open the 4. User Defined swatch library"),
    ("window.userSwatchLibrary5", "User Swatch Library 5", "", "{} open the 5. User Defined swatch library"),
    ("window.userSwatchLibrary6", "User Swatch Library 6", "", "{} open the 6. User Defined swatch library"),
    ("window.userSwatchLibrary7", "User Swatch Library 7", "", "{} open the 7. User Defined swatch library"),
    ("window.userSwatchLibrary8", "User Swatch Library 8", "", "{} open the 8. User Defined swatch library"),
    ("window.userSwatchLibrary9", "User Swatch Library 9", "", "{} open the 9. User Defined swatch library"),
    ("window.userSwatchLibrary10", "User Swatch Library 10", "", "{} open the 10. User Defined swatch library"),
    (
        "ui.mergeGraphicStyles",
        "Merge Graphic Styles…",
        "",
        "{names: [two or more styles]} open Graphic Style Options (dialog `graphicStyleOptions`, field `name`) to name the style OK merges from them (graphicStyle.merge)",
    ),
];

/// Handle a UI command. `None` = not a UI command (the engine handles it).
pub fn run_ui_command(app: &mut VectorcraftApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    if let Some(r) = crate::panels::character::intercept_text_command(app, id) {
        return Some(r);
    }
    if let Some(r) = crate::shortcut_editor::run_command(app, id, p).or_else(|| crate::workspaces::run_command(app, id, p)) {
        return Some(r);
    }
    let s = |k: &str| p.get(k).and_then(Value::as_str).map(str::to_string);
    let flag = |b: &mut bool| {
        *b = !*b;
        Ok(json!(*b))
    };
    let r = match id {
        "file.newDialog" => {
            app.ui.dialog = Some(crate::state::Dialog::new(
                "newDocument",
                json!({"preset": "Letter", "width": "612 pt", "height": "792 pt", "units": "Points", "artboards": 1, "colorMode": "RGB", "name": "Untitled-1"}),
            ));
            Ok(Value::Null)
        }
        "file.open" => match s("path") {
            Some(path) => io::open_path(app, &path).map(|_| Value::Null),
            None => io::open_dialog(app).map(|_| Value::Null),
        },
        "file.save" => io::save(app, s("path"), false).map(|p| json!({"path": p})),
        "file.saveAs" | "file.saveCopy" => io::save(app, s("path"), true).map(|p| json!({"path": p})),
        "document.exportSelection" => {
            io::save_command_output(app, id, "png", if p.is_object() { p.clone() } else { json!({}) }).map(|p| json!({"path": p}))
        }
        "file.saveAsTemplate" => {
            io::save_command_output(app, id, "vectorcraft", if p.is_object() { p.clone() } else { json!({}) }).map(|p| json!({"path": p}))
        }
        "file.newFromTemplate" => match s("path") {
            Some(path) => io::open_path(app, &path).map(|_| Value::Null),
            None => io::open_dialog(app).map(|_| Value::Null),
        },
        "file.revert" => {
            let path = app.session.active().and_then(|d| d.path.clone());
            match path {
                Some(path) => {
                    let i = app.session.active_index().unwrap_or(0);
                    app.session.close_document(i);
                    io::open_path(app, &path).map(|_| Value::Null)
                }
                None => Err("document has never been saved".into()),
            }
        }
        id if id.starts_with("file.openRecent") => {
            let n: usize = id["file.openRecent".len()..].parse().unwrap_or(0);
            match n.checked_sub(1).and_then(|i| app.ui.recent_files.get(i)).cloned() {
                Some(path) => io::open_path(app, &path).map(|_| Value::Null),
                None => Err("no such recent file".into()),
            }
        }
        "type.findFont" => {
            crate::find_font::open(app);
            Ok(Value::Null)
        }
        "file.clearRecent" => {
            app.ui.recent_files.clear();
            Ok(Value::Null)
        }
        "file.recentFiles" => Ok(json!(app.ui.recent_files)),
        "file.place" => {
            let path = match s("path") {
                Some(p) => Some(p),
                None => app.services.pick_open.as_mut().and_then(|f| f()),
            };
            match path {
                Some(path) => {
                    let bytes = app.services.read.as_ref().ok_or("no reader".to_string()).and_then(|r| r(&path));
                    let name = std::path::Path::new(&path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                    bytes.and_then(|b| io::place_bytes(app, &name, &b)).map(|_| Value::Null)
                }
                None => Err("cancelled".into()),
            }
        }
        "file.export.svg" => io::export(app, Some("svg"), s("path"), p).map(|p| json!({"path": p})),
        "file.exportForScreens" if p.as_object().is_none_or(|o| o.is_empty()) => {
            let n = app.session.active().map(|d| d.doc.artboards.len()).unwrap_or(0);
            let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
            app.ui.dialog = Some(crate::state::Dialog::new(
                "exportForScreens",
                json!({"boards": vec![true; n], "formats": [{"format": "png", "scale": 1, "suffix": ""}, {"format": "png", "scale": 2, "suffix": "@2x"}], "folder": format!("{home}/Desktop/VectorCraft Export"), "prefix": ""}),
            ));
            Ok(Value::Null)
        }
        "file.exportForScreens" => app.run("document.exportForScreens", p.clone()),
        "file.export.png" => io::export(app, Some("png"), s("path"), p).map(|p| json!({"path": p})),
        "file.documentSetup" => {
            let units = app.session.active().map(|d| d.doc.units.label()).unwrap_or("Points");
            app.ui.dialog = Some(crate::state::Dialog::new("documentSetup", json!({"units": units})));
            Ok(Value::Null)
        }
        "edit.preferences" => {
            crate::prefs_dialog::open(app, s("category").as_deref());
            Ok(Value::Null)
        }
        // Edit → Color Settings… / Assign Profile… (no params): show the colour-management panel.
        "edit.colorSettings" | "edit.assignProfile" if p.as_object().is_none_or(|o| o.is_empty()) => {
            app.ui.open_panel = Some("separations".into());
            app.ui.dock = true;
            app.session.execute(id, p).map_err(|e| e.to_string())
        }
        "view.outline" => flag(&mut app.ui.view.outline),
        "view.pixelPreview" => flag(&mut app.ui.view.pixel_preview),
        "view.trimView" => flag(&mut app.ui.view.trim_view),
        "view.cornerWidget" => flag(&mut app.ui.view.corner_widgets),
        // New View: the engine stores the current zoom, centre and rotation under the given name.
        "view.saved.new" if p.get("zoom").is_none() => {
            let Some(v) = app.view().copied() else { return Some(Err("no document".into())) };
            let mut params = if p.is_object() { p.clone() } else { json!({}) };
            params["center"] = json!([v.center.x, v.center.y]);
            params["zoom"] = json!(v.zoom);
            params["rotation"] = json!(v.rotation);
            app.session.execute(id, &params).map_err(|e| e.to_string())
        }
        id if id.starts_with("view.goto") => {
            let n: usize = id["view.goto".len()..].parse().unwrap_or(0);
            let saved = app.session.active().and_then(|d| n.checked_sub(1).and_then(|i| d.doc.views.get(i)).cloned());
            match (saved, app.view_mut()) {
                (Some(sv), Some(v)) => {
                    v.center = sv.center;
                    v.zoom = sv.zoom;
                    v.rotation = sv.rotation;
                    Ok(json!({ "name": sv.name }))
                }
                _ => Err("no such view".into()),
            }
        }
        "view.snapToPixel" => flag(&mut app.ui.view.snap_to_pixel),
        "view.textThreads" => flag(&mut app.ui.view.text_threads),
        "type.hiddenCharacters" => flag(&mut app.ui.view.hidden_chars),
        "view.gradientAnnotator" => flag(&mut app.ui.view.gradient_annotator),
        "effect.last" => match app.last_effect.clone() {
            Some((e, params)) => {
                let label = vectorcraft_effects::effect_info(&e).map(|i| i.label.trim_end_matches('…').to_string()).unwrap_or_else(|| e.clone());
                let mut fields = params.as_object().cloned().unwrap_or_default();
                fields.insert("__effect".into(), json!(e));
                fields.insert("__label".into(), json!(label));
                fields.insert("preview".into(), json!(true));
                app.ui.dialog = Some(crate::state::Dialog { kind: "effect".into(), fields });
                Ok(Value::Null)
            }
            None => Err("no effect applied yet".into()),
        },
        id if id.starts_with("type.recentFont") => {
            let n: usize = id["type.recentFont".len()..].parse().unwrap_or(0);
            match n.checked_sub(1).and_then(|i| app.ui.recent_fonts.get(i)).cloned() {
                Some(font) => app.run("text.setStyle", json!({ "font": font })),
                None => Err("no such recent font".into()),
            }
        }
        "view.edges" => flag(&mut app.ui.view.edges),
        "view.artboards" => flag(&mut app.ui.view.artboards),
        "view.rulers" => flag(&mut app.ui.view.rulers),
        "view.boundingBox" => flag(&mut app.ui.view.bounding_box),
        "view.transparencyGrid" => flag(&mut app.ui.view.transparency_grid),
        "view.guides" => flag(&mut app.ui.view.guides),
        "view.smartGuides" => flag(&mut app.ui.view.smart_guides),
        "view.grid" => flag(&mut app.ui.view.grid),
        "view.snapToGrid" => flag(&mut app.ui.view.snap_to_grid),
        "view.snapToPoint" => flag(&mut app.ui.view.snap_to_point),
        "view.zoomIn" | "view.zoomOut" => {
            let up = id == "view.zoomIn";
            match app.view_mut() {
                Some(v) => {
                    v.zoom = next_zoom(v.zoom, up);
                    Ok(json!({"zoom": v.zoom * 100.0}))
                }
                None => Err("no document".into()),
            }
        }
        "view.setZoom" => {
            let z = p.get("zoom").and_then(Value::as_f64).unwrap_or(100.0) / 100.0;
            let center =
                p.get("center").and_then(Value::as_array).and_then(|a| Some(vectorcraft_geom::Point::new(a.first()?.as_f64()?, a.get(1)?.as_f64()?)));
            match app.view_mut() {
                Some(v) => {
                    v.zoom = z.clamp(0.0313, 640.0);
                    v.fitted = true;
                    if let Some(c) = center {
                        v.center = c;
                    }
                    Ok(Value::Null)
                }
                None => Err("no document".into()),
            }
        }
        "view.fitArtboard" | "view.fitAll" | "view.actualSize" => {
            crate::canvas::fit(app, id);
            Ok(Value::Null)
        }
        "view.presentation" => {
            app.ui.screen_mode = if app.ui.screen_mode == 3 { 0 } else { 3 };
            Ok(json!(app.ui.screen_mode))
        }
        "view.screenMode" => {
            app.ui.screen_mode = match p.get("mode").and_then(Value::as_u64) {
                Some(m) => m.min(3) as u8,
                None => (app.ui.screen_mode + 1) % 3,
            };
            Ok(json!(app.ui.screen_mode))
        }
        "view.rotateReset" => {
            if let Some(v) = app.view_mut() {
                v.rotation = 0.0;
            }
            Ok(Value::Null)
        }
        "window.control" => flag(&mut app.ui.control_bar),
        "window.toolbar" => flag(&mut app.ui.toolbar),
        "window.toolbarColumns" => flag(&mut app.ui.toolbar_double),
        "window.toolbarAdvanced" => flag(&mut app.ui.toolbar_advanced),
        "window.taskBar" => flag(&mut app.ui.task_bar),
        "window.dock" => {
            let on = !(app.ui.dock && app.ui.toolbar);
            app.ui.dock = on;
            app.ui.toolbar = on;
            app.ui.control_bar = on;
            Ok(json!(on))
        }
        "window.panel" => {
            let panel = s("panel").unwrap_or_default();
            match panel.as_str() {
                "properties" => {
                    app.ui.dock_tab = DockTab::Properties;
                    Ok(Value::Null)
                }
                "layers" => {
                    app.ui.dock_tab = DockTab::Layers;
                    Ok(Value::Null)
                }
                "libraries" => {
                    app.ui.dock_tab = DockTab::Libraries;
                    Ok(Value::Null)
                }
                p if ICON_PANELS.iter().any(|(id, _, _)| *id == p) => {
                    app.ui.open_panel = if app.ui.open_panel.as_deref() == Some(p) { None } else { Some(p.to_string()) };
                    app.ui.dock = true;
                    Ok(json!({"open": app.ui.open_panel}))
                }
                other => Err(format!("unknown panel `{other}`")),
            }
        }
        "window.brightness" => match s("brightness").as_deref().and_then(Brightness::parse) {
            Some(b) => {
                app.ui.brightness = b;
                app.session.prefs.ui_brightness = b.id().into();
                app.canvas.key = None;
                Ok(json!(b.id()))
            }
            None => Err("brightness must be dark|mediumDark|mediumLight|light".into()),
        },
        "window.newWindow" => Err("multiple windows land with M11.5".into()),
        "tool.select" => match s("tool") {
            Some(t) if vectorcraft_tools::tool_info(&t).is_some() => {
                app.select_tool(&t);
                Ok(json!({"tool": app.session.tool_id()}))
            }
            Some(t) => Err(format!("unknown tool `{t}`")),
            None => Err("missing `tool`".into()),
        },
        "tool.setOption" => {
            let k = s("key").unwrap_or_default();
            app.session.set_tool_option(&k, p.get("value").unwrap_or(&Value::Null));
            Ok(app.session.tool_options())
        }
        "effect.dialog" => crate::dialogs::open_effect_dialog(app, p),
        "ui.recolorDialog" => match app.run("recolor.colors", json!({})) {
            Ok(v) => {
                let pairs: Vec<Value> = v["colors"].as_array().cloned().unwrap_or_default().iter().map(|c| json!([c["hex"], c["hex"]])).collect();
                app.ui.dialog = Some(crate::state::Dialog::new("recolor", json!({"pairs": pairs, "preview": true})));
                Ok(Value::Null)
            }
            Err(e) => Err(e),
        },
        "ui.paramDialog" => {
            let cmd = s("command").unwrap_or_default();
            let mut fields = p.get("params").and_then(Value::as_object).cloned().unwrap_or_default();
            fields.insert("__command".into(), json!(cmd));
            fields.insert("__label".into(), json!(s("label").unwrap_or(cmd.clone())));
            app.ui.dialog = Some(crate::state::Dialog { kind: "command".into(), fields });
            Ok(Value::Null)
        }
        "effect.applyLast" => match app.last_effect.clone() {
            Some((e, params)) => app.run("effect.apply", json!({"effect": e, "params": params})),
            None => Err("no effect applied yet".into()),
        },
        "file.export.pdf" => io::export(app, Some("pdf"), s("path"), p).map(|p| json!({"path": p})),
        "help.about" => {
            app.ui.about = true;
            Ok(Value::Null)
        }
        "help.commandPalette" => {
            app.ui.palette_open = !app.ui.palette_open;
            app.ui.palette_query.clear();
            Ok(Value::Null)
        }
        // Closing asks Save / Don't Save / Cancel for modified documents first (`unsaved`).
        "file.close" => match p.get("index").and_then(Value::as_u64).map(|i| i as usize).or(app.session.active_index()) {
            Some(i) => crate::unsaved::close(app, i),
            None => return None,
        },
        "file.closeAll" => crate::unsaved::close_all(app, "closeAll"),
        "app.quit" => crate::unsaved::close_all(app, "quit"),
        "ui.swatchOptions" => match s("name") {
            Some(name) => crate::dialogs::swatch_options::open(app, &name),
            None => Err("missing `name`".into()),
        },
        "ui.newSwatch" => crate::dialogs::new_swatch::open(app, p.get("spot").and_then(Value::as_bool).unwrap_or(false), s("group").as_deref()),
        "ui.newColorGroup" => {
            let names = p.get("swatches").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect());
            crate::dialogs::new_color_group::open(app, names.unwrap_or_default())
        }
        "ui.colorPicker" => crate::dialogs::open_color_picker(app, p),
        "ui.graphicStyleOptions" => crate::dialogs::graphic_style_options::open(app, s("name").as_deref()),
        "tool.options" => crate::toolbar::open_options(app, &s("tool").unwrap_or_default()),
        "ui.colorGuideOptions" => {
            crate::dialogs::color_guide_options::open(app);
            Ok(Value::Null)
        }
        "ui.colorBalanceDialog" => {
            crate::dialogs::color_balance::open(app);
            Ok(Value::Null)
        }
        "ui.saturateDialog" => {
            crate::dialogs::saturate::open(app);
            Ok(Value::Null)
        }
        "window.swatchLibrary" => crate::panels::swatches::open_library(app, p),
        "window.swatchLibrary.other" => crate::panels::swatches::other_library(app, s("path")),
        "ui.saveSwatchLibrary" => {
            let names = p.get("names").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect());
            crate::dialogs::save_swatch_library::open(app, names.unwrap_or_default())
        }
        id if id.starts_with(crate::panels::swatches::USER_SLOT) => match crate::panels::swatches::user_library(app, id) {
            Some(l) => crate::panels::swatches::open_library(app, &json!({ "library": l.id })),
            None => Err("no such user library".into()),
        },
        "ui.mergeGraphicStyles" => {
            let names = p.get("names").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect());
            crate::dialogs::graphic_style_options::open_merge(app, names.unwrap_or_default())
        }
        _ => return None,
    };
    Some(r)
}

/// Checked state for toggle items.
pub fn checked(app: &VectorcraftApp, id: &str, p: &Value) -> Option<bool> {
    let v = &app.ui.view;
    Some(match id {
        "view.outline" => v.outline,
        "view.pixelPreview" => v.pixel_preview,
        "view.trimView" => v.trim_view,
        "view.snapToPixel" => v.snap_to_pixel,
        // Type on a Path effect of the selected path type.
        "type.pathOptions" if p.get("effect").is_some() => {
            let st = app.session.active()?;
            let t = st.selection.objects.iter().find_map(|id| match st.doc.node(*id).map(|n| &n.kind) {
                Some(vectorcraft_engine::doc::NodeKind::Text(t)) if matches!(t.kind, vectorcraft_engine::doc::TextKind::OnPath { .. }) => Some(t),
                _ => None,
            })?;
            p.get("effect").and_then(Value::as_str) == Some(t.path_effect.id())
        }
        "view.smartGuides" => v.smart_guides,
        "view.grid" => v.grid,
        "view.snapToGrid" => v.snap_to_grid,
        "view.snapToPoint" => v.snap_to_point,
        "view.rulers" => v.rulers,
        "window.control" => app.ui.control_bar,
        "window.toolbar" => app.ui.toolbar,
        "window.toolbarAdvanced" => app.ui.toolbar_advanced,
        "window.taskBar" => app.ui.task_bar,
        "window.panel" => {
            let panel = p.get("panel").and_then(Value::as_str).unwrap_or("");
            match panel {
                "properties" => app.ui.dock_tab == DockTab::Properties,
                "layers" => app.ui.dock_tab == DockTab::Layers,
                "libraries" => app.ui.dock_tab == DockTab::Libraries,
                _ => app.ui.open_panel.as_deref() == Some(panel),
            }
        }
        "window.workspace" => p.get("name").and_then(Value::as_str) == Some(app.ui.workspace.as_str()),
        "window.brightness" => p.get("brightness").and_then(Value::as_str).and_then(Brightness::parse) == Some(app.ui.brightness),
        "view.proofColors" => vectorcraft_render::proof::view().proof_colors,
        "view.overprintPreview" => vectorcraft_render::proof::view().overprint,
        "view.proofSetup" => p.get("target").and_then(Value::as_str) == Some(vectorcraft_render::proof::view().setup.target.id().as_str()),
        "file.documentColorMode" | "object.convertDocumentColorMode" => {
            let cmyk = app.session.active().is_some_and(|d| d.doc.color_mode == vectorcraft_engine::doc::ColorMode::Cmyk);
            p.get("mode").and_then(Value::as_str) == Some(if cmyk { "cmyk" } else { "rgb" })
        }
        _ => return None,
    })
}

/// Label for toggles whose text flips (Outline/Preview, Hide/Show …).
pub fn dynamic_label(app: &VectorcraftApp, id: &str, label: &str) -> String {
    let v = &app.ui.view;
    match id {
        "view.outline" => if v.outline { "Preview" } else { "Outline" }.into(),
        "window.workspace.reset" => format!("Reset {}", app.ui.workspace),
        "view.edges" => if v.edges { "Hide Edges" } else { "Show Edges" }.into(),
        "view.cornerWidget" => if v.corner_widgets { "Hide Corner Widget" } else { "Show Corner Widget" }.into(),
        "view.textThreads" => if v.text_threads { "Hide Text Threads" } else { "Show Text Threads" }.into(),
        "type.hiddenCharacters" => if v.hidden_chars { "Hide Hidden Characters" } else { "Show Hidden Characters" }.into(),
        "view.gradientAnnotator" => if v.gradient_annotator { "Hide Gradient Annotator" } else { "Show Gradient Annotator" }.into(),
        "effect.last" => match &app.last_effect {
            Some((e, _)) => format!("Last Effect: {}", vectorcraft_effects::effect_info(e).map(|i| i.label).unwrap_or(e.as_str())),
            None => label.into(),
        },
        id if id.starts_with("type.recentFont") => {
            let n: usize = id["type.recentFont".len()..].parse().unwrap_or(0);
            n.checked_sub(1).and_then(|i| app.ui.recent_fonts.get(i)).cloned().unwrap_or_else(|| "—".into())
        }
        id if id.starts_with("view.goto") => {
            let n: usize = id["view.goto".len()..].parse().unwrap_or(0);
            app.session.active().and_then(|d| n.checked_sub(1).and_then(|i| d.doc.views.get(i)).map(|v| v.name.clone())).unwrap_or_else(|| "—".into())
        }
        "view.artboards" => if v.artboards { "Hide Artboards" } else { "Show Artboards" }.into(),
        "view.rulers" => if v.rulers { "Hide Rulers" } else { "Show Rulers" }.into(),
        "view.boundingBox" => if v.bounding_box { "Hide Bounding Box" } else { "Show Bounding Box" }.into(),
        "view.transparencyGrid" => if v.transparency_grid { "Hide Transparency Grid" } else { "Show Transparency Grid" }.into(),
        "view.guides" => if v.guides { "Hide Guides" } else { "Show Guides" }.into(),
        "view.grid" => if v.grid { "Hide Grid" } else { "Show Grid" }.into(),
        "view.guides.lock" => if app.session.guides_locked() { "Unlock Guides" } else { "Lock Guides" }.into(),
        id if id.starts_with("file.openRecent") => {
            let n: usize = id["file.openRecent".len()..].parse().unwrap_or(0);
            n.checked_sub(1)
                .and_then(|i| app.ui.recent_files.get(i))
                .map(|p| std::path::Path::new(p).file_name().map_or(p.clone(), |f| f.to_string_lossy().to_string()))
                .unwrap_or_else(|| "—".into())
        }
        "edit.undo" => app.session.active().and_then(|d| d.history.undo.last()).map(|h| format!("Undo {}", h.label)).unwrap_or_else(|| "Undo".into()),
        "edit.redo" => app.session.active().and_then(|d| d.history.redo.last()).map(|h| format!("Redo {}", h.label)).unwrap_or_else(|| "Redo".into()),
        id if id.starts_with(crate::panels::swatches::USER_SLOT) => {
            crate::panels::swatches::user_library(app, id).map_or_else(|| "—".into(), |l| l.name)
        }
        _ => label.into(),
    }
}

/// Menu slots that are left out while they have nothing to show (unused saved views, User
/// Defined swatch libraries).
fn hidden_when_disabled(id: &str) -> bool {
    id.starts_with("view.goto") || id.starts_with(crate::panels::swatches::USER_SLOT)
}

/// File → Open Recent Files slots.
const RECENT_IDS: [&str; 10] = [
    "file.openRecent1",
    "file.openRecent2",
    "file.openRecent3",
    "file.openRecent4",
    "file.openRecent5",
    "file.openRecent6",
    "file.openRecent7",
    "file.openRecent8",
    "file.openRecent9",
    "file.openRecent10",
];

/// Type → Recent Fonts slots.
const RECENT_FONT_IDS: [&str; 10] = [
    "type.recentFont1",
    "type.recentFont2",
    "type.recentFont3",
    "type.recentFont4",
    "type.recentFont5",
    "type.recentFont6",
    "type.recentFont7",
    "type.recentFont8",
    "type.recentFont9",
    "type.recentFont10",
];

/// Effective shortcut of a command: the user's override (Edit → Keyboard Shortcuts) or the default.
pub fn shortcut_of(id: &str) -> Option<&'static str> {
    crate::shortcut_editor::command_shortcut(id)
}

/// Is a command currently enabled?
pub fn enabled(app: &VectorcraftApp, id: &str) -> bool {
    if let Some(c) = vectorcraft_engine::find_command(id) {
        return (c.enabled)(&app.session).is_ok();
    }
    match id {
        "file.save"
        | "file.saveAs"
        | "file.saveCopy"
        | "file.place"
        | "file.export.svg"
        | "file.export.png"
        | "file.exportForScreens"
        | "file.documentSetup"
        | "view.zoomIn"
        | "view.zoomOut"
        | "view.fitArtboard"
        | "view.fitAll"
        | "view.actualSize" => app.session.active().is_some(),
        "file.revert" => app.session.active().is_some_and(|d| d.path.is_some() && d.is_dirty()),
        id if id.starts_with("file.openRecent") => {
            id["file.openRecent".len()..].parse::<usize>().is_ok_and(|n| n >= 1 && n <= app.ui.recent_files.len())
        }
        "file.clearRecent" => !app.ui.recent_files.is_empty(),
        id if id.starts_with("type.recentFont") => {
            id["type.recentFont".len()..].parse::<usize>().is_ok_and(|n| n >= 1 && n <= app.ui.recent_fonts.len()) && app.session.active().is_some()
        }
        id if id.starts_with("view.goto") => {
            id["view.goto".len()..].parse::<usize>().is_ok_and(|n| n >= 1 && app.session.active().is_some_and(|d| n <= d.doc.views.len()))
        }
        "effect.dialog" | "ui.recolorDialog" => app.session.active().is_some_and(|d| !d.selection.is_empty()),
        "effect.applyLast" | "effect.last" => app.last_effect.is_some() && app.session.active().is_some_and(|d| !d.selection.is_empty()),
        "file.export.pdf" => app.session.active().is_some(),
        "ui.swatchOptions" | "ui.newSwatch" | "ui.newColorGroup" => app.session.active().is_some(),
        "ui.graphicStyleOptions" => app.session.active().is_some(),
        "ui.colorBalanceDialog" | "ui.saturateDialog" => app.session.active().is_some_and(|d| !d.selection.is_empty()),
        "ui.saveSwatchLibrary" => app.session.active().is_some(),
        id if id.starts_with(crate::panels::swatches::USER_SLOT) => crate::panels::swatches::user_library(app, id).is_some(),
        _ => true,
    }
}

pub fn menu_tree() -> Vec<(&'static str, Vec<Item>)> {
    let panel = |label: &'static str, id: &'static str| cp(label, "window.panel", json!({ "panel": id }));
    vec![
        (
            "VectorCraft",
            vec![
                c("About VectorCraft", "help.about"),
                c("Join Our Discord", "help.discord"),
                Sep,
                c("Settings…", "edit.preferences"),
                Sep,
                sub("UI Brightness", Brightness::ALL.iter().map(|b| cp(b.label(), "window.brightness", json!({"brightness": b.id()}))).collect()),
                Sep,
                c("Quit VectorCraft", "app.quit"),
            ],
        ),
        (
            "File",
            vec![
                c("New…", "file.newDialog"),
                c("New from Template…", "file.newFromTemplate"),
                c("Open…", "file.open"),
                sub("Open Recent Files", {
                    let mut v: Vec<Item> = RECENT_IDS.iter().map(|id| c("Recent File", id)).collect();
                    v.push(Sep);
                    v.push(c("Clear Recent Files", "file.clearRecent"));
                    v
                }),
                Sep,
                c("Close", "file.close"),
                c("Close All", "file.closeAll"),
                c("Save", "file.save"),
                c("Save As…", "file.saveAs"),
                c("Save a Copy…", "file.saveCopy"),
                c("Save as Template…", "file.saveAsTemplate"),
                c("Revert", "file.revert"),
                Sep,
                c("Place…", "file.place"),
                Sep,
                sub(
                    "Export",
                    vec![
                        c("Export for Screens…", "file.exportForScreens"),
                        c("Export As SVG…", "file.export.svg"),
                        c("Export As PNG…", "file.export.png"),
                        todos("Save for Web (Legacy)…", "Cmd+Alt+Shift+S"),
                    ],
                ),
                c("Export Selection…", "document.exportSelection"),
                Sep,
                todos("Package…", "Cmd+Alt+Shift+P"),
                sub("Scripts", vec![todos("Other Script…", "Cmd+F12")]),
                Sep,
                c("Document Setup…", "file.documentSetup"),
                sub(
                    "Document Color Mode",
                    vec![
                        cp("CMYK Color", "object.convertDocumentColorMode", json!({"mode": "cmyk"})),
                        cp("RGB Color", "object.convertDocumentColorMode", json!({"mode": "rgb"})),
                    ],
                ),
                c("File Info…", "file.info"),
                Sep,
                todos("Print…", "Cmd+P"),
            ],
        ),
        (
            "Edit",
            vec![
                c("Undo", "edit.undo"),
                c("Redo", "edit.redo"),
                Sep,
                c("Cut", "edit.cut"),
                c("Copy", "edit.copy"),
                c("Paste", "edit.paste"),
                c("Paste in Front", "edit.pasteInFront"),
                c("Paste in Back", "edit.pasteInBack"),
                c("Paste in Place", "edit.pasteInPlace"),
                c("Paste on All Artboards", "edit.pasteOnAllArtboards"),
                c("Paste without Formatting", "edit.pasteWithoutFormatting"),
                c("Clear", "edit.clear"),
                Sep,
                cp("Find and Replace…", "edit.findReplace", json!({"find": "", "replace": "", "matchCase": false, "wholeWord": false})),
                cp("Find Next", "edit.findNext", json!({"find": ""})),
                sub("Spelling", vec![todo("Auto Spell Check"), todos("Check Spelling…", "Cmd+I"), todo("Edit Custom Dictionary…")]),
                Sep,
                sub(
                    "Edit Colors",
                    vec![
                        c("Recolor Artwork…", "ui.recolorDialog"),
                        c("Adjust Color Balance…", "ui.colorBalanceDialog"),
                        c("Blend Front to Back", "edit.colors.blendFrontToBack"),
                        c("Blend Horizontally", "edit.colors.blendHorizontally"),
                        c("Blend Vertically", "edit.colors.blendVertically"),
                        c("Convert to CMYK", "edit.colors.toCMYK"),
                        c("Convert to Grayscale", "edit.colors.toGrayscale"),
                        c("Convert to RGB", "edit.colors.toRGB"),
                        c("Invert Colors", "edit.colors.invert"),
                        cp(
                            "Overprint Black…",
                            "edit.colors.overprintBlack",
                            json!({"remove": false, "percentage": 100, "fill": true, "stroke": true, "includeCmyBlacks": false, "includeSpotBlacks": false}),
                        ),
                        c("Saturate…", "ui.saturateDialog"),
                    ],
                ),
                todo("Edit Original"),
                Sep,
                todo("Transparency Flattener Presets…"),
                todo("Print Presets…"),
                todo("PDF Presets…"),
                cp("Perspective Grid Presets…", "perspective.grid.preset", json!({"kind": 2})),
                Sep,
                c("Color Settings…", "edit.colorSettings"),
                c("Assign Profile…", "edit.assignProfile"),
                Sep,
                c("Keyboard Shortcuts…", "edit.keyboardShortcuts"),
                c("Preferences…", "edit.preferences"),
            ],
        ),
        (
            "Object",
            vec![
                sub(
                    "Transform",
                    vec![
                        c("Transform Again", "object.transformAgain"),
                        Sep,
                        c("Move…", "object.move"),
                        c("Rotate…", "object.rotate"),
                        c("Reflect…", "object.reflect"),
                        c("Scale…", "object.scale"),
                        c("Shear…", "object.shear"),
                        Sep,
                        cp(
                            "Transform Each…",
                            "object.transformEach",
                            json!({"scaleH": 100, "scaleV": 100, "moveH": 0, "moveV": 0, "rotate": 0, "reflectX": false, "reflectY": false, "random": false, "reference": 4}),
                        ),
                        Sep,
                        c("Reset Bounding Box", "object.resetBoundingBox"),
                    ],
                ),
                sub(
                    "Arrange",
                    vec![
                        c("Bring to Front", "object.arrange.bringToFront"),
                        c("Bring Forward", "object.arrange.bringForward"),
                        c("Send Backward", "object.arrange.sendBackward"),
                        c("Send to Back", "object.arrange.sendToBack"),
                        Sep,
                        c("Send to Current Layer", "object.arrange.sendToCurrentLayer"),
                    ],
                ),
                sub(
                    "Align",
                    vec![
                        cp("Horizontal Align Left", "object.align", json!({"horizontal": "left"})),
                        cp("Horizontal Align Center", "object.align", json!({"horizontal": "center"})),
                        cp("Horizontal Align Right", "object.align", json!({"horizontal": "right"})),
                        cp("Vertical Align Top", "object.align", json!({"vertical": "top"})),
                        cp("Vertical Align Center", "object.align", json!({"vertical": "center"})),
                        cp("Vertical Align Bottom", "object.align", json!({"vertical": "bottom"})),
                    ],
                ),
                Sep,
                c("Group", "object.group"),
                c("Ungroup", "object.ungroup"),
                sub(
                    "Lock",
                    vec![c("Selection", "object.lock"), c("All Artwork Above", "object.lock.above"), c("Other Layers", "object.lock.otherLayers")],
                ),
                c("Unlock All", "object.unlockAll"),
                sub(
                    "Hide",
                    vec![c("Selection", "object.hide"), c("All Artwork Above", "object.hide.above"), c("Other Layers", "object.hide.otherLayers")],
                ),
                c("Show All", "object.showAll"),
                Sep,
                cp("Expand…", "object.expand", json!({"object": true, "fill": true, "stroke": true})),
                c("Expand Appearance", "effect.expandAppearance"),
                c("Crop Image", "object.cropImage"),
                cp("Rasterize…", "object.rasterize", json!({"ppi": 72, "background": "transparent"})),
                cp("Create Gradient Mesh…", "object.mesh.create", json!({"rows": 4, "cols": 4, "appearance": "flat", "highlight": 100})),
                cp(
                    "Create Object Mosaic…",
                    "object.createObjectMosaic",
                    json!({"columns": 10, "rows": 10, "spacingX": 0, "spacingY": 0, "gray": false, "deleteRaster": false}),
                ),
                c("Create Trim Marks", "object.createTrimMarks"),
                todo("Flatten Transparency…"),
                Sep,
                c("Make Pixel Perfect", "object.makePixelPerfect"),
                Sep,
                sub("Slice", vec![todo("Make"), todo("Release"), todo("Create from Guides"), todo("Create from Selection")]),
                Sep,
                sub(
                    "Path",
                    vec![
                        c("Join", "path.join"),
                        c("Average…", "path.average"),
                        Sep,
                        c("Outline Stroke", "object.path.outlineStroke"),
                        c("Offset Path…", "object.path.offsetPath"),
                        c("Reverse Path Direction", "path.reverse"),
                        Sep,
                        c("Simplify…", "object.path.simplify"),
                        c("Add Anchor Points", "object.path.addAnchorPoints"),
                        c("Remove Anchor Points", "path.deleteAnchors"),
                        c("Divide Objects Below", "object.path.divideObjectsBelow"),
                        c("Split Into Grid…", "object.path.splitIntoGrid"),
                        Sep,
                        cp("Clean Up…", "object.path.cleanUp", json!({"strayPoints": true, "unpaintedObjects": true, "emptyTextPaths": true})),
                    ],
                ),
                sub("Shape", vec![c("Convert to Shape", "object.shape.convertToShape"), c("Expand Shape", "object.expandShape")]),
                sub("Pattern", vec![c("Make", "object.pattern.make"), c("Edit Pattern", "object.pattern.edit"), todo("Tile Edge Color…")]),
                sub(
                    "Repeat",
                    vec![
                        c("Radial", "object.repeat.radial"),
                        c("Grid", "object.repeat.grid"),
                        c("Mirror", "object.repeat.mirror"),
                        Sep,
                        c("Release", "object.repeat.release"),
                        c("Options…", "object.repeat.options"),
                    ],
                ),
                sub(
                    "Blend",
                    vec![
                        c("Make", "object.blend.make"),
                        c("Release", "object.blend.release"),
                        Sep,
                        cp("Blend Options…", "object.blend.options", json!({"steps": 5})),
                        Sep,
                        c("Expand", "object.blend.expand"),
                        Sep,
                        c("Replace Spine", "object.blend.replaceSpine"),
                        c("Reverse Spine", "object.blend.reverseSpine"),
                        c("Reverse Front to Back", "object.blend.reverseFrontToBack"),
                    ],
                ),
                sub(
                    "Envelope Distort",
                    vec![
                        cp(
                            "Make with Warp…",
                            "object.envelope.makeWithWarp",
                            json!({"style": "arc", "bend": 50, "h": 0, "v": 0, "horizontal": true}),
                        ),
                        cp("Make with Mesh…", "object.envelope.makeWithMesh", json!({"rows": 4, "cols": 4})),
                        c("Make with Top Object", "object.envelope.makeWithTopObject"),
                        Sep,
                        c("Release", "object.envelope.release"),
                        cp("Envelope Options…", "object.envelope.options", json!({"fidelity": 50})),
                        c("Expand", "object.envelope.expand"),
                        Sep,
                        c("Edit Contents", "object.envelope.editContents"),
                    ],
                ),
                sub("Perspective", vec![c("Attach to Active Plane", "perspective.attach"), c("Release with Perspective", "perspective.release")]),
                sub(
                    "Live Paint",
                    vec![
                        c("Make", "livePaint.make"),
                        c("Merge", "livePaint.merge"),
                        c("Release", "livePaint.release"),
                        Sep,
                        todo("Gap Options…"),
                        Sep,
                        c("Expand", "livePaint.expand"),
                    ],
                ),
                sub(
                    "Image Trace",
                    vec![
                        cp("Make…", "imageTrace.make", json!({"preset": "Black and White Logo"})),
                        cp("Make and Expand…", "imageTrace.makeAndExpand", json!({"preset": "6 Colors"})),
                        c("Release", "imageTrace.release"),
                        c("Expand", "imageTrace.expand"),
                    ],
                ),
                sub(
                    "Text Wrap",
                    vec![
                        c("Make", "object.textWrap.make"),
                        c("Release", "object.textWrap.release"),
                        c("Text Wrap Options…", "object.textWrap.options"),
                    ],
                ),
                Sep,
                sub(
                    "Clipping Mask",
                    vec![
                        c("Make", "object.clippingMask.make"),
                        c("Release", "object.clippingMask.release"),
                        c("Edit Contents", "object.clippingMask.editContents"),
                        c("Edit Clipping Path", "object.clippingMask.editMask"),
                    ],
                ),
                sub("Compound Path", vec![c("Make", "object.compoundPath.make"), c("Release", "object.compoundPath.release")]),
                sub(
                    "Artboards",
                    vec![
                        c("Convert to Artboards", "artboard.convertToArtboards"),
                        cp("Rearrange All Artboards…", "artboard.rearrange", json!({"columns": 2, "spacing": 20, "moveArtwork": true})),
                        Sep,
                        c("Fit to Artwork Bounds", "artboard.fitToArt"),
                        c("Fit to Selected Art", "artboard.fitToSelection"),
                    ],
                ),
                sub("Graph", vec![c("Type…", "graph.setType"), c("Data…", "graph.setData"), todo("Design…"), todo("Column…"), todo("Marker…")]),
            ],
        ),
        (
            "Type",
            vec![
                sub("Font", font_items()),
                sub("Recent Fonts", RECENT_FONT_IDS.iter().map(|id| c("Recent Font", id)).collect()),
                sub("Size", TYPE_SIZES.iter().map(|(l, n)| cp(l, "text.setStyle", json!({ "size": n }))).collect()),
                Sep,
                panel("Glyphs", "glyphs"),
                sub("Insert Special Character", insert_items(INSERT_SPECIAL)),
                sub("Insert Whitespace Character", insert_items(INSERT_WHITESPACE)),
                sub("Insert Break Character", insert_items(INSERT_BREAK)),
                Sep,
                c("Convert To Area Type", "type.convertToAreaType"),
                c("Convert To Point Type", "type.convertToPointType"),
                c("Area Type Options…", "text.areaOptions"),
                sub(
                    "Type on a Path",
                    vec![
                        cp("Rainbow", "type.pathOptions", json!({"effect": "rainbow"})),
                        cp("Skew", "type.pathOptions", json!({"effect": "skew"})),
                        cp("3D Ribbon", "type.pathOptions", json!({"effect": "3dRibbon"})),
                        cp("Stair Step", "type.pathOptions", json!({"effect": "stairStep"})),
                        cp("Gravity", "type.pathOptions", json!({"effect": "gravity"})),
                        Sep,
                        cp("Type on a Path Options…", "type.pathOptions", json!({"start": 0})),
                    ],
                ),
                sub(
                    "Threaded Text",
                    vec![
                        c("Create", "text.thread.create"),
                        c("Release Selection", "text.thread.releaseSelection"),
                        c("Remove Threading", "text.thread.remove"),
                    ],
                ),
                c("Fit Headline", "text.fitHeadline"),
                Sep,
                c("Find Font…", "type.findFont"),
                sub(
                    "Change Case",
                    vec![
                        cp("UPPERCASE", "type.changeCase", json!({"case": "upper"})),
                        cp("lowercase", "type.changeCase", json!({"case": "lower"})),
                        cp("Title Case", "type.changeCase", json!({"case": "title"})),
                        cp("Sentence case", "type.changeCase", json!({"case": "sentence"})),
                    ],
                ),
                cp("Smart Punctuation…", "type.smartPunctuation", json!({"quotes": true, "dashes": true, "ellipsis": true, "scope": "selection"})),
                Sep,
                c("Create Outlines", "type.createOutlines"),
                todo("Optical Margin Alignment"),
                Sep,
                c("Fill with Placeholder Text", "type.fillPlaceholder"),
                Sep,
                c("Show Hidden Characters", "type.hiddenCharacters"),
                sub("Type Orientation", vec![todo("Horizontal"), todo("Vertical")]),
            ],
        ),
        (
            "Select",
            vec![
                c("All", "select.all"),
                c("All on Active Artboard", "select.allOnArtboard"),
                c("Deselect", "select.none"),
                c("Reselect", "select.reselect"),
                Sep,
                c("Inverse", "select.inverse"),
                Sep,
                c("Next Object Above", "select.nextAbove"),
                c("Next Object Below", "select.nextBelow"),
                Sep,
                sub(
                    "Same",
                    vec![
                        c("Appearance", "select.same.appearance"),
                        c("Appearance Attribute", "select.same.appearanceAttribute"),
                        c("Blending Mode", "select.same.blendingMode"),
                        c("Fill & Stroke", "select.same.fillAndStroke"),
                        c("Fill Color", "select.same.fillColor"),
                        c("Opacity", "select.same.opacity"),
                        c("Stroke Color", "select.same.strokeColor"),
                        c("Stroke Weight", "select.same.strokeWeight"),
                        c("Graphic Style", "select.same.graphicStyle"),
                        c("Shape", "select.same.shapeType"),
                        c("Symbol Instance", "select.same.symbolInstance"),
                        Sep,
                        c("Font Family", "select.same.fontFamily"),
                        c("Font Family & Style", "select.same.fontFamilyStyle"),
                        c("Font Family, Style & Size", "select.same.fontFamilyStyleSize"),
                        c("Font Size", "select.same.fontSize"),
                        c("Text Fill Color", "select.same.textFillColor"),
                        c("Text Stroke Color", "select.same.textStrokeColor"),
                    ],
                ),
                sub(
                    "Object",
                    vec![
                        c("All on Same Layers", "select.object.allOnSameLayers"),
                        c("Direction Handles", "select.object.directionHandles"),
                        Sep,
                        c("Brush Strokes", "select.object.brushStrokes"),
                        c("Bristle Brush Strokes", "select.object.bristleBrushStrokes"),
                        c("Clipping Masks", "select.object.clippingMasks"),
                        c("Stray Points", "select.object.strayPoints"),
                        c("Open Paths", "select.object.openPaths"),
                        Sep,
                        c("All Text Objects", "select.object.textObjects"),
                        c("Point Text Objects", "select.object.pointText"),
                        c("Area Text Objects", "select.object.areaText"),
                    ],
                ),
                todo("Start Global Edit"),
                Sep,
                c("Save Selection…", "select.save"),
                todo("Edit Selection…"),
            ],
        ),
        ("Effect", effect_menu()),
        (
            "View",
            vec![
                c("Outline", "view.outline"),
                c("Overprint Preview", "view.overprintPreview"),
                c("Pixel Preview", "view.pixelPreview"),
                c("Trim View", "view.trimView"),
                c("Presentation Mode", "view.presentation"),
                sub(
                    "Screen Mode",
                    vec![
                        cp("Normal Screen Mode", "view.screenMode", json!({"mode": 0})),
                        cp("Full Screen Mode with Menu Bar", "view.screenMode", json!({"mode": 1})),
                        cp("Full Screen Mode", "view.screenMode", json!({"mode": 2})),
                    ],
                ),
                Sep,
                sub(
                    "Proof Setup",
                    vec![
                        cp("Working CMYK", "view.proofSetup", json!({"target": "workingCmyk", "proof": true})),
                        cp("Legacy Macintosh RGB (Gamma 1.8)", "view.proofSetup", json!({"target": "legacyMacRgb", "proof": true})),
                        cp("Internet Standard RGB (sRGB)", "view.proofSetup", json!({"target": "srgb", "proof": true})),
                        cp("Monitor RGB", "view.proofSetup", json!({"target": "monitorRgb", "proof": true})),
                        cp("Color blindness – Protanopia-type", "view.proofSetup", json!({"target": "protanopia", "proof": true})),
                        cp("Color blindness – Deuteranopia-type", "view.proofSetup", json!({"target": "deuteranopia", "proof": true})),
                        cp("Customize…", "window.panel", json!({"panel": "separations"})),
                    ],
                ),
                c("Proof Colors", "view.proofColors"),
                Sep,
                c("Zoom In", "view.zoomIn"),
                c("Zoom Out", "view.zoomOut"),
                c("Fit Artboard in Window", "view.fitArtboard"),
                c("Fit All in Window", "view.fitAll"),
                c("Actual Size", "view.actualSize"),
                Sep,
                c("Reset Rotate View", "view.rotateReset"),
                Sep,
                c("Hide Edges", "view.edges"),
                c("Hide Artboards", "view.artboards"),
                todo("Show Print Tiling"),
                Sep,
                sub("Rulers", vec![c("Show Rulers", "view.rulers"), todos("Change to Global Rulers", "Cmd+Alt+R"), todo("Show Video Rulers")]),
                c("Hide Bounding Box", "view.boundingBox"),
                c("Show Transparency Grid", "view.transparencyGrid"),
                c("Hide Text Threads", "view.textThreads"),
                c("Hide Gradient Annotator", "view.gradientAnnotator"),
                c("Hide Corner Widget", "view.cornerWidget"),
                Sep,
                sub(
                    "Guides",
                    vec![
                        c("Hide Guides", "view.guides"),
                        c("Lock Guides", "view.guides.lock"),
                        c("Make Guides", "view.guides.make"),
                        c("Release Guides", "view.guides.release"),
                        c("Clear Guides", "view.guides.clear"),
                    ],
                ),
                c("Smart Guides", "view.smartGuides"),
                sub(
                    "Perspective Grid",
                    vec![
                        c("Show Grid", "perspective.grid.show"),
                        Sep,
                        cp("One Point Perspective", "perspective.grid.preset", json!({"kind": 1})),
                        cp("Two Point Perspective", "perspective.grid.preset", json!({"kind": 2})),
                        cp("Three Point Perspective", "perspective.grid.preset", json!({"kind": 3})),
                        cp("Define Grid…", "perspective.grid.set", json!({"kind": 2, "cell": 20, "distance": 300})),
                    ],
                ),
                c("Show Grid", "view.grid"),
                c("Snap to Grid", "view.snapToGrid"),
                c("Snap to Pixel", "view.snapToPixel"),
                c("Snap to Point", "view.snapToPoint"),
                todo("Snap to Glyph"),
                Sep,
                c("New View…", "view.saved.new"),
                c("Edit Views…", "view.saved.edit"),
                Sep,
                c("Saved View 1", "view.goto1"),
                c("Saved View 2", "view.goto2"),
                c("Saved View 3", "view.goto3"),
                c("Saved View 4", "view.goto4"),
                c("Saved View 5", "view.goto5"),
                c("Saved View 6", "view.goto6"),
                c("Saved View 7", "view.goto7"),
                c("Saved View 8", "view.goto8"),
                c("Saved View 9", "view.goto9"),
                c("Saved View 10", "view.goto10"),
            ],
        ),
        (
            "Window",
            vec![
                c("New Window", "window.newWindow"),
                sub("Arrange", vec![todo("Cascade"), todo("Tile"), todo("Float in Window"), todo("Consolidate All Windows")]),
                sub("Workspace", crate::workspaces::menu_items()),
                Sep,
                c("Control", "window.control"),
                c("Contextual Task Bar", "window.taskBar"),
                c("Tools", "window.toolbar"),
                sub("Toolbars", vec![c("Advanced", "window.toolbarAdvanced"), c("Single / Double Column", "window.toolbarColumns")]),
                Sep,
                panel("Actions", "actions"),
                panel("Align", "align"),
                panel("Appearance", "appearance"),
                panel("Artboards", "artboards"),
                todo("Asset Export"),
                todo("Attributes"),
                panel("Brushes", "brushes"),
                panel("Color", "color"),
                panel("Color Guide", "colorGuide"),
                panel("Document Info", "docInfo"),
                panel("Gradient", "gradient"),
                panel("Graphic Styles", "graphicStyles"),
                panel("History", "history"),
                panel("Image Trace", "imageTrace"),
                panel("Info", "info"),
                panel("Layers", "layers"),
                panel("Libraries", "libraries"),
                todo("Links"),
                panel("Magic Wand", "magicWand"),
                panel("Navigator", "navigator"),
                panel("Pathfinder", "pathfinder"),
                panel("Pattern Options", "patternOptions"),
                panel("Properties", "properties"),
                panel("Separations Preview", "separations"),
                panel("Stroke", "stroke"),
                todo("SVG Interactivity"),
                panel("Swatches", "swatches"),
                panel("Symbols", "symbols"),
                panel("Transform", "transform"),
                panel("Transparency", "transparency"),
                sub(
                    "Type",
                    vec![
                        panel("Character", "character"),
                        panel("Character Styles", "charStyles"),
                        panel("Glyphs", "glyphs"),
                        panel("OpenType", "openType"),
                        panel("Paragraph", "paragraph"),
                        panel("Paragraph Styles", "paraStyles"),
                        panel("Tabs", "tabs"),
                    ],
                ),
                todo("Variables"),
                Sep,
                sub("Brush Libraries", library_placeholders()),
                sub("Graphic Style Libraries", library_placeholders()),
                sub("Swatch Libraries", crate::panels::swatches::window_menu()),
                sub("Symbol Libraries", library_placeholders()),
            ],
        ),
        (
            "Help",
            vec![
                c("Join Our Discord", "help.discord"),
                c("ArtCraft Website", "help.website"),
                c("VectorCraft on getartcraft.com", "help.appPage"),
                c("VectorCraft on GitHub", "help.github"),
                Sep,
                c("Search Commands…", "help.commandPalette"),
                todos("VectorCraft Help…", "F1"),
                Sep,
                c("About VectorCraft", "help.about"),
            ],
        ),
    ]
}

/// Render the menu bar.
pub fn menu_bar(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let mut clicked: Option<(String, Value)> = None;
    let tree = menu_tree();
    egui::MenuBar::new().ui(ui, |ui| {
        for (i, (title, items)) in tree.iter().enumerate() {
            let text = if i == 0 {
                egui::RichText::new(*title).font(theme::semibold(13.0)).color(t.text)
            } else {
                egui::RichText::new(*title).size(13.0).color(t.text)
            };
            ui.menu_button(text, |ui| menu_body(app, ui, items, &mut clicked));
        }
    });
    if let Some((id, p)) = clicked {
        invoke(app, &id, p);
    }
}

/// A top-level menu's popup: as wide as its widest item (label plus shortcut), at least 230 pt.
fn menu_body(app: &VectorcraftApp, ui: &mut egui::Ui, items: &[Item], clicked: &mut Option<(String, Value)>) {
    ui.set_min_width(230.0);
    render_items(app, ui, items, clicked);
}

fn render_items(app: &VectorcraftApp, ui: &mut egui::Ui, items: &[Item], clicked: &mut Option<(String, Value)>) {
    let t = Tokens::get(ui.ctx());
    for it in items {
        match it {
            Item::Sep => {
                ui.separator();
            }
            Item::Header(h) => {
                ui.label(egui::RichText::new(*h).size(11.0).color(t.text_dim));
            }
            Item::Sub(label, children) => {
                ui.menu_button(*label, |ui| {
                    ui.set_min_width(200.0);
                    render_items(app, ui, children, clicked);
                });
            }
            Item::Todo(label, sc) => {
                ui.add_enabled_ui(false, |ui| {
                    ui.add(egui::Button::new(*label).shortcut_text(pretty_shortcut(sc)));
                })
                .response
                .on_disabled_hover_text("Coming soon — tracked in the parity plan");
            }
            Item::Cmd(label, id, p) => {
                let en = enabled(app, id);
                // Unused saved-view slots are hidden (Illustrator lists only the saved views).
                if !en && hidden_when_disabled(id) {
                    continue;
                }
                let label = dynamic_label(app, id, label);
                let sc = if p.is_null() { shortcut_of(id).map(pretty_shortcut).unwrap_or_default() } else { String::new() };
                let chk = checked(app, id, p);
                let text = match chk {
                    Some(true) => format!("✓  {label}"),
                    Some(false) => format!("     {label}"),
                    None => label,
                };
                let r = ui.add_enabled(en, egui::Button::new(text).shortcut_text(sc));
                if r.clicked() {
                    *clicked = Some(click_target(label_of(it), id, p));
                    ui.close();
                }
            }
        }
    }
}

fn label_of(it: &Item) -> &'static str {
    match it {
        Item::Cmd(l, _, _) => l,
        _ => "",
    }
}

/// What a menu click runs: items whose label ends with "…" and that carry default params open a
/// generic parameter dialog (fields = the defaults) instead of running immediately.
pub fn click_target(label: &str, id: &str, p: &Value) -> (String, Value) {
    let dialog = label.ends_with('…')
        && p.as_object().is_some_and(|o| !o.is_empty())
        && !matches!(id, "effect.dialog" | "window.panel" | "window.brightness" | "view.screenMode");
    if dialog {
        return ("ui.paramDialog".into(), json!({"command": id, "label": label.trim_end_matches('…'), "params": p}));
    }
    (id.to_string(), if p.is_null() { json!({}) } else { p.clone() })
}

/// Invoke a menu/command id with UI side effects (dialogs for "…" commands that need input).
pub fn invoke(app: &mut VectorcraftApp, id: &str, p: Value) {
    // Commands whose menu item opens a dialog in Illustrator.
    let dialog = match id {
        "object.move" => Some(("move", json!({"dx": "0 pt", "dy": "0 pt"}))),
        "object.rotate" => Some(("rotate", json!({"angle": 0}))),
        "object.scale" => Some(("scale", json!({"sx": 100, "sy": 100, "uniform": true}))),
        "object.reflect" => Some(("reflect", json!({"axis": "vertical"}))),
        "object.shear" => Some(("shear", json!({"angle": 0, "axis": "horizontal"}))),
        "path.average" => Some(("average", json!({"axis": "both"}))),
        "object.path.offsetPath" => Some(("offsetPath", json!({"offset": "10 pt", "joins": "miter", "miterLimit": 4}))),
        "object.path.simplify" => Some(("simplify", json!({"tolerance": "1 pt"}))),
        "object.path.splitIntoGrid" => Some(("splitIntoGrid", json!({"rows": 2, "columns": 2, "gutter": "12 pt"}))),
        _ => None,
    };
    if let Some((kind, fields)) = dialog
        && p.as_object().is_none_or(|o| o.is_empty())
    {
        app.ui.dialog = Some(crate::state::Dialog::new(kind, fields));
        return;
    }
    // Repeat Options: a dialog with the selected repeat's current values.
    if id == "object.repeat.options"
        && p.as_object().is_none_or(|o| o.is_empty())
        && let Some(fields) = crate::panels::pattern_options::repeat_fields(app)
    {
        let _ = app.run("ui.paramDialog", json!({"command": id, "label": "Repeat Options", "params": fields}));
        return;
    }
    // Area Type Options: a dialog with the selected area type's current values.
    if id == "text.areaOptions" && p.as_object().is_none_or(|o| o.is_empty()) {
        match app.session.execute(id, &json!({})) {
            Ok(fields) => {
                let _ = app.run("ui.paramDialog", json!({"command": id, "label": "Area Type Options", "params": fields}));
            }
            Err(e) => app.status(e.to_string()),
        }
        return;
    }
    // Object → Graph → Type… / Data…: dialogs with the selected graph's current values.
    if matches!(id, "graph.setType" | "graph.setData") && p.as_object().is_none_or(|o| o.is_empty()) {
        match app.session.execute(id, &json!({})) {
            Ok(v) => {
                let (label, fields) = if id == "graph.setData" { ("Graph Data", json!({"csv": v["csv"]})) } else { ("Graph Type", v) };
                let _ = app.run("ui.paramDialog", json!({"command": id, "label": label, "params": fields}));
            }
            Err(e) => app.status(e.to_string()),
        }
        return;
    }
    // New View… / Edit Views…: name dialogs.
    if id == "view.saved.new" && p.as_object().is_none_or(|o| o.is_empty()) {
        let n = app.session.active().map_or(0, |d| d.doc.views.len()) + 1;
        let _ = app.run("ui.paramDialog", json!({"command": id, "label": "New View", "params": {"name": format!("View {n}")}}));
        return;
    }
    if id == "view.saved.edit" && p.as_object().is_none_or(|o| o.is_empty()) {
        let first = app.session.active().and_then(|d| d.doc.views.first().map(|v| v.name.clone()));
        match first {
            Some(name) => {
                let _ = app
                    .run("ui.paramDialog", json!({"command": id, "label": "Edit Views", "params": {"name": name, "newName": "", "delete": false}}));
            }
            None => app.status("No saved views (View → New View…)"),
        }
        return;
    }
    // Document Raster Effects Settings: a dialog with the current resolution.
    if id == "document.rasterEffectsSettings" && p.as_object().is_none_or(|o| o.is_empty()) {
        match app.session.execute(id, &json!({})) {
            Ok(v) => {
                let _ = app.run("ui.paramDialog", json!({"command": id, "label": "Document Raster Effects Settings", "params": v}));
            }
            Err(e) => app.status(e.to_string()),
        }
        return;
    }
    // Text Wrap Options: a dialog with the selected wrap object's current values.
    if id == "object.textWrap.options" && p.as_object().is_none_or(|o| o.is_empty()) {
        match app.session.execute(id, &json!({})) {
            Ok(fields) => {
                let _ = app.run("ui.paramDialog", json!({"command": id, "label": "Text Wrap Options", "params": fields}));
            }
            Err(e) => app.status(e.to_string()),
        }
        return;
    }
    // Help links: the command returns the URL; a menu click opens it (agents just get the URL).
    if matches!(id, "help.discord" | "help.website" | "help.appPage" | "help.github") {
        app.open_link(id);
        return;
    }
    if let Err(e) = app.run(id, p) {
        app.status(e);
    } else if matches!(id, "object.pattern.make" | "object.pattern.edit") {
        app.ui.open_panel = Some("patternOptions".into());
    }
}

/// `Cmd+Shift+]` → `⇧⌘]` on macOS, `Ctrl+Shift+]` elsewhere.
pub fn pretty_shortcut(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    if cfg!(target_os = "macos") {
        let mut mods = String::new();
        let parts: Vec<&str> = s.split('+').collect();
        let (key, ms) = if s.ends_with("++") { ("+", &parts[..parts.len() - 2]) } else { (*parts.last().unwrap(), &parts[..parts.len() - 1]) };
        for m in ["Ctrl", "Alt", "Shift", "Cmd"] {
            if ms.contains(&m) {
                mods.push_str(match m {
                    "Ctrl" => "⌃",
                    "Alt" => "⌥",
                    "Shift" => "⇧",
                    _ => "⌘",
                });
            }
        }
        format!("{mods}{key}")
    } else {
        s.replace("Cmd", "Ctrl")
    }
}

#[derive(Serialize)]
pub struct MenuEntry {
    pub path: Vec<String>,
    pub label: String,
    pub command: Option<String>,
    pub params: Value,
    pub enabled: bool,
    pub shortcut: String,
}

/// Flattened menu for `ui.menu.list`.
pub fn menu_entries(app: &VectorcraftApp) -> Vec<MenuEntry> {
    fn walk(app: &VectorcraftApp, path: Vec<String>, items: &[Item], out: &mut Vec<MenuEntry>) {
        for it in items {
            match it {
                Item::Cmd(_, id, _) if hidden_when_disabled(id) && !enabled(app, id) => {}
                Item::Cmd(l, id, p) => out.push(MenuEntry {
                    path: path.clone(),
                    label: dynamic_label(app, id, l),
                    command: Some(id.to_string()),
                    params: p.clone(),
                    enabled: enabled(app, id),
                    shortcut: shortcut_of(id).unwrap_or("").to_string(),
                }),
                Item::Todo(l, sc) => out.push(MenuEntry {
                    path: path.clone(),
                    label: l.to_string(),
                    command: None,
                    params: Value::Null,
                    enabled: false,
                    shortcut: sc.to_string(),
                }),
                Item::Sub(l, ch) => {
                    let mut p = path.clone();
                    p.push(l.to_string());
                    walk(app, p, ch, out);
                }
                _ => {}
            }
        }
    }
    let mut out = vec![];
    for (title, items) in menu_tree() {
        walk(app, vec![title.to_string()], &items, &mut out);
    }
    out
}

/// Type → Size presets.
const TYPE_SIZES: [(&str, u32); 14] = [
    ("6 pt", 6),
    ("8 pt", 8),
    ("9 pt", 9),
    ("10 pt", 10),
    ("11 pt", 11),
    ("12 pt", 12),
    ("14 pt", 14),
    ("18 pt", 18),
    ("24 pt", 24),
    ("30 pt", 30),
    ("36 pt", 36),
    ("48 pt", 48),
    ("60 pt", 60),
    ("72 pt", 72),
];

const INSERT_SPECIAL: &[(&str, &str)] = &[
    ("Bullet", "bullet"),
    ("Copyright Symbol", "copyright"),
    ("Ellipsis", "ellipsis"),
    ("Paragraph Symbol", "paragraph"),
    ("Registered Trademark Symbol", "registered"),
    ("Section Symbol", "section"),
    ("Trademark Symbol", "trademark"),
    ("Em Dash", "emDash"),
    ("En Dash", "enDash"),
    ("Discretionary Hyphen", "discretionaryHyphen"),
    ("Nonbreaking Hyphen", "nonBreakingHyphen"),
    ("Double Left Quotation Marks", "doubleLeftQuote"),
    ("Double Right Quotation Marks", "doubleRightQuote"),
    ("Single Left Quotation Mark", "singleLeftQuote"),
    ("Single Right Quotation Mark", "singleRightQuote"),
];
const INSERT_WHITESPACE: &[(&str, &str)] = &[
    ("Em Space", "emSpace"),
    ("En Space", "enSpace"),
    ("Hair Space", "hairSpace"),
    ("Sixth Space", "sixthSpace"),
    ("Thin Space", "thinSpace"),
    ("Nonbreaking Space", "nonBreakingSpace"),
    ("Figure Space", "figureSpace"),
    ("Punctuation Space", "punctuationSpace"),
    ("Third Space", "thirdSpace"),
    ("Quarter Space", "quarterSpace"),
];
const INSERT_BREAK: &[(&str, &str)] = &[("Forced Line Break", "forcedLineBreak"), ("Tab", "tab")];

fn insert_items(list: &[(&'static str, &'static str)]) -> Vec<Item> {
    list.iter().map(|(l, c)| cp(l, "type.insert", json!({ "char": c }))).collect()
}

/// Type → Font: one item per installed family (labels interned once so the per-frame menu build
/// doesn't allocate forever).
fn font_items() -> Vec<Item> {
    static NAMES: std::sync::Mutex<Vec<&'static str>> = std::sync::Mutex::new(Vec::new());
    let fams = vectorcraft_text::FontDb::global().families();
    let Ok(mut names) = NAMES.lock() else { return vec![] };
    fams.iter()
        .map(|f| {
            let label: &'static str = match names.iter().find(|n| **n == f.as_str()) {
                Some(n) => n,
                None => {
                    let n: &'static str = Box::leak(f.clone().into_boxed_str());
                    names.push(n);
                    n
                }
            };
            cp(label, "text.setStyle", json!({ "font": label }))
        })
        .collect()
}

/// The Effect menu, built from the effects catalogue (vector effects), plus raster effects.
fn effect_menu() -> Vec<Item> {
    let cat = vectorcraft_effects::effect_catalog();
    let mut out = vec![
        c("Apply Last Effect", "effect.applyLast"),
        c("Last Effect…", "effect.last"),
        Sep,
        c("Document Raster Effects Settings…", "document.rasterEffectsSettings"),
        Sep,
        Item::Header("Vector Effects"),
    ];
    // Submenus in Illustrator's order.
    let order = ["3D and Materials", "Convert to Shape", "Distort & Transform", "Path", "Pathfinder", "Stylize", "SVG Filters", "Warp", "Blur"];
    for sub_name in order {
        let items: Vec<Item> = cat
            .iter()
            .filter(|e| e.menu.last().copied() == Some(sub_name))
            .map(|e| match e.defaults.as_object().is_some_and(|o| o.is_empty()) {
                // No options (Effect → Pathfinder): apply directly, like Illustrator.
                true => Item::Cmd(e.label, "effect.apply", json!({ "effect": e.id })),
                false => Item::Cmd(e.label, "effect.dialog", json!({ "effect": e.id })),
            })
            .collect();
        if sub_name == "Blur" {
            if !items.is_empty() {
                out.push(Sep);
                out.push(Item::Header("Raster Effects"));
                out.push(sub(sub_name, items));
            }
            continue;
        }
        if items.is_empty() {
            let placeholder = match sub_name {
                "3D and Materials" => vec![todo("Extrude & Bevel…"), todo("Revolve…"), todo("Inflate…"), todo("Rotate…"), todo("Materials…")],
                "SVG Filters" => vec![todo("Apply SVG Filter…")],
                _ => continue,
            };
            out.push(sub(sub_name, placeholder));
        } else {
            out.push(sub(sub_name, items));
        }
    }
    // Anything not placed above (future effects) still shows up.
    for e in cat.iter().filter(|e| !e.menu.last().is_some_and(|m| order.contains(m))) {
        out.push(Item::Cmd(e.label, "effect.dialog", json!({ "effect": e.id })));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_effect_dialog_and_view_toggles() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        assert!(!enabled(&app, "effect.last"));
        app.last_effect = Some(("distort.roughen".into(), json!({"size": 9})));
        assert!(enabled(&app, "effect.last"));
        assert_eq!(dynamic_label(&app, "effect.last", "Last Effect…"), "Last Effect: Roughen…");
        app.run("effect.last", json!({})).unwrap();
        let d = app.ui.dialog.as_ref().unwrap();
        assert_eq!((d.kind.as_str(), d.str("__effect"), d.f64("size", 0.0)), ("effect", "distort.roughen".to_string(), 9.0));
        for (id, on, off) in [
            ("view.textThreads", "Hide Text Threads", "Show Text Threads"),
            ("view.gradientAnnotator", "Hide Gradient Annotator", "Show Gradient Annotator"),
            ("type.hiddenCharacters", "Show Hidden Characters", "Hide Hidden Characters"),
        ] {
            assert_eq!(dynamic_label(&app, id, ""), on);
            app.run(id, json!({})).unwrap();
            assert_eq!(dynamic_label(&app, id, ""), off);
        }
    }

    #[test]
    fn saved_views_store_and_restore_the_view() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        {
            let v = app.view_mut().unwrap();
            v.zoom = 3.0;
            v.center = vectorcraft_geom::Point::new(120.0, 80.0);
        }
        app.run("view.saved.new", json!({"name": "Detail"})).unwrap();
        assert!(enabled(&app, "view.goto1") && !enabled(&app, "view.goto2"));
        assert_eq!(dynamic_label(&app, "view.goto1", ""), "Detail");
        app.view_mut().unwrap().zoom = 0.5;
        app.run("view.goto1", json!({})).unwrap();
        let v = app.view().unwrap();
        assert_eq!((v.zoom, v.center.x, v.center.y), (3.0, 120.0, 80.0));
        // Unused slots stay out of the menu listing agents see.
        assert!(!menu_entries(&app).iter().any(|e| e.command.as_deref() == Some("view.goto2")));
    }

    #[test]
    fn recent_fonts_and_corner_widget_toggle() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let fams = vectorcraft_text::FontDb::global().families();
        let (a, b) = (fams[0].clone(), fams[fams.len() - 1].clone());
        app.run("text.create", json!({"x": 10, "y": 10, "text": "Hi"})).unwrap();
        app.run("text.setStyle", json!({"font": a})).unwrap();
        app.run("text.setStyle", json!({"font": b})).unwrap();
        assert_eq!(app.ui.recent_fonts, vec![b.clone(), a.clone()]);
        assert_eq!(dynamic_label(&app, "type.recentFont2", "Recent Font"), a);
        assert!(enabled(&app, "type.recentFont2") && !enabled(&app, "type.recentFont3"));
        app.run("type.recentFont2", json!({})).unwrap();
        assert_eq!(app.ui.recent_fonts[0], a);
        assert_eq!(dynamic_label(&app, "view.cornerWidget", ""), "Hide Corner Widget");
        app.run("view.cornerWidget", json!({})).unwrap();
        assert_eq!(dynamic_label(&app, "view.cornerWidget", ""), "Show Corner Widget");
    }

    #[test]
    fn recent_files_track_opens_and_saves() {
        let dir = std::env::temp_dir().join(format!("dc-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = VectorcraftApp::new(
            vectorcraft_engine::Session::new(),
            crate::Services {
                read: Some(Box::new(|p: &str| std::fs::read(p).map_err(|e| e.to_string()))),
                write: Some(Box::new(|p: &str, b: &[u8]| std::fs::write(p, b).map_err(|e| e.to_string()))),
                ..Default::default()
            },
        );
        app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
        let (a, b) = (dir.join("a.vectorcraft"), dir.join("b.vectorcraft"));
        for p in [&a, &b, &a] {
            app.run("file.saveAs", json!({"path": p.to_string_lossy()})).unwrap();
        }
        assert_eq!(app.ui.recent_files, [a.to_string_lossy(), b.to_string_lossy()]);
        assert_eq!(dynamic_label(&app, "file.openRecent2", ""), "b.vectorcraft");
        assert!(enabled(&app, "file.openRecent2") && !enabled(&app, "file.openRecent3"));
        app.run("file.openRecent2", json!({})).unwrap();
        assert_eq!(app.session.documents().len(), 2);
        assert_eq!(app.ui.recent_files[0], b.to_string_lossy(), "reopening moves it to the top");
        app.run("file.clearRecent", json!({})).unwrap();
        assert!(app.ui.recent_files.is_empty() && app.run("file.openRecent1", json!({})).is_err());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn every_bound_menu_command_exists() {
        fn walk(items: &[Item], bad: &mut Vec<String>) {
            for it in items {
                match it {
                    Item::Cmd(_, id, _) => {
                        if vectorcraft_engine::find_command(id).is_none()
                            && !UI_COMMANDS.iter().any(|c| c.0 == *id)
                            && !id.starts_with("object.path.")
                            && *id != "type.createOutlines"
                        {
                            bad.push(id.to_string());
                        }
                    }
                    Item::Sub(_, ch) => walk(ch, bad),
                    _ => {}
                }
            }
        }
        let mut bad = vec![];
        for (_, items) in menu_tree() {
            walk(&items, &mut bad);
        }
        assert!(bad.is_empty(), "menu items bound to unknown commands: {bad:?}");
    }

    #[test]
    fn shortcut_pretty() {
        if cfg!(target_os = "macos") {
            assert_eq!(pretty_shortcut("Cmd+Shift+]"), "⇧⌘]");
            assert_eq!(pretty_shortcut("Cmd+Alt+2"), "⌥⌘2");
        }
        assert_eq!(pretty_shortcut(""), "");
    }

    #[test]
    fn expand_appearance_is_an_object_menu_item_only() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        app.run("file.new", json!({})).unwrap();
        let paths: Vec<Vec<String>> =
            menu_entries(&app).into_iter().filter(|e| e.command.as_deref() == Some("effect.expandAppearance")).map(|e| e.path).collect();
        assert_eq!(paths, [vec!["Object".to_string()]]);
    }

    #[test]
    fn menus_are_as_wide_as_their_items() {
        // Items size the popup to the widest label and shortcut instead of the widest a menu may be.
        let app = VectorcraftApp::new(vectorcraft_engine::Session::new(), crate::Services::default());
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1600.0, 1000.0));
        let tree = menu_tree();
        let (_, file) = tree.iter().find(|(t, _)| *t == "File").unwrap();
        let id = egui::Id::new("test-menu");
        for _ in 0..3 {
            let input = egui::RawInput { screen_rect: Some(screen), ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                egui::Area::new(id).show(ui.ctx(), |ui| {
                    egui::containers::menu::menu_style(ui.style_mut());
                    ui.with_layout(egui::Layout::top_down_justified(egui::Align::Min), |ui| menu_body(&app, ui, file, &mut None));
                });
            });
            out.textures_delta.clear();
        }
        let w = ctx.memory(|m| m.area_rect(id)).unwrap().width();
        assert!((230.0..340.0).contains(&w), "File menu is {w} pt wide");
    }
}
