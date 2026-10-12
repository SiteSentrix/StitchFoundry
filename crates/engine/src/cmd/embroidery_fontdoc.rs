//! The font creator: an embroidery font made in StitchFoundry. A font document has one artboard
//! per glyph, named after it and as wide as its advance, with guides at the baseline and the
//! capitals' height; each glyph is digitized on its artboard with the usual tools. Saving it
//! writes a font folder (font.json and glyphs.stitchfoundry: the glyphs in StitchFoundry's own
//! format, stitching and all) that lettering loads like any embroidery font.

use std::sync::Arc;

use serde_json::{Map, Value, json};
use sf_core::attr::{DocEmbroidery, FontDesign};
use vectorcraft_doc::{Artboard, Document, Guide, LayerColor, Node, Unit};
use vectorcraft_embroidery::PT_PER_MM;
use vectorcraft_embroidery::fonts::EmbFont;
use vectorcraft_geom::{Affine, Rect};

use super::embroidery_lettering::{GLYPHS_FILE, add_font, describe, ensure_fonts, ranged, remember_folder};
use super::*;

const NEW: &str = "embroidery.fontDoc.new";
const SAVE: &str = "embroidery.fontDoc.save";
/// The characters a new font starts with.
pub const DEFAULT_CHARS: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789.,!?'-&";
/// Most glyphs a font document holds, and characters a glyph name has (ligatures such as "fi").
const MAX_DOC_GLYPHS: usize = 500;
const MAX_NAME: usize = 8;
/// Glyph artboards in a row, and the gap between them (mm).
const PER_ROW: usize = 13;
const GAP: f64 = 10.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.fontDoc.new",
            "New Embroidery Font",
            ["Embroidery", "Embroidery Fonts"],
            None,
            "{name?: (My Font), chars?: the glyphs, one a character (A–Z, a–z, 0–9 and .,!?'-&), size?: mm, the capitals' height as digitized (25; 5–200), descent?: mm below the baseline (0.4 × size)} the font creator: a new document for an embroidery font, one artboard per glyph, named after it and as wide as its advance (resize it to change that), with guides at the baseline and the capitals' height: digitize each glyph on its artboard, then embroidery.fontDoc.save; opens a new document → {glyphs}",
            always,
            new_font
        ),
        cmd!(
            "embroidery.fontDoc.addGlyphs",
            "Add Glyphs",
            [],
            None,
            "{chars: one glyph a character, or names: [glyph, …] (ligatures such as \"fi\")} add artboards for glyphs the font document hasn't yet, below the others → {added}",
            has_doc,
            add_glyphs
        ),
        cmd!(
            "embroidery.fontDoc.setProps",
            "Font Properties",
            [],
            None,
            "{name?, leading?: × the size (0.5–5), space?: mm (0–200), letterCase?: upper|lower|none, minScale?, maxScale?: × the size it sews well down and up to (0–10; 0: not said), description?, license?} the font document's font properties → {font}",
            has_doc,
            set_props
        ),
        cmd!(
            "embroidery.fontDoc.kern",
            "Kern Glyph Pair",
            [],
            None,
            "{pair: \"A V\" (two glyphs, a space between), value: mm closer (negative: further apart; 0 removes it)} kerning for a pair of the font document's glyphs → {pairs}",
            has_doc,
            kern
        ),
        cmd!(
            query "embroidery.fontDoc.info",
            "Font Info",
            [],
            None,
            "{} the font document's font: its properties, and its glyphs with their widths (mm) and whether they have art yet → {font, glyphs: [{name, width, done}], missing}",
            has_doc,
            info
        ),
        cmd!(
            "embroidery.fontDoc.save",
            "Save Embroidery Font",
            [],
            None,
            "{folder?: write the font folder there (font.json and glyphs.stitchfoundry), remembered like any font folder} make the font document's glyphs an embroidery font, loaded for lettering at once (without a folder, only for this session): each glyph the art on its artboard (its middle inside it), its advance the artboard's width → {name, glyphs, missing, folder}",
            has_doc,
            save
        ),
    ]
}

