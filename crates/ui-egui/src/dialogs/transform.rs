//! Transform dialogs: Move, Rotate, Scale, Reflect and Shear (with Copy, and Uniform for Scale).

use serde_json::{Value, json};

use super::{DialogSpec, form, run_and_close};
use crate::VectorcraftApp;
use crate::state::Dialog;

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |d| title(&d.kind).into(), body, confirm, ..DialogSpec::FORM };

fn title(kind: &str) -> &'static str {
    match kind {
        "move" => "Move",
        "rotate" => "Rotate",
        "scale" => "Scale",
        "reflect" => "Reflect",
        _ => "Shear",
    }
}

fn body(_: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    form::grid(ui, d);
    ui.add_space(6.0);
    if d.kind == "scale" {
        form::check(ui, d, "uniform", "Uniform");
    }
    form::check(ui, d, "copy", "Copy (make a transformed copy)");
    false
}

/// Pass a tool-chosen reference point (origin) through to transform commands.
fn origin_params(d: &Dialog, mut p: Value) -> Value {
    if let Some(o) = d.fields.get("origin") {
        p["origin"] = o.clone();
    }
    p
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let copy = d.bool("copy");
    let (id, params) = match d.kind.as_str() {
        "move" => return run_and_close(app, "object.move", json!({"dx": d.f64("dx", 0.0), "dy": d.f64("dy", 0.0), "copy": copy})),
        "rotate" => ("object.rotate", json!({"angle": d.f64("angle", 0.0), "copy": copy})),
        "scale" => {
            let sx = d.f64("sx", 100.0);
            let sy = if d.bool("uniform") { sx } else { d.f64("sy", 100.0) };
            ("object.scale", json!({"sx": sx, "sy": sy, "copy": copy}))
        }
        "reflect" => ("object.reflect", json!({"axis": d.fields.get("axis").cloned().unwrap_or(json!("vertical")), "copy": copy})),
        _ => ("object.shear", json!({"angle": d.f64("angle", 0.0), "axis": d.str("axis"), "copy": copy})),
    };
    run_and_close(app, id, origin_params(d, params))
}
