//! Type → Find Font: the fonts a document uses (flagging missing ones), replacing one font with
//! another everywhere or in the selection, and selecting the text that uses a font.

use std::collections::BTreeMap;

use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId, NodeKind};

use super::typecmd::refresh_bounds;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "text.fonts",
            "Fonts in Document",
            [],
            None,
            "{selectionOnly?} → [{family, style, runs, objects, missing}] sorted by name",
            has_doc,
            fonts
        ),
        cmd!(
            "text.replaceFont",
            "Replace Font",
            [],
            None,
            "{from: {family, style?}, to: {family, style?}, selectionOnly?} (style omitted: every style of the family / keep the closest style) → {runs}",
            has_doc,
            replace
        ),
        cmd!(
            "select.font",
            "Select Text by Font",
            [],
            None,
            "{family, style?} select the text objects using the font → {count}",
            has_doc,
            select_font
        ),
    ]
}

/// Text objects in scope: the selection's (and their descendants') or the whole document's.
fn scope(s: &Session, selection_only: bool) -> Result<Vec<NodeId>> {
    let st = s.doc()?;
    let mut v = vec![];
    let mut add = |n: &vectorcraft_doc::Node| {
        n.walk(&mut |c| {
            if matches!(c.kind, NodeKind::Text(_)) && !v.contains(&c.id) {
                v.push(c.id);
            }
        })
    };
    if selection_only {
        for id in &st.selection.objects {
            if let Some(n) = st.doc.node(*id) {
                add(n);
            }
        }
    } else {
        for l in &st.doc.layers {
            add(l);
        }
    }
    Ok(v)
}

fn text(d: &Document, id: NodeId) -> Option<&vectorcraft_doc::TextObject> {
    match d.node(id).map(|n| &n.kind) {
        Some(NodeKind::Text(t)) => Some(t),
        _ => None,
    }
}

fn fonts(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = scope(s, bool_or(p, "selectionOnly", false))?;
    let d = &s.doc()?.doc;
    let db = vectorcraft_text::FontDb::global();
    let families = db.families();
    let mut found: BTreeMap<(String, String), (usize, Vec<NodeId>)> = BTreeMap::new();
    for id in ids {
        for r in text(d, id).map(|t| t.runs.as_slice()).unwrap_or_default() {
            let e = found.entry((r.style.font_family.clone(), r.style.font_style.clone())).or_default();
            e.0 += 1;
            if !e.1.contains(&id) {
                e.1.push(id);
            }
        }
    }
    let list: Vec<Value> = found
        .into_iter()
        .map(|((family, style), (runs, objects))| {
            let have_family = families.iter().any(|f| f.eq_ignore_ascii_case(&family));
            let missing = !have_family || !db.styles(&family).iter().any(|st| st.eq_ignore_ascii_case(&style));
            json!({ "family": family, "style": style, "runs": runs, "objects": objects.len(), "missing": missing })
        })
        .collect();
    Ok(json!(list))
}

fn font_param(p: &Value, k: &str, c: &str) -> Result<(String, Option<String>)> {
    let v = p.get(k).ok_or_else(|| bad(c, format!("missing `{k}`")))?;
    let family =
        v.get("family").and_then(Value::as_str).filter(|f| !f.trim().is_empty()).ok_or_else(|| bad(c, format!("`{k}.family` is required")))?;
    Ok((family.to_string(), v.get("style").and_then(Value::as_str).map(str::to_string)))
}

fn replace(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "text.replaceFont";
    let (ff, fs) = font_param(p, "from", C)?;
    let (tf, ts) = font_param(p, "to", C)?;
    let db = vectorcraft_text::FontDb::global();
    if !db.families().iter().any(|f| f.eq_ignore_ascii_case(&tf)) {
        return Err(bad(C, format!("font family `{tf}` is not available")));
    }
    let ids = scope(s, bool_or(p, "selectionOnly", false))?;
    let matches = |st: &vectorcraft_doc::CharStyle| {
        st.font_family.eq_ignore_ascii_case(&ff) && fs.as_ref().is_none_or(|x| st.font_style.eq_ignore_ascii_case(x))
    };
    let n = s.edit("Replace Font", |d, _| {
        let mut n = 0;
        for id in &ids {
            let Some(NodeKind::Text(t)) = d.node_mut(*id).map(|n| &mut n.kind) else { continue };
            let mut changed = false;
            for r in &mut t.runs {
                if matches(&r.style) {
                    // An explicit style, else the closest the new family has to the old one.
                    let style = ts.clone().unwrap_or_else(|| db.face(&tf, &r.style.font_style).style.clone());
                    r.style.font_family = tf.clone();
                    r.style.font_style = style;
                    changed = true;
                    n += 1;
                }
            }
            if changed {
                vectorcraft_text::edit::normalize(&mut t.runs);
                refresh_bounds(t);
            }
        }
        Ok(n)
    })?;
    Ok(json!({ "runs": n }))
}

