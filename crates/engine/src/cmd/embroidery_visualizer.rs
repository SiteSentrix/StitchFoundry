//! The product visualizer (CW-05): the design's stitches, life size, on a garment or product drawn
//! in its colour (T-shirt, polo, hoodie, cap, tote bag, towel), at the usual place for it, or on
//! a photo of the customer's own product; a new document to show, print or export.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document};
use vectorcraft_embroidery::PT_PER_MM;

use super::embroidery::sheet_text;
use super::*;

/// Margin round the product (mm).
const MARGIN: f64 = 40.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "embroidery.visualize",
        "Product Visualizer",
        ["Embroidery"],
        None,
        "{product?: tshirt|polo|hoodie|cap|tote|towel (tshirt), color?: \"#rrggbb\" (the product's colour; white), placement?: left-chest|right-chest|full-front|full-back|yoke|sleeve|pocket (garments), cap-front (caps), full-front (totes, towels) (the product's usual), image?: a photo of the customer's own product instead (PNG, JPEG…), imageWidth?: mm the photo shows across (500; 50–3000), at?: [x, y] mm from the photo's top left where the design's middle goes (its middle)} the design's stitches life size on the product drawn in its colour (front, or back for full-back and yoke) at the usual place for it, or on the photo; a size larger than usual there is warned of; opens a new document → {product, placement, view, widthMm, heightMm, warnings}",
        has_doc,
        visualize
    )]
}

/// Where the design goes: its middle across at `x`, and its middle, top or bottom at `y` (mm).
#[derive(Clone, Copy)]
enum Anchor {
    Centre(f64, f64),
    Top(f64, f64),
    Bottom(f64, f64),
}

/// A product as drawn: its box (mm), outline and seam lines (SVG path data in mm), and where each
/// placement goes with the widest a design usually is there.
struct Product {
    size: (f64, f64),
    /// Filled in the product's colour.
    body: &'static [&'static str],
    /// Seams, hems and details, drawn in a darker shade.
    lines: &'static [&'static str],
    placements: &'static [(&'static str, Anchor, f64)],
    default: &'static str,
}

// The T-shirt's front: neckline, shoulders, short sleeves, straight body; 820 × 740 mm box.
const TEE: &str =
    "M 320 20 C 340 100 480 100 500 20 L 640 50 L 800 250 L 690 330 L 660 300 L 660 730 L 160 730 L 160 300 L 130 330 L 20 250 L 180 50 Z";
const TEE_BACK: &str =
    "M 320 20 C 340 45 480 45 500 20 L 640 50 L 800 250 L 690 330 L 660 300 L 660 730 L 160 730 L 160 300 L 130 330 L 20 250 L 180 50 Z";
const TEE_LINES: &[&str] = &["M 322 34 C 342 112 478 112 498 34", "M 160 705 L 660 705", "M 784 230 L 674 310", "M 36 230 L 146 310"];
const GARMENT_PLACES: &[(&str, Anchor, f64)] = &[
    ("left-chest", Anchor::Centre(500.0, 220.0), 100.0),
    ("right-chest", Anchor::Centre(320.0, 220.0), 100.0),
    ("full-front", Anchor::Top(410.0, 170.0), 300.0),
    ("full-back", Anchor::Top(410.0, 150.0), 350.0),
    ("yoke", Anchor::Top(410.0, 70.0), 125.0),
    ("sleeve", Anchor::Centre(705.0, 215.0), 100.0),
    ("pocket", Anchor::Centre(500.0, 240.0), 110.0),
];

const HOODIE: &str =
    "M 310 120 L 180 150 L 60 410 L 20 700 L 100 710 L 160 460 L 160 840 L 660 840 L 660 460 L 720 710 L 800 700 L 760 410 L 640 150 L 510 120 Z";
const HOOD: &str = "M 300 125 C 270 0 550 0 520 125 C 470 175 350 175 300 125 Z";
const HOODIE_LINES: &[&str] = &[
    "M 322 128 C 300 40 520 40 498 128",
    "M 260 640 L 560 640 L 600 780 L 220 780 Z",
    "M 160 815 L 660 815",
    "M 24 680 L 98 688",
    "M 796 680 L 722 688",
];
const HOODIE_PLACES: &[(&str, Anchor, f64)] = &[
    ("left-chest", Anchor::Centre(500.0, 300.0), 100.0),
    ("right-chest", Anchor::Centre(320.0, 300.0), 100.0),
    ("full-front", Anchor::Top(410.0, 260.0), 300.0),
    ("full-back", Anchor::Top(410.0, 230.0), 350.0),
    ("yoke", Anchor::Top(410.0, 150.0), 125.0),
    ("sleeve", Anchor::Centre(705.0, 300.0), 100.0),
];

