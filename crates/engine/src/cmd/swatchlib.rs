//! Swatch libraries (Window → Swatch Libraries, the Swatches panel's library button): read-only
//! sets of swatches the library panel shows. The built-in libraries are computed in
//! [`vectorcraft_color::libraries`]; `swatch.library.add` copies swatches into the document.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::libraries::{SWATCH_LIBRARIES, builtin_library};
use vectorcraft_color::{Swatch, SwatchGroup, SwatchLibrary, default_swatches};

use super::menucmds::squash;
use super::swatch::{map_default_paints, str_list, swatch_json};
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "swatch.library.list",
            "Swatch Libraries",
            [],
            None,
            "{} → {libraries: [{id, name, category: \"builtIn\", count}]} every library the library panel opens, in menu order",
            always,
            list
        ),
        cmd!(
            query "swatch.library.get",
            "Swatch Library",
            [],
            None,
            "{library: id or name} → {id, name, category, swatches: [{name, kind, group, global, spot, color?, hex?, gradient?}] (as swatch.list), groups: [{name, swatches: [names]}]}",
            always,
            get
        ),
        cmd!(
            "swatch.library.add",
            "Add to Swatches",
            ["Window", "Swatch Libraries"],
            None,
            "{library: id or name, names?: [swatch or colour group names in the library] (default: all of it; a group comes as a colour group, a single swatch ungrouped), apply?: \"fill\"|\"stroke\" (also apply the first one to the selection and the defaults, as paint.setFill / paint.setStroke), focus?: true (with apply: false keeps the active proxy)} copy library swatches into the document as one undo step. A swatch the document already has (same name and paint) isn't added again; a name taken by another swatch gets a number (\"Clay 1 2\") → {library, added: [names in the document], existing: [names already there], applied?: name}",
            has_doc,
            add
        ),
        cmd!(
            "swatch.resetDefaults",
            "Default Swatches",
            ["Window", "Swatch Libraries"],
            None,
            "{replace?: false} bring back the default swatches and colour groups of the document's colour mode that are missing (by name), as one undo step; replace: true makes the swatches exactly the defaults (art linked to a removed global swatch keeps its colour, unlinked) → {added: [names]}",
            has_doc,
            reset_defaults
        ),
    ]
}

/// A library as the menus and the library panel list it.
#[derive(Clone, Debug, PartialEq)]
pub struct LibraryInfo {
    pub id: String,
    pub name: String,
    /// "builtIn".
    pub category: &'static str,
}

/// Every library, in menu order.
pub fn libraries(_s: &Session) -> Vec<LibraryInfo> {
    SWATCH_LIBRARIES.iter().map(|b| LibraryInfo { id: b.id.into(), name: b.name.into(), category: "builtIn" }).collect()
}

/// Library `key` (an id, or a name in any case) with its info.
pub fn library(s: &Session, key: &str) -> Option<(LibraryInfo, Arc<SwatchLibrary>)> {
    let info = libraries(s).into_iter().find(|l| l.id == key).or_else(|| libraries(s).into_iter().find(|l| l.name.eq_ignore_ascii_case(key)))?;
    let lib = builtin_library(&info.id)?;
    Some((info, lib))
}

fn library_param(s: &Session, p: &Value, cmd: &str) -> Result<(LibraryInfo, Arc<SwatchLibrary>)> {
    let key = str_param(p, "library").ok_or_else(|| bad(cmd, "missing `library` (see swatch.library.list)"))?;
    library(s, key).ok_or_else(|| bad(cmd, format!("no swatch library `{key}` (see swatch.library.list)")))
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    let libs: Vec<Value> = libraries(s)
        .into_iter()
        .map(|l| {
            let count = library(s, &l.id).map_or(0, |(_, lib)| lib.len());
            json!({"id": l.id, "name": l.name, "category": l.category, "count": count})
        })
        .collect();
    Ok(json!({ "libraries": libs }))
}

fn get(s: &mut Session, p: &Value) -> Result<Value> {
    let (info, lib) = library_param(s, p, "swatch.library.get")?;
    let ungrouped = lib.swatches.iter().map(|w| swatch_json(w, None));
    let grouped = lib.groups.iter().flat_map(|g| g.swatches.iter().map(|w| swatch_json(w, Some(&g.name))));
    let groups: Vec<Value> =
        lib.groups.iter().map(|g| json!({"name": g.name, "swatches": g.swatches.iter().map(|w| &w.name).collect::<Vec<_>>()})).collect();
    Ok(
        json!({"id": info.id, "name": info.name, "category": info.category, "swatches": ungrouped.chain(grouped).collect::<Vec<_>>(), "groups": groups}),
    )
}

