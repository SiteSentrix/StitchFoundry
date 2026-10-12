//! Lettering with embroidery fonts: fonts in Ink/Stitch's format, read from folders users have
//! (`embroidery.font.load`; none ship with StitchFoundry, and each keeps its own licence), and
//! text set with them (`embroidery.lettering`): each glyph's digitized art placed along the lines,
//! kerned and scaled, a group per glyph.

use std::sync::Arc;

use serde_json::{Value, json};
use sf_core::attr::{Embroidery, LetterAlign, Lettering, StitchType};
use vectorcraft_doc::{Document, Node, NodeId, NodeKind};
use vectorcraft_embroidery::fonts::{EmbFont, MAX_TEXT};
use vectorcraft_geom::Affine;

use super::*;

const LOAD: &str = "embroidery.font.load";
const SET: &str = "embroidery.lettering";
const EDIT: &str = "embroidery.lettering.edit";
/// Biggest font.json and glyph art read (bytes), most art files one font has, and most fonts
/// loaded at once.
const MAX_JSON: usize = 2 << 20;
const MAX_SVG: usize = 40 << 20;
const MAX_FILES: usize = 64;
const MAX_FONTS: usize = 64;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.font.load",
            "Load Embroidery Font",
            [],
            None,
            "{path?: a font folder in Ink/Stitch's format (font.json, and ltr.svg or an ltr folder of SVGs), json?, svg?: the files' text instead} load an embroidery font for lettering; its glyphs sew the way their Ink/Stitch settings say. It stays loaded until StitchFoundry quits; loading a font of the same name again replaces it → {name, glyphs, size, minSize, maxSize, license}",
            always,
            load_font
        ),
        cmd!(
            "embroidery.fonts",
            "Embroidery Fonts",
            [],
            None,
            "{} the loaded embroidery fonts → {fonts: [{name, glyphs, size, minSize, maxSize, license, description}]}",
            always,
            list_fonts
        ),
        cmd!(
            "embroidery.lettering",
            "Embroidery Lettering",
            [],
            None,
            "{text, font, size?: mm (its capitals' height; default: the font's own), at?: [x, y] (where the first line's baseline starts, or its middle or end by `align`; default: the first artboard's centre), leading?: × the font's line height (1), align?: left|centre|right (left), arc?: mm (the baseline round a circle that radius: positive arched over its top, negative along its bottom; 0 straight)} set text in a loaded embroidery font: each glyph its digitized art, kerned and scaled, a group per glyph inside one for the lettering, which remembers what it says (embroidery.lettering.edit sets it again); a size outside what the font sews well at comes with a warning → {id, glyphs, warnings}",
            has_doc,
            lettering
        ),
        cmd!(
            "embroidery.lettering.edit",
            "Edit Lettering",
            [],
            None,
            "{id?, text?, font?, size?: mm, leading?, align?, arc?: mm} set the selected lettering (in an embroidery font) again with a new text, font, size or line spacing, where it is now, moved, turned and scaled as it is; its font must be loaded → {id, glyphs, warnings}",
            has_selection,
            edit_lettering
        ),
    ]
}

/// What a font tells about itself.
fn describe(f: &EmbFont) -> Value {
    let i = &f.info;
    let size = |k: f64| (i.size > 0.0 && k > 0.0).then_some(i.size * k);
    json!({
        "name": i.name,
        "glyphs": f.glyphs.len(),
        "size": (i.size > 0.0).then_some(i.size),
        "minSize": size(i.min_scale),
        "maxSize": size(i.max_scale),
        "license": i.font_license,
        "description": i.description,
    })
}

/// The `<sodipodi:guide>` labelled `label`: its height (`position="x,y"`, user units up from the
/// bottom of the page).
fn guide_y(svg: &str, label: &str) -> Option<f64> {
    let want = format!("inkscape:label=\"{label}\"");
    let mut rest = svg;
    while let Some(i) = rest.find("<sodipodi:guide") {
        let tail = rest.get(i..)?;
        let end = tail.find('>').unwrap_or(tail.len());
        let el = tail.get(..end)?;
        if el.contains(&want) {
            let pos = el.split("position=\"").nth(1)?.split('"').next()?;
            let y = pos.split([',', ' ']).filter(|v| !v.is_empty()).nth(1)?.parse::<f64>().ok()?;
            return y.is_finite().then_some(y);
        }
        rest = tail.get(end..)?;
    }
    None
}