/// The font the active document is being made into.
fn design(s: &Session, cmd: &str) -> Result<FontDesign> {
    s.doc()?
        .doc
        .embroidery
        .as_deref()
        .and_then(|e| e.font.clone())
        .ok_or_else(|| bad(cmd, "this document isn't an embroidery font (Embroidery › Embroidery Fonts › New Embroidery Font)"))
}

/// A new glyph's advance (mm): capitals wider than small letters and figures, the rest narrow.
fn advance(name: &str, size: f64) -> f64 {
    let c = name.chars().next().unwrap_or(' ');
    let k = if name.chars().count() > 1 {
        0.9
    } else if c.is_uppercase() {
        0.75
    } else if c.is_alphanumeric() {
        0.6
    } else {
        0.35
    };
    size * k
}

/// The glyph names in `p` (`chars`: one a character; `names`: as given), without blanks and
/// repeats.
fn glyph_names(p: &Value, cmd: &str) -> Result<Vec<String>> {
    let mut out: Vec<String> = Vec::new();
    let mut add = |n: String| {
        if !n.trim().is_empty() && n.chars().count() <= MAX_NAME && !out.contains(&n) {
            out.push(n);
        }
    };
    if let Some(chars) = str_param(p, "chars") {
        chars.chars().filter(|c| !c.is_whitespace()).for_each(|c| add(c.to_string()));
    }
    if let Some(names) = p.get("names").and_then(Value::as_array) {
        names.iter().filter_map(Value::as_str).for_each(|n| add(n.trim().to_string()));
    }
    if out.len() > MAX_DOC_GLYPHS {
        return Err(bad(cmd, format!("{MAX_DOC_GLYPHS} glyphs at most")));
    }
    Ok(out)
}

/// The artboards (and their guides) of `names`, laid out in rows below `top` (points), ids from
/// `next`.
fn glyph_boards(names: &[String], f: &FontDesign, top: f64, next: u32) -> (Vec<Artboard>, Vec<Guide>) {
    let pt = PT_PER_MM;
    let height = (f.size * 1.4 + f.descent) * pt;
    let (mut boards, mut guides) = (Vec::new(), Vec::new());
    let (mut x, mut y) = (0.0, top);
    for (i, name) in names.iter().enumerate() {
        if i > 0 && i % PER_ROW == 0 {
            (x, y) = (0.0, y + height + GAP * pt);
        }
        let w = advance(name, f.size) * pt;
        let id = next + i as u32;
        let rect = Rect::new(x, y, x + w, y + height);
        let base = rect.y1 - f.descent * pt;
        for pos in [base, base - f.size * pt] {
            guides.push(Guide { vertical: false, pos, artboard: Some(id), layer: None });
        }
        boards.push(Artboard { id, name: name.clone(), rect, show_center_mark: false, show_cross_hairs: false });
        x += w + GAP * pt;
    }
    (boards, guides)
}

fn new_font(s: &mut Session, p: &Value) -> Result<Value> {
    let mut f = FontDesign::default();
    if let Some(n) = str_param(p, "name").map(str::trim).filter(|n| !n.is_empty()) {
        f.name = n.chars().take(100).collect();
    }
    f.size = ranged(p, "size", 5.0..=200.0, NEW, "`size` is 5 to 200 mm")?.unwrap_or(f.size);
    f.descent = ranged(p, "descent", 0.0..=200.0, NEW, "`descent` is 0 to 200 mm")?.unwrap_or(f.size * 0.4);
    f.space = f.size * 0.3;
    let mut names = glyph_names(p, NEW)?;
    if names.is_empty() {
        names = DEFAULT_CHARS.chars().map(String::from).collect();
    }
    let (boards, guides) = glyph_boards(&names, &f, 0.0, 1);
    let mut doc = Document::new(100.0, 100.0);
    doc.title = f.name.clone();
    doc.units = Unit::Millimeters;
    doc.artboards = boards;
    doc.guides = guides;
    doc.embroidery = Some(Box::new(DocEmbroidery { font: Some(f), ..DocEmbroidery::default() }));
    let count = doc.artboards.len();
    s.add_document(doc, None);
    Ok(json!({ "glyphs": count }))
}

