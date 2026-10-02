//! Modal dialogs. Fields live in `UiState::dialog` (string/number JSON) so agents can fill them
//! through `ui.dialog.set` and press OK with `ui.dialog.confirm`.

use serde_json::{Value, json};

use crate::state::Dialog;
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, widgets};

/// A click with a shape tool opens its size dialog.
pub fn open_tool_dialog(app: &mut VectorcraftApp, kind: &str, p: Value) {
    let x = p.get("x").and_then(Value::as_f64).unwrap_or(0.0);
    let y = p.get("y").and_then(Value::as_f64).unwrap_or(0.0);
    let d = match kind {
        "rectangle" | "ellipse" => Dialog::new(kind, json!({"x": x, "y": y, "width": "100 pt", "height": "100 pt"})),
        "roundedRectangle" => Dialog::new(kind, json!({"x": x, "y": y, "width": "100 pt", "height": "100 pt", "radius": "12 pt"})),
        "polygon" => Dialog::new(kind, json!({"x": x, "y": y, "radius": "50 pt", "sides": 6})),
        "star" => Dialog::new(kind, json!({"x": x, "y": y, "radius1": "50 pt", "radius2": "25 pt", "points": 5})),
        "lineSegment" => Dialog::new(kind, json!({"x": x, "y": y, "length": "100 pt", "angle": 0})),
        // Graph tool click: the graph's size.
        "graph" => Dialog::new(
            "command",
            json!({"__command": "graph.create", "__label": "Graph", "type": p.get("type").cloned().unwrap_or(json!("column")), "x": x, "y": y, "width": 200, "height": 150}),
        ),
        // After drawing a graph: Graph Data (CSV: header row of series, then category, values…).
        "graphData" => match app.session.execute("graph.setData", &json!({})) {
            Ok(v) => Dialog::new("command", json!({"__command": "graph.setData", "__label": "Graph Data", "csv": v["csv"]})),
            Err(_) => return,
        },
        // Flare Tool Options (Center / Halo / Rays / Rings), applied through the generic command dialog.
        "flare" => Dialog::new(
            "command",
            json!({"__command": "shape.flare", "__label": "Flare Tool Options", "cx": x, "cy": y, "diameter": 100, "opacity": 50, "brightness": 30,
                "growth": 20, "fuzziness": 50, "rays": 15, "longest": 300, "rayFuzziness": 100, "pathLength": 300, "rings": 10, "largest": 50, "direction": 45}),
        ),
        "rotate" | "reflect" | "scale" | "shear" | "artboardOptions" => {
            let mut base = match kind {
                "rotate" => json!({"angle": 0}),
                "reflect" => json!({"axis": "vertical"}),
                "scale" => json!({"sx": 100, "sy": 100, "uniform": true}),
                "shear" => json!({"angle": 0, "axis": "horizontal"}),
                _ => json!({}),
            };
            if let (Some(b), Some(o)) = (base.as_object_mut(), p.as_object()) {
                for (k, v) in o {
                    b.insert(k.clone(), v.clone());
                }
            }
            Dialog::new(kind, base)
        }
        _ => return,
    };
    app.ui.dialog = Some(d);
}

/// Pass a tool-chosen reference point (origin) through to transform commands.
fn origin_params(d: &Dialog, mut p: Value) -> Value {
    if let Some(o) = d.fields.get("origin") {
        p["origin"] = o.clone();
    }
    p
}

/// Effect parameters from the dialog fields (drop UI-only keys).
fn effect_params(d: &Dialog) -> Value {
    Value::Object(d.fields.iter().filter(|(k, _)| !k.starts_with("__") && k.as_str() != "preview").map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// Generic editor for an effect dialog: numbers, booleans, strings and colours.
fn effect_fields(ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut changed = false;
    egui::Grid::new("fxgrid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        let keys: Vec<(String, Value)> =
            d.fields.iter().filter(|(k, _)| !k.starts_with("__") && k.as_str() != "preview").map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, v) in keys {
            ui.label(egui::RichText::new(humanize(&k)).color(t.text));
            match v {
                Value::Number(n) => {
                    let mut x = n.as_f64().unwrap_or(0.0);
                    let speed = if x.abs() > 20.0 { 1.0 } else { 0.1 };
                    if ui.add(egui::DragValue::new(&mut x).speed(speed).max_decimals(2)).changed() {
                        d.fields.insert(k, json!(x));
                        changed = true;
                    }
                }
                Value::Bool(mut b) => {
                    if ui.checkbox(&mut b, "").changed() {
                        d.fields.insert(k, json!(b));
                        changed = true;
                    }
                }
                Value::String(mut s) if s.contains('\n') => {
                    // Multi-line values (Graph Data CSV) get a text area.
                    if ui.add(egui::TextEdit::multiline(&mut s).desired_width(260.0).desired_rows(8).font(egui::TextStyle::Monospace)).changed() {
                        d.fields.insert(k, json!(s));
                        changed = true;
                    }
                }
                Value::String(mut s) => {
                    if ui.add(egui::TextEdit::singleline(&mut s).desired_width(140.0)).changed() {
                        d.fields.insert(k, json!(s));
                        changed = true;
                    }
                }
                other => {
                    ui.label(egui::RichText::new(other.to_string()).color(t.text_dim).size(11.0));
                }
            }
            ui.end_row();
        }
    });
    changed
}

