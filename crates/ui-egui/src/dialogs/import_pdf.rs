//! The Import PDF dialog: opening or placing a multi-page or password-protected PDF (or `.ai`)
//! asks which pages (a preview with page navigation), the box to crop to and the password.
//!
//! Fields: `mode` (`open` | `place`), `name`, `path?`, `pageCount`, `page` (the page shown, and the
//! one placed; 1-based), `allPages` and `range` (open), `cropTo` and `password`. `__locked` is set
//! until the password is accepted: OK then unlocks the file. `__place` keeps the other `file.place`
//! params (Link, Template, `at`…). The file's bytes stay out of the fields, in
//! [`crate::state::UiState::dialog_file`].

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use vectorcraft_engine::cmd::fileio;
use vectorcraft_pdf::{Choice, CropTo, PdfError};

use super::DialogSpec;
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::{VectorcraftApp, io, widgets};

pub(super) const KIND: &str = "importPdf";

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |d| if d.str("mode") == "place" { "Place PDF" } else { "Import PDF" }.into(), body, confirm, ..DialogSpec::FORM };

/// The longest side of the page preview (pt).
const PREVIEW: f32 = 200.0;
const LABEL_WIDTH: f32 = 70.0;

/// The file the dialog reads.
#[derive(Debug)]
pub struct DialogFile {
    pub bytes: Vec<u8>,
    /// Unique per file the dialog opened (keys its preview cache).
    token: u64,
}

/// Open `bytes` (or place them with the `file.place` params `place`) through the dialog when the
/// file is a PDF (or `.ai`/`.ait`) with several pages or a password; false when there's nothing to
/// ask (the caller goes on).
pub fn offer(app: &mut VectorcraftApp, name: &str, bytes: &[u8], path: Option<String>, place: Option<&Value>) -> bool {
    if !fileio::detect(name, bytes).is_some_and(|f| matches!(f.id, "pdf" | "ai" | "ait")) {
        return false;
    }
    let (pages, locked) = match vectorcraft_pdf::info(bytes, None) {
        Ok(i) if i.pages.len() > 1 => (i.pages.len(), false),
        Err(PdfError::NeedsPassword) => (0, true),
        _ => return false,
    };
    static TOKEN: AtomicU64 = AtomicU64::new(0);
    app.ui.dialog_file = Some(Arc::new(DialogFile { bytes: bytes.to_vec(), token: TOKEN.fetch_add(1, Ordering::Relaxed) }));
    let mut fields = json!({
        "mode": if place.is_some() { "place" } else { "open" },
        "name": name,
        "pageCount": pages,
        "page": 1,
        "allPages": true,
        "range": "",
        "cropTo": CropTo::default().id(),
        "password": "",
        "__locked": locked,
    });
    if let Some(p) = path {
        fields["path"] = json!(p);
    }
    if let Some(Value::Object(p)) = place {
        let rest: serde_json::Map<String, Value> = p
            .iter()
            .filter(|(k, _)| !matches!(k.as_str(), "path" | "name" | "dataBase64" | "page" | "crop" | "password"))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        fields["__place"] = Value::Object(rest);
    }
    app.ui.dialog = Some(Dialog::new(KIND, fields));
    true
}

/// `file.place` with params `p` asks first (true) when they name a PDF (`.pdf`, `.ai`, `.ait`)
/// with several pages or a password and no `page`.
pub fn offer_place(app: &mut VectorcraftApp, p: &Value) -> bool {
    if p.get("page").is_some() {
        return false;
    }
    let s = |k: &str| p.get(k).and_then(Value::as_str);
    let (name, bytes, path) = match (s("path"), s("dataBase64")) {
        (Some(path), _) if matches!(fileio::extension(path).as_str(), "pdf" | "ai" | "ait") => {
            let Some(bytes) = app.services.read.as_ref().and_then(|r| r(path).ok()) else { return false };
            (path.to_string(), bytes, Some(path.to_string()))
        }
        (None, Some(b64)) => match vectorcraft_format::base64_decode(b64) {
            Some(bytes) => (s("name").unwrap_or("Untitled").to_string(), bytes, None),
            None => return false,
        },
        _ => return false,
    };
    offer(app, &name, &bytes, path, Some(p))
}

