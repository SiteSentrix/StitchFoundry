//! Effect → Document Raster Effects Settings, and raster effects for PDF export.
//!
//! PDF has no live shadows, glows or blurs, so PDF export receives a copy of the document in which
//! every object with a raster effect is accompanied by an image rendered with the document's raster
//! effects settings (resolution, colour model, background, anti-aliasing, room around the art).
//! Shadows and outer glows only add the effect: the image (with the object's own area knocked out)
//! goes under the untouched vector object. Effects that change the object itself (inner glow,
//! feather, Gaussian blur) replace it with the image.

use std::sync::Arc;

use serde_json::{Map, Value, json};
use vectorcraft_color::Paint;
use vectorcraft_doc::rastersettings::MAX_ADD_AROUND;
use vectorcraft_doc::{Appearance, ColorMode, Document, ImageBlob, ImageObject, Node, NodeId, NodeKind, RasterColorModel};
use vectorcraft_geom::{Affine, FillRule, PathData, Rect};
use vectorcraft_render::effects::{self, RasterFx};
use vectorcraft_render::{AntiAlias, RenderOptions};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "document.rasterEffectsSettings",
        "Document Raster Effects Settings…",
        ["Effect"],
        None,
        "{resolution?: ppi (1–2400) | \"screen\" (72) | \"medium\" (150) | \"high\" (300), colorModel?: \"rgb\"|\"cmyk\" (the document's mode)|\"grayscale\"|\"bitmap\", background?: \"transparent\"|\"white\", antiAlias?: bool (off: hard edges), clippingMask?: bool (white background only under the art; Rasterize clips to the art), addAround?: pt (0–1000, room around the art), preserveSpotColors?: bool (stored)} how raster effects (shadows, glows, blurs, feathers) become images in PDF export and Expand Appearance, and the defaults of object.rasterize; one undo step; always → the settings",
        has_doc,
        settings
    )]
}

const C: &str = "document.rasterEffectsSettings";

/// What `document.rasterEffectsSettings` reports (and takes back).
fn settings_json(d: &Document) -> Value {
    let r = &d.raster_effects;
    let model = match (r.color_model, d.color_mode) {
        (RasterColorModel::Document, ColorMode::Rgb) => "rgb",
        (RasterColorModel::Document, ColorMode::Cmyk) => "cmyk",
        (m, _) => m.id(),
    };
    json!({
        "resolution": d.raster_effects_ppi,
        "colorModel": model,
        "background": r.background.id(),
        "antiAlias": r.anti_alias,
        "clippingMask": r.clipping_mask,
        "addAround": r.add_around,
        "preserveSpotColors": r.preserve_spot_colors,
    })
}

fn resolution(v: &Value) -> Result<f64> {
    let ppi = match v {
        Value::String(v) => match v.to_ascii_lowercase().as_str() {
            "screen" => 72.0,
            "medium" => 150.0,
            "high" => 300.0,
            other => other.trim_end_matches("ppi").trim().parse::<f64>().map_err(|_| bad(C, format!("unknown resolution `{v}`")))?,
        },
        v => v.as_f64().ok_or_else(|| bad(C, "resolution must be a number or screen|medium|high"))?,
    };
    if !(1.0..=2400.0).contains(&ppi) {
        return Err(bad(C, "resolution must be between 1 and 2400 ppi"));
    }
    Ok(ppi)
}

/// `colorModel` for a document in `mode`: its own mode's name means the document's model.
pub(crate) fn color_model(v: &Value, mode: ColorMode, cmd: &str) -> Result<RasterColorModel> {
    let s = v.as_str().unwrap_or_default().to_ascii_lowercase();
    match (s.as_str(), mode) {
        ("document", _) | ("rgb", ColorMode::Rgb) | ("cmyk", ColorMode::Cmyk) => Ok(RasterColorModel::Document),
        ("grayscale" | "gray", _) => Ok(RasterColorModel::Grayscale),
        ("bitmap", _) => Ok(RasterColorModel::Bitmap),
        _ => {
            let own = if mode == ColorMode::Rgb { "rgb" } else { "cmyk" };
            Err(bad(cmd, format!("colorModel must be {own} (the document's mode), grayscale or bitmap")))
        }
    }
}

