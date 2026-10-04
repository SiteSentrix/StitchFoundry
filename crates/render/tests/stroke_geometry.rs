//! Pixel tests for the shared stroke geometry (`vectorcraft_effects::stroke`) on the canvas:
//! dotted lines (zero-length dashes).

use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, Dash, Document, LineCap, StrokeLayer};
use vectorcraft_geom::{Point, Rect, shapes};
use vectorcraft_testkit::fixtures::DocBuilder;
use vectorcraft_testkit::raster::{Image, assert_similar, render_region};
use vectorcraft_testkit::svg;

/// Pixels per point in these tests.
const S: f64 = 4.0;

fn stroked(width: f64, f: impl FnOnce(&mut StrokeLayer)) -> Appearance {
    let mut st = StrokeLayer::new(Paint::solid(Color::BLACK), width);
    f(&mut st);
    Appearance { items: vec![AppearanceItem::Stroke(st)], effects: vec![] }
}

/// A horizontal line from x0 to x1 at y = 50 on a 100 × 100 artboard.
fn line_doc(x0: f64, x1: f64, app: Appearance, opacity: f32) -> Document {
    let mut b = DocBuilder::new(100.0, 100.0);
    b.path(shapes::line(Point::new(x0, 50.0), Point::new(x1, 50.0)), app, |n| n.opacity = opacity);
    b.build()
}

fn render(d: &Document) -> Image {
    render_region(d, Rect::new(0.0, 0.0, 100.0, 100.0), S)
}

/// Is the pixel at document point (x, y) dark?
fn dark(img: &Image, x: f64, y: f64) -> bool {
    img.over_white((x * S) as u32, (y * S) as u32)[0] < 128
}

fn dotted(cap: LineCap) -> Document {
    line_doc(
        20.0,
        80.0,
        stroked(4.0, |s| {
            s.cap = cap;
            s.dash = Some(Dash { pattern: vec![0.0, 6.0], offset: 0.0, align_corners: false });
        }),
        1.0,
    )
}

#[test]
fn zero_length_dashes_draw_dots_with_a_round_cap_and_squares_with_a_projecting_cap() {
    let round = render(&dotted(LineCap::Round));
    // 60 pt with a dot every 6 pt: 11 separate dots along the centre line.
    let row: Vec<bool> = (0..round.width).map(|x| round.over_white(x, (50.0 * S) as u32)[0] < 128).collect();
    let runs = row.windows(2).filter(|w| !w[0] && w[1]).count();
    assert_eq!(runs, 11, "dots along the line");
    for k in 0..=10 {
        let x = 20.0 + 6.0 * k as f64;
        assert!(dark(&round, x, 50.0), "dot {k}");
        assert!(!dark(&round, x + 3.0, 50.0), "gap after dot {k}");
    }
    // A disc's corner region is empty; a square fills it.
    assert!(!dark(&round, 20.0 + 1.8, 50.0 + 1.8));
    let square = render(&dotted(LineCap::Square));
    assert!(dark(&square, 20.0 + 1.8, 50.0 + 1.8));
    assert!(dark(&square, 26.0 - 1.8, 50.0 - 1.8));
    // A butt cap gives a zero-length dash no area at all.
    assert_eq!(render(&dotted(LineCap::Butt)).ink(), 0);
}

#[test]
fn dotted_lines_survive_svg_export_and_render_the_same() {
    for cap in [LineCap::Round, LineCap::Square] {
        let d = dotted(cap);
        let text = svg::export(&d, &Default::default());
        let back = svg::import(&text).unwrap();
        assert_similar(&render(&d), &render(&back), 0.05, 0.001);
    }
}
