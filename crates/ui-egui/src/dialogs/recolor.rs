//! Edit → Edit Colors → Recolor Artwork: the selection's colours in rows of current → new colours,
//! previewed live (engine: `recolor.reduce`, `recolor.randomize`, `recolor.apply`).
//!
//! Assign tab: Colors (Auto, or a count: similar colours merge into rows), Preserve White, Black and
//! Grays, the rows (click to select, Shift or Cmd/Ctrl-click to add; a row's arrow excludes or
//! includes it; the hex field edits its new colour), Merge and Separate, Randomize order or
//! saturation and brightness, Reset, the Method and Limit to Library. Edit tab: a hue and
//! saturation wheel with a marker per new colour (drag one; with linked handles they all turn and
//! scale together), a brightness slider and the harmony rules. The preset menu sets up a 1, 2 or 3
//! colour job or a library. Opened on a colour group (Edit or Apply Color Group), OK also rewrites
//! the group with the new colours (renamed by `groupName`); without art only the group changes.
//! Opened on colours alone (the Color Guide's Edit or Apply Colors without art), OK saves the new
//! colours as a colour group named `groupName`.
//!
//! Fields (`ui.dialog.set`): `rows` ([{from: [keys], to: key, exclude?}]), `colors` (null: Auto, or a
//! count), `method`, `preserveWhite`, `preserveBlack`, `preserveGrays`, `limitTo` (library id,
//! "document" for the document's swatches, "" for none), `group`, `groupName`, `tab` (`assign` or
//! `edit`), `rule` (harmony id), `linked`, `preview`. Changing `colors`, a preserve flag or
//! `limitTo` reduces the rows again.

use egui::{Pos2, Rect, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};
use vectorcraft_color::harmony::Harmony;
use vectorcraft_color::recolor::{ColorKey, Method};
use vectorcraft_color::{Color, keep_model};
use vectorcraft_engine::cmd::color_value;
use vectorcraft_engine::cmd::swatchlib;

use super::{DialogSpec, form};
use crate::panels::c32;
use crate::panels::swatches::{colour_libraries, limit_key, limit_name};
use crate::state::Dialog;
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, widgets};

/// The dialog kind of Recolor Artwork.
pub const KIND: &str = "recolor";

pub(super) const SPEC: DialogSpec =
    DialogSpec { heading: |_| "Recolor Artwork".into(), body, confirm, preview: true, min_width: 540.0, max_width: Some(560.0), ..DialogSpec::FORM };

const CMD: &str = "recolor.apply";
/// Width of the current colours of a row.
const CURRENT_W: f32 = 250.0;
/// Diameter of the Edit tab's colour wheel.
const WHEEL: f32 = 220.0;
/// Height of the rows table, and of either tab (so switching tabs doesn't resize the dialog).
const TABLE_H: f32 = 220.0;
const TAB_H: f32 = 400.0;
const PRESETS: [&str; 5] = ["Custom", "1 Color Job", "2 Color Job", "3 Color Job", "Color Library"];