/// The root element's `viewBox` height (user units).
fn view_box_height(svg: &str) -> Option<f64> {
    let tail = svg.get(svg.find("<svg")?..)?;
    let el = tail.get(..tail.find('>')?)?;
    let vb = el.split("viewBox=\"").nth(1)?.split('"').next()?;
    let h = vb.split([',', ' ']).filter(|v| !v.is_empty()).nth(3)?.parse::<f64>().ok()?;
    (h.is_finite() && h > 0.0).then_some(h)
}

/// One SVG of glyph art: imported, sewn the way its Ink/Stitch settings say, with its glyphs'
/// baseline (pt from the top of the page) and its font unit (pt).
fn glyph_art(svg: &str) -> Result<(Document, f64, f64)> {
    let (mut doc, _) = vectorcraft_svg::import_with_report(svg).map_err(|e| bad(LOAD, format!("the glyph art can't be read: {e}")))?;
    vectorcraft_embroidery::inkstitch::stitch_all(&mut doc);
    let page = doc.artboards.first().map(|a| a.rect.height()).filter(|h| h.is_finite() && *h > 0.0).unwrap_or(0.0);
    // a user unit: the page over its viewBox (no viewBox: CSS pixels)
    let unit = view_box_height(svg).filter(|_| page > 0.0).map_or(0.75, |h| page / h);
    let baseline = page - guide_y(svg, "baseline").unwrap_or(0.0) * unit;
    Ok((doc, baseline, unit))
}

fn read_text(path: &str, max: usize, what: &str) -> Result<String> {
    let bytes = crate::cmd::fileio::read_file(path)?;
    if bytes.len() > max {
        return Err(bad(LOAD, format!("{what} is too big ({} MB at most)", max >> 20)));
    }
    String::from_utf8(bytes).map_err(|_| bad(LOAD, format!("{what} is not text")))
}

/// The font's files in folder `dir`: font.json and its art (ltr.svg, →.svg or rtl.svg; else the
/// SVGs in an ltr folder).
#[cfg(not(target_arch = "wasm32"))]
fn read_folder(dir: &str) -> Result<(String, Vec<String>)> {
    let at = |name: &str| std::path::Path::new(dir).join(name).to_string_lossy().to_string();
    let json = read_text(&at("font.json"), MAX_JSON, "font.json")?;
    let mut svgs = Vec::new();
    if let Some(file) = ["ltr.svg", "→.svg", "rtl.svg", "←.svg"].iter().map(|n| at(n)).find(|p| std::path::Path::new(p).is_file()) {
        svgs.push(read_text(&file, MAX_SVG, "the glyph art")?);
    } else if let Ok(rd) = std::fs::read_dir(at("ltr")) {
        let mut files: Vec<String> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("svg")))
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        files.sort();
        for f in files.iter().take(MAX_FILES) {
            svgs.push(read_text(f, MAX_SVG, "the glyph art")?);
        }
    }
    if svgs.is_empty() {
        return Err(bad(LOAD, format!("{dir}: no glyph art (ltr.svg, or SVGs in an ltr folder)")));
    }
    Ok((json, svgs))
}

#[cfg(target_arch = "wasm32")]
fn read_folder(dir: &str) -> Result<(String, Vec<String>)> {
    Err(bad(LOAD, format!("{dir}: no file system here (pass json and svg)")))
}

fn load_font(s: &mut Session, p: &Value) -> Result<Value> {
    let (json, svgs) = match (str_param(p, "json"), str_param(p, "svg"), str_param(p, "path")) {
        (Some(j), Some(v), _) => {
            if j.len() > MAX_JSON || v.len() > MAX_SVG {
                return Err(bad(LOAD, "the font is too big"));
            }
            (j.to_string(), vec![v.to_string()])
        }
        (_, _, Some(path)) => read_folder(path)?,
        _ => return Err(bad(LOAD, "give the font's folder (`path`), or its `json` and `svg`")),
    };
    let art = svgs.iter().map(|svg| glyph_art(svg)).collect::<Result<Vec<_>>>()?;
    let font = EmbFont::new(&json, art).map_err(|e| bad(LOAD, e))?;
    let out = describe(&font);
    let fonts = &mut s.embroidery_fonts;
    fonts.retain(|f| !f.info.name.eq_ignore_ascii_case(&font.info.name));
    if fonts.len() >= MAX_FONTS {
        fonts.remove(0);
    }
    fonts.push(Arc::new(font));
    Ok(out)
}