fn add_glyphs(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.fontDoc.addGlyphs";
    let f = design(s, CMD)?;
    let d = &s.doc()?.doc;
    let have: Vec<&str> = d.artboards.iter().map(|a| a.name.as_str()).collect();
    let names: Vec<String> = glyph_names(p, CMD)?.into_iter().filter(|n| !have.contains(&n.as_str())).collect();
    if names.is_empty() {
        return Err(bad(CMD, "no glyphs the font hasn't already: give `chars` or `names`"));
    }
    if d.artboards.len() + names.len() > MAX_DOC_GLYPHS {
        return Err(bad(CMD, format!("{MAX_DOC_GLYPHS} glyphs at most")));
    }
    let top = d.artboards.iter().map(|a| a.rect.y1).fold(0.0f64, f64::max) + GAP * PT_PER_MM;
    let next = d.artboards.iter().map(|a| a.id).max().unwrap_or(0) + 1;
    let (boards, guides) = glyph_boards(&names, &f, top, next);
    let added = boards.len();
    s.edit("Add Glyphs", |d, _| {
        d.artboards.extend(boards);
        d.guides.extend(guides);
        Ok(())
    })?;
    Ok(json!({ "added": added }))
}

fn set_props(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.fontDoc.setProps";
    let mut f = design(s, CMD)?;
    if let Some(n) = str_param(p, "name").map(str::trim) {
        if n.is_empty() {
            return Err(bad(CMD, "`name` can't be blank"));
        }
        f.name = n.chars().take(100).collect();
    }
    f.leading = ranged(p, "leading", 0.5..=5.0, CMD, "`leading` is 0.5 to 5")?.unwrap_or(f.leading);
    f.space = ranged(p, "space", 0.0..=200.0, CMD, "`space` is 0 to 200 mm")?.unwrap_or(f.space);
    f.min_scale = ranged(p, "minScale", 0.0..=10.0, CMD, "`minScale` is 0 to 10")?.unwrap_or(f.min_scale);
    f.max_scale = ranged(p, "maxScale", 0.0..=10.0, CMD, "`maxScale` is 0 to 10")?.unwrap_or(f.max_scale);
    match str_param(p, "letterCase") {
        None => {}
        Some("upper" | "lower") => f.letter_case = str_param(p, "letterCase").unwrap_or_default().to_string(),
        Some("none" | "") => f.letter_case.clear(),
        Some(o) => return Err(bad(CMD, format!("unknown letterCase `{o}`: upper, lower or none"))),
    }
    if let Some(v) = str_param(p, "description") {
        f.description = v.chars().take(2000).collect();
    }
    if let Some(v) = str_param(p, "license") {
        f.license = v.chars().take(200).collect();
    }
    let out = json!(f);
    s.edit("Font Properties", |d, _| {
        d.embroidery.get_or_insert_with(Box::default).font = Some(f);
        Ok(())
    })?;
    Ok(json!({ "font": out }))
}

fn kern(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.fontDoc.kern";
    let mut f = design(s, CMD)?;
    let pair = str_param(p, "pair")
        .map(|v| v.split_whitespace().collect::<Vec<_>>())
        .filter(|v| v.len() == 2)
        .ok_or_else(|| bad(CMD, "`pair` is two glyphs with a space between: \"A V\""))?;
    let key = pair.join(" ");
    let value = p
        .get("value")
        .and_then(Value::as_f64)
        .filter(|v| v.is_finite() && v.abs() <= f.size * 2.0)
        .ok_or_else(|| bad(CMD, format!("`value` is mm, at most {} either way", f.size * 2.0)))?;
    if value == 0.0 {
        f.kerning.remove(&key);
    } else {
        f.kerning.insert(key, value);
    }
    let pairs = json!(f.kerning);
    s.edit("Kern Glyph Pair", |d, _| {
        d.embroidery.get_or_insert_with(Box::default).font = Some(f);
        Ok(())
    })?;
    Ok(json!({ "pairs": pairs }))
}

/// The art on each glyph's artboard: the top-level objects of the visible layers whose middle is
/// inside it, by artboard.
fn glyph_art(d: &Document) -> Vec<(&Artboard, Vec<Arc<Node>>)> {
    let tops: Vec<(Arc<Node>, vectorcraft_geom::Point)> = d
        .layers
        .iter()
        .filter(|l| l.visible)
        .flat_map(|l| l.children().into_iter().flatten())
        .filter_map(|n| Some((n.clone(), n.geometric_bounds()?.center())))
        .collect();
    d.artboards
        .iter()
        .filter(|a| !a.name.trim().is_empty() && a.name.chars().count() <= MAX_NAME)
        .take(MAX_DOC_GLYPHS)
        .map(|a| (a, tops.iter().filter(|(_, c)| a.rect.contains(*c)).map(|(n, _)| n.clone()).collect()))
        .collect()
}