/// Open Recolor Artwork (`ui.recolorDialog {colors?, library?, group?}`) on the selected art and,
/// with `group`, on that colour group: art is reduced to as many rows as the group has colours,
/// which become their new colours; without art the rows are the group's colours. `colors` is a
/// count (an n-colour job) or the new colours to assign (without art or a group they are the
/// rows, which OK saves as a new colour group); `library` limits to a library or "document" (""
/// picks the first library).
pub fn open(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document open")?;
    let art = !st.selection.is_empty();
    let group = p.get("group").and_then(Value::as_str);
    let group_colors: Option<Vec<Color>> = match group {
        Some(g) => {
            let g = st.doc.swatch_groups.iter().find(|x| x.name == g).ok_or_else(|| format!("no colour group `{g}`"))?;
            Some(g.swatches.iter().filter_map(|w| w.paint.color()).collect())
        }
        None => None,
    };
    let assign: Vec<Value> = match (&group_colors, p.get("colors")) {
        (Some(cs), _) => cs.iter().map(key).collect(),
        (None, Some(Value::Array(cs))) => cs.iter().filter_map(color_value).map(|c| key(&c)).collect(),
        _ => vec![],
    };
    // Colours alone (no art, no group) become a new colour group.
    let new_group = !art && group.is_none();
    if new_group && assign.is_empty() {
        return Err("select artwork (or a colour group) to recolor".into());
    }
    let count = match p.get("colors").and_then(Value::as_u64) {
        Some(n) => json!(n.max(1)),
        None if !assign.is_empty() => json!(assign.len()),
        None => Value::Null,
    };
    let limit = match p.get("library").and_then(Value::as_str) {
        Some("") => first_library(app).unwrap_or_default(),
        Some(l) => limit_key(app, l)?,
        None => String::new(),
    };
    // Without art the rows are the group's colours, as they are.
    let start: Vec<Value> = if art { vec![] } else { assign.iter().map(|k| json!({"from": [k], "to": k})).collect() };
    let fields = json!({
        "rows": start, "colors": count, "method": Method::ScaleTints.id(),
        "preserveWhite": true, "preserveBlack": true, "preserveGrays": false, "limitTo": limit,
        "group": group, "groupName": if new_group { Some(NEW_GROUP) } else { group }, "tab": "assign", "rule": Harmony::Complementary.id(), "linked": true,
        "preview": true, "__art": art, "__assign": assign, "__newGroup": new_group,
    });
    let mut d = Dialog::new(KIND, fields);
    // Without art the rows are there from the start: limit them now (art's rows snap as they reduce).
    let mut rs = rows(&d);
    snap(app, &d, &mut rs);
    d.fields.insert("rows".into(), Value::Array(rs));
    app.ui.dialog = Some(d);
    Ok(Value::Null)
}

fn key(c: &Color) -> Value {
    json!(ColorKey::of(c).to_string())
}

/// The name a new colour group (opened on colours alone) starts with.
const NEW_GROUP: &str = "Color Group";

fn first_library(app: &VectorcraftApp) -> Option<String> {
    colour_libraries(app).into_iter().next().map(|l| l.id)
}

/// `recolor.reduce` parameters (Method only picks each row's colour, so changing it keeps the rows).
fn reduce_params(d: &Dialog) -> Value {
    json!({
        "colors": d.fields.get("colors").cloned().unwrap_or(Value::Null),
        "preserve": {"white": d.bool("preserveWhite"), "black": d.bool("preserveBlack"), "grays": d.bool("preserveGrays")},
        "limitTo": d.str("limitTo"),
    })
}

/// Reduce the art's colours into rows again when the count, preserve flags or library changed
/// (assigning a group's colours or the opener's colours as the new colours).
fn sync(app: &mut VectorcraftApp, d: &mut Dialog) {
    const DONE: &str = "__reduced";
    let sig = reduce_params(d);
    if !d.bool("__art") || d.fields.get(DONE) == Some(&sig) {
        return;
    }
    let mut p = sig.clone();
    p["method"] = json!(d.str("method"));
    match app.session.execute("recolor.reduce", &p) {
        Ok(r) => {
            let mut rows = r["map"].as_array().cloned().unwrap_or_default();
            let assign = d.fields.get("__assign").and_then(Value::as_array).cloned().unwrap_or_default();
            for (row, to) in rows.iter_mut().zip(assign) {
                row["to"] = to;
            }
            snap(app, d, &mut rows);
            d.fields.insert("rows".into(), Value::Array(rows));
            d.fields.insert("__sel".into(), json!([]));
        }
        Err(e) => app.status(e.to_string()),
    }
    d.fields.insert(DONE.into(), sig);
}

/// With Limit to Library, the rows' new colours snap to the library's nearest colours.
fn snap(app: &VectorcraftApp, d: &Dialog, rows: &mut [Value]) {
    let Some(palette) = Some(d.str("limitTo")).filter(|l| !l.is_empty()).and_then(|l| swatchlib::limit_palette(&app.session, &l)) else { return };
    for r in rows {
        if let Some(c) = to_color(r) {
            r["to"] = key(&palette.nearest(c));
        }
    }
}

fn rows(d: &Dialog) -> Vec<Value> {
    d.fields.get("rows").and_then(Value::as_array).cloned().unwrap_or_default()
}

fn from_colors(row: &Value) -> Vec<Color> {
    row["from"].as_array().map(|a| a.iter().filter_map(color_value).collect()).unwrap_or_default()
}

