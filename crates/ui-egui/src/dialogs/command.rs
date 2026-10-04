//! The generic parameter dialog (`ui.paramDialog`): edits a command's parameters (`__command`,
//! headed `__label`) and runs it on OK.

use serde_json::Value;

use super::{DialogSpec, form};
use crate::VectorcraftApp;
use crate::state::Dialog;

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |d| d.str("__label"),
    body: |_, ui, d| {
        form::param_fields(ui, d);
        false
    },
    confirm,
    ..DialogSpec::FORM
};

/// Closes before running, so a dialog the command opens stays open.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let cmd = d.str("__command");
    let params = form::params(d);
    app.ui.dialog = None;
    app.run(&cmd, params)
}