fn info(s: &mut Session, _: &Value) -> Result<Value> {
    let f = design(s, "embroidery.fontDoc.info")?;
    let d = &s.doc()?.doc;
    let glyphs: Vec<Value> =
        glyph_art(d).iter().map(|(a, art)| json!({ "name": a.name, "width": a.rect.width() / PT_PER_MM, "done": !art.is_empty() })).collect();
    let missing = glyphs.iter().filter(|g| g["done"] == false).count();
    Ok(json!({ "font": f, "glyphs": glyphs, "missing": missing }))
}

/// The font files of font document `d`: font.json and the glyphs' art (each glyph a layer, its
/// artboard's left edge at 0 and its baseline at 0, in points), and the glyphs without art.
fn font_files(d: &Document, f: &FontDesign) -> Result<(String, Document, Vec<String>)> {
    let pt = PT_PER_MM;
    let mut art = Document::new(100.0, 100.0);
    art.layers.clear();
    let (mut advances, mut missing) = (Map::new(), Vec::new());
    for (board, nodes) in glyph_art(d) {
        let name = board.name.trim();
        if nodes.is_empty() {
            missing.push(name.to_string());
            continue;
        }
        let r = board.rect;
        let m = Affine::translate((-r.x0, -(r.y1 - f.descent * pt)));
        let children: Vec<Arc<Node>> = nodes
            .iter()
            .map(|n| {
                let mut c = art.reid(n);
                c.transform(m, true);
                Arc::new(c)
            })
            .collect();
        let mut layer = Node::layer(art.alloc_id(), &format!("GlyphLayer-{name}"), LayerColor::Preset(0));
        if let Some(ch) = layer.children_mut() {
            *ch = children;
        }
        art.layers.push(Arc::new(layer));
        advances.insert(name.to_string(), json!(r.width()));
    }
    if art.layers.is_empty() {
        return Err(bad(SAVE, "nothing is digitized yet: draw each glyph on its artboard"));
    }
    let kerning: Map<String, Value> = f.kerning.iter().map(|(k, v)| (k.clone(), json!(v * pt))).collect();
    let json = json!({
        "name": f.name,
        "description": f.description,
        "font_license": f.license,
        "size": f.size,
        "leading": f.size * f.leading * pt,
        "min_scale": f.min_scale,
        "max_scale": f.max_scale,
        "letter_case": f.letter_case,
        "horiz_adv_x_space": f.space * pt,
        "horiz_adv_x": advances,
        "kerning_pairs": kerning,
    });
    Ok((json.to_string(), art, missing))
}

fn save(s: &mut Session, p: &Value) -> Result<Value> {
    let f = design(s, SAVE)?;
    let (json, art, missing) = font_files(&s.doc()?.doc, &f)?;
    let mut font = EmbFont::new(&json, vec![(art.clone(), 0.0, 1.0)]).map_err(|e| bad(SAVE, e))?;
    let folder = str_param(p, "folder").map(str::trim).filter(|f| !f.is_empty()).map(str::to_string);
    if let Some(dir) = &folder {
        crate::cmd::fileio::create_dir(dir)?;
        let at = |name: &str| std::path::Path::new(dir).join(name).to_string_lossy().to_string();
        crate::cmd::fileio::write_file(&at("font.json"), json.as_bytes())?;
        crate::cmd::fileio::write_file(&at(GLYPHS_FILE), &vectorcraft_format::save(&art, false))?;
        font.folder = Some(dir.clone());
    }
    let mut out = describe(&font);
    // the remembered folders first, so this font replaces one of the same name among them
    let _ = ensure_fonts(s);
    add_font(s, font);
    if let Some(dir) = &folder {
        remember_folder(s, dir);
    }
    if let Some(o) = out.as_object_mut() {
        o.insert("missing".into(), json!(missing));
    }
    Ok(out)
}
