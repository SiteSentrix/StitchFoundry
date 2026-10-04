//! File → Document Setup: the document's ruler units.

use serde_json::json;

use super::{DialogSpec, run_and_close};

pub(super) const SPEC: DialogSpec = DialogSpec {
    heading: |_| "Document Setup".into(),
    confirm: |app, d| run_and_close(app, "document.setUnits", json!({"units": d.str("units")})),
    ..DialogSpec::FORM
};
