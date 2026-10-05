//! File → Place in the app: picking the file, the Place dialog's options and the Control bar's
//! image details. The engine places (`file.place`); this gathers the file and options.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use vectorcraft_doc::NodeKind;
use vectorcraft_engine::cmd::fileio;

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// A file to place that arrived asynchronously (web): picked for File → Place.
pub struct PlaceArrival {
    pub name: String,
    pub bytes: Vec<u8>,
}

pub type PlaceInbox = Arc<Mutex<Vec<PlaceArrival>>>;

/// The app's Place state (not saved with the preferences).
#[derive(Default)]
pub struct PlaceState {
    /// Web: the bytes of the files picked for the Place dialog, by name.
    picked: Vec<(String, Vec<u8>)>,
    /// The Control bar's `image.info`, for (document uid, revision, image id).
    info: Option<((u64, u64, u64), Value)>,
}

/// Run engine command `id` itself (past the UI's handling of the same id); an error shows in the
/// status bar.
fn engine(app: &mut VectorcraftApp, id: &str, p: &Value) -> Result<Value, String> {
    let r = app.session.execute(id, p).map_err(|e| e.to_string());
    app.sync_views();
    if let Err(e) = &r {
        app.status(e.clone());
    }
    r
}

// ---------- File → Place… ----------

/// File → Place… (`file.place`): with a file (`path` or `name` + `dataBase64`), place it, centred
/// in the view unless `at`, `rect` or `replace` say otherwise; without, pick the file first.
pub fn run(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    if p.get("path").is_none() && p.get("dataBase64").is_none() {
        return pick(app).map(|_| Value::Null);
    }
    let mut p = p.clone();
    if ["at", "rect", "replace"].iter().all(|k| p.get(k).is_none())
        && let Some(v) = app.view()
    {
        p["at"] = json!([v.center.x, v.center.y]);
    }
    engine(app, "file.place", &p)
}

/// Pick the file to place: the Place dialog opens with it (on the web once it arrives).
fn pick(app: &mut VectorcraftApp) -> Result<(), String> {
    if let Some(f) = app.services.place_async.as_mut() {
        f();
        return Ok(());
    }
    let path = app.services.pick_open.as_mut().and_then(|f| f()).ok_or("cancelled")?;
    open_dialog(app, vec![json!({ "path": path })]);
    Ok(())
}

/// The Place dialog for `files` (`{path}`, or `{name}` with the bytes in [`PlaceState`]).
fn open_dialog(app: &mut VectorcraftApp, files: Vec<Value>) {
    let units = app.session.active().map(|d| d.doc.units).unwrap_or_default();
    let info: Vec<String> = files.iter().map(|f| summary(app, f, units)).collect();
    let one_object = app.session.active().is_some_and(|d| d.selection.objects.len() == 1);
    app.ui.dialog = Some(crate::state::Dialog::new(
        crate::dialogs::place::KIND,
        json!({
            "files": files,
            "link": app.ui.place_link,
            "template": false,
            "replace": false,
            "__replace": files.len() == 1 && one_object,
            "__info": info,
        }),
    ));
}

/// The engine params naming `file` (a dialog file entry).
fn source(app: &VectorcraftApp, file: &Value) -> Option<Value> {
    if let Some(path) = file.get("path").and_then(Value::as_str) {
        return Some(json!({ "path": path }));
    }
    let name = file.get("name").and_then(Value::as_str)?;
    let bytes = app.place.picked.iter().find(|(n, _)| n == name).map(|(_, b)| b)?;
    Some(json!({ "name": name, "dataBase64": vectorcraft_format::base64_encode(bytes) }))
}

