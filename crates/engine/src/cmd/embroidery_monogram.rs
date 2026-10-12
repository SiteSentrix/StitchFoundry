//! Monograms: one to three letters in an embroidery font, the middle one of three larger, and a
//! border round them. Each letter is lettering that can be set again.

use std::sync::Arc;

use serde_json::{Value, json};
use sf_core::attr::{Embroidery, LetterAlign, LetterSewing, Lettering, StitchType};
use sf_core::model::{Motif, RunKind, RunParams, SatinParams};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Node};
use vectorcraft_embroidery::PT_PER_MM;
use vectorcraft_geom::Rect;
use vectorcraft_geom::path::{PathData, SubPath};

use super::embroidery_lettering::{ensure_fonts, glyphs, lettering_name, loaded, not_loaded, own_size, ranged, size_warnings};
use super::*;

const CMD: &str = "embroidery.monogram";
/// The middle letter of a classic monogram of three, × the others.
const BIG: f64 = 1.6;
/// How far the border keeps clear of the letters, and a satin border's width (mm).
const MARGIN: f64 = 3.0;
const BORDER_WIDTH: f64 = 2.5;
/// The borders there are.
pub const BORDERS: [&str; 5] = ["none", "circle", "diamond", "square", "scallop"];

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "embroidery.monogram",
        "Monogram",
        [],
        None,
        "{letters: 1 to 3 characters, font: a loaded embroidery font, size?: mm (the letters' height, the middle one of three larger in a classic monogram; default: the font's own), style?: classic|line (classic: the middle of three letters larger, each centred on the monogram's middle; line: all one size on one baseline), border?: none|circle|diamond|square|scallop (none), at?: [x, y] (its middle; default: the first artboard's centre)} a monogram: each letter lettering in the font (set again with embroidery.lettering.edit), round them a satin border (a scalloped one a scallop motif run), in a group → {id, letters, warnings}",
        has_doc,
        monogram
    )]
}

/// A border's outline round `b` (points), `margin` clear of it: a circle (scalloped too), a
/// diamond or a square.
fn border_points(kind: &str, b: Rect, margin: f64) -> Vec<Point> {
    let c = b.center();
    let (hw, hh) = (b.width() / 2.0 + margin, b.height() / 2.0 + margin);
    match kind {
        // through the corners' diagonals: |x| + |y| reaches hw + hh
        "diamond" => {
            let r = hw + hh;
            vec![Point::new(c.x, c.y - r), Point::new(c.x + r, c.y), Point::new(c.x, c.y + r), Point::new(c.x - r, c.y)]
        }
        "square" => {
            let r = hw.max(hh);
            vec![Point::new(c.x - r, c.y - r), Point::new(c.x + r, c.y - r), Point::new(c.x + r, c.y + r), Point::new(c.x - r, c.y + r)]
        }
        _ => {
            let r = hw.hypot(hh);
            (0..96)
                .map(|k| {
                    let a = f64::from(k) / 96.0 * std::f64::consts::TAU;
                    Point::new(c.x + r * a.cos(), c.y + r * a.sin())
                })
                .collect()
        }
    }
}

