//! Perspective Grid attachments (Object › Perspective).
//!
//! The grid itself is document data (`Document.unknown["perspectiveGrid"]`, modelled by the tools
//! crate). Which plane an object lies on is kept on the object, so copies, duplicates and pastes
//! stay attached and deleted objects leave nothing stale behind.

use serde::{Deserialize, Serialize};

/// The plane an object in perspective lies on.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerspectiveAttachment {
    /// `left`, `right` or `ground`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub plane: String,
    /// Where along the plane's normal (points) the parallel plane the object lies on is: 0 is the
    /// grid plane in its original place.
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub depth: f64,
}

impl PerspectiveAttachment {
    pub fn new(plane: &str, depth: f64) -> Self {
        Self { plane: plane.to_string(), depth: if depth.is_finite() { depth } else { 0.0 } }
    }
}
