//! The generic parameter dialog (`ui.paramDialog`): edits a command's parameters (`__command`,
//! headed `__label`) and runs it on OK.

use serde_json::Value;

use super::{DialogSpec, form};
use crate::VectorcraftApp;
use crate::state::Dialog;

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |d| d.str("__label"),
    body: |app, ui, d| {
        let lengths = lengths(&d.str("__command"));
        form::param_fields(ui, d, &|k| lengths.contains(&k), app.session.general_unit());
        false
    },
    confirm,
    ..DialogSpec::FORM
};

/// The parameters of the commands this dialog edits that are distances.
fn lengths(command: &str) -> &'static [&'static str] {
    match command {
        "graph.create" => &["width", "height"],
        "shape.flare" => &["diameter", "pathLength"],
        "artboard.rearrange" => &["spacing"],
        "perspective.grid.set" => &["cell", "distance"],
        "object.repeat.options" => &["radius", "hSpacing", "vSpacing"],
        "text.areaOptions" => &["gutter", "inset", "firstBaselineMin"],
        _ => &[],
    }
}

/// Closes before running, so a dialog the command opens stays open.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let cmd = d.str("__command");
    let params = form::params(d);
    app.ui.dialog = None;
    app.run(&cmd, params)
}