fn title(kind: &str) -> &'static str {
    match kind {
        "newDocument" => "New Document",
        "rectangle" => "Rectangle",
        "roundedRectangle" => "Rounded Rectangle",
        "ellipse" => "Ellipse",
        "polygon" => "Polygon",
        "star" => "Star",
        "lineSegment" => "Line Segment Tool Options",
        "move" => "Move",
        "rotate" => "Rotate",
        "scale" => "Scale",
        "reflect" => "Reflect",
        "shear" => "Shear",
        "average" => "Average",
        "offsetPath" => "Offset Path",
        "simplify" => "Simplify",
        "splitIntoGrid" => "Split Into Grid",
        "documentSetup" => "Document Setup",
        "preferences" => "Preferences",
        "shortcuts" => "Keyboard Shortcuts",
        "artboardOptions" => "Artboard Options",
        "allTools" => "All Tools",
        "exportForScreens" => "Export for Screens",
        "recolor" => "Recolor Artwork",
        crate::unsaved::KIND => "Save Changes",
        _ => "Dialog",
    }
}

/// Apply the open dialog (OK).
pub fn confirm(app: &mut VectorcraftApp) -> Result<Value, String> {
    let Some(d) = app.ui.dialog.clone() else { return Err("no dialog open".into()) };
    match d.kind.as_str() {
        "preferences" => return crate::prefs_dialog::confirm(app),
        "shortcuts" => return crate::shortcut_editor::confirm(app),
        "newWorkspace" | "manageWorkspaces" => return crate::workspaces::confirm(app),
        "findFont" => return crate::find_font::confirm(app),
        crate::unsaved::KIND => return crate::unsaved::confirm(app),
        _ => {}
    }
    let copy = d.bool("copy");
    if d.kind == "recolor" {
        let map = recolor_map(&d);
        app.ui.dialog = None;
        if app.session.in_interaction() {
            let _ = app.session.preview("recolor.apply", &json!({"map": map}));
            return app.session.commit_interaction().map(|_| Value::Null).map_err(|e| e.to_string());
        }
        return app.run("recolor.apply", json!({"map": map}));
    }
    if d.kind == "exportForScreens" {
        let boards: Vec<usize> = d
            .fields
            .get("boards")
            .and_then(Value::as_array)
            .map(|a| a.iter().enumerate().filter(|(_, b)| b.as_bool() == Some(true)).map(|(i, _)| i).collect())
            .unwrap_or_default();
        let params = json!({"folder": d.str("folder"), "artboards": boards, "formats": d.fields.get("formats").cloned().unwrap_or(json!([])), "prefix": d.str("prefix")});
        app.ui.dialog = None;
        let r = app.run("document.exportForScreens", params);
        if let Ok(v) = &r {
            let n = v["files"].as_array().map(|a| a.len()).unwrap_or(0);
            app.status(format!("Exported {n} file(s) to {}", d.str("folder")));
        }
        return r;
    }
    if d.kind == "command" {
        let cmd = d.str("__command");
        let params = effect_params(&d);
        app.ui.dialog = None;
        return app.run(&cmd, params);
    }
    if d.kind == "effect" {
        let effect = d.str("__effect");
        let params = effect_params(&d);
        let r = if app.session.in_interaction() {
            // Live preview already applied: keep it (the interaction commits as one undo step).
            let _ = app.session.preview("effect.apply", &json!({"effect": effect, "params": params}));
            app.session.commit_interaction().map(|_| Value::Null).map_err(|e| e.to_string())
        } else {
            app.run("effect.apply", json!({"effect": effect, "params": params}))
        };
        app.last_effect = Some((effect, params));
        app.ui.dialog = None;
        return r;
    }
    let r = match d.kind.as_str() {
        "newDocument" => {
            let r = app.run("file.new", json!({"width": d.f64("width", 612.0), "height": d.f64("height", 792.0), "units": d.str("units"), "title": d.str("name"), "artboards": d.f64("artboards", 1.0), "colorMode": d.str("colorMode").to_lowercase()}));
            if r.is_ok() {
                app.ui.dialog = None;
            }
            return r;
        }
        "rectangle" | "roundedRectangle" => app.run("shape.rectangle", json!({"x": d.f64("x", 0.0), "y": d.f64("y", 0.0), "width": d.f64("width", 100.0), "height": d.f64("height", 100.0), "radius": d.f64("radius", 0.0)})),
        "ellipse" => app.run("shape.ellipse", json!({"x": d.f64("x", 0.0), "y": d.f64("y", 0.0), "width": d.f64("width", 100.0), "height": d.f64("height", 100.0)})),
        "polygon" => app.run("shape.polygon", json!({"cx": d.f64("x", 0.0), "cy": d.f64("y", 0.0), "radius": d.f64("radius", 50.0), "sides": d.f64("sides", 6.0) as u64})),
        "star" => app.run("shape.star", json!({"cx": d.f64("x", 0.0), "cy": d.f64("y", 0.0), "radius1": d.f64("radius1", 50.0), "radius2": d.f64("radius2", 25.0), "points": d.f64("points", 5.0) as u64})),
        "lineSegment" => {
            let (x, y, l, a) = (d.f64("x", 0.0), d.f64("y", 0.0), d.f64("length", 100.0), d.f64("angle", 0.0).to_radians());
            app.run("shape.line", json!({"x1": x, "y1": y, "x2": x + l * a.cos(), "y2": y - l * a.sin()}))
        }
        "move" => app.run("object.move", json!({"dx": d.f64("dx", 0.0), "dy": d.f64("dy", 0.0), "copy": copy})),
        "rotate" => app.run("object.rotate", origin_params(&d, json!({"angle": d.f64("angle", 0.0), "copy": copy}))),
        "scale" => {
            let sx = d.f64("sx", 100.0);
            let sy = if d.bool("uniform") { sx } else { d.f64("sy", 100.0) };
            app.run("object.scale", origin_params(&d, json!({"sx": sx, "sy": sy, "copy": copy})))
        }
        "reflect" => app.run("object.reflect", origin_params(&d, json!({"axis": d.fields.get("axis").cloned().unwrap_or(json!("vertical")), "copy": copy}))),
        "shear" => app.run("object.shear", origin_params(&d, json!({"angle": d.f64("angle", 0.0), "axis": d.str("axis"), "copy": copy}))),
        "artboardOptions" => {
            let mut p = serde_json::Value::Object(d.fields.clone());
            for k in ["x", "y", "width", "height"] {
                if d.fields.contains_key(k) {
                    p[k] = json!(d.f64(k, 0.0));
                }
            }
            app.run("artboard.setProps", p)
        }
        "average" => app.run("path.average", json!({"axis": d.str("axis")})),
        "offsetPath" => app.run("object.path.offsetPath", json!({"offset": d.f64("offset", 10.0), "joins": d.str("joins"), "miterLimit": d.f64("miterLimit", 4.0)})),
        "simplify" => app.run("object.path.simplify", json!({"tolerance": d.f64("tolerance", 1.0)})),
        "splitIntoGrid" => app.run("object.path.splitIntoGrid", json!({"rows": d.f64("rows", 2.0), "columns": d.f64("columns", 2.0), "gutter": d.f64("gutter", 12.0)})),
        "documentSetup" => app.run("document.setUnits", json!({"units": d.str("units")})),
        _ => Ok(Value::Null),
    };
    app.ui.dialog = None;
    r
}

