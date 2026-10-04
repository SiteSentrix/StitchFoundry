//! The Swatches panel: new, delete and duplicate swatches, colour groups and sorting.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint, Swatch, SwatchGroup};
use vectorcraft_doc::Document;

use super::paint::paint_from;
use super::*;
use crate::EngineError;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "swatch.new",
            "New Swatch",
            ["Window", "Swatches"],
            None,
            "{name?, color?|gradient? (as paint.setFill), global?} (default: current fill)",
            has_doc,
            swatch_new
        ),
        cmd!("swatch.delete", "Delete Swatch", ["Window", "Swatches"], None, "{name}", has_doc, swatch_delete),
        cmd!(
            "swatch.newGroup",
            "New Color Group",
            ["Window", "Swatches"],
            None,
            "{name?, swatches?: [names] (moved into the group), colors?: [colour] (added as new swatches)}",
            has_doc,
            swatch_new_group
        ),
        cmd!("swatch.duplicate", "Duplicate Swatch", ["Window", "Swatches"], None, "{name}", has_doc, swatch_duplicate),
        cmd!("swatch.sortByName", "Sort by Name", ["Window", "Swatches"], None, "{}", has_doc, swatch_sort),
    ]
}

fn swatch_new(s: &mut Session, p: &Value) -> Result<Value> {
    let paint = match paint_from(s, p)? {
        Some(x) => x,
        None => s.paint.fill.clone(),
    };
    let name = match str_param(p, "name") {
        Some(n) => n.to_string(),
        None => match &paint {
            Paint::Solid { color, .. } => rgb_name(*color),
            _ => format!("Swatch {}", s.doc()?.doc.swatches.len() + 1),
        },
    };
    let global = bool_or(p, "global", false);
    s.edit("New Swatch", |d, _| {
        d.swatches.push(Swatch { name: name.clone(), paint, global, spot: false });
        Ok(())
    })?;
    Ok(json!({ "name": name }))
}

fn swatch_delete(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("swatch.delete", "missing name"))?.to_string();
    s.edit("Delete Swatch", |d, _| {
        d.swatches.retain(|sw| sw.name != name);
        for g in &mut d.swatch_groups {
            g.swatches.retain(|sw| sw.name != name);
        }
        Ok(())
    })?;
    ok()
}

/// The default name of a new solid swatch: its 8-bit RGB values.
fn rgb_name(c: Color) -> String {
    let [r, g, b] = c.to_rgb();
    format!("R={} G={} B={}", (r * 255.0).round(), (g * 255.0).round(), (b * 255.0).round())
}

/// A swatch or colour-group name not used yet in `d`.
fn free_name(d: &Document, base: &str) -> String {
    unique_name(base, |n| d.swatch(n).is_some() || d.swatch_groups.iter().any(|g| g.name == n))
}

fn swatch_new_group(s: &mut Session, p: &Value) -> Result<Value> {
    let names: Vec<String> =
        p.get("swatches").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let colors: Vec<Color> = p.get("colors").and_then(Value::as_array).map(|a| a.iter().filter_map(color_value).collect()).unwrap_or_default();
    let requested = str_param(p, "name").map(str::to_string);
    let name = s.edit("New Color Group", |d, _| {
        let name = free_name(d, requested.as_deref().unwrap_or("Color Group"));
        let mut group = SwatchGroup { name: name.clone(), swatches: vec![] };
        for n in &names {
            if let Some(pos) = d.swatches.iter().position(|sw| &sw.name == n) {
                group.swatches.push(d.swatches.remove(pos));
            }
        }
        for c in &colors {
            group.swatches.push(Swatch { name: rgb_name(*c), paint: Paint::solid(*c), global: false, spot: false });
        }
        d.swatch_groups.push(group);
        Ok(name)
    })?;
    Ok(json!({"name": name}))
}

fn swatch_duplicate(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("swatch.duplicate", "missing name"))?.to_string();
    let new = s.edit("Duplicate Swatch", |d, _| {
        let src = d.swatch(&name).cloned().ok_or_else(|| EngineError::Other(format!("no swatch `{name}`")))?;
        let nm = free_name(d, &format!("{name} copy"));
        let copy = Swatch { name: nm.clone(), ..src };
        if let Some(pos) = d.swatches.iter().position(|sw| sw.name == name) {
            d.swatches.insert(pos + 1, copy);
        } else if let Some(g) = d.swatch_groups.iter_mut().find(|g| g.swatches.iter().any(|sw| sw.name == name)) {
            let pos = g.swatches.iter().position(|sw| sw.name == name).unwrap_or(0);
            g.swatches.insert(pos + 1, copy);
        }
        Ok(nm)
    })?;
    Ok(json!({"name": new}))
}

fn swatch_sort(s: &mut Session, _: &Value) -> Result<Value> {
    s.edit("Sort Swatches", |d, _| {
        // [None] and other bracketed specials stay first, like the reference app.
        let key = |sw: &Swatch| (!sw.name.starts_with('['), sw.name.to_lowercase());
        d.swatches.sort_by_key(key);
        for g in &mut d.swatch_groups {
            g.swatches.sort_by_key(key);
        }
        Ok(())
    })?;
    ok()
}
