//! My Machine (E4): choose the embroidery machine once — its hoops, its file format, inches or
//! millimetres and the folder or USB stick designs go to — and Save for My Machine writes the
//! design there in one step (`embroidery.myMachine`, `embroidery.send`).

use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::{self, Tokens};

/// The My Machine window.
#[derive(Clone, Debug, Default)]
pub struct MyMachineState {
    pub open: bool,
}

/// `ui.myMachine`: opens the window.
pub fn open(app: &mut VectorcraftApp) -> Result<Value, String> {
    app.ui.my_machine.open = true;
    Ok(json!({ "open": true }))
}

/// The machine chosen, its format and folder (`embroidery.myMachine`).
fn info(app: &mut VectorcraftApp) -> Value {
    app.session.execute("embroidery.myMachine", &json!({})).unwrap_or_default()
}

/// `ui.saveForMyMachine`: the design written for My Machine in its folder. With no machine
/// chosen yet the My Machine window opens; with no folder, or the folder not there (the USB stick
/// out), a folder is picked first.
pub fn save(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    if app.session.active().is_none() {
        return Err("open or start a design first".into());
    }
    let mine = info(app);
    if mine["machine"].is_null() {
        app.ui.my_machine.open = true;
        return Ok(json!({ "open": true }));
    }
    // the web app has no folders: the machine file downloads, to copy onto the USB stick
    if crate::io::is_web(app) && p.get("folder").is_none() {
        return download(app);
    }
    let ready = app.session.execute("embroidery.send.targets", &json!({})).ok().and_then(|t| t["targets"].get(0).and_then(|x| x["ready"].as_bool()));
    let folder = match p.get("folder").and_then(Value::as_str) {
        Some(f) => f.to_string(),
        None if ready == Some(true) => mine["folder"].as_str().unwrap_or_default().to_string(),
        // none yet, or not there: pick one (the answer runs this again)
        None => match crate::picks::folder(app) {
            Some(f) => f,
            None => return Ok(Value::Null),
        },
    };
    let r = app.session.execute("embroidery.send", &json!({ "folder": folder })).map_err(|e| e.to_string())?;
    let path = r["path"].as_str().unwrap_or_default();
    app.status(format!("{} {}", tl!("Saved for your machine:"), path));
    Ok(r)
}

/// Web: the design's machine file in My Machine's format, downloaded.
fn download(app: &mut VectorcraftApp) -> Result<Value, String> {
    let title = app.session.active().map(|d| d.doc.title.clone()).unwrap_or_default();
    let stem = vectorcraft_engine::cmd::fileio::file_stem(&title);
    let stem = if stem.trim().is_empty() { "Design".to_string() } else { stem };
    let r = app.session.execute("embroidery.export", &json!({ "center": true, "name": stem })).map_err(|e| e.to_string())?;
    let bytes = r["dataBase64"].as_str().and_then(vectorcraft_format::base64_decode).ok_or("the machine file didn't encode")?;
    let name = format!("{stem}.{}", r["format"].as_str().unwrap_or("dst"));
    crate::io::write_to(&mut app.services, &name, &bytes)?;
    app.status(format!("{} {name}", tl!("Downloaded for your machine:")));
    Ok(json!({ "downloaded": name, "format": r["format"], "bytes": bytes.len() }))
}

/// The window, while open.
pub(super) fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !app.ui.my_machine.open {
        return;
    }
    let mut open = true;
    let mut done = false;
    let mine = info(app);
    egui::Window::new(tl!("My Machine"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos(ctx.content_rect().center() - egui::vec2(240.0, 220.0))
        .show(ctx, |ui| {
            let t = Tokens::get(ui.ctx());
            ui.set_width(460.0);
            ui.label(egui::RichText::new(tl!("Which embroidery machine do you have?")).font(theme::semibold(15.0)).color(t.text));
            ui.add_space(6.0);
            let current = mine["machine"].as_str().unwrap_or_default().to_string();
            for m in mine["machines"].as_array().map(Vec::as_slice).unwrap_or_default() {
                let Some(name) = m["name"].as_str() else { continue };
                let label = egui::RichText::new(machine_name(name)).size(15.0);
                if ui.selectable_label(name == current, label).clicked() {
                    app.run("embroidery.myMachine.set", json!({ "machine": name })).ok();
                }
            }
            ui.add_space(10.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(tl!("Measure in")).size(14.0));
                let units = app.session.prefs.units_general.clone();
                for (id, label) in [("millimeters", tl!("Millimeters")), ("inches", tl!("Inches"))] {
                    if ui.selectable_label(units == id, egui::RichText::new(label).size(14.0)).clicked() {
                        app.session.execute("prefs.set", &json!({ "key": "unitsGeneral", "value": id })).ok();
                    }
                }
            });
            ui.add_space(10.0);
            if crate::io::is_web(app) {
                ui.label(
                    egui::RichText::new(tl!("Save for My Machine downloads the design: copy it to the USB stick you take to your machine."))
                        .size(12.0)
                        .color(t.text_dim),
                );
            } else {
                ui.label(egui::RichText::new(tl!("Save designs to")).size(14.0));
                ui.horizontal(|ui| {
                    let folder = mine["folder"].as_str().map(str::to_string);
                    ui.label(egui::RichText::new(folder.unwrap_or_else(|| tl!("No folder chosen yet").into())).color(t.text_dim));
                    if ui.button(tl!("Choose Folder…")).clicked() {
                        crate::picks::button(app, |app| {
                            if let Some(f) = crate::picks::folder(app) {
                                app.session.execute("embroidery.send.targets", &json!({ "add": f })).ok();
                            }
                        });
                    }
                });
                ui.label(
                    egui::RichText::new(tl!("Plug in the USB stick you take to your machine, then choose it here.")).size(12.0).color(t.text_dim),
                );
            }
            ui.add_space(12.0);
            if crate::widgets::primary_button(ui, tl!("Done")).clicked() {
                done = true;
            }
        });
    if !open || done || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        app.ui.my_machine.open = false;
    }
}

/// A machine's name in the UI language (the brands stay as they are).
pub fn machine_name(name: &str) -> &str {
    match name {
        "Husqvarna Viking (other models)" => tl!("Husqvarna Viking (other models)"),
        "Brother or Baby Lock" => tl!("Brother or Baby Lock"),
        "Janome or Elna" => tl!("Janome or Elna"),
        "Another machine (DST)" => tl!("Another machine (DST)"),
        other => other,
    }
}
