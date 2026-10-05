//! Document Info panel data and Object → Make Pixel Perfect.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use vectorcraft_color::Paint;
use vectorcraft_doc::{ColorMode, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, Rect};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "document.info",
            "Document Info",
            ["Window", "Document Info"],
            None,
            "{selectionOnly?} → {document, objects: {paths, compoundPaths, groups, …}, fonts, images, swatches, graphicStyleNames (with selectionOnly: the styles the selected objects are linked to), …}",
            has_doc,
            info
        ),
        cmd!(
            "object.makePixelPerfect",
            "Make Pixel Perfect",
            ["Object"],
            None,
            "{ids?} snap each object's edges to the pixel grid (odd stroke weights on half pixels) and round stroke weights",
            has_selection,
            pixel_perfect
        ),
    ]
}

fn paint_kind(p: &Paint) -> Option<&'static str> {
    match p {
        Paint::Gradient(_) => Some("gradients"),
        Paint::Pattern { .. } => Some("patterns"),
        _ => None,
    }
}

fn info(s: &mut Session, p: &Value) -> Result<Value> {
    let st = s.doc()?;
    let d = &st.doc;
    let selection_only = bool_or(p, "selectionOnly", false);
    let roots: Vec<&Node> =
        if selection_only { st.selection.objects.iter().filter_map(|id| d.node(*id)).collect() } else { d.layers.iter().map(|l| &**l).collect() };
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut fonts = BTreeSet::new();
    let mut images: BTreeMap<String, Value> = BTreeMap::new();
    let mut symbols = BTreeSet::new();
    let mut spot = BTreeSet::new();
    let mut styles: Vec<&str> = if selection_only { vec![] } else { d.graphic_styles.iter().map(|g| g.name.as_str()).collect() };
    for r in roots {
        r.walk(&mut |n| {
            let k = match &n.kind {
                NodeKind::Layer { .. } => None,
                NodeKind::Group { clip: true, .. } => Some("clipGroups"),
                NodeKind::Group { .. } => Some("groups"),
                NodeKind::Path { guide: true, .. } => Some("guides"),
                NodeKind::Path { .. } => Some("paths"),
                NodeKind::Compound { .. } => Some("compoundPaths"),
                NodeKind::Text(t) => {
                    for run in &t.runs {
                        fonts.insert(format!("{} {}", run.style.font_family, run.style.font_style));
                    }
                    Some("textObjects")
                }
                NodeKind::Image(im) => {
                    images.insert(
                        im.key.clone(),
                        json!({ "width": im.width, "height": im.height, "linked": im.link.is_some(), "link": im.link.as_ref().map(|l| &l.path) }),
                    );
                    Some("images")
                }
                NodeKind::SymbolInstance { symbol, .. } => {
                    symbols.insert(symbol.clone());
                    Some("symbolInstances")
                }
                NodeKind::Blend { .. } => Some("blends"),
                NodeKind::Envelope { .. } => Some("envelopes"),
                NodeKind::Mesh(_) => Some("meshes"),
                NodeKind::Repeat(_) => Some("repeats"),
            };
            if let Some(k) = k {
                *counts.entry(k).or_default() += 1;
            }
            for item in &n.appearance.items {
                let paint = match item {
                    vectorcraft_doc::AppearanceItem::Fill(f) => &f.paint,
                    vectorcraft_doc::AppearanceItem::Stroke(s) => &s.paint,
                };
                if let Some(k) = paint_kind(paint) {
                    *counts.entry(k).or_default() += 1;
                }
                if let Paint::Solid { swatch: Some(name), .. } = paint
                    && d.swatch(name).is_some_and(|s| s.spot)
                {
                    spot.insert(name.clone());
                }
            }
            if n.mask.is_some() {
                *counts.entry("opacityMasks").or_default() += 1;
            }
            if !n.appearance.effects.is_empty() {
                *counts.entry("liveEffects").or_default() += 1;
            }
            if selection_only
                && let Some(g) = super::style::linked_style(d, n)
                && !styles.contains(&g.name.as_str())
            {
                styles.push(&g.name);
            }
        });
    }
    Ok(json!({
        "document": document_summary(d),
        "objects": counts,
        "fonts": fonts,
        "images": images,
        "symbols": symbols,
        "spotColors": spot,
        "swatches": d.swatches_iter().count(),
        "graphicStyles": d.graphic_styles.len(),
        "graphicStyleNames": styles,
        "characterStyles": d.char_styles.len(),
        "paragraphStyles": d.para_styles.len(),
        "patterns": d.patterns.len(),
    }))
}

