//! Native saves: `document.save` and `file.saveAsTemplate`.

use serde_json::{Value, json};

use super::super::*;
use super::write_or_return;

pub(super) fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let path = str_param(p, "path").map(str::to_string).or_else(|| s.active().and_then(|d| d.path.clone()));
    let bytes = vectorcraft_format::save_file(&s.doc()?.doc);
    let Some(path) = path else {
        // Nowhere to save to (web, agents): hand the bytes back; the document stays modified.
        return write_or_return(None, &bytes, json!({}));
    };
    super::write_file(&path, &bytes)?;
    let st = s.doc_mut()?;
    st.path = Some(path.clone());
    st.mark_saved();
    Ok(json!({ "path": path }))
}

/// File → Save as Template: a native copy flagged so opening it starts a new untitled document.
pub(super) fn save_template(s: &mut Session, p: &Value) -> Result<Value> {
    let mut d = (*s.doc()?.doc).clone();
    d.template = true;
    write_or_return(str_param(p, "path"), &vectorcraft_format::save_file(&d), json!({}))
}