fn select_font(s: &mut Session, p: &Value) -> Result<Value> {
    let family = str_param(p, "family").ok_or_else(|| bad("select.font", "missing `family`"))?.to_string();
    let style = str_param(p, "style").map(str::to_string);
    let ids = scope(s, false)?;
    let d = &s.doc()?.doc;
    let hits: Vec<NodeId> = ids
        .into_iter()
        .filter(|id| {
            text(d, *id).is_some_and(|t| {
                t.runs.iter().any(|r| {
                    r.style.font_family.eq_ignore_ascii_case(&family) && style.as_ref().is_none_or(|x| r.style.font_style.eq_ignore_ascii_case(x))
                })
            })
        })
        .collect();
    let n = hits.len();
    s.select(|_, sel| sel.set(hits))?;
    Ok(json!({ "count": n }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_flag_missing_replace_and_select() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        let a = s.execute("text.create", &json!({"x": 10, "y": 20, "text": "Serif", "font": "Source Serif 4"})).unwrap()["id"].as_u64().unwrap();
        let b = s.execute("text.create", &json!({"x": 10, "y": 60, "text": "Gone", "font": "No Such Font Family"})).unwrap()["id"].as_u64().unwrap();
        let list = s.execute("text.fonts", &json!({})).unwrap();
        let missing: Vec<&str> = list.as_array().unwrap().iter().filter(|f| f["missing"] == true).map(|f| f["family"].as_str().unwrap()).collect();
        assert_eq!(missing, ["No Such Font Family"]);
        assert_eq!(s.execute("select.font", &json!({"family": "No Such Font Family"})).unwrap()["count"], 1);
        assert_eq!(s.doc().unwrap().selection.objects, [NodeId(b)]);
        let r = s.execute("text.replaceFont", &json!({"from": {"family": "No Such Font Family"}, "to": {"family": "Source Sans 3"}})).unwrap();
        assert_eq!(r["runs"], 1);
        let fams: Vec<String> =
            s.execute("text.fonts", &json!({})).unwrap().as_array().unwrap().iter().map(|f| f["family"].as_str().unwrap().to_string()).collect();
        assert_eq!(fams, ["Source Sans 3", "Source Serif 4"]);
        assert!(s.execute("text.replaceFont", &json!({"from": {"family": "Source Serif 4"}, "to": {"family": "Nope"}})).is_err());
        let _ = a;
    }
}

#[cfg(test)]
mod open_tests {
    use super::*;

    #[test]
    fn opened_documents_have_exact_text_bounds() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 200})).unwrap();
        let id = s.execute("text.create", &json!({"x": 10, "y": 60, "text": "WIDE TRACKED", "size": 40})).unwrap()["id"].as_u64().unwrap();
        s.execute("text.setStyle", &json!({"tracking": 400})).unwrap();
        let want = s.doc().unwrap().doc.node(NodeId(id)).unwrap().geometric_bounds().unwrap();
        // Save and reopen: the cache isn't in the file, yet the bounds must match.
        let bytes = vectorcraft_format::save(&s.doc().unwrap().doc, false);
        let reopened = vectorcraft_format::load(&bytes).unwrap();
        let mut t = Session::new();
        t.add_document(reopened, None);
        let got = t.doc().unwrap().doc.node(NodeId(id)).unwrap().geometric_bounds().unwrap();
        assert!((got.width() - want.width()).abs() < 1e-6, "{got:?} vs {want:?}");
    }
}

#[cfg(test)]
mod dirty_tests {
    use super::*;

    #[test]
    fn selection_never_marks_modified_and_undo_to_saved_is_clean() {
        let mut s = Session::new();
        s.add_document(Document::new(100.0, 100.0), None);
        let id = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"].clone();
        s.doc_mut().unwrap().mark_saved();
        s.execute("select.none", &json!({})).unwrap();
        s.execute("select.set", &json!({"ids": [id]})).unwrap();
        assert!(!s.doc().unwrap().is_dirty(), "selecting is not an edit");
        s.execute("object.move", &json!({"dx": 5, "dy": 0})).unwrap();
        assert!(s.doc().unwrap().is_dirty());
        s.execute("edit.undo", &json!({})).unwrap();
        assert!(!s.doc().unwrap().is_dirty(), "back to the saved state");
    }
}