fn document_summary(d: &Document) -> Value {
    let u = d.units.points();
    json!({
        "title": d.title,
        "colorMode": match d.color_mode { ColorMode::Rgb => "RGB", ColorMode::Cmyk => "CMYK" },
        "units": d.units.label(),
        "artboards": d.artboards.iter().map(|a| json!({ "name": a.name, "width": a.rect.width() / u, "height": a.rect.height() / u })).collect::<Vec<_>>(),
        "rasterEffectsPpi": d.raster_effects_ppi,
    })
}

/// The affine snapping `b` to whole pixels (`half`: edges on half pixels, for odd stroke weights).
fn snap_xf(b: Rect, half: bool) -> Affine {
    let off = if half { 0.5 } else { 0.0 };
    let snap = |v: f64| (v - off).round() + off;
    let (x0, y0) = (snap(b.x0), snap(b.y0));
    // Keep at least one pixel of size; widths snap to whole pixels.
    let (w, h) = (b.width().round().max(if b.width() > 0.0 { 1.0 } else { 0.0 }), b.height().round().max(if b.height() > 0.0 { 1.0 } else { 0.0 }));
    let sx = if b.width() > 1e-9 { w / b.width() } else { 1.0 };
    let sy = if b.height() > 1e-9 { h / b.height() } else { 1.0 };
    Affine::translate((x0, y0)) * Affine::scale_non_uniform(sx, sy) * Affine::translate((-b.x0, -b.y0))
}

fn pixel_perfect(s: &mut Session, p: &Value) -> Result<Value> {
    let ids: Vec<NodeId> = if p.get("ids").is_some() { targets(s, p)? } else { super::edit::selected_roots(s)? };
    let n = ids.len();
    s.edit("Make Pixel Perfect", |d, _| {
        for id in &ids {
            let Some(node) = d.node_mut(*id) else { continue };
            // Whole-pixel stroke weights; odd ones centre on half pixels.
            let mut odd = false;
            for item in &mut node.appearance.items {
                if let vectorcraft_doc::AppearanceItem::Stroke(st) = item
                    && st.width > 0.0
                {
                    st.width = st.width.round().max(1.0);
                    odd |= st.width as i64 % 2 == 1 && st.align == vectorcraft_doc::StrokeAlign::Center;
                }
            }
            if let Some(b) = node.geometric_bounds() {
                node.transform(snap_xf(b, odd), false);
            }
        }
        Ok(())
    })?;
    Ok(json!({ "count": n }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_counts_objects_fonts_and_selection_only() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
        let r = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"].clone();
        s.execute(
            "paint.setFill",
            &json!({"gradient": {"kind": "linear", "stops": [{"offset": 0, "color": "#000"}, {"offset": 1, "color": "#fff"}]}}),
        )
        .unwrap();
        s.execute("text.create", &json!({"x": 10, "y": 40, "text": "Hi"})).unwrap();
        let i = s.execute("document.info", &json!({})).unwrap();
        assert_eq!(
            (i["objects"]["paths"].as_u64(), i["objects"]["textObjects"].as_u64(), i["objects"]["gradients"].as_u64()),
            (Some(1), Some(1), Some(1))
        );
        assert_eq!(i["fonts"][0], "Source Sans 3 Regular");
        assert_eq!(i["document"]["artboards"][0]["width"], 200.0);
        s.execute("select.set", &json!({"ids": [r]})).unwrap();
        let i = s.execute("document.info", &json!({"selectionOnly": true})).unwrap();
        assert!(i["objects"]["textObjects"].is_null() && i["objects"]["paths"] == 1);
    }

    #[test]
    fn pixel_perfect_snaps_edges_and_strokes() {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
        let id = s.execute("shape.rectangle", &json!({"x": 10.3, "y": 20.6, "width": 30.4, "height": 9.7})).unwrap()["id"].as_u64().unwrap();
        s.execute("stroke.set", &json!({"weight": 0.8})).unwrap();
        s.execute("object.makePixelPerfect", &json!({})).unwrap();
        let n = s.doc().unwrap().doc.node(NodeId(id)).unwrap().clone();
        let b = n.geometric_bounds().unwrap();
        // 1 pt (odd) stroke → edges on half pixels, whole-pixel size.
        assert_eq!((b.x0, b.y0, b.width(), b.height()), (10.5, 20.5, 30.0, 10.0));
        assert_eq!(n.appearance.stroke_width(), 1.0);
        s.execute("stroke.set", &json!({"weight": 2})).unwrap();
        s.execute("object.makePixelPerfect", &json!({})).unwrap();
        let b = s.doc().unwrap().doc.node(NodeId(id)).unwrap().geometric_bounds().unwrap();
        assert_eq!((b.x0.fract(), b.y0.fract()), (0.0, 0.0));
    }
}