const POLO_LINES: &[&str] = &[
    "M 395 75 L 425 75 L 425 230 L 395 230 Z",
    "M 320 20 L 400 95 L 360 112 L 300 30 Z",
    "M 500 20 L 420 95 L 460 112 L 520 30 Z",
    "M 160 705 L 660 705",
    "M 784 230 L 674 310",
    "M 36 230 L 146 310",
];

// crown, then the brim in front of it: separate shapes, so their overlap is filled
const CAP_CROWN: &str = "M 40 140 C 40 20 260 20 260 140 C 200 156 100 156 40 140 Z";
const CAP_BRIM: &str = "M 20 140 C 60 200 240 200 280 140 C 240 148 60 148 20 140 Z";
const CAP_LINES: &[&str] = &["M 150 52 L 150 146"];
const CAP_PLACES: &[(&str, Anchor, f64)] = &[("cap-front", Anchor::Bottom(150.0, 124.0), 130.0)];

const TOTE: &str = "M 20 160 L 400 160 L 400 550 L 20 550 Z";
const TOTE_LINES: &[&str] = &["M 100 160 C 100 10 200 10 200 160", "M 220 160 C 220 10 320 10 320 160", "M 20 185 L 400 185"];
const TOTE_PLACES: &[(&str, Anchor, f64)] = &[("full-front", Anchor::Centre(210.0, 370.0), 300.0)];

const TOWEL: &str = "M 20 20 L 520 20 L 520 920 L 20 920 Z";
const TOWEL_LINES: &[&str] = &["M 20 760 L 520 760", "M 20 820 L 520 820", "M 20 900 L 520 900", "M 20 40 L 520 40"];
const TOWEL_PLACES: &[(&str, Anchor, f64)] = &[("full-front", Anchor::Bottom(270.0, 740.0), 400.0)];

fn product(name: &str, back: bool) -> Option<Product> {
    let garment = |body, lines| Product { size: (820.0, 740.0), body, lines, placements: GARMENT_PLACES, default: "left-chest" };
    Some(match name {
        "tshirt" if back => garment(&[TEE_BACK], &TEE_LINES[1..]),
        "tshirt" => garment(&[TEE], TEE_LINES),
        "polo" if back => garment(&[TEE_BACK], &POLO_LINES[3..]),
        "polo" => garment(
            &["M 320 20 C 350 60 470 60 500 20 L 640 50 L 800 250 L 690 330 L 660 300 L 660 730 L 160 730 L 160 300 L 130 330 L 20 250 L 180 50 Z"],
            POLO_LINES,
        ),
        "hoodie" => Product { size: (820.0, 860.0), body: &[HOOD, HOODIE], lines: HOODIE_LINES, placements: HOODIE_PLACES, default: "full-front" },
        "cap" => Product { size: (300.0, 200.0), body: &[CAP_CROWN, CAP_BRIM], lines: CAP_LINES, placements: CAP_PLACES, default: "cap-front" },
        "tote" => Product { size: (420.0, 570.0), body: &[TOTE], lines: TOTE_LINES, placements: TOTE_PLACES, default: "full-front" },
        "towel" => Product { size: (540.0, 940.0), body: &[TOWEL], lines: TOWEL_LINES, placements: TOWEL_PLACES, default: "full-front" },
        _ => return None,
    })
}

/// What a product and a placement are called on the caption.
fn spoken(id: &str) -> &str {
    match id {
        "tshirt" => "T-shirt",
        "polo" => "polo shirt",
        "tote" => "tote bag",
        "left-chest" => "left chest",
        "right-chest" => "right chest",
        "full-front" => "full front",
        "full-back" => "full back",
        "cap-front" => "cap front",
        "yoke" => "back yoke",
        other => other,
    }
}