fn page_count(d: &Dialog) -> usize {
    d.fields.get("pageCount").and_then(Value::as_u64).unwrap_or(0) as usize
}

/// The page shown (1-based, within the file).
fn page(d: &Dialog) -> usize {
    (d.f64("page", 1.0).max(1.0) as usize).min(page_count(d).max(1))
}

fn crop(d: &Dialog) -> CropTo {
    CropTo::IDS.iter().position(|id| *id == d.str("cropTo")).and_then(|i| CropTo::ALL.get(i).copied()).unwrap_or_default()
}

fn password(d: &Dialog) -> Option<String> {
    Some(d.str("password")).filter(|p| !p.is_empty())
}

/// OK: unlock the file with the password, else open the pages or place the page.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let file = app.ui.dialog_file.clone().ok_or("the PDF is no longer loaded")?;
    let mut d = d.clone();
    if d.bool("__locked") {
        let info = vectorcraft_pdf::info(&file.bytes, password(&d).as_deref()).map_err(|e| e.to_string())?;
        d.fields.insert("__locked".into(), json!(false));
        d.fields.insert("pageCount".into(), json!(info.pages.len()));
        if info.pages.len() > 1 {
            // Unlocked: now pick the pages.
            app.ui.dialog = Some(d);
            return Ok(Value::Null);
        }
    }
    let name = d.str("name");
    let path = d.fields.get("path").and_then(Value::as_str).map(str::to_string);
    let r = if d.str("mode") == "place" {
        let mut p = d.fields.get("__place").cloned().filter(Value::is_object).unwrap_or_else(|| json!({}));
        match &path {
            Some(path) => p["path"] = json!(path),
            None => {
                p["name"] = json!(name);
                p["dataBase64"] = json!(vectorcraft_format::base64_encode(&file.bytes));
            }
        }
        p["page"] = json!(page(&d));
        p["crop"] = json!(crop(&d).id());
        if let Some(pw) = password(&d) {
            p["password"] = json!(pw);
        }
        crate::place::run(app, &p).map(|_| ())
    } else {
        let mut p = json!({"cropTo": crop(&d).id(), "password": password(&d)});
        let range = d.str("range");
        if !d.bool("allPages") && !range.trim().is_empty() {
            p["pages"] = json!(range);
        }
        io::open_document(app, &name, &file.bytes, path, &p)
    };
    match r {
        Ok(()) => {
            app.ui.dialog = None;
            Ok(Value::Null)
        }
        Err(e) => {
            // Keep what was typed (the dialog stays open on errors such as a bad range).
            app.ui.dialog = Some(d);
            Err(e)
        }
    }
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let t = Tokens::get(ui.ctx());
    if d.bool("__locked") {
        let name = d.str("name");
        let file = std::path::Path::new(&name).file_name().map_or(name.clone(), |f| f.to_string_lossy().into_owned());
        ui.label(egui::RichText::new(format!("“{file}” is protected by a password.")).color(t.text));
        ui.add_space(8.0);
        password_row(ui, d);
        return false;
    }
    let count = page_count(d);
    let mut current = page(d);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            let (r, _) = ui.allocate_exact_size(egui::vec2(PREVIEW, PREVIEW), egui::Sense::hover());
            ui.painter().rect_filled(r, 2.0, t.pasteboard);
            if let Some(tex) = preview(app, ui.ctx(), d, current) {
                let sz = tex.size_vec2();
                let ir = egui::Rect::from_center_size(r.center(), sz * (PREVIEW / sz.x.max(sz.y)));
                ui.painter().image(tex.id(), ir, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                ui.painter().rect_stroke(ir, 0.0, egui::Stroke::new(1.0, t.divider), egui::StrokeKind::Outside);
            }
            ui.add_space(6.0);
            // Page navigation: first, previous, the page number, next, last.
            ui.horizontal(|ui| {
                let nav = |ui: &mut egui::Ui, icon: &str, tip: &str, to: usize, enabled: bool| {
                    widgets::icon_button_enabled(ui, icon, tip, false, enabled, 22.0).clicked().then_some(to)
                };
                let go = [
                    nav(ui, "chevrons-left", "First Page", 1, current > 1),
                    nav(ui, "chevron-left", "Previous Page", current.saturating_sub(1).max(1), current > 1),
                ];
                if let Some(v) = widgets::plain_field(ui, "pdf-page", current as f64, "", 0, 40.0) {
                    current = (v.max(1.0) as usize).min(count.max(1));
                }
                ui.label(egui::RichText::new(format!("of {count}")).color(t.text_dim));
                let go2 = [
                    nav(ui, "chevron-right", "Next Page", (current + 1).min(count), current < count),
                    nav(ui, "chevrons-right", "Last Page", count, current < count),
                ];
                if let Some(to) = go.into_iter().chain(go2).flatten().next() {
                    current = to;
                }
            });
        });
        ui.add_space(14.0);
        ui.vertical(|ui| {
            widgets::label_row(ui, "Crop To", LABEL_WIDTH, |ui| {
                if let Some(i) = widgets::dropdown(ui, "pdf-crop", crop(d).label(), CropTo::LABELS, 130.0)
                    && let Some(id) = CropTo::IDS.get(i)
                {
                    d.fields.insert("cropTo".into(), json!(id));
                }
            });
            if d.str("mode") != "place" {
                ui.add_space(10.0);
                ui.label(egui::RichText::new("Pages").color(t.text));
                let all = d.bool("allPages");
                if widgets::radio(ui, "All", all, true) {
                    d.fields.insert("allPages".into(), json!(true));
                }
                ui.horizontal(|ui| {
                    if widgets::radio(ui, "Range:", !all, true) {
                        d.fields.insert("allPages".into(), json!(false));
                    }
                    if super::form::text_edit(ui, d, "range", 90.0).gained_focus() {
                        d.fields.insert("allPages".into(), json!(false));
                    }
                });
                widgets::dim_label(ui, "e.g. 1-3, 5");
            }
        });
    });
    d.fields.insert("page".into(), json!(current));
    false
}

