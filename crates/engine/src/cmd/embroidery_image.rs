//! The design as an image (RP-04): its stitches drawn as lit thread, as the TrueView display shows
//! them (or flat), on white, a colour or transparent, written as PNG for a website, a customer or a
//! catalogue.

use serde_json::{Value, json};

use vectorcraft_doc::Document;
use vectorcraft_embroidery::PT_PER_MM;

use super::*;

/// Thread thickness of 40 wt polyester lying on fabric (mm), as the display draws it.
const THREAD_WIDTH_MM: f64 = 0.36;
/// The largest image (pixels a side, and in all).
const MAX_SIDE: u32 = 16_000;
const MAX_PIXELS: u64 = 120_000_000;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.exportImage",
            "Export Stitch Image",
            [],
            None,
            "{path?: a .png file to write (without it: the bytes), ppmm?: pixels a millimetre (10; 1–40), background?: \"#rrggbb\"|transparent (white), margin?: mm round the design (3; 0–100), view?: trueview|flat (trueview: lit thread as the TrueView display shows it)} the design's stitches as a PNG image → {path, format, bytes, width, height} (no path: {dataBase64, format, bytes, width, height})",
            has_doc,
            export_image
        ),
        cmd!(
            "embroidery.printView",
            "Print Stitch View",
            ["Embroidery"],
            None,
            "{scale?: life|fit (life: the stitches at their real size, over as many pages as it takes; fit: on one page), page?: a4|letter (a4), view?: trueview|flat} the stitch view as a document of pages to print (File › Print) or export, the stitches drawn as embroidery.exportImage draws them, 12 pixels a mm → {pages, scale}",
            has_doc,
            print_view
        ),
    ]
}

/// Light from the top left, a little towards the viewer (as the display's).
const LIGHT: [f64; 3] = [-0.45, -0.55, 0.70];

