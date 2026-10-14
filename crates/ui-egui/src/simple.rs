//! Simple mode (E2): what someone new to embroidery sees once they choose "New to embroidery".
//! In place of the toolbar, a column of large labelled buttons (add words, a shape, an appliqué
//! piece or a picture, watch it sew, save for my machine); in place of the panels, Your Design:
//! how the chosen part sews, its thread and size, or the whole design's figures. Every button
//! runs an ordinary command.

use egui::{Color32, RichText, Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::NodeKind;

use crate::VectorcraftApp;
use crate::state::Experience;
use crate::theme::{self, Tokens};

/// The left column's width.
const BAR: f32 = 176.0;
/// Your Design's width.
const PANEL: f32 = 290.0;

/// Whether simple mode is on (Window › Experience › New to Embroidery).
pub fn active(app: &VectorcraftApp) -> bool {
    app.ui.experience == Experience::Beginner
}

/// What simple mode is asking for: words to add, or a shape to pick.
#[derive(Clone, Debug, Default)]
pub struct SimpleState {
    /// Add Words is open, with what has been typed and how tall (mm).
    pub words: Option<(String, f64)>,
    /// The shape picker is open, for appliqué or not.
    pub shapes: Option<bool>,
}

/// A large button filling the column's width.
fn big(ui: &mut Ui, text: &str, primary: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let label = RichText::new(text).size(15.0).color(if primary { Color32::WHITE } else { t.text });
    let b = egui::Button::new(label).min_size(vec2(ui.available_width(), 40.0)).corner_radius(6.0);
    ui.add(if primary { b.fill(t.accent) } else { b })
}

/// The left column of actions.
pub fn left_bar(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let frame = egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::same(10)).stroke(egui::Stroke::new(1.0, t.border));
    egui::Panel::left("simple-bar").resizable(false).exact_size(BAR).frame(frame).show(ui, |ui| {
        let doc = app.session.active().is_some();
        ui.spacing_mut().item_spacing.y = 8.0;
        // the guide works with or without a design open
        if big(ui, tl!("Make a Design…"), false).on_hover_text(tl!("Step by step: words, a shape, appliqué, a picture or a sample")).clicked() {
            app.run("ui.makeDesign", json!({})).ok();
        }
        ui.add_space(6.0);
        ui.add_enabled_ui(doc, |ui| {
            if big(ui, tl!("Select"), false).on_hover_text(tl!("Click a part of the design to choose it, drag it to move it")).clicked() {
                app.run("tool.select", json!({ "tool": "selection" })).ok();
            }
            if big(ui, tl!("Add Words…"), false).on_hover_text(tl!("A name or a few words, ready to sew")).clicked() {
                app.simple.words = Some((String::new(), 20.0));
            }
            if big(ui, tl!("Add a Shape…"), false).on_hover_text(tl!("A heart, a star, a flower and more, ready to sew")).clicked() {
                app.simple.shapes = Some(false);
            }
            if big(ui, tl!("Add Appliqué…"), false)
                .on_hover_text(tl!("A fabric shape: the machine stops for you to lay the fabric, then sews it down"))
                .clicked()
            {
                app.simple.shapes = Some(true);
            }
            if big(ui, tl!("Add a Picture…"), false).on_hover_text(tl!("A photo or drawing to turn into stitches")).clicked() {
                app.run("file.place", json!({})).ok();
            }
            ui.add_space(6.0);
            if big(ui, tl!("Watch It Sew"), false).on_hover_text(tl!("See the design sew on screen, stitch by stitch")).clicked() {
                let p = &mut app.embroidery.player;
                (p.active, p.playing, p.pos) = (true, true, 0.0);
            }
            if big(ui, tl!("Undo"), false).on_hover_text(tl!("Take back the last change")).clicked() {
                app.run("edit.undo", json!({})).ok();
            }
            ui.add_space(6.0);
            if big(ui, tl!("Save for My Machine"), true).on_hover_text(tl!("Write the design for your machine to its USB stick or folder")).clicked()
            {
                app.run("ui.saveForMyMachine", json!({})).ok();
            }
        });
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            if ui.link(tl!("Show everything")).clicked() {
                app.run("window.experience", json!({ "level": "all" })).ok();
            }
            if ui.link(tl!("Home")).clicked() {
                app.run("app.home", json!({})).ok();
            }
        });
    });
}

/// How a part sews, as simple mode names it: label, stitch type id.
const SEW_AS: [(&str, &str); 5] =
    [("Filled", "fill"), ("Smooth edge (satin)", "satin"), ("Outline", "run"), ("Appliqué", "applique"), ("Not sewn", "none")];

