//! File → Export → Export for Screens: artboards (thumbnails and checkboxes), scale/format rows,
//! the destination folder (typed or picked; the web downloads instead) and file-name prefix.

use serde_json::{Value, json};
use vectorcraft_engine::cmd::fileio;

use super::DialogSpec;
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, io, widgets};

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |_| "Export for Screens".into(),
    body,
    confirm,
    ok: Some("Export Artboard"),
    ok_label: Some(|app| if io::is_web(app) { "Download" } else { "Export Artboard" }),
    ..DialogSpec::FORM
};

/// `Dialog::kind` of this dialog.
pub const KIND: &str = "exportForScreens";

/// Open the dialog: every artboard, PNG at 1x and 2x, into the Desktop (else the home folder).
pub fn open(app: &mut VectorcraftApp) {
    let n = app.session.active().map_or(0, |d| d.doc.artboards.len());
    app.ui.dialog = Some(Dialog::new(
        KIND,
        json!({"boards": vec![true; n], "formats": [{"format": "png", "scale": 1, "suffix": ""}, {"format": "png", "scale": 2, "suffix": "@2x"}], "folder": fileio::export_folder().unwrap_or_default(), "prefix": ""}),
    ));
}

/// Export the checked artboards (the web: no folder, one ZIP for several files); the dialog stays
/// open when the export fails.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let boards: Vec<usize> = d
        .fields
        .get("boards")
        .and_then(Value::as_array)
        .map(|a| a.iter().enumerate().filter(|(_, b)| b.as_bool() == Some(true)).map(|(i, _)| i).collect())
        .unwrap_or_default();
    let formats = d.fields.get("formats").cloned().unwrap_or(json!([]));
    let files = boards.len() * formats.as_array().map_or(0, Vec::len);
    let mut params = json!({"artboards": boards, "formats": formats, "prefix": d.str("prefix")});
    if io::is_web(app) {
        params["zip"] = json!(files > 1);
    } else {
        let folder = d.str("folder");
        if folder.trim().is_empty() {
            return Err("choose a folder to export to".into());
        }
        params["folder"] = json!(folder.trim());
    }
    let r = io::export_for_screens(app, params);
    if r.is_ok() {
        app.ui.dialog = None;
    }
    r
}

/// Export for Screens: artboard picker (thumbnails + checkboxes), format/scale rows, destination.
fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
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
            if io::is_web(app) {
                widgets::dim_label(ui, "Your browser downloads the files (several as one .zip).");
            } else {
                let pick = app.services.pick_folder.is_some();
                ui.horizontal(|ui| {
                    let mut folder = d.str("folder");
                    if ui.add(egui::TextEdit::singleline(&mut folder).desired_width(if pick { 290.0 } else { 320.0 })).changed() {
                        d.fields.insert("folder".into(), json!(folder));
                    }
                    if pick
                        && widgets::icon_button(ui, "folder-open", "Choose a folder", false, 26.0).clicked()
                        && let Some(f) = app.services.pick_folder.as_mut().and_then(|pick| pick())
                    {
                        d.fields.insert("folder".into(), json!(f));
                    }
                });
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
                        for fm in ["png", "jpg", "webp", "gif", "png8", "svg", "pdf"] {
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
    false
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
    let tex = crate::widgets::region_texture(ctx, &mut app.canvas.renderer, &format!("ab-thumb-{i}"), &doc, r, 92.0);
    ctx.data_mut(|d| d.insert_temp(key, tex.clone()));
    Some(tex)
}