fn field(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(label).color(t.text_dim));
    let mut s = d.str(key);
    let r = egui::Frame::NONE
        .fill(t.input)
        .stroke(egui::Stroke::new(1.0, t.input_border))
        .corner_radius(egui::CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| ui.add(egui::TextEdit::singleline(&mut s).frame(egui::Frame::NONE).desired_width(120.0)));
    if r.inner.changed() {
        d.fields.insert(key.into(), Value::String(s));
    }
    ui.end_row();
}

fn check(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str) {
    let mut b = d.bool(key);
    if ui.checkbox(&mut b, label).changed() {
        d.fields.insert(key.into(), Value::Bool(b));
    }
}

pub fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if app.ui.about {
        let mut open = true;
        egui::Window::new("About VectorCraft")
            .collapsible(false)
            .resizable(false)
            .open(&mut open)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .frame(egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(18)))
            .show(ctx, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                    crate::brand::paint_mark(ui.painter(), r);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("VectorCraft").font(theme::semibold(22.0)));
                        ui.label(format!("Version {} — open-source vector illustration in pure Rust.", env!("CARGO_PKG_VERSION")));
                    });
                });
                ui.add_space(12.0);
                crate::community::links(app, ui);
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(
                        "Part of ArtCraft. MIT OR Apache-2.0. Fonts: Source Sans 3, Inter, JetBrains Mono (OFL). Icons: Lucide (ISC) + VectorCraft.",
                    )
                    .size(11.0),
                );
            });
        app.ui.about = open;
    }
    let Some(mut d) = app.ui.dialog.clone() else { return };
    match d.kind.as_str() {
        "preferences" => return crate::prefs_dialog::show(app, ctx),
        "shortcuts" => return crate::shortcut_editor::show(app, ctx),
        "newWorkspace" | "manageWorkspaces" => return crate::workspaces::show(app, ctx),
        "findFont" => return crate::find_font::show(app, ctx),
        _ => {}
    }
    let t = Tokens::get(ctx);
    let mut ok = false;
    let mut cancel = false;
    let mut discard = false;
    egui::Area::new(egui::Id::new("modal-dim")).order(egui::Order::Middle).fixed_pos(egui::pos2(0.0, 0.0)).show(ctx, |ui| {
        // Illustrator's dialogs are modal but don't dim the canvas (previews stay readable).
        ui.allocate_rect(ctx.content_rect(), egui::Sense::click());
    });
    egui::Window::new(title(&d.kind))
        .id(egui::Id::new("dialog"))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0])
        .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_min_width(if d.kind == "newDocument" { 560.0 } else { 320.0 });
            if d.kind == crate::unsaved::KIND {
                // Long document names wrap instead of widening the dialog.
                ui.set_max_width(420.0);
            }
            let heading = match d.kind.as_str() {
                "effect" | "command" => d.str("__label"),
                crate::unsaved::KIND => format!("Do you want to save the changes you made to “{}”?", d.str("name")),
                k => title(k).to_string(),
            };
            ui.label(egui::RichText::new(heading).font(theme::semibold(16.0)).color(t.text));
            ui.add_space(12.0);
            match d.kind.as_str() {
                crate::unsaved::KIND => {
                    ui.label(egui::RichText::new("Your changes will be lost if you don't save them.").color(t.text_dim));
                }
                "exportForScreens" => export_for_screens_ui(app, ui, &mut d),
                "recolor" => {
                    if recolor_ui(ui, &mut d) || !app.session.in_interaction() {
                        let _ = app.session.begin_interaction("Recolor Artwork");
                        let _ = app.session.preview("recolor.apply", &json!({"map": recolor_map(&d)}));
                    }
                }
                "command" => {
                    effect_fields(ui, &mut d);
                }
                "effect" => {
                    let changed = effect_fields(ui, &mut d);
                    ui.add_space(6.0);
                    let mut pv = d.bool("preview");
                    let pv_changed = ui.checkbox(&mut pv, "Preview").changed();
                    d.fields.insert("preview".into(), json!(pv));
                    if pv && (changed || pv_changed || !app.session.in_interaction()) {
                        let label = d.str("__label");
                        let _ = app.session.begin_interaction(&label);
                        let _ = app.session.preview("effect.apply", &json!({"effect": d.str("__effect"), "params": effect_params(&d)}));
                    } else if !pv && pv_changed {
                        let _ = app.session.cancel_interaction();
                    }
                }
                "newDocument" => new_document(ui, &mut d),
                "allTools" => {
                    for g in vectorcraft_tools::TOOL_GROUPS {
                        ui.horizontal_wrapped(|ui| {
                            for tool in g.iter() {
                                if ui.button(tool.label.trim_end_matches(" Tool")).clicked() {
                                    app.select_tool(tool.id);
                                    cancel = true;
                                }
                            }
                        });
                    }
                }
                _ => {
                    egui::Grid::new("dlg").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                        let keys: Vec<(String, Value)> = d.fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                        for (k, v) in keys {
                            if k == "x" || k == "y" || k == "origin" || k == "index" || v.is_boolean() {
                                continue;
                            }
                            field(ui, &mut d, &k, &humanize(&k));
                        }
                    });
                    if matches!(d.kind.as_str(), "move" | "rotate" | "scale" | "reflect" | "shear") {
                        ui.add_space(6.0);
                        if d.kind == "scale" {
                            check(ui, &mut d, "uniform", "Uniform");
                        }
                        check(ui, &mut d, "copy", "Copy (make a transformed copy)");
                    }
                }
            }
            ui.add_space(16.0);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = match d.kind.as_str() {
                    "newDocument" => "Create",
                    "exportForScreens" => "Export Artboard",
                    crate::unsaved::KIND => "Save",
                    _ => "OK",
                };
                if d.kind != "allTools" && widgets::primary_button(ui, label).clicked() {
                    ok = true;
                }
                ui.add_space(8.0);
                if widgets::secondary_button(ui, if d.kind == "allTools" { "Close" } else { "Cancel" }).clicked() {
                    cancel = true;
                }
                if d.kind == crate::unsaved::KIND {
                    ui.add_space(28.0);
                    discard = widgets::secondary_button(ui, "Don't Save").clicked();
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && d.kind != "allTools" {
        ok = true;
    }
    if discard {
        d.fields.insert("discard".into(), json!(true));
        ok = true;
    }
    let is_effect = d.kind == "effect" || d.kind == "recolor";
    app.ui.dialog = Some(d);
    if cancel {
        if is_effect {
            let _ = app.session.cancel_interaction();
        }
        app.ui.dialog = None;
    } else if ok && let Err(e) = confirm(app) {
        app.status(e);
    }
}

