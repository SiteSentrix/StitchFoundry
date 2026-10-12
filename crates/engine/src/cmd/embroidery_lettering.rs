//! Lettering with embroidery fonts: fonts in Ink/Stitch's format, read from folders users have
//! (`embroidery.font.load`; none ship with StitchFoundry, and each keeps its own licence), and
//! text set with them (`embroidery.lettering`): each glyph's digitized art placed along the lines,
//! kerned and scaled, a group per glyph.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node};
use vectorcraft_embroidery::fonts::{EmbFont, MAX_TEXT};
use vectorcraft_geom::Affine;

use super::*;

const LOAD: &str = "embroidery.font.load";
const SET: &str = "embroidery.lettering";
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
            "{text, font, size?: mm (its capitals' height; default: the font's own), at?: [x, y] (the start of the first line's baseline; default: the first artboard's centre), leading?: × the font's line height (1)} set text in a loaded embroidery font: each glyph its digitized art, kerned and scaled, a group per glyph inside one for the lettering; a size outside what the font sews well at comes with a warning → {id, glyphs, warnings}",
            has_doc,
            lettering
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

fn lettering(s: &mut Session, p: &Value) -> Result<Value> {
    let text = str_param(p, "text").filter(|t| !t.trim().is_empty()).ok_or_else(|| bad(SET, "missing `text`"))?;
    if text.chars().count() > MAX_TEXT {
        return Err(bad(SET, format!("at most {MAX_TEXT} characters")));
    }
    let name = str_param(p, "font").ok_or_else(|| bad(SET, "missing `font`"))?;
    let font = s
        .embroidery_fonts
        .iter()
        .find(|f| f.info.name.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| bad(SET, format!("no embroidery font `{name}` is loaded (embroidery.font.load)")))?;
    let own = if font.info.size > 0.0 { font.info.size } else { 20.0 };
    let size = match p.get("size") {
        None | Some(Value::Null) => own,
        Some(v) => v.as_f64().filter(|v| (1.0..=500.0).contains(v)).ok_or_else(|| bad(SET, "`size` is 1 to 500 mm"))?,
    };
    let leading = match p.get("leading") {
        None | Some(Value::Null) => 1.0,
        Some(v) => v.as_f64().filter(|v| (0.2..=5.0).contains(v)).ok_or_else(|| bad(SET, "`leading` is 0.2 to 5"))?,
    };
    let at = match p.get("at") {
        None | Some(Value::Null) => s.doc()?.doc.artboards.first().map_or(Point::new(0.0, 0.0), |a| a.rect.center()),
        Some(_) => point_param(p, "at").filter(|q| q.x.is_finite() && q.y.is_finite()).ok_or_else(|| bad(SET, "`at` is [x, y]"))?,
    };
    // scaled from the size the font was digitized at; outside what it sews well at, a warning
    let k = size / own;
    let mut warnings = Vec::new();
    let i = &font.info;
    if i.min_scale > 0.0 && k < i.min_scale - 1e-9 {
        warnings.push(format!("{} sews well from {:.1} mm", i.name, own * i.min_scale));
    }
    if i.max_scale > 0.0 && k > i.max_scale + 1e-9 {
        warnings.push(format!("{} sews well up to {:.1} mm", i.name, own * i.max_scale));
    }
    let placed = font.layout(text);
    let parent = s.doc()?.insertion_parent();
    let (id, count) = s.edit("Embroidery Lettering", |d, sel| {
        let mut glyphs = Vec::new();
        for g in &placed {
            let Some(glyph) = font.glyphs.get(&g.glyph) else { continue };
            let u = glyph.unit;
            // the glyph's art from its page to its place: its baseline on the line, its origin at x
            let m = Affine::translate((at.x + g.x * u * k, at.y + g.line as f64 * i.leading * leading * u * k))
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
            glyphs.push(Arc::new(grp));
        }
        if glyphs.is_empty() {
            return Err(bad(SET, format!("{} has none of these characters", i.name)));
        }
        let count = glyphs.len();
        let mut all = Node::group(d.alloc_id(), glyphs);
        let short: String = text.chars().take(40).collect();
        all.name = Some(format!("{short} ({})", i.name));
        let id = all.id;
        d.insert(parent, usize::MAX, all)?;
        sel.set([id]);
        Ok((id, count))
    })?;
    Ok(json!({ "id": id.0, "glyphs": count, "warnings": warnings }))
}