/// `addAround`/`padding` in points.
pub(crate) fn add_around(v: &Value, cmd: &str, key: &str) -> Result<f64> {
    v.as_f64().filter(|x| (0.0..=MAX_ADD_AROUND).contains(x)).ok_or_else(|| bad(cmd, format!("{key} must be 0–{MAX_ADD_AROUND} pt")))
}

fn flag(k: &str, v: &Value) -> Result<bool> {
    v.as_bool().ok_or_else(|| bad(C, format!("{k} must be true or false")))
}

fn settings(s: &mut Session, p: &Value) -> Result<Value> {
    let empty = Map::new();
    let o = match p {
        Value::Null => &empty,
        Value::Object(o) => o,
        _ => return Err(bad(C, "params must be an object")),
    };
    let d = &s.doc()?.doc;
    let (mut ppi, mut r) = (d.raster_effects_ppi, d.raster_effects.clone());
    // A null value leaves its setting as it is.
    for (k, v) in o.iter().filter(|(_, v)| !v.is_null()) {
        match k.as_str() {
            "resolution" => ppi = resolution(v)?,
            "colorModel" => r.color_model = color_model(v, d.color_mode, C)?,
            "background" => r.background = super::docsetup::background_param(v, C)?,
            "antiAlias" => r.anti_alias = flag(k, v)?,
            "clippingMask" => r.clipping_mask = flag(k, v)?,
            "addAround" => r.add_around = add_around(v, C, k)?,
            "preserveSpotColors" => r.preserve_spot_colors = flag(k, v)?,
            _ => return Err(bad(C, format!("unknown setting `{k}`"))),
        }
    }
    if ppi != d.raster_effects_ppi || r != d.raster_effects {
        s.edit("Document Raster Effects Settings", |d, _| {
            d.raster_effects_ppi = ppi;
            d.raster_effects = r;
            Ok(())
        })?;
    }
    Ok(settings_json(&s.doc()?.doc))
}

/// `art` in a clip group clipped by `path` filled by `rule` (a clipping path that paints nothing).
pub(crate) fn clip_group(d: &mut Document, path: PathData, rule: FillRule, art: Node) -> Node {
    let mut clip = super::pathops::shape_node(d, path, None);
    clip.appearance = Appearance::basic(Paint::None, Paint::None, 0.0);
    match &mut clip.kind {
        NodeKind::Path { clipping, rule: r, .. } => (*clipping, *r) = (true, rule),
        NodeKind::Compound { rule: r, .. } => *r = rule,
        _ => {}
    }
    Node::new(d.alloc_id(), NodeKind::Group { children: vec![Arc::new(clip), Arc::new(art)], clip: true })
}

fn raster_fx(n: &Node) -> Vec<RasterFx> {
    if !matches!(n.kind, NodeKind::Path { guide: false, .. } | NodeKind::Compound { .. }) {
        return vec![];
    }
    effects::raster_effects(&n.appearance.effects)
}

fn needs(n: &Node) -> bool {
    !raster_fx(n).is_empty() || n.children().is_some_and(|ch| ch.iter().any(|c| needs(c)))
}

/// Render `nodes` alone over transparency: (premultiplied pixels, width, height).
fn render(doc: &Document, nodes: Vec<Node>, region: Rect, scale: f64, anti_alias: AntiAlias) -> vectorcraft_render::Rendered {
    let mut tmp = doc.clone();
    let mut layer = Node::layer(NodeId(u64::MAX), "raster", vectorcraft_doc::LayerColor::Preset(0));
    if let Some(ch) = layer.children_mut() {
        *ch = nodes.into_iter().map(Arc::new).collect();
    }
    tmp.layers = vec![Arc::new(layer)];
    let mut r = vectorcraft_render::Renderer::new();
    r.threads = 0;
    r.render_region_with(&tmp, region, scale, &RenderOptions { skip_templates: true, anti_alias, ..Default::default() })
}