fn monogram(s: &mut Session, p: &Value) -> Result<Value> {
    let letters: Vec<char> = str_param(p, "letters").map(|t| t.chars().filter(|c| !c.is_whitespace()).collect()).unwrap_or_default();
    if !(1..=3).contains(&letters.len()) {
        return Err(bad(CMD, "`letters` are 1 to 3 characters"));
    }
    let name = str_param(p, "font").ok_or_else(|| bad(CMD, "missing `font`"))?;
    let mut warnings = ensure_fonts(s);
    let font = loaded(s, name).ok_or_else(|| not_loaded(CMD, name))?;
    let size = ranged(p, "size", 1.0..=300.0, CMD, "`size` is 1 to 300 mm")?.unwrap_or_else(|| own_size(&font));
    let classic = match str_param(p, "style") {
        None | Some("classic") => true,
        Some("line") => false,
        Some(o) => return Err(bad(CMD, format!("unknown style `{o}`: classic or line"))),
    };
    let border = match str_param(p, "border") {
        None => "none",
        Some(b) => {
            BORDERS.iter().copied().find(|k| *k == b).ok_or_else(|| bad(CMD, format!("unknown border `{b}`: one of {}", BORDERS.join(", "))))?
        }
    };
    let at = match p.get("at") {
        None | Some(Value::Null) => s.doc()?.doc.artboards.first().map_or(Point::new(0.0, 0.0), |a| a.rect.center()),
        Some(_) => point_param(p, "at").filter(|q| q.x.is_finite() && q.y.is_finite()).ok_or_else(|| bad(CMD, "`at` is [x, y]"))?,
    };
    // each letter's height, and its width from the font's advances (points)
    let sizes: Vec<f64> = (0..letters.len()).map(|i| if classic && letters.len() == 3 && i == 1 { size * BIG } else { size }).collect();
    let width = |c: char, h: f64| {
        let k = h / own_size(&font);
        font.layout(&c.to_string()).iter().fold(0.0f64, |w, g| w.max((g.x + g.advance) * font.glyphs.get(&g.glyph).map_or(0.0, |gl| gl.unit) * k))
    };
    let widths: Vec<f64> = letters.iter().zip(&sizes).map(|(c, h)| width(*c, *h)).collect();
    let gap = size * PT_PER_MM * 0.25;
    let total = widths.iter().sum::<f64>() + gap * (letters.len() - 1) as f64;
    let tallest = sizes.iter().fold(0.0f64, |m, h| m.max(*h)) * PT_PER_MM;
    // left to right, centred on `at`: classic letters each centred on it up and down, a line on
    // one baseline
    let mut x = at.x - total / 2.0;
    let mut specs = Vec::new();
    for ((c, h), w) in letters.iter().zip(&sizes).zip(&widths) {
        let base = if classic { at.y + h * PT_PER_MM / 2.0 } else { at.y + tallest / 2.0 };
        specs.push(Lettering {
            text: c.to_string(),
            font: font.info.name.clone(),
            size: *h,
            leading: 1.0,
            arc: 0.0,
            align: LetterAlign::Centre,
            sew: LetterSewing::default(),
            letters: Vec::new(),
            puff: None,
            frame: sf_core::Affine::translate(sf_core::Point::new(x + w / 2.0, base)),
        });
        x += w + gap;
        for w in size_warnings(&font, *h) {
            if !warnings.contains(&w) {
                warnings.push(w);
            }
        }
    }
    let paint = [&s.paint.stroke, &s.paint.fill]
        .into_iter()
        .find(|p| matches!(p, Paint::Solid { .. }))
        .cloned()
        .unwrap_or_else(|| Paint::solid(Color::rgb8(0x1a, 0x3e, 0x8c)));
    let label: String = letters.iter().collect();
    let parent = s.doc()?.insertion_parent();
    let (id, count) = s.edit("Monogram", |d, sel| {
        let mut parts = Vec::new();
        for spec in &specs {
            let glyphs = glyphs(d, &font, spec);
            if glyphs.is_empty() {
                return Err(bad(CMD, format!("{} has none of these characters", font.info.name)));
            }
            let mut g = Node::group(d.alloc_id(), glyphs);
            g.name = Some(lettering_name(spec));
            g.embroidery = Some(Box::new(Embroidery::new(StitchType::Lettering(Box::new(spec.clone())))));
            parts.push(Arc::new(g));
        }
        let count = parts.len();
        // the border round them all
        let bounds = parts.iter().filter_map(|n| n.geometric_bounds()).reduce(|a, b| a.union(b));
        if let Some(b) = bounds.filter(|_| border != "none") {
            let pts = border_points(border, b, MARGIN * PT_PER_MM);
            let mut e = if border == "scallop" {
                Embroidery::new(StitchType::Run(RunParams {
                    kind: RunKind::Motif,
                    motif: Motif::Scallop,
                    width: 4.0,
                    spacing: 5.0,
                    ..RunParams::default()
                }))
            } else {
                Embroidery::new(StitchType::Satin(SatinParams::default()))
            };
            if border != "scallop" {
                e.width = Some(BORDER_WIDTH);
            }
            let mut n = Node::path(d.alloc_id(), PathData::single(SubPath::polyline(&pts, true)), Appearance::basic(Paint::None, paint.clone(), 1.0));
            n.name = Some("Border".into());
            n.embroidery = Some(Box::new(e));
            parts.push(Arc::new(n));
        }
        let mut all = Node::group(d.alloc_id(), parts);
        all.name = Some(format!("Monogram {label}"));
        let id = all.id;
        d.insert(parent, usize::MAX, all)?;
        sel.set([id]);
        Ok((id, count))
    })?;
    Ok(json!({ "id": id.0, "letters": count, "warnings": warnings }))
}