fn list_fonts(s: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({ "fonts": s.embroidery_fonts.iter().map(|f| describe(f)).collect::<Vec<_>>() }))
}

/// The size (mm) `font` was digitized at.
fn own_size(font: &EmbFont) -> f64 {
    if font.info.size > 0.0 { font.info.size } else { 20.0 }
}

/// What `font` sews well at, said when `size` (mm) is outside it.
fn size_warnings(font: &EmbFont, size: f64) -> Vec<String> {
    let (i, own) = (&font.info, own_size(font));
    let k = size / own;
    let mut warnings = Vec::new();
    if i.min_scale > 0.0 && k < i.min_scale - 1e-9 {
        warnings.push(format!("{} sews well from {:.1} mm", i.name, own * i.min_scale));
    }
    if i.max_scale > 0.0 && k > i.max_scale + 1e-9 {
        warnings.push(format!("{} sews well up to {:.1} mm", i.name, own * i.max_scale));
    }
    warnings
}

/// A loaded font, by name (any case).
fn loaded(s: &Session, name: &str) -> Option<Arc<EmbFont>> {
    s.embroidery_fonts.iter().find(|f| f.info.name.eq_ignore_ascii_case(name)).cloned()
}

fn not_loaded(cmd: &str, name: &str) -> EngineError {
    bad(cmd, format!("no embroidery font `{name}` is loaded (embroidery.font.load)"))
}

/// `text`, when given: 1 to [`MAX_TEXT`] characters, not all blank.
fn text_param<'a>(p: &'a Value, cmd: &str) -> Result<Option<&'a str>> {
    match p.get("text") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) if !t.trim().is_empty() && t.chars().count() <= MAX_TEXT => Ok(Some(t.as_str())),
        Some(_) => Err(bad(cmd, format!("`text` is 1 to {MAX_TEXT} characters"))),
    }
}

/// A number param in `range`, when given.
fn ranged(p: &Value, key: &str, range: std::ops::RangeInclusive<f64>, cmd: &str, what: &str) -> Result<Option<f64>> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_f64().filter(|v| range.contains(v)).map(Some).ok_or_else(|| bad(cmd, what)),
    }
}

/// The lettering among `ids`, or holding one of them (a glyph selected inside it), and what it
/// says.
fn lettering_of(d: &Document, ids: &[NodeId]) -> Option<(NodeId, Lettering)> {
    ids.iter().find_map(|id| {
        let mut at = Some(*id);
        while let Some(c) = at {
            if let Some(StitchType::Lettering(l)) = d.node(c)?.embroidery.as_deref().map(|e| &e.stitch) {
                return Some((c, (**l).clone()));
            }
            at = d.parent_of(c);
        }
        None
    })
}

/// The lettering group's name: its text (the start of it) and font.
fn lettering_name(spec: &Lettering) -> String {
    let short: String = spec.text.chars().take(40).map(|c| if c.is_control() { ' ' } else { c }).collect();
    format!("{short} ({})", spec.font)
}

/// The glyphs `spec` sets in `font`, placed by its frame: a group per glyph, its art sewn as the
/// font says.
fn glyphs(d: &mut Document, font: &EmbFont, spec: &Lettering) -> Vec<Arc<Node>> {
    let i = &font.info;
    let k = spec.size / own_size(font);
    let frame = Affine::new(spec.frame.0);
    let placed = font.layout(&spec.text);
    // each line's width (font units), to align it
    let mut widths: Vec<f64> = Vec::new();
    for g in &placed {
        if widths.len() <= g.line {
            widths.resize(g.line + 1, 0.0);
        }
        if let Some(w) = widths.get_mut(g.line) {
            *w = w.max(g.x + g.advance);
        }
    }
    let r = spec.arc * vectorcraft_embroidery::PT_PER_MM;
    let mut out = Vec::new();
    for g in &placed {
        let Some(glyph) = font.glyphs.get(&g.glyph) else { continue };
        let u = glyph.unit * k;
        let width = widths.get(g.line).copied().unwrap_or(0.0);
        // the glyph's art from its page to its place: the middle of its baseline along the line
        // (from the origin, by the alignment), down to its line, round the arc
        let along = (g.x + g.advance * 0.5 - width * spec.align.share()) * u;
        let down = g.line as f64 * i.leading * spec.leading * u;
        let m = frame
            * place(r, along, down)
            * Affine::translate((-g.advance * 0.5 * u, 0.0))
            * Affine::scale(k)
            * Affine::translate((0.0, -glyph.baseline));
        let children: Vec<Arc<Node>> = glyph
            .art
            .children()
            .map(|c| {
                c.iter()
                    .map(|n| {
                        let mut n = d.reid(n);
                        n.visible = true;
                        n.transform(m, true);
                        Arc::new(n)
                    })
                    .collect()
            })
            .unwrap_or_default();
        if children.is_empty() {
            continue;
        }
        let mut grp = Node::group(d.alloc_id(), children);
        grp.name = Some(g.glyph.clone());
        out.push(Arc::new(grp));
    }
    out
}