fn to_color(row: &Value) -> Option<Color> {
    color_value(&row["to"])
}

fn excluded(row: &Value) -> bool {
    row["exclude"].as_bool().unwrap_or(false)
}

fn selection(d: &Dialog) -> Vec<usize> {
    d.fields.get("__sel").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|i| i as usize).collect()).unwrap_or_default()
}

/// `recolor.apply` parameters: the rows, method, library and the group with its new name and
/// colours: the rows' new colours, then (with art) the group's colours no row took.
fn apply_params(d: &Dialog) -> Value {
    let rs = rows(d);
    let group = d.fields.get("group").and_then(Value::as_str).map(|g| {
        let rest =
            d.fields.get("__assign").and_then(Value::as_array).filter(|_| d.bool("__art")).map_or(&[][..], |a| a.get(rs.len()..).unwrap_or_default());
        let colors: Vec<&Value> = rs.iter().map(|r| &r["to"]).chain(rest).collect();
        let name = d.str("groupName");
        let rename = (!name.trim().is_empty() && name != g).then_some(name);
        json!({"group": g, "groupColors": colors, "rename": rename})
    });
    let mut p = json!({"map": rs, "method": d.str("method"), "limitTo": d.str("limitTo")});
    if let (Some(Value::Object(g)), Some(p)) = (group, p.as_object_mut()) {
        p.extend(g.into_iter().filter(|(_, v)| !v.is_null()));
    }
    p
}

fn body(app: &mut VectorcraftApp, ui: &mut Ui, d: &mut Dialog) -> bool {
    sync(app, d);
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        let tab = d.str("tab");
        for (id, label) in [("assign", "Assign"), ("edit", "Edit")] {
            if ui.selectable_label(tab == id || (id == "assign" && tab != "edit"), egui::RichText::new(label).font(theme::semibold(12.5))).clicked() {
                d.fields.insert("tab".into(), json!(id));
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if let Some(i) = widgets::dropdown(ui, "recolor-preset", PRESETS[preset_of(d)], &PRESETS, 130.0) {
                set_preset(app, d, i);
            }
            ui.label(egui::RichText::new("Preset:").color(t.text_dim));
        });
    });
    widgets::divider(ui);
    ui.add_space(4.0);
    let mut rs = rows(d);
    let top = ui.cursor().top();
    let changed = if d.str("tab") == "edit" { edit_tab(ui, d, &mut rs) } else { assign_tab(app, ui, d, &mut rs) };
    ui.add_space((TAB_H - (ui.cursor().top() - top)).max(0.0));
    if changed {
        snap(app, d, &mut rs);
        d.fields.insert("rows".into(), Value::Array(rs));
    }
    if d.fields.get("group").is_some_and(Value::is_string) || d.bool("__newGroup") {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Color Group:").color(t.text_dim));
            form::text(ui, d, "groupName", 200.0);
        });
    }
    sync(app, d);
    if d.bool("__art") {
        form::preview(app, ui, d, "Recolor Artwork", CMD, apply_params(d));
    }
    false
}

/// OK: keep the previewed recolour as one undo step (or apply it, e.g. to a colour group only; or
/// save colours opened alone as a new colour group).
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let mut d = d.clone();
    sync(app, &mut d);
    if d.bool("__newGroup") {
        let colors: Vec<Value> = rows(&d).into_iter().filter(|r| !excluded(r)).map(|r| r["to"].clone()).collect();
        return form::commit_preview(app, "swatch.newGroup", json!({"name": d.str("groupName"), "colors": colors}));
    }
    form::commit_preview(app, CMD, apply_params(&d))
}

/// The preset the fields match: an n-colour job (n rows, Scale Tints), a library, or Custom.
fn preset_of(d: &Dialog) -> usize {
    match d.fields.get("colors").and_then(Value::as_u64) {
        Some(n @ 1..=3) if d.str("method") == Method::ScaleTints.id() => n as usize,
        _ if !d.str("limitTo").is_empty() => 4,
        _ => 0,
    }
}