fn humanize(k: &str) -> String {
    let mut s = String::new();
    for (i, c) in k.chars().enumerate() {
        if i == 0 {
            s.extend(c.to_uppercase());
        } else if c.is_uppercase() {
            s.push(' ');
            s.push(c);
        } else {
            s.push(c);
        }
    }
    match s.as_str() {
        "Dx" => "Horizontal".into(),
        "Dy" => "Vertical".into(),
        "Sx" => "Horizontal %".into(),
        "Sy" => "Vertical %".into(),
        "Radius1" => "Radius 1".into(),
        "Radius2" => "Radius 2".into(),
        _ => format!("{s}:"),
    }
}

fn new_document(ui: &mut egui::Ui, d: &mut Dialog) {
    let t = Tokens::get(ui.ctx());
    let presets: [(&str, &str, &str, &str); 8] = [
        ("Letter", "612 pt", "792 pt", "Points"),
        ("Legal", "612 pt", "1008 pt", "Points"),
        ("Tabloid", "792 pt", "1224 pt", "Points"),
        ("A4", "595.28 pt", "841.89 pt", "Points"),
        ("A3", "841.89 pt", "1190.55 pt", "Points"),
        ("Web 1920", "1920 px", "1080 px", "Pixels"),
        ("iPhone", "390 px", "844 px", "Pixels"),
        ("Square Post", "1080 px", "1080 px", "Pixels"),
    ];
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.set_width(300.0);
            ui.label(egui::RichText::new("Presets").color(t.text_dim));
            ui.horizontal_wrapped(|ui| {
                for (name, w, h, u) in presets {
                    let sel = d.str("preset") == name;
                    if ui.selectable_label(sel, name).clicked() {
                        d.fields.insert("preset".into(), json!(name));
                        d.fields.insert("width".into(), json!(w));
                        d.fields.insert("height".into(), json!(h));
                        d.fields.insert("units".into(), json!(u));
                    }
                }
            });
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label(egui::RichText::new("Preset Details").font(theme::semibold(12.5)));
            egui::Grid::new("newdoc").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
                field(ui, d, "name", "Name:");
                field(ui, d, "width", "Width:");
                field(ui, d, "height", "Height:");
                field(ui, d, "artboards", "Artboards:");
                field(ui, d, "colorMode", "Color Mode:");
            });
        });
    });
}