/// Where a glyph's baseline middle goes, `along` a line `down` from the first (points):
/// straight, or round a circle of radius `r` (points; positive, the lines arched over its top,
/// negative, along its bottom) whose edge the first line's baseline meets at the origin, the
/// glyph upright to it. A line the circle's centre doesn't reach is straight.
fn place(r: f64, along: f64, down: f64) -> Affine {
    let rl = r - down;
    if r != 0.0 && rl * r > 0.0 && rl.abs() > 1e-3 {
        let th = along / rl;
        if th.is_finite() {
            return Affine::translate((rl * th.sin(), r - rl * th.cos())) * Affine::rotate(th);
        }
    }
    Affine::translate((along, down))
}

/// `arc`, when given: 0 (straight) or a radius of 1 to 10000 mm either way.
fn arc_param(p: &Value, cmd: &str) -> Result<Option<f64>> {
    match p.get("arc") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v
            .as_f64()
            .filter(|a| *a == 0.0 || (1.0..=10000.0).contains(&a.abs()))
            .map(Some)
            .ok_or_else(|| bad(cmd, "`arc` is 0 (straight) or a radius of 1 to 10000 mm (negative: along a circle's bottom)")),
    }
}

/// `align`, when given.
fn align_param(p: &Value, cmd: &str) -> Result<Option<LetterAlign>> {
    match p.get("align") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_str().and_then(LetterAlign::from_name).map(Some).ok_or_else(|| bad(cmd, "`align` is left, centre or right")),
    }
}

fn lettering(s: &mut Session, p: &Value) -> Result<Value> {
    let text = text_param(p, SET)?.ok_or_else(|| bad(SET, "missing `text`"))?;
    let name = str_param(p, "font").ok_or_else(|| bad(SET, "missing `font`"))?;
    let font = loaded(s, name).ok_or_else(|| not_loaded(SET, name))?;
    let size = ranged(p, "size", 1.0..=500.0, SET, "`size` is 1 to 500 mm")?.unwrap_or_else(|| own_size(&font));
    let leading = ranged(p, "leading", 0.2..=5.0, SET, "`leading` is 0.2 to 5")?.unwrap_or(1.0);
    let at = match p.get("at") {
        None | Some(Value::Null) => s.doc()?.doc.artboards.first().map_or(Point::new(0.0, 0.0), |a| a.rect.center()),
        Some(_) => point_param(p, "at").filter(|q| q.x.is_finite() && q.y.is_finite()).ok_or_else(|| bad(SET, "`at` is [x, y]"))?,
    };
    let spec = Lettering {
        text: text.to_string(),
        font: font.info.name.clone(),
        size,
        leading,
        arc: arc_param(p, SET)?.unwrap_or(0.0),
        align: align_param(p, SET)?.unwrap_or_default(),
        frame: sf_core::Affine::translate(sf_core::Point::new(at.x, at.y)),
    };
    // scaled from the size the font was digitized at; outside what it sews well at, a warning
    let warnings = size_warnings(&font, size);
    let parent = s.doc()?.insertion_parent();
    let (id, count) = s.edit("Embroidery Lettering", |d, sel| {
        let glyphs = glyphs(d, &font, &spec);
        if glyphs.is_empty() {
            return Err(bad(SET, format!("{} has none of these characters", font.info.name)));
        }
        let count = glyphs.len();
        let mut all = Node::group(d.alloc_id(), glyphs);
        all.name = Some(lettering_name(&spec));
        all.embroidery = Some(Box::new(Embroidery::new(StitchType::Lettering(Box::new(spec.clone())))));
        let id = all.id;
        d.insert(parent, usize::MAX, all)?;
        sel.set([id]);
        Ok((id, count))
    })?;
    Ok(json!({ "id": id.0, "glyphs": count, "warnings": warnings }))
}