/// Pixels per point of raster effects rendered as images (the document's raster effects
/// resolution).
pub(crate) fn effects_scale(doc: &Document) -> f64 {
    (doc.raster_effects_ppi / 72.0).clamp(1.0 / 72.0, 2400.0 / 72.0)
}

/// An embedded image (a new node of `out`, its pixels in `out.images`) of `whole` rendered alone
/// with `out`'s resources at `scale` over its visual bounds and the reach of its raster effects
/// (its own and its fills' and strokes'). With `knockout`, the coverage of that art is knocked out
/// of the image, which then only holds what the effects add around it (a shadow to go under the
/// vector object). The document's raster effects settings add room around it and finish its pixels.
pub(crate) fn effect_image(out: &mut Document, whole: &Node, knockout: Option<&Node>, scale: f64) -> Option<Node> {
    let b = whole.visual_bounds()?;
    let look = out.raster_effects.clone();
    let reach = whole.appearance.items.iter().map(|i| effects::outset(i.effects())).fold(effects::outset(&whole.appearance.effects), f64::max)
        + 2.0
        + look.add_around;
    let b = b.inflate(reach, reach);
    // Whole pixels, and no larger than 64 Mpx.
    let scale = scale.min((64.0e6 / (b.width() * b.height()).max(1.0)).sqrt());
    let region = Rect::new(b.x0, b.y0, b.x0 + (b.width() * scale).ceil().max(1.0) / scale, b.y0 + (b.height() * scale).ceil().max(1.0) / scale);
    // Anti-alias off: hard edges (the effects themselves stay smooth).
    let anti_alias = if look.anti_alias { AntiAlias::Art } else { AntiAlias::None };
    let mut img = render(out, vec![whole.clone()], region, scale, anti_alias);
    if let Some(bare) = knockout {
        let obj = render(out, vec![bare.clone()], region, scale, anti_alias);
        for (px, o) in img.pixels.as_chunks_mut::<4>().0.iter_mut().zip(obj.pixels.as_chunks::<4>().0) {
            let keep = 1.0 - o[3] as f32 / 255.0;
            for c in px.iter_mut() {
                *c = (*c as f32 * keep).round() as u8;
            }
        }
    }
    // Document Raster Effects Settings: colour model, background.
    look.finish_pixels(&mut img.pixels);
    // Can't encode: keep the object as it is (vector, effects ignored) rather than fail the export.
    let png = img.to_png().ok()?;
    let id = out.alloc_id();
    let mut key = format!("raster-effect-{}", id.0);
    while out.images.contains_key(&key) {
        key.push('+');
    }
    out.images.insert(key.clone(), ImageBlob { mime: "image/png".into(), bytes: Arc::new(png) });
    let xf = Affine::translate(region.origin().to_vec2()) * Affine::scale(1.0 / scale);
    let mut image = Node::new(id, NodeKind::Image(ImageObject { key, width: img.width, height: img.height, xf, link: None }));
    image.name = Some("Raster effect".into());
    Some(image)
}

/// The image (and, for shadows and outer glows, the vector object above it) standing in for `n`.
fn flatten_node(out: &mut Document, n: &Node, scale: f64) -> Option<Node> {
    let below_only = raster_fx(n).iter().all(RasterFx::is_below);
    let mut whole = n.clone();
    whole.blend = vectorcraft_doc::color::BlendMode::Normal;
    // Shadows only: knock the object's own coverage out of the image; the vector object goes on top.
    let bare = below_only.then(|| {
        let mut bare = whole.clone();
        bare.appearance.effects.retain(|e| !effects::is_raster(&e.id));
        bare.opacity = 1.0;
        bare
    });
    let mut image = effect_image(out, &whole, bare.as_ref(), scale)?;
    let vector = bare.map(|_| {
        let mut v = n.clone();
        v.appearance.effects.retain(|e| !effects::is_raster(&e.id));
        v.blend = vectorcraft_doc::color::BlendMode::Normal;
        v
    });
    Some(match vector {
        Some(v) => {
            let mut g = Node::group(out.alloc_id(), vec![Arc::new(image), Arc::new(v)]);
            g.blend = n.blend;
            g.name = n.name.clone();
            g
        }
        None => {
            image.blend = n.blend;
            image
        }
    })
}

