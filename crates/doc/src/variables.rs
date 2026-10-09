//! Variables (data merge): named values bound to objects, with datasets that apply
//! them all at once. One template document produces many variants (badges, price
//! lists, localized copies) by selecting a dataset instead of editing each object.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::NodeId;

/// What a variable drives.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableKind {
    /// A type object's characters.
    #[default]
    Text,
    /// Whether bound objects show.
    Visibility,
}

impl VariableKind {
    /// Parse the `kind` of `variable.define` (`text` or `visibility`).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "visibility" | "visible" => Some(Self::Visibility),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Visibility => "visibility",
        }
    }
}

/// One named variable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub name: String,
    pub kind: VariableKind,
}

/// One value a dataset gives a variable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataValue {
    Text(String),
    Visible(bool),
}

impl DataValue {
    /// Whether this value fits the variable's kind (a dataset may carry values for
    /// variables of either kind; each binding takes what fits it).
    pub fn fits(&self, kind: VariableKind) -> bool {
        matches!((self, kind), (Self::Text(_), VariableKind::Text) | (Self::Visible(_), VariableKind::Visibility))
    }
}

/// One named row of values, applied with `dataset.select`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DataSet {
    pub name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub values: BTreeMap<String, DataValue>,
}

/// The variable state of a document: definitions, datasets, bindings
/// (object id → variable name) and which dataset is active.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Variables {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<Variable>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub datasets: Vec<DataSet>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bindings: BTreeMap<NodeId, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_dataset: Option<String>,
}

impl Variables {
    pub fn variable(&self, name: &str) -> Option<&Variable> {
        self.variables.iter().find(|v| v.name == name)
    }

    /// Drop the bindings (and the active dataset) that name what is gone.
    pub fn prune(&mut self, names: &[String]) {
        self.variables.retain(|v| !names.contains(&v.name));
        for dataset in &mut self.datasets {
            dataset.values.retain(|k, _| !names.contains(k));
        }
        self.bindings.retain(|_, v| !names.contains(v));
        if self.active_dataset.as_ref().is_some_and(|a| names.contains(a)) {
            self.active_dataset = None;
        }
    }
}
