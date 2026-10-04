//! Pixel tests for dashes fitted to corners, drawn on the canvas from the shared stroke geometry
//! (`vectorcraft_effects::stroke`).

use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, Dash, Document, StrokeLayer};
use vectorcraft_geom::{PathData, Rect, shapes};
use vectorcraft_testkit::fixtures::DocBuilder;
use vectorcraft_testkit::raster::{Image, render_region};

/// Pixels per point in these tests.
const S: f64 = 4.0;

fn doc(path: PathData, width: f64, f: impl FnOnce(&mut StrokeLayer)) -> Document {
    let mut st = StrokeLayer::new(Paint::solid(Color::BLACK), width);
    f(&mut st);
    let mut b = DocBuilder::new(160.0, 120.0);
    b.path(path, Appearance { items: vec![AppearanceItem::Stroke(st)], effects: vec![] }, |_| {});
    b.build()
}

fn render(d: &Document) -> Image {
    render_region(d, Rect::new(0.0, 0.0, 160.0, 120.0), S)
}

/// Is the pixel at document point (x, y) dark?
fn dark(img: &Image, x: f64, y: f64) -> bool {
    img.over_white((x * S) as u32, (y * S) as u32)[0] < 128
}

fn rect_doc(align_corners: bool) -> Document {
    doc(shapes::rectangle(Rect::new(20.0, 20.0, 120.0, 70.0)), 2.0, |s| s.dash = Some(Dash { pattern: vec![12.0, 6.0], offset: 0.0, align_corners }))
}

#[test]
fn fitted_dashes_wrap_every_corner_of_a_rectangle() {
    let img = render(&rect_doc(true));
    // Half a dash (6 · 100/108) runs each way from every corner, then a gap of the same length.
    let half = 6.0 * 100.0 / 108.0;
    for (c, dirs) in [
        ((20.0, 20.0), [(1.0, 0.0), (0.0, 1.0)]),
        ((120.0, 20.0), [(-1.0, 0.0), (0.0, 1.0)]),
        ((120.0, 70.0), [(-1.0, 0.0), (0.0, -1.0)]),
        ((20.0, 70.0), [(1.0, 0.0), (0.0, -1.0)]),
    ] {
        for (dx, dy) in dirs {
            let at = |d: f64| (c.0 + dx * d, c.1 + dy * d);
            let (x, y) = at(half / 2.0);
            assert!(dark(&img, x, y), "dash beside corner {c:?}");
            let (x, y) = at(half * 1.5);
            assert!(!dark(&img, x, y), "gap after corner {c:?} along ({dx}, {dy})");
        }
    }
    // Exact dashes keep their lengths instead (the exact golden covers how they look).
    assert_ne!(render(&rect_doc(false)).rgba, img.rgba);
}