fn set_preset(app: &VectorcraftApp, d: &mut Dialog, i: usize) {
    match i {
        1..=3 => {
            d.fields.insert("colors".into(), json!(i));
            d.fields.insert("method".into(), json!(Method::ScaleTints.id()));
        }
        4 if d.str("limitTo").is_empty() => {
            d.fields.insert("limitTo".into(), json!(first_library(app).unwrap_or_default()));
        }
        _ => {}
    }
}

/// A colour chip with a hairline frame.
fn chip(ui: &Ui, r: Rect, c: Color, dim: bool) {
    let t = Tokens::get(ui.ctx());
    let fill = c32(&c);
    ui.painter().rect_filled(r, 1.0, if dim { fill.gamma_multiply(0.35) } else { fill });
    ui.painter().rect_stroke(r, 1.0, egui::Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
}

/// The Assign tab. Returns true when the rows changed.
fn assign_tab(app: &mut VectorcraftApp, ui: &mut Ui, d: &mut Dialog, rows: &mut Vec<Value>) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut changed = false;
    let total: usize = rows.iter().map(|r| r["from"].as_array().map_or(0, Vec::len)).sum();
    // Reduction and the preserve rules apply to art (a colour group alone has a row per colour).
    if d.bool("__art") {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Colors:").color(t.text_dim));
            let labels: Vec<String> = std::iter::once("Auto".to_string()).chain((1..=total).map(|n| n.to_string())).collect();
            let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
            let cur = d.fields.get("colors").and_then(Value::as_u64).map_or("Auto".to_string(), |n| n.to_string());
            if let Some(i) = widgets::dropdown(ui, "recolor-count", &cur, &refs, 70.0) {
                d.fields.insert("colors".into(), if i == 0 { Value::Null } else { json!(i) });
            }
            ui.add_space(18.0);
            ui.label(egui::RichText::new("Preserve:").color(t.text_dim));
            for (k, label) in [("preserveWhite", "White"), ("preserveBlack", "Black"), ("preserveGrays", "Grays")] {
                let on = d.bool(k);
                if widgets::check(ui, label, on, true) {
                    d.fields.insert(k.into(), json!(!on));
                }
            }
        });
        ui.add_space(6.0);
    }
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        ui.add_sized([CURRENT_W, 16.0], egui::Label::new(egui::RichText::new(format!("Current Colors ({total})")).color(t.text_dim)));
        ui.add_space(60.0);
        ui.label(egui::RichText::new("New").color(t.text_dim));
    });
    let mut sel = selection(d);
    let mut clicked: Option<usize> = None;
    widgets::list_box(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("recolor-rows").max_height(TABLE_H).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.set_min_height(TABLE_H);
            if rows.is_empty() {
                ui.add_space(8.0);
                widgets::dim_label(ui, "No colors to recolor (preserved colors are left out).");
            }
            for (i, row) in rows.iter_mut().enumerate() {
                let bg = ui.painter().add(egui::Shape::Noop);
                let off = excluded(row);
                let r = ui.horizontal(|ui| {
                    ui.add_space(4.0);
                    let from = from_colors(row);
                    let (cur, cr) = ui.allocate_exact_size(vec2(CURRENT_W, 20.0), Sense::click());
                    let w = CURRENT_W / from.len().max(1) as f32;
                    for (j, c) in from.iter().enumerate() {
                        let x = cur.left() + j as f32 * w;
                        chip(ui, Rect::from_min_max(pos2(x, cur.top()), pos2(x + w - if w > 4.0 { 2.0 } else { 0.0 }, cur.bottom())), *c, false);
                    }
                    if cr.clicked() {
                        clicked = Some(i);
                    }
                    let (ar, aresp) = ui.allocate_exact_size(vec2(44.0, 20.0), Sense::click());
                    arrow(ui, ar, !off, aresp.hovered());
                    if aresp.on_hover_text(if off { "Recolor this row" } else { "Keep this row's colors (exclude)" }).clicked() {
                        row["exclude"] = json!(!off);
                        changed = true;
                    }
                    let (nr, nresp) = ui.allocate_exact_size(vec2(46.0, 20.0), Sense::click());
                    let to = to_color(row).unwrap_or_default();
                    chip(ui, nr, to, off);
                    if nresp.clicked() {
                        clicked = Some(i);
                    }
                    ui.add_space(6.0);
                    let hex = crate::panels::color::hex_digits(&to);
                    if let Some(h) = widgets::hex_field(ui, ("recolor-hex", i), &hex)
                        && let Some(c) = crate::panels::color::parse_hex(&h)
                    {
                        row["to"] = key(&keep_model(to, c));
                        changed = true;
                    }
                });
                if sel.contains(&i) {
                    ui.painter().set(bg, egui::Shape::rect_filled(r.response.rect.expand2(vec2(2.0, 1.0)), 2.0, t.row_selected));
                }
            }
        });
    });
    if let Some(i) = clicked {
        let m = ui.input(|i| i.modifiers);
        match sel.iter().position(|s| *s == i) {
            Some(p) if m.shift || m.command => {
                sel.remove(p);
            }
            None if m.shift || m.command => sel.push(i),
            _ => sel = vec![i],
        }
        d.fields.insert("__sel".into(), json!(sel));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let can_merge = sel.len() > 1;
        if widgets::icon_button_enabled(ui, "combine", "Merge colors into one row", false, can_merge, 24.0).clicked() {
            merge(rows, &sel);
            d.fields.insert("__sel".into(), json!([sel.iter().min()]));
            changed = true;
        }
        let can_split = sel.iter().any(|i| rows.get(*i).is_some_and(|r| r["from"].as_array().is_some_and(|a| a.len() > 1)));
        if widgets::icon_button_enabled(ui, "ungroup", "Separate colors into rows", false, can_split, 24.0).clicked() {
            separate(rows, &sel);
            d.fields.insert("__sel".into(), json!([]));
            changed = true;
        }
        ui.add_space(12.0);
        for (icon, tip, sb) in
            [("arrow-left-right", "Randomly change color order", false), ("sparkles", "Randomly change saturation and brightness", true)]
        {
            if widgets::icon_button_enabled(ui, icon, tip, false, rows.len() > 1 || sb, 24.0).clicked() {
                let seed = d.f64("__seed", 0.0) as u64 + 1;
                d.fields.insert("__seed".into(), json!(seed));
                let p = json!({"map": rows, "order": !sb, "saturationBrightness": sb, "seed": seed});
                match app.session.execute("recolor.randomize", &p) {
                    Ok(r) => {
                        *rows = r["map"].as_array().cloned().unwrap_or_default();
                        changed = true;
                    }
                    Err(e) => app.status(e.to_string()),
                }
            }
        }
        if widgets::icon_button(ui, "rotate-ccw", "Reset the rows to the artwork's colors", false, 24.0).clicked() {
            d.fields.remove("__reduced");
        }
    });
    ui.add_space(8.0);
    egui::Grid::new("recolor-options").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
        ui.label(egui::RichText::new("Method:").color(t.text_dim));
        let m = Method::parse(&d.str("method")).unwrap_or_default();
        let labels: Vec<&str> = Method::ALL.iter().map(|m| m.label()).collect();
        if let Some(i) = widgets::dropdown(ui, "recolor-method", m.label(), &labels, 180.0) {
            d.fields.insert("method".into(), json!(Method::ALL[i].id()));
        }
        ui.end_row();
        ui.label(egui::RichText::new("Limit to Library:").color(t.text_dim));
        // None, the document's swatches, then the colour libraries.
        let libs = colour_libraries(app);
        let keys: Vec<&str> = ["", swatchlib::DOCUMENT_SWATCHES].into_iter().chain(libs.iter().map(|l| l.id.as_str())).collect();
        let labels: Vec<String> = keys.iter().map(|k| limit_name(app, k).unwrap_or_else(|| "None".into())).collect();
        let cur = d.str("limitTo");
        let name = keys.iter().position(|k| *k == cur).map_or("None", |i| labels[i].as_str());
        let labels: Vec<&str> = labels.iter().map(String::as_str).collect();
        if let Some(i) = widgets::dropdown(ui, "recolor-library", name, &labels, 180.0) {
            d.fields.insert("limitTo".into(), json!(keys[i]));
        }
        ui.end_row();
    });
    changed
}

