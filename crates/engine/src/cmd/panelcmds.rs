//! Commands the panels need beyond the core set: artboard reorder/duplicate and the advanced
//! Character/Paragraph attributes.

use serde_json::{Value, json};
use vectorcraft_doc::NodeKind;

use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("artboard.reorder", "Move Artboard Up/Down", [], None, "{index, to} change an artboard's number", has_doc, artboard_reorder),
        cmd!(
            "artboard.duplicate",
            "Duplicate Artboard",
            ["Window", "Artboards"],
            None,
            "{index} copy placed right of the last artboard",
            has_doc,
            artboard_duplicate
        ),
        cmd!(
            "text.setFormat",
            "Character / Paragraph",
            [],
            None,
            "{ids?|id?, kerning?: 1/1000 em|\"auto\", baselineShift?: pt, hScale?: %, vScale?: %, rotation?: deg, underline?, strikethrough?, allCaps?, leftIndent?, rightIndent?, firstLineIndent?, spaceBefore?, spaceAfter?: pt, hyphenate?: bool}",
            has_doc,
            set_format
        ),
    ]
}

// ---------- artboards ----------

fn artboard_reorder(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "artboard.reorder";
    let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad(C, "missing index"))? as usize;
    let to = p.get("to").and_then(Value::as_u64).ok_or_else(|| bad(C, "missing to"))? as usize;
    s.edit("Reorder Artboards", |d, _| {
        if i >= d.artboards.len() {
            return Err(EngineError::Other("no such artboard".into()));
        }
        let a = d.artboards.remove(i);
        let to = to.min(d.artboards.len());
        d.artboards.insert(to, a);
        Ok(())
    })?;
    ok()
}

fn artboard_duplicate(s: &mut Session, p: &Value) -> Result<Value> {
    let i = p.get("index").and_then(Value::as_u64).unwrap_or(0) as usize;
    let index = s.edit("Duplicate Artboard", |d, _| {
        let src = d.artboards.get(i).cloned().ok_or_else(|| EngineError::Other("no such artboard".into()))?;
        let right = d.artboards.iter().map(|a| a.rect.x1).fold(f64::MIN, f64::max);
        let mut a = src.clone();
        a.id = d.artboards.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        a.name = format!("{} copy", src.name);
        let dx = right + 20.0 - src.rect.x0;
        a.rect = vectorcraft_geom::Rect::new(src.rect.x0 + dx, src.rect.y0, src.rect.x1 + dx, src.rect.y1);
        d.artboards.push(a);
        Ok(d.artboards.len() - 1)
    })?;
    Ok(json!({"index": index}))
}

// ---------- text ----------

fn set_format(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "text.setFormat";
    let ids: Vec<NodeId> = {
        let ids = targets(s, p)?;
        let d = &s.doc()?.doc;
        ids.into_iter().filter(|id| matches!(d.node(*id).map(|n| &n.kind), Some(NodeKind::Text(_)))).collect()
    };
    if ids.is_empty() {
        return Err(bad(C, "no text objects selected"));
    }
    let num = |k: &str| p.get(k).and_then(Value::as_f64);
    let flag = |k: &str| p.get(k).and_then(Value::as_bool);
    let kerning = match p.get("kerning") {
        None | Some(Value::Null) => None,
        Some(Value::String(a)) if a.eq_ignore_ascii_case("auto") => Some(None),
        Some(v) => Some(Some(v.as_f64().ok_or_else(|| bad(C, "kerning must be a number or \"auto\""))?.clamp(-1000.0, 10000.0))),
    };
    let keys = [
        "kerning",
        "baselineShift",
        "hScale",
        "vScale",
        "rotation",
        "underline",
        "strikethrough",
        "allCaps",
        "leftIndent",
        "rightIndent",
        "firstLineIndent",
        "spaceBefore",
        "spaceAfter",
        "hyphenate",
    ];
    if !keys.iter().any(|k| p.get(*k).is_some()) {
        return Err(bad(C, "nothing to change"));
    }
    s.edit("Character", |d, _| {
        for id in &ids {
            let Some(NodeKind::Text(t)) = d.node_mut(*id).map(|n| &mut n.kind) else { continue };
            for r in &mut t.runs {
                let st = &mut r.style;
                if let Some(k) = kerning {
                    st.kerning = k;
                }
                if let Some(v) = num("baselineShift") {
                    st.baseline_shift = v.clamp(-1296.0, 1296.0);
                }
                if let Some(v) = num("hScale") {
                    st.h_scale = v.clamp(1.0, 10000.0);
                }
                if let Some(v) = num("vScale") {
                    st.v_scale = v.clamp(1.0, 10000.0);
                }
                if let Some(v) = num("rotation") {
                    st.rotation = ((v + 180.0).rem_euclid(360.0)) - 180.0;
                }
                if let Some(v) = flag("underline") {
                    st.underline = v;
                }
                if let Some(v) = flag("strikethrough") {
                    st.strikethrough = v;
                }
                if let Some(v) = flag("allCaps") {
                    st.all_caps = v;
                }
            }
            let para = &mut t.para;
            if let Some(v) = num("leftIndent") {
                para.left_indent = v;
            }
            if let Some(v) = num("rightIndent") {
                para.right_indent = v;
            }
            if let Some(v) = num("firstLineIndent") {
                para.first_line_indent = v;
            }
            if let Some(v) = num("spaceBefore") {
                para.space_before = v;
            }
            if let Some(v) = num("spaceAfter") {
                para.space_after = v;
            }
            if let Some(v) = flag("hyphenate") {
                para.hyphenate = v;
            }
            super::typecmd::refresh_bounds(t);
        }
        Ok(())
    })?;
    Ok(json!({ "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>() }))
}
