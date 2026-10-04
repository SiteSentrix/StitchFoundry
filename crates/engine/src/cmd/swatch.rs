//! The Swatches panel: new, delete and duplicate swatches, colour groups and sorting.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint, Swatch, SwatchGroup};
use vectorcraft_doc::pattern::pattern_paint;
use vectorcraft_doc::swatches::{color_name, node_colors};
use vectorcraft_doc::{Document, NodeId};

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
            "{name?, color? | swatch? | gradient? | pattern?: name (default: the current fill), mode?: \"gray\"|\"rgb\"|\"hsb\"|\"cmyk\"|\"web\" (convert the colour), global?, spot? (a spot colour, always global), group?: colour group name (solid colours only; default: ungrouped)} save a colour, gradient or pattern as a swatch. Names are unique (\"Sky 2\"); a colour's default name is its values (\"C=10 M=20 Y=30 K=0\", \"R=255 G=128 B=0\", \"Gray K=40\") → {name}",
            has_doc,
            swatch_new
        ),
        cmd!(
            "swatch.delete",
            "Delete Swatch",
            ["Window", "Swatches"],
            None,
            "{name? | names?: [swatch or colour group names] (a group goes with its swatches), unlink?: true (art using a deleted global swatch keeps its colour, unlinked; false keeps the stale link)} delete in one undo step → {deleted, unlinked: paints unlinked}",
            has_doc,
            swatch_delete
        ),
        cmd!(
            "swatch.newGroup",
            "New Color Group",
            ["Window", "Swatches"],
            None,
            "{name?, swatches?: [names] (solid colours move into the group; gradients, patterns and None stay), colors?: [colour] (added as new swatches), fromArtwork?: false (add the unique colours of the selected art: fills, strokes, text, gradient stops and mesh points; a colour linked to a global swatch moves that swatch into the group), toGlobal?: true (with fromArtwork: the new swatches are global and the selected art's matching unlinked colours link to them), includeTints?: false (with fromArtwork: tints of global swatches also get swatches of their own)} → {name, swatches: [names in the group], linked: paints linked}",
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
    let mode = str_param(p, "mode");
    let group = str_param(p, "group");
    let paint = match paint {
        // The swatch holds the colour itself, not a link to the swatch it may have come from.
        Paint::Solid { color, .. } => Paint::solid(match mode {
            Some(m) => convert_to_mode(color, m, C)?,
            None => color,
        }),
        Paint::None => return Err(bad(C, "a swatch needs a colour, gradient or pattern")),
        _ if spot || mode.is_some() || group.is_some() => return Err(bad(C, "spot, mode and group apply to solid colours only")),
        other => other,
    };
    let requested = name_param(p, "name");
    let name = s.edit("New Swatch", |d, _| {
        let name = match requested {
            Some(n) => d.free_swatch_name(&n),
            None => d.new_swatch_name(&paint),
        };
        let swatch = Swatch { name: name.clone(), paint, global, spot };
        match group {
            Some(g) => {
                d.swatch_groups.iter_mut().find(|x| x.name == g).ok_or_else(|| bad(C, format!("no colour group `{g}`")))?.swatches.push(swatch)
            }
            None => d.swatches.push(swatch),
        }
        Ok(name)
    })?;
    Ok(json!({ "name": name }))
}

fn swatch_delete(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "swatch.delete";
    let mut names: Vec<String> = vec![];
    for n in str_list(p, "names").into_iter().chain(str_param(p, "name").map(str::to_string)) {
        if !names.contains(&n) {
            names.push(n);
        }
    }
    if names.is_empty() {
        return Err(bad(C, "give `name` or `names`"));
    }
    let d = &s.doc()?.doc;
    // Global swatches going away (named ones and those in named groups): their links are dropped.
    let mut global: Vec<String> = vec![];
    for n in &names {
        let swatches: Vec<&Swatch> = match d.swatch_groups.iter().find(|g| g.name == *n) {
            Some(g) => g.swatches.iter().collect(),
            None => vec![d.swatch(n).ok_or_else(|| bad(C, format!("no swatch or colour group `{n}`")))?],
        };
        if swatches.iter().any(|w| w.paint.is_none()) {
            return Err(bad(C, format!("`{n}` can't be deleted")));
        }
        global.extend(swatches.iter().filter(|w| w.global).map(|w| w.name.clone()));
    }
    if !bool_or(p, "unlink", true) {
        global.clear();
    }
    let mut unlink = |_: &mut Color, link: &mut Option<String>| link.as_ref().is_some_and(|l| global.contains(l)) && link.take().is_some();
    let unlinked = s.edit("Delete Swatch", |d, _| {
        d.swatch_groups.retain(|g| !names.contains(&g.name));
        for n in &names {
            d.remove_swatch(n);
        }
        Ok(d.map_solid_paints(&mut unlink))
    })?;
    map_default_paints(s, &mut unlink);
    Ok(json!({"deleted": names, "unlinked": unlinked}))
}