/// The right column: the chosen part, else the whole design.
pub fn right_panel(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let frame = egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin::same(12)).stroke(egui::Stroke::new(1.0, t.border));
    egui::Panel::right("simple-panel").resizable(false).exact_size(PANEL).frame(frame).show(ui, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 8.0;
                let Some(st) = app.session.active() else {
                    ui.label(RichText::new(tl!("Start a design from the Home screen.")).size(14.0).color(t.text_dim));
                    return;
                };
                let selected = st.selection.objects.clone();
                let image = selected.first().and_then(|id| st.doc.node(*id)).is_some_and(|n| matches!(n.kind, NodeKind::Image(_)));
                if selected.is_empty() {
                    design_summary(app, ui, &t);
                } else if image {
                    ui.label(RichText::new(tl!("A picture")).font(theme::semibold(16.0)).color(t.text));
                    ui.label(RichText::new(tl!("Turn it into stitches in your thread colours.")).size(13.0).color(t.text_dim));
                    if big(ui, tl!("Turn Into Stitches"), true).clicked() {
                        app.run("embroidery.autoDigitize", json!({})).ok();
                    }
                    part_actions(app, ui);
                } else {
                    part(app, ui, &t, selected.len());
                }
            });
        });
    });
}

/// The chosen part: how it sews, its thread, its size.
fn part(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens, count: usize) {
    let title = if count == 1 { tl!("The chosen part") } else { tl!("The chosen parts") };
    ui.label(RichText::new(title).font(theme::semibold(16.0)).color(t.text));
    // how the first chosen part sews, and in what thread
    let (ty, hex) = app
        .session
        .active()
        .and_then(|st| {
            // the part itself, or the first stitched part inside it (a shape is a group)
            let top = st.doc.node(*st.selection.objects.first()?)?;
            let mut found = None;
            top.walk(&mut |m| {
                if found.is_none() {
                    found = m.embroidery.as_deref().map(|e| (m, e));
                }
            });
            let (n, e) = found?;
            use vectorcraft_embroidery::sf_core::attr::StitchType;
            let ty = match &e.stitch {
                StitchType::Fill(_) => "fill",
                StitchType::Satin(_) => "satin",
                StitchType::Run(_) | StitchType::Redwork(_) => "run",
                StitchType::Applique(_) => "applique",
                _ => "other",
            };
            Some((ty.to_string(), vectorcraft_embroidery::node_thread(n, e).color.to_hex()))
        })
        .unwrap_or_else(|| ("none".into(), "#1a3e8c".into()));
    ui.label(RichText::new(tl!("Sew it as")).size(14.0).color(t.text));
    for (label, id) in SEW_AS {
        let on = ty == id;
        if ui.selectable_label(on, RichText::new(sew_label(label)).size(14.0)).clicked() {
            app.run("embroidery.set", json!({ "type": id })).ok();
        }
    }
    if ty != "none" {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(tl!("Thread colour")).size(14.0));
            let mut c = vectorcraft_embroidery::sf_core::Rgb::from_hex(&hex).map_or([26, 62, 140], |c| [c.r, c.g, c.b]);
            if ui.color_edit_button_srgb(&mut c).changed() {
                let hex = format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
                app.run("embroidery.setThread", json!({ "color": hex })).ok();
            }
        });
    }
    part_actions(app, ui);
}

/// Size and delete, for whatever is chosen.
fn part_actions(app: &mut VectorcraftApp, ui: &mut Ui) {
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new(tl!("Size")).size(14.0));
        if ui.button(RichText::new(tl!("Smaller")).size(14.0)).clicked() {
            app.run("object.scale", json!({ "sx": 90 })).ok();
        }
        if ui.button(RichText::new(tl!("Bigger")).size(14.0)).clicked() {
            app.run("object.scale", json!({ "sx": 110 })).ok();
        }
    });
    if ui.button(RichText::new(tl!("Delete")).size(14.0)).clicked() {
        app.run("edit.clear", json!({})).ok();
    }
}

/// The whole design: its hoop, size, stitches, colours and sewing time.
fn design_summary(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens) {
    ui.label(RichText::new(tl!("Your design")).font(theme::semibold(16.0)).color(t.text));
    let info = app.session.execute("embroidery.info", &json!({})).unwrap_or_default();
    let stitches = info["stitches"].as_u64().unwrap_or(0);
    if stitches == 0 {
        ui.label(
            RichText::new(tl!("Nothing to sew yet. Add words, a shape or a picture with the buttons on the left.")).size(14.0).color(t.text_dim),
        );
    } else {
        let row = |ui: &mut Ui, k: &str, v: String| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(k).size(14.0).color(t.text_dim));
                ui.label(RichText::new(v).size(14.0).color(t.text));
            });
        };
        let (w, h) = (info["widthMm"].as_f64().unwrap_or(0.0), info["heightMm"].as_f64().unwrap_or(0.0));
        let inches = app.session.prefs.units_general == "inches";
        let size = if inches { format!("{:.1} × {:.1} in", w / 25.4, h / 25.4) } else { format!("{w:.0} × {h:.0} mm") };
        row(ui, tl!("Size"), size);
        row(ui, tl!("Stitches"), stitches.to_string());
        row(ui, tl!("Thread colours"), info["colors"].as_array().map_or(0, Vec::len).to_string());
        if let Some(time) = info["time"].as_str() {
            row(ui, tl!("Sewing time"), time.to_string());
        }
        if info["fits"].as_bool() == Some(false) {
            ui.label(RichText::new(tl!("The design is bigger than the hoop: make it smaller before you sew.")).size(13.0).color(t.error));
        }
        // swatches of the thread colours, in sewing order
        ui.horizontal_wrapped(|ui| {
            for c in info["colors"].as_array().map(Vec::as_slice).unwrap_or_default() {
                let hex = c["color"].as_str().unwrap_or("#000000");
                let rgb = vectorcraft_embroidery::sf_core::Rgb::from_hex(hex).unwrap_or_default();
                let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 4.0, Color32::from_rgb(rgb.r, rgb.g, rgb.b));
                resp.on_hover_text(c["name"].as_str().unwrap_or(hex));
            }
        });
    }
    ui.add_space(8.0);
    ui.label(RichText::new(tl!("Click a part of the design to change how it sews.")).size(13.0).color(t.text_dim));
}

