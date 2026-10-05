//! Marks and Bleeds: page boxes, the art kept in the bleed and non-printing layers.

use std::sync::Arc;

use serde_json::json;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, LayerColor, Node, NodeKind};
use vectorcraft_geom::{Rect, shapes};

use crate::*;

/// A 200 × 100 document with a red square reaching 5 pt left of its artboard.
fn doc() -> Document {
    let mut d = Document::new(200.0, 100.0);
    let layer = d.default_layer().unwrap();
    let id = d.alloc_id();
    let red = Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::None, 0.0);
    d.insert(Some(layer), 0, Node::path(id, shapes::rectangle(Rect::new(-5.0, 10.0, 40.0, 50.0)), red)).unwrap();
    d
}

fn settings(v: serde_json::Value) -> PdfSettings {
    serde_json::from_value(v).unwrap()
}

/// The PDF of `d` with settings `v`; the bleed raises no warning.
fn pdf(d: &Document, v: serde_json::Value) -> Vec<u8> {
    let opts = PdfOptions { settings: settings(v), created: Some(1_791_200_000), ..PdfOptions::uncompressed() };
    let r = export_with_report(d, &opts).unwrap();
    assert!(r.warnings.iter().all(|w| !w.contains("bleed")), "{:?}", r.warnings);
    r.bytes
}

/// Page 1's box `which` (PDF space: y up).
fn page_box(bytes: &[u8], which: CropTo) -> Rect {
    info(bytes, None).unwrap().pages[0].boxes.iter().find(|(c, _)| *c == which).unwrap().1
}

/// The bounds of every path of `d` painted with a paint `pick` accepts.
fn painted(d: &Document, pick: impl Fn(&Paint) -> bool) -> Vec<Rect> {
    let mut out = vec![];
    d.walk(|n| {
        let paints = n.appearance.fill().map(|f| &f.paint).into_iter().chain(n.appearance.stroke().map(|s| &s.paint));
        if paints.into_iter().any(&pick)
            && let Some(b) = n.path_data().and_then(|p| p.bounds())
        {
            out.push(b);
        }
    });
    out
}

fn rgb(p: &Paint, rgb: [u8; 3]) -> bool {
    p.color().is_some_and(|c| c.to_rgba8(1.0)[..3] == rgb)
}

#[test]
fn bleed_grows_the_media_box_around_the_trim_box() {
    let d = doc();
    // No bleed and no marks: the page is the artboard.
    let plain = pdf(&d, json!({}));
    assert_eq!(page_box(&plain, CropTo::Media), Rect::new(0.0, 0.0, 200.0, 100.0));
    assert_eq!(page_box(&plain, CropTo::Trim), Rect::new(0.0, 0.0, 200.0, 100.0));
    // A 9 pt bleed, custom or the document's.
    let mut nine = d.clone();
    nine.setup.bleed = [9.0; 4];
    for v in [json!({"bleed": {"top": 9, "bottom": 9, "left": 9, "right": 9}}), json!({"bleed": {"useDocument": true, "top": 1}})] {
        let bytes = pdf(&nine, v.clone());
        let media = page_box(&bytes, CropTo::Media);
        assert_eq!((media.width(), media.height()), (218.0, 118.0), "{v}: MediaBox = artboard + 2 × 9 pt");
        assert_eq!(page_box(&bytes, CropTo::Trim), Rect::new(9.0, 9.0, 209.0, 109.0), "{v}: TrimBox = artboard");
        assert_eq!(page_box(&bytes, CropTo::Bleed), media, "{v}: BleedBox = artboard + bleed");
    }
    // A document bleed read from a file is kept in range.
    nine.setup.bleed = [f64::NAN, -3.0, 1e9, 2.0];
    let b = pdf(&nine, json!({"bleed": {"useDocument": true}}));
    assert_eq!(page_box(&b, CropTo::Media), Rect::new(0.0, 0.0, 274.0, 100.0));
    // Uneven bleed (PDF space is y up: the bottom bleed is below the trim box).
    let bytes = pdf(&d, json!({"bleed": {"top": 4, "bottom": 2, "left": 3, "right": 1}}));
    assert_eq!(page_box(&bytes, CropTo::Media), Rect::new(0.0, 0.0, 204.0, 106.0));
    assert_eq!(page_box(&bytes, CropTo::Trim), Rect::new(3.0, 2.0, 203.0, 102.0));
}

#[test]
fn art_in_the_bleed_is_kept() {
    let d = doc();
    let back = import(&pdf(&d, json!({"bleed": {"left": 9, "top": 9, "bottom": 9, "right": 9}}))).unwrap();
    let red = painted(&back, |p| rgb(p, [255, 0, 0]));
    assert_eq!(red.len(), 1);
    assert!((red[0].x0 - 4.0).abs() < 0.01, "the square reaches 5 pt into the 9 pt bleed: {red:?}");
}

#[test]
fn non_printing_layers_are_left_out_unless_asked_for() {
    let mut d = doc();
    let mut layer = Node::layer(d.alloc_id(), "Notes", LayerColor::Preset(1));
    let blue = Appearance::basic(Paint::solid(Color::rgb(0.0, 0.0, 1.0)), Paint::None, 0.0);
    let note = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(100.0, 20.0, 150.0, 60.0)), blue);
    if let NodeKind::Layer { children, printable, .. } = &mut layer.kind {
        *printable = false;
        children.push(Arc::new(note));
    }
    d.layers.push(Arc::new(layer));
    let blues = |v| painted(&import(&pdf(&d, v)).unwrap(), |p| rgb(p, [0, 0, 255])).len();
    assert_eq!(blues(json!({})), 0, "a non-printing layer is absent");
    assert_eq!(blues(json!({"includeNonPrinting": true})), 1);
    assert_eq!(blues(json!({"createLayers": true})), 1, "PDF layers keep every layer");
}