/// The colours of the art `ids` for a new colour group, in paint order: the swatches that linked
/// colours belong to (added to `swatches` once each) and the other colours (added to `colors` once
/// each). A tint (a linked colour that differs from its swatch's) brings its swatch, and with
/// `tints` its own colour too. Links to missing swatches count as unlinked.
fn artwork_colors(d: &Document, ids: &[NodeId], tints: bool, swatches: &mut Vec<String>, colors: &mut Vec<Color>) {
    for n in ids.iter().filter_map(|id| d.node(*id)) {
        node_colors(n, &mut |c, link| {
            let sw = link.and_then(|l| d.swatch(l)).filter(|w| w.paint.color().is_some());
            if let Some(w) = sw
                && !swatches.contains(&w.name)
            {
                swatches.push(w.name.clone());
            }
            let own = sw.is_none_or(|w| tints && w.paint.color() != Some(*c));
            if own && !colors.contains(c) {
                colors.push(*c);
            }
        });
    }
}

fn swatch_new_group(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "swatch.newGroup";
    let mut names = str_list(p, "swatches");
    let colors: Vec<Color> = p.get("colors").and_then(Value::as_array).map(|a| a.iter().filter_map(color_value).collect()).unwrap_or_default();
    let from_art = bool_or(p, "fromArtwork", false);
    let to_global = from_art && bool_or(p, "toGlobal", true);
    let tints = bool_or(p, "includeTints", false);
    let requested = name_param(p, "name");
    let (name, members, linked) = s.edit("New Color Group", |d, sel| {
        // New swatches: (colour, global).
        let mut new: Vec<(Color, bool)> = colors.iter().map(|c| (*c, false)).collect();
        if from_art {
            if sel.objects.is_empty() {
                return Err(bad(C, "select artwork to make a group of its colours"));
            }
            let mut art = vec![];
            artwork_colors(d, &sel.objects, tints, &mut names, &mut art);
            new.extend(art.into_iter().map(|c| (c, to_global)));
        }
        let name = d.free_swatch_name(requested.as_deref().unwrap_or("Color Group"));
        let mut group = SwatchGroup { name: name.clone(), swatches: vec![] };
        // Colour groups hold solid colours only.
        for n in &names {
            if d.swatch(n).is_some_and(|sw| matches!(sw.paint, Paint::Solid { .. }))
                && let Some(sw) = d.remove_swatch(n)
            {
                group.swatches.push(sw);
            }
        }
        // The global swatches made from the art's colours, which its unlinked paints link to.
        let mut links: Vec<(Color, String)> = vec![];
        for (c, global) in new {
            let nm = unique_name(&color_name(c), |n| n == name || d.swatch_name_taken(n) || group.swatches.iter().any(|sw| sw.name == n));
            if global {
                links.push((c, nm.clone()));
            }
            group.swatches.push(Swatch { name: nm, paint: Paint::solid(c), global, spot: false });
        }
        let members: Vec<String> = group.swatches.iter().map(|w| w.name.clone()).collect();
        d.swatch_groups.push(group);
        let linked = if links.is_empty() {
            0
        } else {
            let live: Vec<String> = d.swatches_iter().map(|w| w.name.clone()).collect();
            d.map_solid_paints_in(&sel.objects, &mut |c, link| {
                if link.as_ref().is_some_and(|l| live.contains(l)) {
                    return false;
                }
                match links.iter().find(|(lc, _)| lc == c) {
                    Some((_, n)) => {
                        *link = Some(n.clone());
                        true
                    }
                    None => false,
                }
            })
        };
        Ok((name, members, linked))
    })?;
    Ok(json!({"name": name, "swatches": members, "linked": linked}))
}

fn swatch_duplicate(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "name").ok_or_else(|| bad("swatch.duplicate", "missing name"))?.to_string();
    let new = s.edit("Duplicate Swatch", |d, _| {
        let src = d.swatch(&name).cloned().ok_or_else(|| EngineError::Other(format!("no swatch `{name}`")))?;
        let nm = d.free_swatch_name(&format!("{name} copy"));
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

/// A list-of-strings parameter (missing or non-string entries are skipped).
fn str_list(p: &Value, key: &str) -> Vec<String> {
    p.get(key).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default()
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
    /// false: drop the links (the swatch stopped being global); paints keep their colour.
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

    fn defaults(&self, s: &mut Session) {
        map_default_paints(s, &mut |c, l| self.apply(c, l));
    }
}

/// Apply a swatch-link rewrite to the default fill and stroke for new art, except during a live
/// preview (Cancel rolls the document back, not the session).
fn map_default_paints(s: &mut Session, f: &mut dyn FnMut(&mut Color, &mut Option<String>) -> bool) {
    if s.in_interaction() {
        return;
    }
    for p in [&mut s.paint.fill, &mut s.paint.stroke] {
        if let Paint::Solid { color, swatch } = p {
            f(color, swatch);
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
    let to = name_param(p, "newName").filter(|n| *n != name).map_or_else(|| name.clone(), |n| d.free_swatch_name(&n));
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
