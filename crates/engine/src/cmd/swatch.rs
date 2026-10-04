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
        cmd!(
            "swatch.edit",
            "Swatch Options",
            ["Window", "Swatches"],
            None,
            "{name, newName?, color?, mode?: \"gray\"|\"rgb\"|\"hsb\"|\"cmyk\"|\"web\" (convert the colour), global?, spot? (spot colours are always global)} edit a swatch in any colour group, as one undo step. Fills, strokes and text linked to a global swatch take its new colour and name; turning Global off unlinks them (they keep their colour). Colour, mode and spot apply to solid colours only → {name, relinked: paints changed}",
            has_doc,
            swatch_edit
        ),
        cmd!(
            query "swatch.list",
            "Swatches",
            [],
            None,
            "{group?: name (only that colour group's swatches)} → {swatches: [{name, kind: \"none\"|\"color\"|\"gradient\"|\"pattern\", group, global, spot, color?, hex?, gradient?, pattern?}], groups: [{name, swatches: [names]}]}",
            has_doc,
            swatch_list
        ),
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
    let requested = name_param(p, "name");
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

/// A trimmed, non-empty name parameter.
fn name_param(p: &Value, key: &str) -> Option<String> {
    str_param(p, key).map(str::trim).filter(|n| !n.is_empty()).map(str::to_string)
}

/// `c` converted to a Swatch Options colour mode through the colour settings (HSB is an RGB view;
/// Web snaps each channel to a multiple of 0x33).
fn convert_to_mode(c: Color, mode: &str, cmd: &str) -> Result<Color> {
    use vectorcraft_color::cms::Model;
    let cms = vectorcraft_color::cms::active();
    let to = |m: Model| cms.convert(&c, m, cms.settings().intent);
    Ok(match mode.to_ascii_lowercase().as_str() {
        "gray" | "grayscale" => to(Model::Gray),
        "rgb" | "hsb" => to(Model::Rgb),
        "cmyk" => to(Model::Cmyk),
        "web" => {
            let snap = |v: f32| (v.clamp(0.0, 1.0) * 5.0).round() / 5.0;
            let [r, g, b] = to(Model::Rgb).to_rgb_uncalibrated();
            Color::rgb(snap(r), snap(g), snap(b))
        }
        other => return Err(bad(cmd, format!("unknown mode `{other}` (gray, rgb, hsb, cmyk or web)"))),
    })
}

/// How a swatch edit reaches the solid paints linked to swatch `from`.
struct Relink {
    from: String,
    /// The swatch's name after the edit.
    to: String,
    /// The new colour of a global swatch (linked paints take it).
    color: Option<Color>,
    /// false: drop the links (the swatch stopped being global or is deleted); paints keep their colour.
    keep: bool,
}

impl Relink {
    /// Apply to one paint; true when it changed.
    fn apply(&self, c: &mut Color, link: &mut Option<String>) -> bool {
        if link.as_deref() != Some(self.from.as_str()) {
            return false;
        }
        if !self.keep {
            *link = None;
            return true;
        }
        let renamed = self.to != self.from;
        if renamed {
            *link = Some(self.to.clone());
        }
        let recolored = self.color.is_some_and(|n| n != *c);
        if let Some(n) = self.color {
            *c = n;
        }
        renamed || recolored
    }

    /// Apply to the default fill and stroke for new art, except during a live preview (Cancel rolls
    /// the document back, not the session).
    fn defaults(&self, s: &mut Session) {
        if s.in_interaction() {
            return;
        }
        for p in [&mut s.paint.fill, &mut s.paint.stroke] {
            if let Paint::Solid { color, swatch } = p {
                self.apply(color, swatch);
            }
        }
    }
}

fn swatch_edit(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "swatch.edit";
    let name = str_param(p, "name").ok_or_else(|| bad(C, "missing `name`"))?.to_string();
    let d = &s.doc()?.doc;
    let sw = d.swatch(&name).cloned().ok_or_else(|| bad(C, format!("no swatch `{name}`")))?;
    if sw.paint.is_none() {
        return Err(bad(C, format!("`{name}` can't be edited")));
    }
    let given = |k: &str| p.get(k).is_some_and(|v| !v.is_null());
    let solid = sw.paint.color();
    if solid.is_none() && (given("color") || given("mode") || bool_or(p, "spot", false)) {
        return Err(bad(C, "colour, mode and spot apply to solid-colour swatches only"));
    }
    let mut color = match p.get("color").filter(|v| !v.is_null()) {
        Some(v) => Some(color_value(v).ok_or_else(|| bad(C, format!("bad color {v}")))?),
        None => solid,
    };
    if let (Some(c), Some(m)) = (color, str_param(p, "mode")) {
        color = Some(convert_to_mode(c, m, C)?);
    }
    let spot = bool_or(p, "spot", sw.spot);
    let global = spot || bool_or(p, "global", sw.global);
    let to = name_param(p, "newName").filter(|n| *n != name).map_or_else(|| name.clone(), |n| free_name(d, &n));
    let relink = Relink { from: name.clone(), to: to.clone(), color: color.filter(|_| global), keep: global };
    let relinked = s.edit("Swatch Options", |d, _| {
        let w = d.swatch_mut(&name).ok_or_else(|| bad(C, format!("no swatch `{name}`")))?;
        w.name = to.clone();
        w.spot = spot;
        w.global = global;
        if let (Some(c), Paint::Solid { color: wc, .. }) = (color, &mut w.paint) {
            *wc = c;
        }
        Ok(d.map_solid_paints(&mut |c, l| relink.apply(c, l)))
    })?;
    relink.defaults(s);
    Ok(json!({"name": to, "relinked": relinked}))
}

fn swatch_json(sw: &Swatch, group: Option<&str>) -> Value {
    let mut v = json!({"name": sw.name, "group": group, "global": sw.global, "spot": sw.spot});
    match &sw.paint {
        Paint::None => v["kind"] = json!("none"),
        Paint::Solid { color, .. } => {
            v["kind"] = json!("color");
            v["color"] = json!(color);
            v["hex"] = json!(color.to_hex());
        }
        Paint::Gradient(g) => {
            v["kind"] = json!("gradient");
            v["gradient"] = json!(g.gradient.kind.label().to_lowercase());
        }
        Paint::Pattern { pattern, .. } => {
            v["kind"] = json!("pattern");
            v["pattern"] = json!(pattern);
        }
    }
    v
}

fn swatch_list(s: &mut Session, p: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let only = str_param(p, "group");
    let groups: Vec<&SwatchGroup> = match only {
        Some(g) => vec![d.swatch_groups.iter().find(|x| x.name == g).ok_or_else(|| bad("swatch.list", format!("no colour group `{g}`")))?],
        None => d.swatch_groups.iter().collect(),
    };
    let ungrouped = d.swatches.iter().filter(|_| only.is_none()).map(|w| swatch_json(w, None));
    let grouped = groups.iter().flat_map(|g| g.swatches.iter().map(|w| swatch_json(w, Some(&g.name))));
    let swatches: Vec<Value> = ungrouped.chain(grouped).collect();
    let groups: Vec<Value> =
        groups.iter().map(|g| json!({"name": g.name, "swatches": g.swatches.iter().map(|w| &w.name).collect::<Vec<_>>()})).collect();
    Ok(json!({"swatches": swatches, "groups": groups}))
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