/// Export for Screens: artboard picker (thumbnails + checkboxes), format/scale rows, destination.
fn export_for_screens_ui(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) {
    let t = Tokens::get(ui.ctx());
    let names: Vec<String> = app.session.active().map(|s| s.doc.artboards.iter().map(|a| a.name.clone()).collect()).unwrap_or_default();
    let mut boards: Vec<bool> =
        d.fields.get("boards").and_then(Value::as_array).map(|a| a.iter().map(|b| b.as_bool().unwrap_or(false)).collect()).unwrap_or_default();
    boards.resize(names.len(), true);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(260.0);
            ui.label(egui::RichText::new("Artboards").color(t.text));
            ui.horizontal(|ui| {
                if ui.small_button("Select All").clicked() {
                    boards.iter_mut().for_each(|b| *b = true);
                }
                if ui.small_button("Clear").clicked() {
                    boards.iter_mut().for_each(|b| *b = false);
                }
            });
            egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                for (i, name) in names.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let (r, _) = ui.allocate_exact_size(egui::vec2(46.0, 46.0), egui::Sense::hover());
                        match artboard_thumb(app, ui.ctx(), i) {
                            Some(tex) => {
                                let sz = tex.size_vec2();
                                let s = (46.0 / sz.x.max(sz.y)).min(1.0);
                                let ir = egui::Rect::from_center_size(r.center(), sz * s);
                                ui.painter().image(
                                    tex.id(),
                                    ir,
                                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                    egui::Color32::WHITE,
                                );
                            }
                            None => {
                                ui.painter().rect_filled(r, 2.0, egui::Color32::WHITE);
                            }
                        }
                        ui.checkbox(&mut boards[i], name);
                    });
                }
            });
        });
        let (sep, _) = ui.allocate_exact_size(egui::vec2(9.0, 360.0), egui::Sense::hover());
        ui.painter().line_segment([sep.center_top(), sep.center_bottom()], egui::Stroke::new(1.0, t.divider));
        ui.vertical(|ui| {
            ui.set_width(330.0);
            ui.label(egui::RichText::new("Export to").color(t.text));
            let mut folder = d.str("folder");
            if ui.add(egui::TextEdit::singleline(&mut folder).desired_width(320.0)).changed() {
                d.fields.insert("folder".into(), json!(folder));
            }
            ui.add_space(8.0);
            ui.label(egui::RichText::new("Formats").color(t.text));
            let mut formats: Vec<Value> = d.fields.get("formats").and_then(Value::as_array).cloned().unwrap_or_default();
            let mut remove = None;
            egui::Grid::new("efs-formats").num_columns(4).spacing([8.0, 6.0]).show(ui, |ui| {
                ui.label(egui::RichText::new("Scale").color(t.text_dim));
                ui.label(egui::RichText::new("Suffix").color(t.text_dim));
                ui.label(egui::RichText::new("Format").color(t.text_dim));
                ui.label("");
                ui.end_row();
                for (i, f) in formats.iter_mut().enumerate() {
                    let mut sc = f["scale"].as_f64().unwrap_or(1.0);
                    if ui.add(egui::DragValue::new(&mut sc).range(0.1..=10.0).speed(0.5).suffix("x")).changed() {
                        f["scale"] = json!(sc);
                    }
                    let mut suffix = f["suffix"].as_str().unwrap_or("").to_string();
                    if ui.add(egui::TextEdit::singleline(&mut suffix).desired_width(60.0)).changed() {
                        f["suffix"] = json!(suffix);
                    }
                    let cur = f["format"].as_str().unwrap_or("png").to_string();
                    egui::ComboBox::from_id_salt(("efs-fmt", i)).selected_text(cur.to_uppercase()).width(70.0).show_ui(ui, |ui| {
                        for fm in ["png", "jpg", "webp", "svg", "pdf"] {
                            if ui.selectable_label(cur == fm, fm.to_uppercase()).clicked() {
                                f["format"] = json!(fm);
                            }
                        }
                    });
                    if ui.small_button("×").clicked() {
                        remove = Some(i);
                    }
                    ui.end_row();
                }
            });
            if let Some(i) = remove {
                formats.remove(i);
            }
            if ui.button("+ Add Scale").clicked() {
                let next = formats.len() as f64 + 1.0;
                formats.push(json!({"format": "png", "scale": next, "suffix": format!("@{next}x")}));
            }
            d.fields.insert("formats".into(), Value::Array(formats));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Prefix").color(t.text_dim));
                let mut prefix = d.str("prefix");
                if ui.add(egui::TextEdit::singleline(&mut prefix).desired_width(120.0)).changed() {
                    d.fields.insert("prefix".into(), json!(prefix));
                }
            });
        });
    });
    d.fields.insert("boards".into(), json!(boards));
}