fn mix(dst: &mut [u8; 4], src: [f64; 3], a: f64) {
    // source over destination, straight alpha
    let da = f64::from(dst[3]) / 255.0;
    let oa = a + da * (1.0 - a);
    if oa <= 0.0 {
        return;
    }
    for (i, s) in src.iter().enumerate() {
        let d = f64::from(dst[i]);
        dst[i] = ((s * a + d * da * (1.0 - a)) / oa).round().clamp(0.0, 255.0) as u8;
    }
    dst[3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// One stitch from `a` to `b` (pixels) as thread `half` px wide each side: shaded as a lit
/// cylinder (or flat), its ends tucked in, edges anti-aliased.
fn stitch(img: &mut image::RgbaImage, a: (f64, f64), b: (f64, f64), half: f64, color: sf_core::Rgb, flat: bool) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    if !len.is_finite() || len < 0.01 {
        return;
    }
    let (tx, ty) = (dx / len, dy / len);
    let (nx, ny) = (-ty, tx);
    let inset = (half * 0.35).min(len * 0.2);
    let (w, h) = (img.width(), img.height());
    let lo_x = (a.0.min(b.0) - half - 1.0).floor().max(0.0) as u32;
    let lo_y = (a.1.min(b.1) - half - 1.0).floor().max(0.0) as u32;
    let hi_x = ((a.0.max(b.0) + half + 1.0).ceil().max(0.0) as u32).min(w);
    let hi_y = ((a.1.max(b.1) + half + 1.0).ceil().max(0.0) as u32).min(h);
    let base = [f64::from(color.r), f64::from(color.g), f64::from(color.b)];
    for y in lo_y..hi_y {
        for x in lo_x..hi_x {
            let (vx, vy) = (f64::from(x) + 0.5 - a.0, f64::from(y) + 0.5 - a.1);
            let along = vx * tx + vy * ty;
            let across = vx * nx + vy * ny;
            let cover_along = (along - inset + 0.5).min(len - inset - along + 0.5).clamp(0.0, 1.0);
            let cover_across = (half - across.abs() + 0.5).clamp(0.0, 1.0);
            let cover = cover_along * cover_across;
            if cover <= 0.0 {
                continue;
            }
            let rgb = if flat {
                base
            } else {
                let u = (across / half).clamp(-1.0, 1.0);
                let z = (1.0 - u * u).max(0.0).sqrt();
                let ndl = nx * u * LIGHT[0] + ny * u * LIGHT[1] + z * LIGHT[2];
                let k = 0.38 + 0.75 * ndl.max(0.0);
                let spec = (2.0 * ndl * z - LIGHT[2]).max(0.0).powf(12.0);
                let t = (spec * 0.55).min(0.6);
                base.map(|c| {
                    let lit = (c * k).clamp(0.0, 255.0);
                    if spec > 0.01 { lit + (255.0 - lit) * t } else { lit }
                })
            };
            let px = img.get_pixel_mut(x, y);
            let mut d = px.0;
            mix(&mut d, rgb, cover);
            px.0 = d;
        }
    }
}

/// The pattern drawn at `ppmm` pixels a millimetre with `margin` mm round it.
fn render(pattern: &sf_core::Pattern, ppmm: f64, margin: f64, background: [u8; 4], flat: bool) -> std::result::Result<image::RgbaImage, String> {
    let b = pattern.bounds();
    if b.is_empty() {
        return Err("the design has no stitches".into());
    }
    let (w, h) = ((b.width() + 2.0 * margin) * ppmm, (b.height() + 2.0 * margin) * ppmm);
    if !(w.is_finite() && h.is_finite())
        || w.ceil() > f64::from(MAX_SIDE)
        || h.ceil() > f64::from(MAX_SIDE)
        || (w.ceil() * h.ceil()) as u64 > MAX_PIXELS
    {
        return Err(format!("the image would be {w:.0} × {h:.0} pixels: lower ppmm"));
    }
    let (w, h) = ((w.ceil() as u32).max(1), (h.ceil() as u32).max(1));
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba(background));
    let to_px = |p: sf_core::Point| ((p.x - b.min.x + margin) * ppmm, (p.y - b.min.y + margin) * ppmm);
    let half = (THREAD_WIDTH_MM * ppmm * 0.5).max(0.5);
    let (mut thread, mut prev, mut sewing) = (0usize, None, false);
    let mut color = pattern.thread(0).color;
    for st in &pattern.stitches {
        let p = to_px(st.pos());
        match st.cmd {
            sf_core::Cmd::Stitch => {
                if let Some(a) = prev
                    && sewing
                {
                    stitch(&mut img, a, p, half, color, flat);
                }
                prev = Some(p);
                sewing = true;
            }
            sf_core::Cmd::Jump => {
                prev = Some(p);
                sewing = false;
            }
            sf_core::Cmd::Trim => sewing = false,
            sf_core::Cmd::ColorChange => {
                thread += 1;
                color = pattern.thread(thread).color;
                sewing = false;
            }
            sf_core::Cmd::Stop | sf_core::Cmd::End => {}
        }
    }
    Ok(img)
}

fn export_image(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.exportImage";
    let num = |key: &str, default: f64, range: std::ops::RangeInclusive<f64>, what: &str| -> Result<f64> {
        match p.get(key) {
            None | Some(Value::Null) => Ok(default),
            Some(v) => v.as_f64().filter(|v| range.contains(v)).ok_or_else(|| bad(CMD, what.to_string())),
        }
    };
    let ppmm = num("ppmm", 10.0, 1.0..=40.0, "`ppmm` is 1 to 40")?;
    let margin = num("margin", 3.0, 0.0..=100.0, "`margin` is 0 to 100 mm")?;
    let background = match str_param(p, "background") {
        None => [255, 255, 255, 255],
        Some("transparent") => [255, 255, 255, 0],
        Some(c) => {
            let c = sf_core::Rgb::from_hex(c).ok_or_else(|| bad(CMD, "`background` is \"#rrggbb\" or transparent"))?;
            [c.r, c.g, c.b, 255]
        }
    };
    let flat = match str_param(p, "view").unwrap_or("trueview") {
        "trueview" => false,
        "flat" => true,
        _ => return Err(bad(CMD, "`view` is trueview or flat")),
    };
    let plan = s.embroidery_plan().ok_or_else(|| bad(CMD, "no document"))?;
    if plan.is_empty() {
        return Err(bad(CMD, "nothing is stitched yet: give objects a stitch type first"));
    }
    let img = render(plan.pattern(), ppmm, margin, background, flat).map_err(|e| bad(CMD, e))?;
    let (width, height) = (img.width(), img.height());
    let mut bytes = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png)
        .map_err(|e| EngineError::Other(format!("PNG encoding failed: {e}")))?;
    match str_param(p, "path").map(str::trim).filter(|p| !p.is_empty()) {
        Some(path) => {
            super::fileio::write_file(path, &bytes)?;
            Ok(json!({ "path": path, "format": "png", "bytes": bytes.len(), "width": width, "height": height }))
        }
        None => Ok(json!({
            "dataBase64": vectorcraft_format::base64_encode(&bytes),
            "format": "png",
            "bytes": bytes.len(),
            "width": width,
            "height": height,
        })),
    }
}

