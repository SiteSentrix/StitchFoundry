//! Live effect dialogs (Effect menu, Appearance panel): the effect's parameters (`__effect`, headed
//! `__label`) with a live Preview that OK keeps as one undo step and Cancel rolls back. `__item`,
//! when present, is the appearance item the effect goes to (else the Appearance panel's active
//! item applies). With `__index` the dialog edits that applied effect in place
//! (`effect.setParams`) instead of adding one (`effect.apply`).
//!
//! Choosing an effect that the target already carries first asks ([`EXISTS`]) whether to edit it
//! or add another one.

use serde_json::{Map, Value, json};

use super::{DialogSpec, form};
use crate::VectorcraftApp;
use crate::state::Dialog;
use crate::theme::Tokens;

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |d| d.str("__label"), body, confirm, preview: true, ..DialogSpec::FORM };

/// The dialog kind asking whether to edit an effect that is already applied or add another.
/// Fields: `__effect`, `__label`, `__item?` (as given to `effect.dialog`), `__target` and
/// `__index` (the applied effect's item and position). OK edits it; `discard: true` then OK adds
/// a new one.
pub const EXISTS: &str = "effectExists";

pub(super) const EXISTS_SPEC: DialogSpec = DialogSpec {
    heading: |d| format!("{} is already applied", d.str("__label")),
    body: |_, ui, _| {
        ui.label(egui::RichText::new("Edit the applied effect, or add another one?").color(Tokens::get(ui.ctx()).text_dim));
        false
    },
    confirm: confirm_exists,
    ok: Some("Edit"),
    discard: Some("Add New"),
    max_width: Some(420.0),
    ..DialogSpec::FORM
};

/// `effect.dialog {effect, index?, item?}`: edit applied effect `index` of `item` (null: the
/// object's effects; omitted: the active item's, else the object's) prefilled with its values,
/// or add `effect`, first asking when that list already has it.
pub fn open(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    let id = p.get("effect").and_then(Value::as_str).unwrap_or_default();
    let info = vectorcraft_effects::effect_info(id).ok_or_else(|| format!("unknown effect `{id}`"))?;
    // The list the effect goes to: `item` as given, else the active item's (the object's without
    // one). A new effect keeps `item` unresolved, so `effect.apply` targets each object as usual.
    let target = p.get("item").cloned().unwrap_or_else(|| json!(app.session.appearance_item()));
    let applied = target_effects(app, &target);
    let mut fields = Map::new();
    fields.insert("__effect".into(), json!(id));
    fields.insert("__label".into(), json!(info.label.trim_end_matches('…')));
    if let Some(item) = p.get("item") {
        fields.insert("__item".into(), item.clone());
    }
    match p.get("index").and_then(Value::as_u64) {
        Some(k) => {
            let e = applied.get(k as usize).ok_or_else(|| format!("no effect at index {k}"))?;
            if e.id != id {
                return Err(format!("effect {k} is `{}`, not `{id}`", e.id));
            }
            fields.insert("__item".into(), target);
            fields.insert("__index".into(), json!(k));
            fields.extend(params_of(e));
        }
        None => {
            if let Some(k) = applied.iter().rposition(|e| e.id == id) {
                fields.insert("__target".into(), target);
                fields.insert("__index".into(), json!(k));
                app.ui.dialog = Some(Dialog { kind: EXISTS.into(), fields });
                return Ok(json!({ "pending": EXISTS }));
            }
            fields.extend(info.defaults.as_object().cloned().unwrap_or_default());
        }
    }
    fields.insert("preview".into(), json!(true));
    app.ui.dialog = Some(Dialog { kind: "effect".into(), fields });
    Ok(Value::Null)
}

/// The effects of the first selected object that `item` (an index or null) addresses.
fn target_effects(app: &VectorcraftApp, item: &Value) -> Vec<vectorcraft_doc::Effect> {
    let Some(st) = app.session.active() else { return vec![] };
    let node = st.selection.objects.first().and_then(|id| st.doc.node(*id));
    node.and_then(|n| n.appearance.effects_at(item.as_u64().map(|i| i as usize))).cloned().unwrap_or_default()
}

/// An applied effect's values over its defaults.
fn params_of(e: &vectorcraft_doc::Effect) -> Map<String, Value> {
    match vectorcraft_effects::merged_params(&e.id, &e.params) {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

/// OK in [`EXISTS`]: the dialog editing the applied effect, or (`discard`) a fresh one.
fn confirm_exists(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let mut fields = d.fields.clone();
    fields.remove("discard");
    let target = fields.remove("__target").unwrap_or(Value::Null);
    if d.bool("discard") {
        fields.remove("__index");
        let defaults = vectorcraft_effects::default_params(&d.str("__effect"));
        fields.extend(defaults.and_then(|v| v.as_object().cloned()).unwrap_or_default());
    } else {
        let k = fields.get("__index").and_then(Value::as_u64).unwrap_or(0) as usize;
        fields.extend(target_effects(app, &target).get(k).map(params_of).unwrap_or_default());
        fields.insert("__item".into(), target);
    }
    fields.insert("preview".into(), json!(true));
    app.ui.dialog = Some(Dialog { kind: "effect".into(), fields });
    Ok(Value::Null)
}

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let (id, relative) = (d.str("__effect"), d.bool("relative"));
    // Plug-in effects get fields from their parameter schema.
    let changed = match vectorcraft_plugins::effect::installed(&id) {
        Some(plugin) => form::schema_fields(ui, d, &plugin.manifest().params),
        None => form::param_fields(ui, d, &|k| vectorcraft_effects::is_length(&id, k, relative), app.session.general_unit()),
    };
    ui.add_space(6.0);
    let mut pv = d.bool("preview");
    let pv_changed = ui.checkbox(&mut pv, "Preview").changed();
    d.fields.insert("preview".into(), json!(pv));
    if pv && (changed || pv_changed || !app.session.in_interaction()) {
        let label = d.str("__label");
        let _ = app.session.begin_interaction(&label);
        let (cmd, p) = command(d);
        let _ = app.session.preview(cmd, &p);
    } else if !pv && pv_changed {
        let _ = app.session.cancel_interaction();
    }
    false
}

/// What OK runs: `effect.setParams` on the edited effect (`__index`), else `effect.apply` of the
/// dialog's effect with its values on the target item.
fn command(d: &Dialog) -> (&'static str, Value) {
    let params = form::params(d);
    let item = d.fields.get("__item").cloned();
    if let Some(index) = d.fields.get("__index") {
        return ("effect.setParams", json!({"index": index, "item": item.unwrap_or(Value::Null), "params": params}));
    }
    let mut p = json!({"effect": d.str("__effect"), "params": params});
    if let Some(item) = item {
        p["item"] = item;
    }
    ("effect.apply", p)
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let (cmd, p) = command(d);
    let r = if app.session.in_interaction() {
        // Live preview already applied: keep it (the interaction commits as one undo step).
        let _ = app.session.preview(cmd, &p);
        app.session.commit_interaction().map(|_| Value::Null).map_err(|e| e.to_string())
    } else {
        app.run(cmd, p.clone())
    };
    // Last Effect repeats applied effects, not edits.
    if cmd == "effect.apply" {
        app.last_effect = Some((d.str("__effect"), p["params"].clone()));
    }
    app.ui.dialog = None;
    r
}
