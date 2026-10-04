//! The Swatches panel: new, delete and duplicate swatches, colour groups and sorting.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint, Swatch, SwatchGroup};
use vectorcraft_doc::Document;
use vectorcraft_doc::pattern::pattern_paint;

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
            "{name?, color? | swatch? | gradient? | pattern?: name (default: the current fill), global?, spot? (a spot colour, always global)} save a colour, gradient or pattern as a swatch. Names are unique (\"Sky 2\"); a colour's default name is its values (\"C=10 M=20 Y=30 K=0\", \"R=255 G=128 B=0\", \"Gray K=40\") → {name}",
            has_doc,
            swatch_new
        ),
        cmd!("swatch.delete", "Delete Swatch", ["Window", "Swatches"], None, "{name}", has_doc, swatch_delete),
        cmd!(
            "swatch.newGroup",
            "New Color Group",
            ["Window", "Swatches"],
            None,
            "{name?, swatches?: [names] (solid colours move into the group; gradients, patterns and None stay), colors?: [colour] (added as new swatches)} → {name}",
            has_doc,
            swatch_new_group
        ),
        cmd!("swatch.duplicate", "Duplicate Swatch", ["Window", "Swatches"], None, "{name}", has_doc, swatch_duplicate),
        cmd!("swatch.sortByName", "Sort by Name", ["Window", "Swatches"], None, "{}", has_doc, swatch_sort),
    ]
}

fn swatch_new(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "swatch.new";
    let paint = match str_param(p, "pattern") {
        Some(name) if s.doc()?.doc.pattern(name).is_some() => pattern_paint(name),
        Some(name) => return Err(bad(C, format!("no pattern `{name}`"))),
        None => paint_from(s, p)?.unwrap_or_else(|| s.paint.fill.clone()),
    };
    let spot = bool_or(p, "spot", false);
    let global = spot || bool_or(p, "global", false);
    let paint = match paint {
        // The swatch holds the colour itself, not a link to the swatch it may have come from.
        Paint::Solid { color, .. } => Paint::solid(color),
        Paint::None => return Err(bad(C, "a swatch needs a colour, gradient or pattern")),
        _ if spot => return Err(bad(C, "only solid colours can be spot colours")),
        other => other,
    };
    let requested = str_param(p, "name").map(str::to_string);
    let name = s.edit("New Swatch", |d, _| {
        let name = match requested {
            Some(n) => free_name(d, &n),
            None => default_name(d, &paint),
        };
        d.swatches.push(Swatch { name: name.clone(), paint, global, spot });
        Ok(name)
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

/// The default name of a new solid swatch: its values in its own colour model.
fn color_name(c: Color) -> String {
    let pct = |v: f32| (v * 100.0).round();
    let byte = |v: f32| (v * 255.0).round();
    match c {
        Color::Cmyk { c, m, y, k } => format!("C={} M={} Y={} K={}", pct(c), pct(m), pct(y), pct(k)),
        Color::Rgb { r, g, b } => format!("R={} G={} B={}", byte(r), byte(g), byte(b)),
        Color::Gray { k } => format!("Gray K={}", pct(k)),
    }
}

/// A free default name for a new swatch of `paint`: its colour values, or "New Gradient Swatch 1"…
fn default_name(d: &Document, paint: &Paint) -> String {
    let base = match paint {
        Paint::Solid { color, .. } => return free_name(d, &color_name(*color)),
        Paint::Gradient(_) => "New Gradient Swatch",
        _ => "New Pattern Swatch",
    };
    (1..).map(|i| format!("{base} {i}")).find(|n| !d.swatch_name_taken(n)).unwrap_or_else(|| base.to_string())
}

/// A swatch or colour-group name not used yet in `d`.
fn free_name(d: &Document, base: &str) -> String {
    unique_name(base, |n| d.swatch_name_taken(n))
}

fn swatch_new_group(s: &mut Session, p: &Value) -> Result<Value> {
    let names: Vec<String> =
        p.get("swatches").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default();
    let colors: Vec<Color> = p.get("colors").and_then(Value::as_array).map(|a| a.iter().filter_map(color_value).collect()).unwrap_or_default();
    let requested = str_param(p, "name").map(str::to_string);
    let name = s.edit("New Color Group", |d, _| {
        let name = free_name(d, requested.as_deref().unwrap_or("Color Group"));
        let mut group = SwatchGroup { name: name.clone(), swatches: vec![] };
        // Colour groups hold solid colours only.
        for n in &names {
            if d.swatch(n).is_some_and(|sw| matches!(sw.paint, Paint::Solid { .. }))
                && let Some(sw) = d.remove_swatch(n)
            {
                group.swatches.push(sw);
            }
        }
        for c in &colors {
            let nm = unique_name(&color_name(*c), |n| d.swatch_name_taken(n) || group.swatches.iter().any(|sw| sw.name == n));
            group.swatches.push(Swatch { name: nm, paint: Paint::solid(*c), global: false, spot: false });
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
