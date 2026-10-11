//! What renderers keep between frames: decoded images shared by all of them, image mips, and
//! caches swept by every kind of render.

use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Document, ImageBlob, ImageObject, Node, NodeKind};

use super::*;

/// A `w`×`h` PNG whose pixels depend on `seed` (each test its own image: the store is shared by
/// the tests running alongside).
fn png(w: u32, h: u32, seed: u8) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| {
        let v = (x * 7 + y * 13) as u8;
        image::Rgba([v ^ seed, v.wrapping_mul(3), seed, 255])
    });
    let mut out = vec![];
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
    out
}

/// A document showing `bytes` as image `key` of `w`×`h` pixels, one point a pixel.
fn doc_with_image(key: &str, bytes: Vec<u8>, w: u32, h: u32) -> Document {
    let mut d = Document::new(w as f64, h as f64);
    d.images.insert(key.into(), ImageBlob::new("image/png", bytes));
    let (id, layer) = (d.alloc_id(), d.layers[0].id);
    let im = ImageObject { key: key.into(), width: w, height: h, xf: Affine::IDENTITY, link: None, placement: Default::default() };
    d.insert(Some(layer), 0, Node::new(id, NodeKind::Image(im))).unwrap();
    d
}

#[test]
fn renderers_share_one_decode_of_an_image() {
    let bytes = png(16, 16, 101);
    let a = doc_with_image("img", bytes.clone(), 16, 16);
    // Another document holding the same image under another key (its own copy of the bytes).
    let b = doc_with_image("photo", bytes, 16, 16);
    let (mut r1, mut r2) = (Renderer::new(), Renderer::new());
    let (k1, p1) = r1.image_pixmap(&a, "img", false, true).unwrap();
    let (k2, p2) = r2.image_pixmap(&b, "photo", false, true).unwrap();
    assert_eq!(k1, k2);
    assert!(Arc::ptr_eq(&p1, &p2), "decoded once for both renderers");
    // Drawing goes through the store too: the pixels stay the ones decoded.
    let _ = r2.render(&b, 16, 16, Affine::IDENTITY, &RenderOptions::default());
    assert!(Arc::ptr_eq(&r1.image_pixmap(&b, "photo", false, true).unwrap().1, &p1));
    // A document using the same key for other pixels gets its own.
    let c = doc_with_image("img", png(16, 16, 102), 16, 16);
    let (k3, p3) = r1.image_pixmap(&c, "img", false, true).unwrap();
    assert!(k3 != k1 && !Arc::ptr_eq(&p3, &p1));
    assert!(
        r1.render(&c, 16, 16, Affine::IDENTITY, &RenderOptions::default()).pixels
            != r1.render(&a, 16, 16, Affine::IDENTITY, &RenderOptions::default()).pixels
    );
}

/// Thumbnails and single-node renders end their frame as full renders do: entries of nodes no
/// longer drawn are dropped, with the nodes they kept alive.
#[test]
fn node_renders_sweep_stale_cache_entries() {
    let mut d = Document::new(10.0, 10.0);
    let id = d.alloc_id();
    let square = |x: f64| {
        let p = vectorcraft_geom::shapes::rectangle(Rect::new(x, 0.0, x + 5.0, 5.0));
        Arc::new(Node::path(id, p, vectorcraft_doc::Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::None, 0.0)))
    };
    let first = square(0.0);
    let mut r = Renderer::new();
    r.render_node(&d, &first, 10, 10, Affine::IDENTITY).unwrap();
    assert!(Arc::strong_count(&first) > 1, "cached while drawn");
    for i in 0..1100 {
        let n = square(i as f64);
        if i % 2 == 0 {
            r.render_node(&d, &n, 10, 10, Affine::IDENTITY).unwrap();
        } else {
            // A thumbnail of a group caches its member's geometry.
            r.render_node_thumbnail(&d, &Node::group(id, vec![n]), 16, None).unwrap();
        }
    }
    assert_eq!(Arc::strong_count(&first), 1, "swept: the renderer no longer keeps the node");
    assert!(r.geom.len() <= 1025, "{} geometry entries", r.geom.len());
}

/// With `image_mips` off (the default; exports) an image draws from its full pixels whatever the
/// store holds; on, a reduced image draws from a box-filtered half (quarter…) instead, and at
/// more than half size from the full pixels as before.
#[test]
fn image_mips_are_opt_in_and_change_nothing_off() {
    let d = doc_with_image("img", png(64, 64, 103), 64, 64);
    let (off, on) = (RenderOptions::default(), RenderOptions { image_mips: true, ..Default::default() });
    let quarter = Affine::scale(0.25);
    let cold = Renderer::new().render(&d, 16, 16, quarter, &off);
    // Mips made by another renderer don't change what this one draws with them off.
    let mipped = Renderer::new().render(&d, 16, 16, quarter, &on);
    let mut warm = Renderer::new();
    assert!(warm.render(&d, 16, 16, quarter, &off).pixels == cold.pixels);
    assert!(warm.render(&d, 16, 16, quarter, &off).pixels == cold.pixels, "warm");
    // On, the quarter-size image samples the quarter-size mip: close to the full pixels' average
    // but not the same pixels.
    assert!(mipped.pixels != cold.pixels);
    let (key, _) = warm.image_pixmap(&d, "img", false, true).unwrap();
    let mip = images::store().peek(&images::ImageKey { mip: 2, ..key }).expect("quarter mip kept");
    assert_eq!((mip.width(), mip.height()), (16, 16));
    // Above half size, mips on draw the full pixels.
    let near = Affine::scale(0.75);
    assert!(Renderer::new().render(&d, 48, 48, near, &on).pixels == Renderer::new().render(&d, 48, 48, near, &off).pixels);
}
