//! PDF presets: named [`PdfSettings`]. The built-in ones are generated here (the app default
//! keeps the document editable).

use serde::{Deserialize, Serialize};

use crate::PdfSettings;

/// The built-in preset every PDF export starts from.
pub const DEFAULT_PRESET: &str = "VectorCraft Default";

/// A named set of PDF settings: a built-in preset or one the user saved.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfPreset {
    pub name: String,
    /// What the preset is for.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    #[serde(default)]
    pub settings: PdfSettings,
}

/// Every built-in preset, the app default first.
pub fn builtin_presets() -> Vec<PdfPreset> {
    vec![PdfPreset {
        name: DEFAULT_PRESET.into(),
        description: "Keeps the document editable: VectorCraft reopens the PDF with nothing lost. For files you'll edit again.".into(),
        settings: PdfSettings { preserve_editing: true, ..Default::default() },
    }]
}

/// The built-in preset `name` names (any case; `default` is the app default).
pub fn builtin_preset(name: &str) -> Option<PdfPreset> {
    let name = name.trim();
    let name = if name.eq_ignore_ascii_case("default") { DEFAULT_PRESET } else { name };
    builtin_presets().into_iter().find(|p| p.name.eq_ignore_ascii_case(name))
}