/// The stitch view as pages to print: the design's stitches as an image, life size (tiled over
/// as many pages as it takes) or fitted to one page.
fn print_view(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.printView";
    let (pw, ph) = match str_param(p, "page").unwrap_or("a4") {
        "a4" | "A4" => (595.0, 842.0),
        "letter" | "Letter" => (612.0, 792.0),
        _ => return Err(bad(CMD, "`page` is a4 or letter")),
    };
    let fit = match str_param(p, "scale").unwrap_or("life") {
        "life" => false,
        "fit" => true,
        _ => return Err(bad(CMD, "`scale` is life or fit")),
    };
    let view = str_param(p, "view").unwrap_or("trueview");
    let margin = 36.0;
    let img = s.execute("embroidery.exportImage", &json!({ "ppmm": 12, "margin": 2, "view": view }))?;
    let (iw, ih) = (img["width"].as_f64().unwrap_or(0.0), img["height"].as_f64().unwrap_or(0.0));
    if iw <= 0.0 || ih <= 0.0 {
        return Err(bad(CMD, "nothing to print"));
    }
    // the image's size on paper (pt): 12 pixels a mm
    let (aw, ah) = (pw - 2.0 * margin, ph - 2.0 * margin);
    let life = (iw / 12.0 * PT_PER_MM, ih / 12.0 * PT_PER_MM);
    let (w, h) = if fit || (life.0 <= aw && life.1 <= ah) {
        let k = (aw / life.0).min(ah / life.1).min(if fit { f64::INFINITY } else { 1.0 });
        (life.0 * k, life.1 * k)
    } else {
        life
    };
    let (cols, rows) = ((w / aw).ceil().max(1.0) as usize, (h / ah).ceil().max(1.0) as usize);
    if cols * rows > 64 {
        return Err(bad(CMD, "more than 64 pages: use scale fit"));
    }
    let title = super::fileio::file_stem(&s.doc()?.doc.title);
    let mut doc = Document::new(pw, ph);
    doc.title = format!("{title} print");
    // the pages in a grid, touching, so the image runs across them
    for r in 0..rows {
        for c in 0..cols {
            if r == 0 && c == 0 {
                continue;
            }
            let id = doc.next_artboard_id();
            let (x, y) = (c as f64 * aw, r as f64 * ah);
            doc.artboards.push(vectorcraft_doc::Artboard {
                id,
                name: format!("Page {}", r * cols + c + 1),
                rect: vectorcraft_geom::Rect::new(x - margin, y - margin, x - margin + pw, y - margin + ph),
                show_center_mark: false,
                show_cross_hairs: false,
            });
        }
    }
    if let Some(a) = doc.artboards.first_mut() {
        a.rect = vectorcraft_geom::Rect::new(-margin, -margin, pw - margin, ph - margin);
    }
    let pages = doc.artboards.len();
    s.add_document(doc, None);
    // the image centred on a single page, else from the first page's corner
    let (x, y) = if pages == 1 { ((aw - w) / 2.0, (ah - h) / 2.0) } else { (0.0, 0.0) };
    s.execute("file.place", &json!({ "name": format!("{title}.png"), "dataBase64": img["dataBase64"], "rect": [x, y, w, h] }))?;
    s.execute("select.none", &json!({}))?;
    Ok(json!({ "pages": pages, "scale": if (w - life.0).abs() < 0.5 { 1.0 } else { (w / life.0 * 1000.0).round() / 1000.0 } }))
}