/// The arrow between a row's current and new colours: solid when the row is recoloured, a dashed
/// line without a head when it is excluded.
fn arrow(ui: &Ui, r: Rect, on: bool, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    let c = if hovered {
        t.text_strong
    } else if on {
        t.text
    } else {
        t.text_disabled
    };
    let (a, b) = (pos2(r.left() + 6.0, r.center().y), pos2(r.right() - 6.0, r.center().y));
    let stroke = egui::Stroke::new(1.2, c);
    if on {
        ui.painter().line_segment([a, b], stroke);
        ui.painter().line_segment([b, b + vec2(-5.0, -4.0)], stroke);
        ui.painter().line_segment([b, b + vec2(-5.0, 4.0)], stroke);
    } else {
        ui.painter().extend(egui::Shape::dashed_line(&[a, b], stroke, 3.0, 3.0));
    }
}

/// Merge the selected rows into the first of them (its new colour stays).
fn merge(rows: &mut Vec<Value>, sel: &[usize]) {
    let mut sel: Vec<usize> = sel.iter().copied().filter(|i| *i < rows.len()).collect();
    sel.sort_unstable();
    let Some((&first, rest)) = sel.split_first() else { return };
    let mut moved = vec![];
    for &i in rest.iter().rev() {
        let row = rows.remove(i);
        moved.splice(0..0, row["from"].as_array().cloned().unwrap_or_default());
    }
    if let Some(dst) = rows[first]["from"].as_array_mut() {
        dst.extend(moved);
    }
}