/// One line about `file` for the Place dialog ("300 × 150 px, 300 ppi, RGB (1 in × 0.5 in)"), or
/// why it can't be placed.
fn summary(app: &mut VectorcraftApp, file: &Value, units: vectorcraft_doc::Unit) -> String {
    let Some(p) = source(app, file) else { return "not found".into() };
    let i = match app.session.execute("file.place.info", &p) {
        Ok(i) => i,
        Err(e) => return e.to_string(),
    };
    let size = format!("{} × {}", units.format(i["width"].as_f64().unwrap_or(0.0)), units.format(i["height"].as_f64().unwrap_or(0.0)));
    match (i["pixelWidth"].as_u64(), i["pixelHeight"].as_u64()) {
        (Some(w), Some(h)) => {
            let ppi = i["ppi"][0].as_f64().unwrap_or(72.0);
            format!("{w} × {h} px, {} ppi, {} ({size})", fmt_ppi(ppi), i["colorMode"].as_str().unwrap_or("RGB"))
        }
        _ => format!("{} ({size})", i["format"].as_str().unwrap_or_default().to_uppercase()),
    }
}

/// A resolution as the Control bar and the Place dialog show it ("300", "299.5").
fn fmt_ppi(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map(str::to_string).unwrap_or(s)
}

/// Place (the dialog's OK): the file is placed centred in the view (or replaces the selection).
pub fn confirm(app: &mut VectorcraftApp, d: &crate::state::Dialog) -> Result<Value, String> {
    let files: Vec<Value> = d.fields.get("files").and_then(Value::as_array).cloned().unwrap_or_default();
    let Some(mut p) = files.first().and_then(|f| source(app, f)) else { return Err("no file to place".into()) };
    let link = d.bool("link");
    app.ui.place_link = link;
    app.ui.dialog = None;
    p["link"] = json!(link);
    p["template"] = json!(d.bool("template"));
    if d.bool("replace") && d.bool("__replace") {
        p["replace"] = json!(true);
    }
    let r = run(app, &p);
    app.place.picked.clear();
    r
}

/// Files picked for File → Place that arrived (web): the Place dialog for them.
pub fn drain(app: &mut VectorcraftApp) {
    let arrived: Vec<PlaceArrival> =
        app.services.place_inbox.as_ref().map(|q| std::mem::take(&mut *q.lock().unwrap_or_else(|e| e.into_inner()))).unwrap_or_default();
    let mut picked = vec![];
    for a in arrived {
        picked.push(json!({ "name": a.name }));
        app.place.picked.retain(|(n, _)| *n != a.name);
        app.place.picked.push((a.name, a.bytes));
    }
    if !picked.is_empty() {
        open_dialog(app, picked);
    }
}

// ---------- the Control bar ----------

/// The Control bar's details for the one selected image: Linked File or Embedded, its file name,
/// colour mode and effective resolution (`image.info`, fetched again only when it changes).
pub fn control_bar_details(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    let Some(st) = app.session.active() else { return };
    let key = match &st.selection.objects[..] {
        [id] if st.doc.node(*id).is_some_and(|n| matches!(n.kind, NodeKind::Image(_))) => (st.uid, st.revision, id.0),
        _ => return,
    };
    if app.place.info.as_ref().is_none_or(|(k, _)| *k != key) {
        let info = app.session.execute("image.info", &json!({ "id": key.2 })).unwrap_or_default();
        app.place.info = Some((key, info));
    }
    let Some((_, i)) = &app.place.info else { return };
    let t = Tokens::get(ui.ctx());
    let name = i["link"].as_str().map(fileio::file_name).or_else(|| i["name"].as_str().map(str::to_string)).unwrap_or_default();
    ui.label(egui::RichText::new(name).size(12.0).color(t.text_strong));
    ui.separator();
    let ppi = i["ppi"].as_array().and_then(|a| a.first()).and_then(Value::as_f64).unwrap_or(0.0);
    ui.label(egui::RichText::new(format!("{}   PPI: {}", i["colorMode"].as_str().unwrap_or("RGB"), fmt_ppi(ppi))).size(12.0).color(t.text_dim));
    ui.separator();
}