/// A small cached rendering of artboard `i` (keyed by document revision).
fn artboard_thumb(app: &mut VectorcraftApp, ctx: &egui::Context, i: usize) -> Option<egui::TextureHandle> {
    let st = app.session.active()?;
    let key = egui::Id::new(("ab-thumb", i, st.revision));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return Some(t);
    }
    let doc = st.doc.clone();
    let r = doc.artboards.get(i)?.rect;
    let scale = 92.0 / r.width().max(r.height()).max(1.0);
    let img = app.canvas.renderer.render_region(&doc, r, scale, true);
    let color = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
    let tex = ctx.load_texture(format!("ab-thumb-{i}"), color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(key, tex.clone()));
    Some(tex)
}

fn recolor_map(d: &Dialog) -> Value {
    let mut m = serde_json::Map::new();
    for p in d.fields.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default() {
        if let (Some(a), Some(b)) = (p.get(0).and_then(Value::as_str), p.get(1).and_then(Value::as_str)) {
            m.insert(a.to_string(), json!(b));
        }
    }
    Value::Object(m)
}

/// Recolor Artwork: current → new colour rows, harmony rules, randomize. Returns true when changed.
fn recolor_ui(ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    use vectorcraft_color::Color;
    let t = Tokens::get(ui.ctx());
    let mut pairs: Vec<Value> = d.fields.get("pairs").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut changed = false;
    let chip = |ui: &mut egui::Ui, hex: &str| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(34.0, 22.0), egui::Sense::hover());
        let c = Color::from_hex(hex).unwrap_or(Color::BLACK).to_rgba8(1.0);
        ui.painter().rect_filled(r, 2.0, egui::Color32::from_rgb(c[0], c[1], c[2]));
        ui.painter().rect_stroke(r, 2.0, egui::Stroke::new(1.0, t.input_border), egui::StrokeKind::Inside);
    };
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Current Colors").color(t.text_dim));
        ui.add_space(70.0);
        ui.label(egui::RichText::new("New").color(t.text_dim));
    });
    egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
        for p in pairs.iter_mut() {
            let from = p[0].as_str().unwrap_or("#000000").to_string();
            let mut to = p[1].as_str().unwrap_or("#000000").to_string();
            ui.horizontal(|ui| {
                chip(ui, &from);
                ui.label(egui::RichText::new("→").color(t.text_dim));
                chip(ui, &to);
                if ui.add(egui::TextEdit::singleline(&mut to).desired_width(80.0)).changed() && Color::from_hex(&to).is_some() {
                    p[1] = json!(to);
                    changed = true;
                }
            });
        }
    });
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button("Randomize").clicked() {
            // Deterministic scramble of hues (seeded by the colour itself), keeping brightness.
            for (i, p) in pairs.iter_mut().enumerate() {
                let c = Color::from_hex(p[0].as_str().unwrap_or("#000")).unwrap_or(Color::BLACK);
                let [h, s, v] = c.to_hsb();
                let nh = (h + 97.0 * (i as f32 + 1.0) + 41.0) % 360.0;
                p[1] = json!(Color::from_hsb(nh, s.max(0.35), v.max(0.3)).to_hex());
            }
            changed = true;
        }
        for (label, deg) in [("Complement", 180.0), ("Triad", 120.0), ("Analogous", 30.0)] {
            if ui.button(label).clicked() {
                for (i, p) in pairs.iter_mut().enumerate() {
                    let c = Color::from_hex(p[0].as_str().unwrap_or("#000")).unwrap_or(Color::BLACK);
                    let [h, s, v] = c.to_hsb();
                    let shift = if label == "Complement" { deg } else { deg * (i as f32 + 1.0) };
                    p[1] = json!(Color::from_hsb(h + shift, s, v).to_hex());
                }
                changed = true;
            }
        }
        if ui.button("Reset").clicked() {
            for p in pairs.iter_mut() {
                p[1] = p[0].clone();
            }
            changed = true;
        }
    });
    d.fields.insert("pairs".into(), Value::Array(pairs));
    changed
}