/// The library swatches `names` stand for, each once, with the colour group it goes into (`None`:
/// ungrouped): a group's name brings the group, a swatch's name the swatch alone; no names, all.
fn picked<'a>(lib: &'a SwatchLibrary, names: &[String], cmd: &str) -> Result<Vec<(Option<&'a str>, &'a Swatch)>> {
    let mut out: Vec<(Option<&str>, &Swatch)> = vec![];
    let mut push = |g: Option<&'a str>, w: &'a Swatch| {
        if !out.iter().any(|(_, x)| x.name == w.name) {
            out.push((g, w));
        }
    };
    if names.is_empty() {
        lib.swatches.iter().for_each(|w| push(None, w));
        lib.groups.iter().for_each(|g| g.swatches.iter().for_each(|w| push(Some(&g.name), w)));
    }
    for n in names {
        match (lib.group(n), lib.swatch(n)) {
            (Some(g), _) => g.swatches.iter().for_each(|w| push(Some(&g.name), w)),
            (None, Some(w)) => push(None, w),
            (None, None) => return Err(bad(cmd, format!("no swatch or colour group `{n}` in `{}`", lib.name))),
        }
    }
    Ok(out)
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "swatch.library.add";
    let (info, lib) = library_param(s, p, C)?;
    let apply = match str_param(p, "apply") {
        None => None,
        Some("fill") => Some("paint.setFill"),
        Some("stroke") => Some("paint.setStroke"),
        Some(other) => return Err(bad(C, format!("apply is \"fill\" or \"stroke\", not `{other}`"))),
    };
    let picked = picked(&lib, &str_list(p, "names"), C)?;
    // Swatches the document already has (same name and paint) are reused, not added again.
    let d = &s.doc()?.doc;
    let (existing, fresh): (Vec<_>, Vec<_>) = picked.iter().partition(|(_, w)| d.swatch(&w.name).is_some_and(|x| x.paint == w.paint));
    let existing: Vec<String> = existing.iter().map(|(_, w)| w.name.clone()).collect();
    let undo_before = s.doc()?.history.undo.len();
    // Library name → the name it got in the document.
    let added: Vec<(String, String)> = if fresh.is_empty() {
        vec![]
    } else {
        s.edit("Add to Swatches", |d, _| {
            let mut out = vec![];
            for (group, w) in fresh {
                let name = d.free_swatch_name(&w.name);
                let sw = Swatch { name: name.clone(), ..(*w).clone() };
                match group {
                    Some(g) => {
                        let i = match d.swatch_groups.iter().position(|x| x.name == *g) {
                            Some(i) => i,
                            None => {
                                let name = d.free_swatch_name(g);
                                d.swatch_groups.push(SwatchGroup { name, swatches: vec![] });
                                d.swatch_groups.len() - 1
                            }
                        };
                        d.swatch_groups[i].swatches.push(sw);
                    }
                    None => d.swatches.push(sw),
                }
                out.push((w.name.clone(), name));
            }
            Ok(out)
        })?
    };
    let mut out = json!({"library": info.id, "added": added.iter().map(|(_, n)| n).collect::<Vec<_>>(), "existing": existing});
    if let (Some(cmd), Some((_, first))) = (apply, picked.first()) {
        let name = added.iter().find(|(l, _)| *l == first.name).map_or(first.name.clone(), |(_, n)| n.clone());
        s.execute(cmd, &json!({"swatch": name, "focus": bool_or(p, "focus", true)}))?;
        // Adding and applying undo together.
        squash(s, undo_before, "Add to Swatches");
        out["applied"] = json!(name);
    }
    Ok(out)
}

fn reset_defaults(s: &mut Session, p: &Value) -> Result<Value> {
    let replace = bool_or(p, "replace", false);
    let model = s.doc()?.doc.color_mode.model();
    let (defaults, groups) = default_swatches(model);
    // With replace, links to swatches that are no longer global ones are dropped (the art keeps
    // its colour); filled in by the edit.
    let mut live: Vec<String> = vec![];
    let added = s.edit("Default Swatches", |d, _| {
        let mut added = vec![];
        if replace {
            let before: Vec<String> = d.swatches_iter().map(|w| w.name.clone()).collect();
            (d.swatches, d.swatch_groups) = (defaults, groups);
            added.extend(d.swatches_iter().filter(|w| !before.contains(&w.name)).map(|w| w.name.clone()));
            live.extend(d.swatches_iter().filter(|w| w.global).map(|w| w.name.clone()));
            d.map_solid_paints(&mut |_, l| unlink_dead(&live, l));
            return Ok(added);
        }
        for (i, w) in defaults.into_iter().enumerate() {
            if !d.swatch_name_taken(&w.name) {
                added.push(w.name.clone());
                // None leads, as in a new document; the others go after the existing swatches.
                let at = if i == 0 && w.paint.is_none() { 0 } else { d.swatches.len() };
                d.swatches.insert(at, w);
            }
        }
        for g in groups {
            let i = match d.swatch_groups.iter().position(|x| x.name == g.name) {
                Some(i) => i,
                None if d.swatch_name_taken(&g.name) => continue,
                None => {
                    d.swatch_groups.push(SwatchGroup { name: g.name.clone(), swatches: vec![] });
                    d.swatch_groups.len() - 1
                }
            };
            for w in g.swatches {
                if !d.swatch_name_taken(&w.name) {
                    added.push(w.name.clone());
                    d.swatch_groups[i].swatches.push(w);
                }
            }
        }
        Ok(added)
    })?;
    if replace {
        map_default_paints(s, &mut |_, l| unlink_dead(&live, l));
    }
    Ok(json!({ "added": added }))
}

/// Drop a link to a swatch not in `live`; true when it did.
fn unlink_dead(live: &[String], link: &mut Option<String>) -> bool {
    link.as_ref().is_some_and(|n| !live.contains(n)) && link.take().is_some()
}