fn edit_lettering(s: &mut Session, p: &Value) -> Result<Value> {
    let ids = targets(s, p)?;
    let (id, mut spec) = lettering_of(&s.doc()?.doc, &ids).ok_or_else(|| bad(EDIT, "select lettering set in an embroidery font"))?;
    if let Some(t) = text_param(p, EDIT)? {
        spec.text = t.to_string();
    }
    if let Some(f) = str_param(p, "font") {
        spec.font = f.to_string();
    }
    let font = loaded(s, &spec.font).ok_or_else(|| not_loaded(EDIT, &spec.font))?;
    spec.font = font.info.name.clone();
    if let Some(v) = ranged(p, "size", 1.0..=500.0, EDIT, "`size` is 1 to 500 mm")? {
        spec.size = v;
    }
    if let Some(v) = ranged(p, "leading", 0.2..=5.0, EDIT, "`leading` is 0.2 to 5")? {
        spec.leading = v;
    }
    if let Some(v) = arc_param(p, EDIT)? {
        spec.arc = v;
    }
    if let Some(v) = align_param(p, EDIT)? {
        spec.align = v;
    }
    let warnings = size_warnings(&font, spec.size);
    let count = s.edit("Edit Lettering", |d, sel| {
        let glyphs = glyphs(d, &font, &spec);
        if glyphs.is_empty() {
            return Err(bad(EDIT, format!("{} has none of these characters", font.info.name)));
        }
        let count = glyphs.len();
        let n = d.node_mut(id).ok_or(EngineError::NoNode(id))?;
        let NodeKind::Group { children, .. } = &mut n.kind else { return Err(EngineError::NoNode(id)) };
        *children = glyphs;
        n.name = Some(lettering_name(&spec));
        if let Some(e) = n.embroidery.as_deref_mut() {
            e.stitch = StitchType::Lettering(Box::new(spec.clone()));
        }
        sel.set([id]);
        Ok(count)
    })?;
    Ok(json!({ "id": id.0, "glyphs": count, "warnings": warnings }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glyphs_go_round_arcs_upright_to_them() {
        let at = |m: Affine| m * vectorcraft_geom::Point::new(0.0, 0.0);
        let r = 100.0;
        let centre = vectorcraft_geom::Point::new(0.0, r);
        // arched over a circle: the origin on its top, the glyphs round it, their tops outward
        assert!(at(place(r, 0.0, 0.0)).distance(vectorcraft_geom::Point::new(0.0, 0.0)) < 1e-9);
        for along in [-80.0, 30.0, 150.0] {
            let m = place(r, along, 0.0);
            let p = at(m);
            assert!((p.distance(centre) - r).abs() < 1e-9, "{p:?}");
            let up = m * vectorcraft_geom::Point::new(0.0, -1.0) - p;
            let out = (p - centre) / r;
            assert!((up - out).hypot() < 1e-9, "{up:?} {out:?}");
        }
        // mirrored either side of the top
        let (a, b) = (at(place(r, -40.0, 0.0)), at(place(r, 40.0, 0.0)));
        assert!((a.x + b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9 && a.y > 0.0);
        // a second line 20 down: a circle 20 smaller
        assert!((at(place(r, 50.0, 20.0)).distance(centre) - 80.0).abs() < 1e-9);
        // along a circle's bottom: the ends rise, tops inward
        let c = vectorcraft_geom::Point::new(0.0, -r);
        let p = at(place(-r, 40.0, 0.0));
        assert!((p.distance(c) - r).abs() < 1e-9 && p.y < 0.0, "{p:?}");
        // a line past the centre, and a straight baseline, stay straight
        assert_eq!(at(place(r, 5.0, 150.0)), vectorcraft_geom::Point::new(5.0, 150.0));
        assert_eq!(at(place(0.0, 5.0, 7.0)), vectorcraft_geom::Point::new(5.0, 7.0));
    }
}