fn password_row(ui: &mut egui::Ui, d: &mut Dialog) {
    widgets::label_row(ui, "Password", LABEL_WIDTH, |ui| {
        let mut s = d.str("password");
        let r = ui.add(egui::TextEdit::singleline(&mut s).password(true).desired_width(180.0));
        if r.changed() {
            d.fields.insert("password".into(), json!(s));
        }
        if ui.memory(|m| m.focused().is_none()) {
            r.request_focus();
        }
    });
}

/// Page `page` (1-based) cropped as the dialog says, rendered once per file, page and box.
fn preview(app: &mut VectorcraftApp, ctx: &egui::Context, d: &Dialog, page: usize) -> Option<egui::TextureHandle> {
    let file = app.ui.dialog_file.clone()?;
    let crop = crop(d);
    let key = egui::Id::new(("pdf-preview", file.token, page, crop.id()));
    if let Some(t) = ctx.data(|m| m.get_temp::<Option<egui::TextureHandle>>(key)) {
        return t;
    }
    let opts = fileio::LoadOptions { crop, password: password(d), ..Default::default() };
    // A page that can't be read shows an empty preview (and isn't read again).
    let tex = fileio::page_document(&file.bytes, page - 1, &opts).ok().and_then(|(doc, _)| {
        let r = doc.artboards.first()?.rect;
        let px = PREVIEW as f64 * ctx.pixels_per_point() as f64;
        Some(widgets::region_texture(ctx, &mut app.canvas.renderer, &format!("pdf-preview-{page}"), &doc, r, px))
    });
    ctx.data_mut(|m| m.insert_temp(key, tex.clone()));
    tex
}
