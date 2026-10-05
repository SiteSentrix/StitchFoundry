//! Field widgets shared by the dialogs: labelled text fields, checkboxes, the generic field grid
//! and the typed parameter editor of the command and effect dialogs.

use serde_json::{Value, json};
use vectorcraft_doc::Unit;

use crate::state::Dialog;
use crate::theme::Tokens;

/// Width of a dialog's value fields ([`text`] and [`length`], frame included).
const FIELD_W: f32 = 132.0;

/// A labelled text field bound to `d.fields[key]` (one grid row).
pub(super) fn field(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(label).color(t.text_dim));
    text(ui, d, key, FIELD_W - 12.0);
    ui.end_row();
}

/// A labelled [`length`] field (one grid row).
pub(super) fn length_field(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str, unit: Unit) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(label).color(t.text_dim));
    length(ui, d, key, unit, FIELD_W);
    ui.end_row();
}

/// A distance field `width` wide bound to `d.fields[key]`: shown and typed in `unit` (a typed unit,
/// "5 mm", wins), kept in points (a string with a unit, "12 pt", also reads). Returns true when it
/// changed.
pub(super) fn length(ui: &mut egui::Ui, d: &mut Dialog, key: &str, unit: Unit, width: f32) -> bool {
    let value = d.fields.contains_key(key).then(|| d.f64(key, 0.0));
    let Some(v) = crate::widgets::num_field(ui, ("dlg-len", key), value, unit, width) else { return false };
    d.fields.insert(key.into(), json!(v));
    true
}

/// The fields of the form dialogs ([`grid`]) that are distances, by dialog kind.
fn lengths(kind: &str) -> &'static [&'static str] {
    match kind {
        "move" => &["dx", "dy"],
        "rectangle" | "ellipse" | "artboardOptions" => &["width", "height"],
        "roundedRectangle" => &["width", "height", "radius"],
        "polygon" => &["radius"],
        "star" => &["radius1", "radius2"],
        "lineSegment" => &["length"],
        "offsetPath" => &["offset"],
        "simplify" => &["tolerance"],
        "splitIntoGrid" => &["gutter"],
        _ => &[],
    }
}

/// A text field `width` wide bound to `d.fields[key]`. Returns true when it changed.
pub(super) fn text(ui: &mut egui::Ui, d: &mut Dialog, key: &str, width: f32) -> bool {
    text_edit(ui, d, key, width).changed()
}

/// [`text`] returning the field's response (focus, Enter).
pub(super) fn text_edit(ui: &mut egui::Ui, d: &mut Dialog, key: &str, width: f32) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let mut s = d.str(key);
    let r = egui::Frame::NONE
        .fill(t.input)
        .stroke(egui::Stroke::new(1.0, t.input_border))
        .corner_radius(egui::CornerRadius::same(3))
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| ui.add(egui::TextEdit::singleline(&mut s).frame(egui::Frame::NONE).desired_width(width)));
    if r.inner.changed() {
        d.fields.insert(key.into(), Value::String(s));
    }
    r.inner
}

/// A checkbox bound to `d.fields[key]`.
pub(super) fn check(ui: &mut egui::Ui, d: &mut Dialog, key: &str, label: &str) {
    let mut b = d.bool(key);
    if ui.checkbox(&mut b, label).changed() {
        d.fields.insert(key.into(), Value::Bool(b));
    }
}

/// The generic dialog body: a field per non-boolean value (positions and indices hidden), the
/// distances in `unit`.
pub(super) fn grid(ui: &mut egui::Ui, d: &mut Dialog, unit: Unit) {
    let lengths = lengths(&d.kind);
    egui::Grid::new("dlg").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
        let keys: Vec<(String, Value)> = d.fields.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, v) in keys {
            if k == "x" || k == "y" || k == "origin" || k == "index" || v.is_boolean() {
                continue;
            }
            if lengths.contains(&k.as_str()) {
                length_field(ui, d, &k, &humanize(&k), unit);
            } else {
                field(ui, d, &k, &humanize(&k));
            }
        }
    });
}

/// Command/effect parameters from the dialog fields (drop UI-only keys).
pub(super) fn params(d: &Dialog) -> Value {
    Value::Object(d.fields.iter().filter(|(k, _)| !k.starts_with("__") && k.as_str() != "preview").map(|(k, v)| (k.clone(), v.clone())).collect())
}