/// Split the selected rows into one row per colour, each its own new colour.
fn separate(rows: &mut Vec<Value>, sel: &[usize]) {
    let mut out = vec![];
    for (i, row) in rows.drain(..).enumerate() {
        match row["from"].as_array() {
            Some(a) if sel.contains(&i) && a.len() > 1 => out.extend(a.iter().map(|k| json!({"from": [k], "to": k}))),
            _ => out.push(row),
        }
    }
    *rows = out;
}

/// Position of hue `h` (degrees) and saturation `s` on a wheel of radius `r` around `c`.
fn wheel_pos(c: Pos2, r: f32, h: f32, s: f32) -> Pos2 {
    let a = h.to_radians();
    c + vec2(a.cos(), -a.sin()) * r * s
}

/// The hue and saturation wheel at brightness `v`: hue around, saturation outwards.
fn wheel_shape(c: Pos2, r: f32, v: f32) -> egui::Shape {
    const SEG: u32 = 72;
    const RINGS: u32 = 8;
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(c, c32(&Color::from_hsb(0.0, 0.0, v)));
    for ring in 1..=RINGS {
        let s = ring as f32 / RINGS as f32;
        for k in 0..SEG {
            let h = k as f32 * 360.0 / SEG as f32;
            mesh.colored_vertex(wheel_pos(c, r, h, s), c32(&Color::from_hsb(h, s, v)));
        }
    }
    let at = |ring: u32, k: u32| if ring == 0 { 0 } else { 1 + (ring - 1) * SEG + k % SEG };
    for ring in 0..RINGS {
        for k in 0..SEG {
            if ring == 0 {
                mesh.add_triangle(0, at(1, k), at(1, k + 1));
            } else {
                mesh.add_triangle(at(ring, k), at(ring + 1, k), at(ring + 1, k + 1));
                mesh.add_triangle(at(ring, k), at(ring + 1, k + 1), at(ring, k + 1));
            }
        }
    }
    egui::Shape::mesh(mesh)
}

/// Set row `i`'s new colour to hue/saturation/brightness `hsb` (keeping its model); with `linked`
/// every other row turns by the same hue, scales its saturation and shifts its brightness alike.
/// Excluded rows stay.
fn move_color(rows: &mut [Value], i: usize, hsb: [f32; 3], linked: bool) {
    let Some(old) = rows.get(i).and_then(to_color) else { return };
    let [h0, s0, v0] = old.to_hsb();
    let (dh, ratio, dv) = (hsb[0] - h0, if s0 > 0.01 { hsb[1] / s0 } else { 1.0 }, hsb[2] - v0);
    for (j, row) in rows.iter_mut().enumerate() {
        let Some(c) = to_color(row).filter(|_| !excluded(row)) else { continue };
        let new = if j == i {
            hsb
        } else if linked {
            let [h, s, v] = c.to_hsb();
            [h + dh, (s * ratio).clamp(0.0, 1.0), (v + dv).clamp(0.0, 1.0)]
        } else {
            continue;
        };
        row["to"] = key(&keep_model(c, Color::from_hsb(new[0], new[1], new[2])));
    }
}