fn walk(out: &mut Document, n: &Node, scale: f64) -> Option<Node> {
    if !needs(n) || !n.visible {
        return None;
    }
    if !raster_fx(n).is_empty() {
        return flatten_node(out, n, scale);
    }
    let mut m = n.clone();
    let ch: Vec<Arc<Node>> = n.children()?.iter().map(|c| walk(out, c, scale).map(Arc::new).unwrap_or_else(|| c.clone())).collect();
    if let Some(slot) = m.children_mut() {
        *slot = ch;
    }
    Some(m)
}

/// A copy of `doc` with raster effects turned into images at the document's raster effects
/// resolution, or `None` when there are none.
pub fn flatten_raster_effects(doc: &Document) -> Option<Document> {
    if !doc.layers.iter().any(|l| needs(l)) {
        return None;
    }
    // Geometry effects first, so the vector objects kept above shadows are final.
    let baked = effects::bake_document(doc);
    let src = baked.as_ref().unwrap_or(doc);
    let scale = effects_scale(src);
    let mut out = src.clone();
    let layers = src.layers.clone();
    out.layers = layers.iter().map(|l| walk(&mut out, l, scale).map(Arc::new).unwrap_or_else(|| l.clone())).collect();
    Some(out)
}

/// PDF bytes for `doc`, with raster effects rendered as images.
pub fn export_pdf(doc: &Document, opts: &vectorcraft_pdf::PdfOptions) -> std::result::Result<Vec<u8>, vectorcraft_pdf::PdfError> {
    export_pdf_with_report(doc, opts).map(|r| r.bytes)
}

/// Like [`export_pdf`], also returning the writer's warnings (options not applied yet, features
/// approximated or left out).
pub fn export_pdf_with_report(
    doc: &Document,
    opts: &vectorcraft_pdf::PdfOptions,
) -> std::result::Result<vectorcraft_pdf::ExportReport, vectorcraft_pdf::PdfError> {
    match flatten_raster_effects(doc) {
        Some(d) => vectorcraft_pdf::export_with_report(&d, opts),
        None => vectorcraft_pdf::export_with_report(doc, opts),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_doc::NodeKind;

    use crate::Session;

    #[test]
    fn settings_and_pdf_flattening() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
        assert_eq!(s.execute("document.rasterEffectsSettings", &json!({})).unwrap()["resolution"], json!(72.0));
        s.execute("document.rasterEffectsSettings", &json!({"resolution": "high"})).unwrap();
        assert!(s.execute("document.rasterEffectsSettings", &json!({"resolution": 0})).is_err());
        let r = s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap()["id"].as_u64().unwrap();
        let b = s.execute("shape.ellipse", &json!({"x": 180, "y": 50, "width": 80, "height": 80})).unwrap()["id"].as_u64().unwrap();
        s.execute("effect.apply", &json!({"effect": "stylize.dropShadow", "ids": [r]})).unwrap();
        s.execute("effect.apply", &json!({"effect": "blur.gaussian", "params": {"radius": 4}, "ids": [b]})).unwrap();
        let doc = s.doc().unwrap().doc.clone();
        let flat = super::flatten_raster_effects(&doc).unwrap();
        let kids = flat.layers[0].children().unwrap();
        // Shadow: an image (300 ppi) under the vector rectangle, which keeps no raster effect.
        let NodeKind::Group { children, .. } = &kids[0].kind else { panic!("shadow becomes a group: {:?}", kids[0].kind_label()) };
        let NodeKind::Image(im) = &children[0].kind else { panic!() };
        assert!(im.width as f64 > 100.0 * 300.0 / 72.0);
        assert!(children[1].path_data().is_some() && children[1].appearance.effects.is_empty());
        // Blur changes the object itself: an image alone.
        assert!(matches!(kids[1].kind, NodeKind::Image(_)));
        let pdf = super::export_pdf(&doc, &Default::default()).unwrap();
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.contains("/Image"), "the PDF embeds the effect images");
        // The source document is untouched.
        assert!(s.doc().unwrap().doc.node(vectorcraft_doc::NodeId(r)).unwrap().appearance.effects.len() == 1);
    }
}
