//! System clipboard interchange: the copied objects as SVG markup (what other apps paste), and
//! SVG markup from other apps turned into clipboard objects (pasted with the Paste commands).
//!
//! The UI owns the platform clipboard; these commands only convert. Within VectorCraft the
//! internal clipboard stays lossless (live effects, masks, symbols); SVG is the outside format.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Node, NodeId};
use vectorcraft_geom::{Affine, Point};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "clipboard.exportSvg", "Clipboard as SVG", [], None, "{} → {svg} the copied objects as standalone SVG (null when the clipboard is empty)", always, export_svg),
        cmd!(
            query "clipboard.importSvg",
            "Load SVG into Clipboard",
            [],
            None,
            "{svg, center?: [x, y]} replace the clipboard with the SVG's objects, centred on `center` (default: the first artboard) → {count}; then run edit.pasteInPlace",
            has_doc,
            import_svg
        ),
    ]
}

/// Is `text` SVG markup (as other apps put on the clipboard)?
pub fn looks_like_svg(text: &str) -> bool {
    let t = text.trim_start_matches('\u{feff}').trim_start();
    (t.starts_with("<svg") || t.starts_with("<?xml") || t.starts_with("<!--") || t.starts_with("<!DOCTYPE svg")) && t.contains("<svg")
}

impl Session {
    /// The internal clipboard as standalone SVG (`None` when it is empty).
    pub fn clipboard_svg(&self) -> Option<String> {
        if self.clipboard.is_empty() {
            return None;
        }
        // The active document supplies swatches, symbols, patterns and image blobs.
        let mut d = self.active().map(|st| (*st.doc).clone()).unwrap_or_else(|| Document::new(100.0, 100.0));
        let mut layer = Node::layer(NodeId(u64::MAX), "Clipboard", vectorcraft_doc::LayerColor::Preset(0));
        if let Some(ch) = layer.children_mut() {
            *ch = self.clipboard.iter().cloned().map(Arc::new).collect();
        }
        d.layers = vec![Arc::new(layer)];
        Some(vectorcraft_svg::export(
            &d,
            &vectorcraft_svg::ExportOptions { artboard: None, object_ids: vectorcraft_svg::ObjectIds::Minimal, ..Default::default() },
        ))
    }
}

fn export_svg(s: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({ "svg": s.clipboard_svg() }))
}

fn import_svg(s: &mut Session, p: &Value) -> Result<Value> {
    let svg = str_param(p, "svg").ok_or_else(|| bad("clipboard.importSvg", "missing svg"))?;
    let src = vectorcraft_svg::import(svg).map_err(|e| bad("clipboard.importSvg", e.to_string()))?;
    let mut nodes: Vec<Node> = src.layers.iter().flat_map(|l| l.children().cloned().unwrap_or_default()).map(|n| (*n).clone()).collect();
    if nodes.is_empty() {
        return Err(bad("clipboard.importSvg", "the SVG has no drawable objects"));
    }
    let center = match p.get("center").and_then(Value::as_array) {
        Some(c) if c.len() == 2 => Point::new(c[0].as_f64().unwrap_or(0.0), c[1].as_f64().unwrap_or(0.0)),
        _ => s.doc()?.doc.artboards.first().map(|a| a.rect.center()).unwrap_or_default(),
    };
    let bounds = nodes.iter().filter_map(|n| n.visual_bounds()).reduce(|a, b| a.union(b));
    if let Some(b) = bounds {
        let xf = Affine::translate(center - b.center());
        for n in &mut nodes {
            n.transform(xf, false);
        }
    }
    // Embedded images: their blobs join the document (keys are content hashes).
    if !src.images.is_empty() {
        let st = s.doc_mut()?;
        let d = Arc::make_mut(&mut st.doc);
        for (k, b) in &src.images {
            d.images.entry(k.clone()).or_insert_with(|| b.clone());
        }
    }
    let count = nodes.len();
    s.clipboard = nodes;
    Ok(json!({ "count": count }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        s
    }

    #[test]
    fn copy_exports_svg_that_round_trips_through_paste() {
        let mut s = session();
        let id = s.execute("shape.rectangle", &json!({"x": 10, "y": 20, "width": 30, "height": 40})).unwrap()["id"].as_u64().unwrap();
        s.execute("paint.setFill", &json!({"color": "#12ab34"})).unwrap();
        s.execute("select.set", &json!({"ids": [id]})).unwrap();
        assert!(s.execute("clipboard.exportSvg", &json!({})).unwrap()["svg"].is_null() || !s.clipboard.is_empty());
        s.execute("edit.copy", &json!({})).unwrap();
        let svg = s.execute("clipboard.exportSvg", &json!({})).unwrap()["svg"].as_str().unwrap().to_string();
        assert!(looks_like_svg(&svg) && svg.contains("#12ab34"), "{svg}");
        // As if pasted into another VectorCraft window.
        let mut t = session();
        assert_eq!(t.execute("clipboard.importSvg", &json!({"svg": svg, "center": [200, 150]})).unwrap()["count"], 1);
        t.execute("edit.pasteInPlace", &json!({})).unwrap();
        let st = t.doc().unwrap();
        let b = st.doc.bounds_of(&st.selection.in_paint_order(&st.doc), false).unwrap();
        assert!((b.center().x - 200.0).abs() < 1e-6 && (b.center().y - 150.0).abs() < 1e-6, "{b:?}");
        assert!((b.width() - 30.0).abs() < 1e-6 && (b.height() - 40.0).abs() < 1e-6);
    }

    #[test]
    fn paste_never_lands_in_an_object_that_reused_an_undone_layers_id() {
        // Found by the model-based test: undo restores the id counter, so a compound path made
        // after undoing New Layer gets the dead layer's id, which was still the active layer.
        let mut s = session();
        let r = s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"].clone();
        s.execute("select.set", &json!({"ids": [r]})).unwrap();
        s.execute("edit.copy", &json!({})).unwrap();
        let layer = s.execute("layer.new", &json!({})).unwrap()["id"].as_u64().unwrap();
        s.execute("edit.undo", &json!({})).unwrap();
        s.execute("select.set", &json!({"ids": [r]})).unwrap();
        let c = s.execute("object.compoundPath.make", &json!({})).unwrap()["id"].as_u64().unwrap();
        assert_eq!(c, layer, "the scenario needs the id to be reused");
        s.execute("edit.paste", &json!({})).unwrap();
        let d = &s.doc().unwrap().doc;
        let compound = d.node(NodeId(c)).unwrap();
        assert!(compound.children().unwrap().iter().all(|ch| matches!(ch.kind, vectorcraft_doc::NodeKind::Path { .. })));
    }

    #[test]
    fn foreign_svg_and_non_svg_text() {
        let mut s = session();
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="5" fill="red"/><rect width="2" height="2"/></svg>"##;
        assert!(looks_like_svg(svg) && looks_like_svg(&format!("<?xml version=\"1.0\"?>\n{svg}")));
        assert!(!looks_like_svg("hello <svg> world") && !looks_like_svg("plain text"));
        assert_eq!(s.execute("clipboard.importSvg", &json!({"svg": svg})).unwrap()["count"], 2);
        assert!(s.execute("clipboard.importSvg", &json!({"svg": "<svg xmlns=\"http://www.w3.org/2000/svg\"/>"})).is_err());
        assert!(s.execute("clipboard.importSvg", &json!({"svg": "not svg"})).is_err());
    }
}
