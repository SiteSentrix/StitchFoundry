//! The Graphic Styles panel: apply, new, delete and duplicate.

use serde_json::{Value, json};
use vectorcraft_geom::Rect;

use super::*;
use crate::EngineError;

/// The box a style's placed gradients are stored relative to.
const UNIT_BOX: Rect = Rect::new(0.0, 0.0, 1.0, 1.0);

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "graphicStyle.apply",
            "Apply Graphic Style",
            ["Window", "Graphic Styles"],
            None,
            "{name, ids?} give the selection (or ids) the style's appearance; its placed gradients land at the same place relative to each object's bounds",
            has_selection,
            style_apply
        ),
        cmd!(
            "graphicStyle.new",
            "New Graphic Style",
            ["Window", "Graphic Styles"],
            None,
            "{name?} from the first selected object's appearance (placed gradients are kept relative to its bounds) → {name}",
            has_selection,
            style_new
        ),
        cmd!("graphicStyle.delete", "Delete Graphic Style", ["Window", "Graphic Styles"], None, "{name}", has_doc, style_delete),
        cmd!("graphicStyle.duplicate", "Duplicate Graphic Style", ["Window", "Graphic Styles"], None, "{name}", has_doc, style_duplicate),
    ]
}

fn style_apply(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("graphicStyle.apply", "missing name"))?;
    let style = s
        .doc()?
        .doc
        .graphic_styles
        .iter()
        .find(|g| g.name == name)
        .cloned()
        .ok_or_else(|| EngineError::Other(format!("no graphic style `{name}`")))?;
    let ids = paint_targets(s, p)?;
    s.edit("Apply Graphic Style", |d, _| {
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                let mut ap = style.appearance.clone();
                if style.unit_box
                    && let Some(b) = n.geometric_bounds()
                {
                    ap.rebase_gradients(UNIT_BOX, b);
                }
                n.appearance = ap;
            }
        }
        Ok(())
    })?;
    ok()
}

fn style_new(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let id = *st.selection.objects.first().unwrap();
    let ap = st
        .doc
        .node(id)
        .map(|n| {
            let mut ap = n.appearance.clone();
            if let Some(b) = n.geometric_bounds() {
                ap.rebase_gradients(b, UNIT_BOX);
            }
            ap
        })
        .unwrap_or_default();
    let name = str_param(p, "name").map(str::to_string).unwrap_or_else(|| format!("Graphic Style {}", st.doc.graphic_styles.len() + 1));
    s.edit("New Graphic Style", |d, _| {
        d.graphic_styles.push(vectorcraft_doc::GraphicStyle::new(name.clone(), ap));
        Ok(())
    })?;
    Ok(json!({ "name": name }))
}

fn style_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("graphicStyle.delete", "missing name"))?.to_string();
    s.edit("Delete Graphic Style", |d, _| {
        let before = d.graphic_styles.len();
        d.graphic_styles.retain(|g| g.name != name);
        if d.graphic_styles.len() == before { Err(EngineError::Other(format!("no graphic style `{name}`"))) } else { Ok(()) }
    })?;
    ok()
}

fn style_duplicate(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("graphicStyle.duplicate", "missing name"))?.to_string();
    let new = s.edit("Duplicate Graphic Style", |d, _| {
        let pos = d.graphic_styles.iter().position(|g| g.name == name).ok_or_else(|| EngineError::Other(format!("no graphic style `{name}`")))?;
        let mut g = d.graphic_styles[pos].clone();
        let nm = unique_name(&format!("{name} copy"), |n| d.graphic_styles.iter().any(|x| x.name == n));
        g.name = nm.clone();
        d.graphic_styles.insert(pos + 1, g);
        Ok(nm)
    })?;
    Ok(json!({"name": new}))
}
