//! Live effect dialogs (Effect menu): the effect's parameters (`__effect`, headed `__label`) with a
//! live Preview that OK keeps as one undo step and Cancel rolls back.

use serde_json::{Value, json};

use super::{DialogSpec, form};
use crate::VectorcraftApp;
use crate::state::Dialog;

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |d| d.str("__label"), body, confirm, preview: true, ..DialogSpec::FORM };

fn body(app: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let changed = form::param_fields(ui, d);
    ui.add_space(6.0);
    let mut pv = d.bool("preview");
    let pv_changed = ui.checkbox(&mut pv, "Preview").changed();
    d.fields.insert("preview".into(), json!(pv));
    if pv && (changed || pv_changed || !app.session.in_interaction()) {
        let label = d.str("__label");
        let _ = app.session.begin_interaction(&label);
        let _ = app.session.preview("effect.apply", &json!({"effect": d.str("__effect"), "params": form::params(d)}));
    } else if !pv && pv_changed {
        let _ = app.session.cancel_interaction();
    }
    false
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let effect = d.str("__effect");
    let params = form::params(d);
    let r = if app.session.in_interaction() {
        // Live preview already applied: keep it (the interaction commits as one undo step).
        let _ = app.session.preview("effect.apply", &json!({"effect": effect, "params": params}));
        app.session.commit_interaction().map(|_| Value::Null).map_err(|e| e.to_string())
    } else {
        app.run("effect.apply", json!({"effect": effect, "params": params}))
    };
    app.last_effect = Some((effect, params));
    app.ui.dialog = None;
    r
}