/// `svg` (mm) as a path in page space: scaled to points and moved by `(dx, dy)` mm.
fn svg_path(svg: &str, dx: f64, dy: f64) -> Result<vectorcraft_geom::path::PathData> {
    let bp = kurbo::BezPath::from_svg(svg).map_err(|e| EngineError::Other(format!("product outline: {e}")))?;
    let bp = kurbo::Affine::scale(PT_PER_MM) * kurbo::Affine::translate((dx, dy)) * bp;
    Ok(vectorcraft_geom::path::PathData::from_bezpath(&bp))
}

/// A path drawn as `appearance` on the first layer.
fn add_path(doc: &mut Document, path: vectorcraft_geom::path::PathData, appearance: Appearance) -> Result<()> {
    let n = vectorcraft_doc::Node::path(doc.alloc_id(), path, appearance);
    let layer = doc.layers.first().map(|l| l.id);
    doc.insert(layer, usize::MAX, n).map_err(|e| EngineError::Other(e.to_string()))?;
    Ok(())
}

/// A shade of `c` (`k` < 1 darker), for seams on the product's colour.
fn shade(c: sf_core::Rgb, k: f64) -> Color {
    let f = |v: u8| (f64::from(v) * k).round().clamp(0.0, 255.0) as u8;
    Color::rgb8(f(c.r), f(c.g), f(c.b))
}