/// A Sew It As label in the UI language.
fn sew_label(label: &str) -> &str {
    match label {
        "Filled" => tl!("Filled"),
        "Smooth edge (satin)" => tl!("Smooth edge (satin)"),
        "Outline" => tl!("Just the outline"),
        "Appliqué" => tl!("Appliqué"),
        _ => tl!("Not sewn"),
    }
}

/// Add Words and the shape picker, while open.
pub fn windows(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if let Some((mut text, mut height)) = app.simple.words.clone() {
        let mut open = true;
        let mut action: Option<bool> = None;
        egui::Window::new(tl!("Add Words")).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.label(RichText::new(tl!("What should it say?")).size(15.0));
            let r = ui.add(egui::TextEdit::singleline(&mut text).font(egui::FontId::proportional(18.0)).desired_width(360.0));
            r.request_focus();
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(tl!("How tall")).size(15.0));
                ui.add(egui::Slider::new(&mut height, 8.0..=80.0).suffix(" mm").fixed_decimals(0));
            });
            ui.label(RichText::new(format!("≈ {:.1} in", height / 25.4)).size(12.0));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if crate::widgets::primary_button(ui, tl!("Add")).clicked() || (r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) {
                    action = Some(true);
                }
                if ui.button(tl!("Cancel")).clicked() {
                    action = Some(false);
                }
            });
        });
        match action {
            Some(true) if !text.trim().is_empty() => {
                if let Err(e) = app.run("embroidery.addWords", json!({ "text": text, "height": height })) {
                    app.status(e);
                }
                app.simple.words = None;
            }
            Some(false) => app.simple.words = None,
            _ if !open => app.simple.words = None,
            _ => app.simple.words = Some((text, height)),
        }
    }
    if let Some(applique) = app.simple.shapes {
        let mut open = true;
        let mut chosen: Option<&str> = None;
        let title = if applique { tl!("Add Appliqué") } else { tl!("Add a Shape") };
        egui::Window::new(title).open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.label(RichText::new(tl!("Choose a shape")).size(15.0));
            ui.horizontal_wrapped(|ui| {
                for name in vectorcraft_engine::cmd::embroidery_simple::SIMPLE_SHAPES {
                    if ui.add(egui::Button::new(RichText::new(shape_name(name)).size(15.0)).min_size(vec2(100.0, 36.0))).clicked() {
                        chosen = Some(name);
                    }
                }
            });
        });
        if let Some(name) = chosen {
            if let Err(e) = app.run("embroidery.addShape", json!({ "name": name, "applique": applique })) {
                app.status(e);
            }
            app.simple.shapes = None;
        } else if !open {
            app.simple.shapes = None;
        }
    }
}

/// A shape's name in the UI language.
pub(crate) fn shape_name(name: &str) -> &str {
    match name {
        "Heart" => tl!("Heart"),
        "Star" => tl!("Star"),
        "Flower" => tl!("Flower"),
        "Leaf" => tl!("Leaf"),
        "Sun" => tl!("Sun"),
        "Moon" => tl!("Moon"),
        "Paw" => tl!("Paw"),
        "Crown" => tl!("Crown"),
        _ => tl!("Snowflake"),
    }
}

/// The menus simple mode keeps: the everyday file, edit, view and help commands.
pub fn keeps(id: &str, _p: &Value) -> bool {
    const KEEP: &[&str] = &[
        "file.newDialog",
        "ui.makeDesign",
        "file.open",
        "file.save",
        "file.saveAs",
        "file.close",
        "file.print",
        "file.quit",
        "file.place",
        "ui.saveForMyMachine",
        "ui.myMachine",
        "edit.undo",
        "edit.redo",
        "edit.cut",
        "edit.copy",
        "edit.paste",
        "edit.clear",
        "select.all",
        "view.zoomIn",
        "view.zoomOut",
        "view.fitArtboard",
        "view.actualSize",
        "ui.calibrateLifeSize",
        "help.show",
        "help.about",
        "window.experience",
        "app.home",
    ];
    KEEP.contains(&id)
}