/// The Edit tab. Returns true when the rows changed.
fn edit_tab(ui: &mut Ui, d: &mut Dialog, rows: &mut [Value]) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut changed = false;
    let linked = d.bool("linked");
    let active: Vec<usize> = (0..rows.len()).filter(|i| !excluded(&rows[*i]) && to_color(&rows[*i]).is_some()).collect();
    let base = selection(d).into_iter().find(|i| active.contains(i)).or(active.first().copied());
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Harmony Rules:").color(t.text_dim));
        let rule = Harmony::parse(&d.str("rule")).unwrap_or(Harmony::Complementary);
        let labels: Vec<&str> = Harmony::ALL.iter().map(|h| h.label()).collect();
        // Choosing a rule gives the new colours its colours, from the base colour.
        if let Some(i) = widgets::dropdown(ui, "recolor-rule", rule.label(), &labels, 170.0) {
            d.fields.insert("rule".into(), json!(Harmony::ALL[i].id()));
            if let Some((b, c)) = base.and_then(|b| Some((b, to_color(&rows[b])?))) {
                apply_rule(rows, Harmony::ALL[i], b, c, &active);
                changed = true;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let tip = if linked { "Unlink harmony colors" } else { "Link harmony colors" };
            if widgets::icon_button(ui, if linked { "link" } else { "link-2-off" }, tip, linked, 24.0).clicked() {
                d.fields.insert("linked".into(), json!(!linked));
            }
        });
    });
    ui.add_space(8.0);
    let v = base.and_then(|b| to_color(&rows[b])).map_or(1.0, |c| c.to_hsb()[2]);
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - WHEEL) / 2.0).max(0.0));
        let (rect, resp) = ui.allocate_exact_size(vec2(WHEEL, WHEEL), Sense::click_and_drag());
        let (c, r) = (rect.center(), WHEEL / 2.0 - 10.0);
        let painter = ui.painter_at(rect);
        painter.add(wheel_shape(c, r, v.max(0.15)));
        painter.circle_stroke(c, r, egui::Stroke::new(1.0, t.border));
        let at = |rows: &[Value], i: usize| {
            to_color(&rows[i]).map(|col| {
                let [h, s, _] = col.to_hsb();
                wheel_pos(c, r, h, s)
            })
        };
        for &i in &active {
            let (Some(p), Some(col)) = (at(rows, i), to_color(&rows[i])) else { continue };
            let big = Some(i) == base;
            painter.line_segment([c, p], egui::Stroke::new(1.0, egui::Color32::from_white_alpha(160)));
            painter.circle(p, if big { 8.0 } else { 5.5 }, c32(&col), egui::Stroke::new(1.5, egui::Color32::WHITE));
            painter.circle_stroke(p, if big { 9.0 } else { 6.5 }, egui::Stroke::new(1.0, egui::Color32::BLACK));
        }
        // Drag a marker: the nearest one within reach of the press.
        let id = ui.id().with("recolor-wheel-drag");
        if (resp.drag_started() || resp.clicked())
            && let Some(p) = resp.interact_pointer_pos()
        {
            let near = active.iter().filter_map(|&i| Some((i, at(rows, i)?.distance(p)))).filter(|x| x.1 <= 14.0).min_by(|a, b| a.1.total_cmp(&b.1));
            ui.data_mut(|m| m.insert_temp(id, near.map(|x| x.0)));
            if let Some((i, _)) = near {
                d.fields.insert("__sel".into(), json!([i]));
            }
        }
        if resp.dragged()
            && let (Some(i), Some(p)) = (ui.data(|m| m.get_temp::<Option<usize>>(id)).flatten(), resp.interact_pointer_pos())
            && let Some(old) = to_color(&rows[i])
        {
            let dv = p - c;
            let h = (-dv.y).atan2(dv.x).to_degrees().rem_euclid(360.0);
            move_color(rows, i, [h, (dv.length() / r).clamp(0.0, 1.0), old.to_hsb()[2]], linked);
            changed = true;
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - WHEEL - 76.0) / 2.0).max(0.0));
        ui.add_sized([70.0, 22.0], egui::Label::new(egui::RichText::new("Brightness").color(t.text_dim)));
        if let Some(b) = base
            && let Some(col) = to_color(&rows[b])
        {
            let [h, s, _] = col.to_hsb();
            if let (Some(nv), _) = widgets::color_slider(ui, "recolor-brightness", v, WHEEL, &|x| c32(&Color::from_hsb(h, s, x))) {
                move_color(rows, b, [h, s, nv], linked);
                changed = true;
            }
        }
    });
    ui.add_space(6.0);
    // The new colours: click one to make it the base colour.
    ui.horizontal_wrapped(|ui| {
        for &i in &active {
            let (r, resp) = ui.allocate_exact_size(vec2(28.0, 20.0), Sense::click());
            chip(ui, r, to_color(&rows[i]).unwrap_or_default(), false);
            if Some(i) == base {
                ui.painter().rect_stroke(r.expand(1.5), 2.0, egui::Stroke::new(1.5, t.accent), egui::StrokeKind::Outside);
            }
            if resp.on_hover_text(rows[i]["to"].as_str().unwrap_or_default()).clicked() {
                d.fields.insert("__sel".into(), json!([i]));
            }
        }
    });
    changed
}