fn visualize(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.visualize";
    let plan = s.embroidery_plan().ok_or_else(|| bad(CMD, "no document"))?;
    if plan.is_empty() {
        return Err(bad(CMD, "nothing is stitched yet: give objects a stitch type first"));
    }
    let pattern = plan.pattern().clone();
    let title = super::fileio::file_stem(&s.doc()?.doc.title);
    let b = pattern.bounds();
    let (w, h) = (b.width(), b.height());
    let image = str_param(p, "image").map(str::trim).filter(|i| !i.is_empty()).map(str::to_string);
    let name = str_param(p, "product").unwrap_or("tshirt");
    let color = match str_param(p, "color") {
        None => sf_core::Rgb::new(0xff, 0xff, 0xff),
        Some(c) => sf_core::Rgb::from_hex(c).ok_or_else(|| bad(CMD, "`color` is \"#rrggbb\""))?,
    };
    let mut warnings = Vec::new();
    // where the design's top left goes (mm), the page's size (mm) and what is drawn under it
    let (doc, page, origin, placement, view) = if let Some(path) = &image {
        let width = match p.get("imageWidth") {
            None | Some(Value::Null) => 500.0,
            Some(v) => v.as_f64().filter(|v| (50.0..=3000.0).contains(v)).ok_or_else(|| bad(CMD, "`imageWidth` is 50 to 3000 mm"))?,
        };
        let info = s.execute("file.place.info", &json!({ "path": path }))?;
        let (iw, ih) = (info["width"].as_f64().unwrap_or(0.0), info["height"].as_f64().unwrap_or(0.0));
        if !(iw.is_finite() && ih.is_finite() && iw > 0.0 && ih > 0.0) {
            return Err(bad(CMD, "the image has no size"));
        }
        let height = width * ih / iw;
        let at = match p.get("at") {
            None | Some(Value::Null) => (width / 2.0, height / 2.0),
            Some(v) => {
                let pt = v.as_array().filter(|a| a.len() == 2).and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)));
                pt.filter(|(x, y)| x.is_finite() && y.is_finite()).ok_or_else(|| bad(CMD, "`at` is [x, y] in mm"))?
            }
        };
        let mut doc = Document::new(width * PT_PER_MM, height * PT_PER_MM);
        doc.title = format!("{title} mock-up");
        (doc, (width, height), (at.0 - w / 2.0, at.1 - h / 2.0), "photo".to_string(), "photo")
    } else {
        let placement = str_param(p, "placement");
        let back = matches!(placement, Some("full-back" | "yoke"));
        let prod = product(name, back).ok_or_else(|| bad(CMD, "`product` is tshirt, polo, hoodie, cap, tote or towel"))?;
        let placement = placement.unwrap_or(prod.default);
        let Some((_, anchor, widest)) = prod.placements.iter().find(|(id, ..)| *id == placement) else {
            let ids: Vec<&str> = prod.placements.iter().map(|(id, ..)| *id).collect();
            return Err(bad(CMD, format!("on a {name}, `placement` is {}", ids.join(", "))));
        };
        if w > *widest {
            warnings.push(format!("{w:.0} mm wide: designs there are usually at most {widest:.0} mm"));
        }
        let (pw, ph) = (prod.size.0 + 2.0 * MARGIN, prod.size.1 + 2.0 * MARGIN + 20.0);
        let mut doc = Document::new(pw * PT_PER_MM, ph * PT_PER_MM);
        doc.title = format!("{title} on {}", spoken(name));
        let fill = Color::rgb8(color.r, color.g, color.b);
        // a light product gets a darker outline, a dark one a lighter
        let light = u32::from(color.r) + u32::from(color.g) + u32::from(color.b) > 3 * 90;
        let ink =
            if light { shade(color, 0.62) } else { Color::rgb8(color.r.saturating_add(70), color.g.saturating_add(70), color.b.saturating_add(70)) };
        for body in prod.body {
            let path = svg_path(body, MARGIN, MARGIN)?;
            add_path(&mut doc, path, Appearance::basic(Paint::solid(fill), Paint::solid(ink), 1.2 * PT_PER_MM))?;
        }
        for line in prod.lines {
            let path = svg_path(line, MARGIN, MARGIN)?;
            add_path(&mut doc, path, Appearance::basic(Paint::None, Paint::solid(ink), 0.8 * PT_PER_MM))?;
        }
        // a pocket drawn round the design
        if placement == "pocket" && !matches!(name, "hoodie" | "cap" | "tote" | "towel") {
            let path = svg_path("M 440 165 L 560 165 L 560 305 C 520 320 480 320 440 305 Z", MARGIN, MARGIN)?;
            add_path(&mut doc, path, Appearance::basic(Paint::solid(fill), Paint::solid(ink), 0.8 * PT_PER_MM))?;
        }
        let (ax, ay) = match *anchor {
            Anchor::Centre(x, y) => (x - w / 2.0, y - h / 2.0),
            Anchor::Top(x, y) => (x - w / 2.0, y),
            Anchor::Bottom(x, y) => (x - w / 2.0, y - h),
        };
        let caption = format!("{title} on a {}, {}: {w:.0} × {h:.0} mm, life size", spoken(name), spoken(placement));
        sheet_text(&mut doc, MARGIN * PT_PER_MM, (ph - 14.0) * PT_PER_MM, &caption, 24.0, Color::rgb8(0x44, 0x44, 0x44))?;
        (doc, (pw, ph), (ax + MARGIN, ay + MARGIN), placement.to_string(), if back { "back" } else { "front" })
    };
    // the stitches, life size, in their thread colours
    let mut doc = doc;
    for (i, runs) in super::embroidery::stitch_runs(&pattern).iter().enumerate() {
        use vectorcraft_geom::path::{PathData, SubPath};
        let subpaths: Vec<SubPath> = runs
            .iter()
            .filter(|r| r.len() >= 2)
            .map(|r| {
                let pts: Vec<kurbo::Point> =
                    r.iter().map(|q| kurbo::Point::new((q.x - b.min.x + origin.0) * PT_PER_MM, (q.y - b.min.y + origin.1) * PT_PER_MM)).collect();
                SubPath::polyline(&pts, false)
            })
            .collect();
        if subpaths.is_empty() {
            continue;
        }
        let c = pattern.thread(i).color;
        add_path(&mut doc, PathData::new(subpaths), Appearance::basic(Paint::None, Paint::solid(Color::rgb8(c.r, c.g, c.b)), 0.35 * PT_PER_MM))?;
    }
    let (wpt, hpt) = (page.0 * PT_PER_MM, page.1 * PT_PER_MM);
    s.add_document(doc, None);
    if let Some(path) = &image {
        // the photo under the design, filling the page
        s.execute("file.place", &json!({ "path": path, "rect": [0.0, 0.0, wpt, hpt] }))?;
        s.execute("object.arrange.sendToBack", &json!({}))?;
        s.execute("select.none", &json!({}))?;
    }
    Ok(json!({
        "product": if image.is_some() { "photo" } else { name },
        "placement": placement,
        "view": view,
        "widthMm": (w * 10.0).round() / 10.0,
        "heightMm": (h * 10.0).round() / 10.0,
        "warnings": warnings,
    }))
}
