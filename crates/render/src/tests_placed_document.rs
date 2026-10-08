//! Placed documents drawn exactly: with their own art, as their previews when that is all there
//! is, and a few levels deep when they place themselves.

use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, ImageBlob, LinkInfo, Node, NodeId, NodeKind, PlacedDocument};
use vectorcraft_geom::shapes;

use super::*;

/// Every file reads as a 40×20 artboard filled blue; a file whose bytes start with `loop` also
/// places itself (the same key) in its middle.
fn blue_page(p: &PlacedDocument, bytes: &[u8]) -> Option<(Document, Node, Rect)> {
    let mut d = Document::new(40.0, 20.0);
    d.layers.clear();
    let blue = Appearance::basic(Paint::solid(Color::rgb(0.0, 0.0, 1.0)), Paint::None, 0.0);
    let mut children = vec![Arc::new(Node::path(NodeId(1), shapes::rectangle(Rect::new(0.0, 0.0, 40.0, 20.0)), blue))];
    if bytes.starts_with(b"loop") {
        d.images.insert(p.key.clone(), ImageBlob::new("application/json", bytes.to_vec()));
        let me = PlacedDocument { xf: Affine::translate((10.0, 5.0)) * Affine::scale(0.5), ..p.clone() };
        children.push(Arc::new(Node::new(NodeId(2), NodeKind::PlacedDocument(Box::new(me)))));
    }
    Some((d, Node::new(NodeId(0), NodeKind::Group { children, clip: false }), Rect::new(0.0, 0.0, 40.0, 20.0)))
}

/// A 100×100 document showing one placed file (`bytes` under `key`) in the box (10, 10)–(90, 50).
fn doc_with(key: &str, bytes: &[u8]) -> Document {
    vectorcraft_doc::placed_document::set_loader(blue_page);
    let mut d = Document::new(100.0, 100.0);
    d.images.insert(key.into(), ImageBlob::new("application/json", bytes.to_vec()));
    let p = PlacedDocument {
        link: LinkInfo::new("/art/card.vectorcraft"),
        key: key.into(),
        bounding: false,
        width: 40.0,
        height: 20.0,
        xf: Affine::translate((10.0, 10.0)) * Affine::scale(2.0),
        placement: Default::default(),
    };
    let l = d.layers[0].id;
    d.insert(Some(l), 0, Node::new(NodeId(50), NodeKind::PlacedDocument(Box::new(p)))).unwrap();
    d
}

fn doc(key: &str) -> Document {
    doc_with(key, format!("{{\"doc\": \"{key}\"}}").as_bytes())
}

fn render(d: &Document) -> Rendered {
    let opts = RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() };
    Renderer::new().render(d, 100, 100, Affine::IDENTITY, &opts)
}

#[test]
fn a_placed_document_draws_exactly_with_its_own_art() {
    let d = doc("exact-1");
    let r = render(&d);
    assert_eq!(&r.pixel(50, 30)[..3], &[0, 0, 255], "blue inside the box");
    assert_eq!(&r.pixel(95, 95)[..3], &[255, 255, 255], "nothing outside");
}

#[test]
fn a_placed_document_kept_as_its_preview_draws_the_preview() {
    let mut d = doc("preview-1");
    // Only the preview (a 20×10 green PNG) is there, and the file can't be read here.
    let img = image::RgbaImage::from_pixel(20, 10, image::Rgba([0, 200, 0, 255]));
    let mut png = vec![];
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let mut blob = ImageBlob::new("image/png", png);
    blob.proxy = Some(blob.bytes.clone());
    d.images.insert("preview-1".into(), blob);
    let exact = render(&d);
    assert_eq!(&exact.pixel(50, 30)[..3], &[0, 200, 0], "the preview, in the box");
    assert_eq!(&exact.pixel(95, 95)[..3], &[255, 255, 255]);
}

#[test]
fn a_document_that_places_itself_draws_a_few_levels_deep_then_stops() {
    let d = doc_with("loop-1", b"loop");
    let t = std::time::Instant::now();
    let r = render(&d);
    assert!(t.elapsed().as_secs() < 10);
    assert_eq!(&r.pixel(50, 30)[..3], &[0, 0, 255]);
}
