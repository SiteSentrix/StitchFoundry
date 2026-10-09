//! Variables (data merge): named values bound to objects, with datasets that apply
//! them all at once. One template document produces many variants (badges, price
//! lists, localized copies) by selecting a dataset instead of editing each object.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Document, NodeId};

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
    /// Parse the `kind` of `variable.define`: `text` or `visibility` (`visible` is accepted
    /// as the same thing, as the panel and the docs call it both).
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "text" => Some(Self::Text),
            "visibility" | "visible" => Some(Self::Visibility),
            _ => None,
        }
    }

    /// The kind's own name, as `variable.define` takes it and `variable.list` reports it.
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
    /// The variable `name`, if the document defines it.
    pub fn variable(&self, name: &str) -> Option<&Variable> {
        self.variables.iter().find(|v| v.name == name)
    }

    /// The dataset `name`, if the document has it.
    pub fn dataset(&self, name: &str) -> Option<&DataSet> {
        self.datasets.iter().find(|d| d.name == name)
    }

    /// The objects bound to the variable `name`, in document order.
    pub fn objects_of(&self, name: &str) -> Vec<NodeId> {
        let mut ids: Vec<NodeId> = self.bindings.iter().filter(|(_, v)| *v == name).map(|(id, _)| *id).collect();
        ids.sort_unstable();
        ids
    }

    /// Drop the variables `names`, the dataset values that name them and the bindings to
    /// them. Datasets named here are left alone: their names are their own namespace (the
    /// active one is dropped by whoever deletes the dataset).
    pub fn prune(&mut self, names: &[String]) -> usize {
        let before = self.variables.len();
        self.variables.retain(|v| !names.contains(&v.name));
        for dataset in &mut self.datasets {
            dataset.values.retain(|k, _| !names.contains(k));
        }
        self.bindings.retain(|_, v| !names.contains(v));
        before - self.variables.len()
    }
}

impl Document {
    /// Forget the bindings of objects that are no longer in the document (after every edit),
    /// like [`Document::prune_assets`] does for Asset Export's. A binding outliving its object
    /// would count as a skip in every dataset application and grow the saved file forever.
    pub fn prune_variable_bindings(&mut self) {
        if self.variables.bindings.keys().all(|id| self.node(*id).is_some()) {
            return;
        }
        let mut bindings = std::mem::take(&mut self.variables.bindings);
        bindings.retain(|id, _| self.node(*id).is_some());
        self.variables.bindings = bindings;
    }
}