/// Give the rows `active` the colours of `rule` from the base colour `b` of row `base` (the base row
/// first, then the others in order, the rule's colours repeating), each in its row's model.
fn apply_rule(rows: &mut [Value], rule: Harmony, base: usize, b: Color, active: &[usize]) {
    let colors = rule.apply(b);
    let order = std::iter::once(base).chain(active.iter().copied().filter(|i| *i != base));
    for (n, i) in order.enumerate() {
        let old = to_color(&rows[i]).unwrap_or(b);
        rows[i]["to"] = key(&keep_model(old, colors[n % colors.len()]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<Value> {
        vec![
            json!({"from": ["rgb 255 0 0"], "to": "rgb 255 0 0"}),
            json!({"from": ["rgb 0 0 255", "rgb 0 0 128"], "to": "rgb 0 0 255"}),
            json!({"from": ["rgb 0 255 0"], "to": "rgb 0 255 0"}),
        ]
    }

    #[test]
    fn merge_and_separate_rows() {
        let mut r = rows();
        merge(&mut r, &[2, 0]);
        assert_eq!(r.len(), 2);
        assert_eq!(r[0]["from"], json!(["rgb 255 0 0", "rgb 0 255 0"]));
        assert_eq!(r[0]["to"], "rgb 255 0 0", "the first row's new colour stays");
        separate(&mut r, &[1]);
        assert_eq!(r.len(), 3);
        assert_eq!(r[1], json!({"from": ["rgb 0 0 255"], "to": "rgb 0 0 255"}));
        assert_eq!(r[2]["to"], "rgb 0 0 128");
    }

    #[test]
    fn linked_handles_move_together() {
        let mut r = rows();
        move_color(&mut r, 0, [120.0, 1.0, 1.0], false);
        assert_eq!(r[0]["to"], "rgb 0 255 0");
        assert_eq!(r[1]["to"], "rgb 0 0 255", "unlinked: only that colour");
        let mut r = rows();
        move_color(&mut r, 0, [120.0, 1.0, 1.0], true);
        assert_eq!(r[1]["to"], "rgb 255 0 0", "linked: every colour turns by the same hue");
        assert_eq!(r[2]["to"], "rgb 0 0 255");
        // Excluded rows stay.
        let mut r = rows();
        r[2]["exclude"] = json!(true);
        move_color(&mut r, 0, [120.0, 1.0, 1.0], true);
        assert_eq!(r[2]["to"], "rgb 0 255 0");
    }

    #[test]
    fn harmony_rules_fill_the_rows_from_the_base() {
        let mut r = rows();
        apply_rule(&mut r, Harmony::Triad, 0, Color::rgb(1.0, 0.0, 0.0), &[0, 1, 2]);
        assert_eq!([&r[0]["to"], &r[1]["to"], &r[2]["to"]], ["rgb 255 0 0", "rgb 0 255 0", "rgb 0 0 255"]);
    }
}