/// Generic editor for command/effect parameters: numbers, booleans, strings and colours; the
/// parameters `is_length` names are distances shown and typed in `unit`. Returns true when a
/// value changed.
pub(super) fn param_fields(ui: &mut egui::Ui, d: &mut Dialog, is_length: &dyn Fn(&str) -> bool, unit: Unit) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut changed = false;
    egui::Grid::new("fxgrid").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        let keys: Vec<(String, Value)> =
            d.fields.iter().filter(|(k, _)| !k.starts_with("__") && k.as_str() != "preview").map(|(k, v)| (k.clone(), v.clone())).collect();
        for (k, v) in keys {
            ui.label(egui::RichText::new(humanize(&k)).color(t.text));
            if let Some(cur) = crate::widgets::blend_param(&k, &v) {
                if let Some(m) = crate::widgets::blend_param_dropdown(ui, ("fx-blend", &k), cur) {
                    d.fields.insert(k, m);
                    changed = true;
                }
                ui.end_row();
                continue;
            }
            if is_length(&k) {
                changed |= length(ui, d, &k, unit, 140.0);
                ui.end_row();
                continue;
            }
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

/// A field label from its camelCase key (`miterLimit` → "Miter Limit:").
pub(super) fn humanize(k: &str) -> String {
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
        "Include Cmy Blacks" => "Include Blacks with CMY:".into(),
        _ => format!("{s}:"),
    }
}

/// Widths of [`slider`]'s label column and rail.
pub(super) const SLIDER_LABEL: f32 = 64.0;
pub(super) const SLIDER_WIDTH: f32 = 180.0;

/// A labelled slider with a value field for the number `d.fields[key]` in `range` (whole numbers,
/// `suffix` after the value); `track(t)` colours the rail at 0..1.
pub(super) fn slider(
    ui: &mut egui::Ui,
    d: &mut Dialog,
    key: &str,
    label: &str,
    range: std::ops::RangeInclusive<f64>,
    suffix: &str,
    track: &dyn Fn(f32) -> egui::Color32,
) {
    let t = Tokens::get(ui.ctx());
    let (min, max) = (*range.start(), *range.end());
    let v = d.f64(key, 0.0).clamp(min, max);
    let mut new = None;
    ui.horizontal(|ui| {
        ui.add_sized([SLIDER_LABEL, 22.0], egui::Label::new(egui::RichText::new(label).color(t.text)));
        if let (Some(x), _) = crate::widgets::color_slider(ui, ("dlg-slider", key), ((v - min) / (max - min)) as f32, SLIDER_WIDTH, track) {
            new = Some((min + x as f64 * (max - min)).round());
        }
        if let Some(x) = crate::widgets::plain_field(ui, ("dlg-field", key), v, suffix, 0, 52.0) {
            new = Some(x.round().clamp(min, max));
        }
    });
    if let Some(n) = new.filter(|n| *n != v) {
        d.fields.insert(key.into(), json!(n));
    }
}

/// The field where [`preview`] keeps the parameters it last previewed.
pub(super) const PREVIEWED: &str = "__previewed";

/// The Preview checkbox of a dialog that previews `cmd` on the canvas: while it is on, `params` run
/// again on the interaction's snapshot whenever they differ from the last preview (whether a widget
/// or `ui.dialog.set` changed them); turning it off rolls back.
pub(super) fn preview(app: &mut crate::VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog, label: &str, cmd: &str, params: Value) {
    const LAST: &str = PREVIEWED;
    ui.add_space(8.0);
    let mut on = d.bool("preview");
    if crate::widgets::check(ui, "Preview", on, true) {
        on = !on;
        d.fields.insert("preview".into(), json!(on));
    }
    if !on {
        if d.fields.remove(LAST).is_some() {
            let _ = app.session.cancel_interaction();
        }
        return;
    }
    if d.fields.get(LAST) != Some(&params) || !app.session.in_interaction() {
        let _ = app.session.begin_interaction(label);
        if let Err(e) = app.session.preview(cmd, &params) {
            app.status(e.to_string());
        }
        d.fields.insert(LAST.into(), params);
    }
}

/// OK of a dialog using [`preview`]: keep the previewed `cmd` as one undo step (or run it when no
/// preview runs) and close. On an error the dialog stays open.
pub(super) fn commit_preview(app: &mut crate::VectorcraftApp, cmd: &str, params: Value) -> Result<Value, String> {
    let r = if app.session.in_interaction() {
        match app.session.preview(cmd, &params) {
            Ok(_) => app.session.commit_interaction().map(|_| Value::Null).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        app.run(cmd, params)
    };
    if r.is_ok() {
        app.ui.dialog = None;
    }
    r
}
