//! Legacy `.ai` files (version 7) written, then read back as File › Open reads them.

use std::sync::Arc;

use vectorcraft_color::{Color, Gradient, GradientKind, GradientPaint, GradientStop, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, Document, ImageBlob, ImageObject, Node, NodeKind};
use vectorcraft_geom::{Affine, AnchorKind, FillRule, PathData, Rect, shapes};

use super::*;

/// A node of `doc` filled `fill` (and stroked `stroke` 2 pt wide), path `path`.
fn shape(doc: &mut Document, path: PathData, fill: Paint, stroke: Paint) -> Node {
    Node::path(doc.alloc_id(), path, Appearance::basic(fill, stroke, 2.0))
}

fn red() -> Paint {
    Paint::solid(Color::rgb(1.0, 0.0, 0.0))
}

/// A 200 × 100 document: a layer `Back` holding a group of a red square (stroked blue) and a
/// locked one, a compound ring, a clipping group and a gradient square; a hidden layer `Front`
/// holding an ellipse and a 2 × 1 image.
fn sample() -> Document {
    let mut doc = Document::new(200.0, 100.0);
    doc.layers.clear();
    let back = doc.add_layer(Some("Back"));
    let front = doc.add_layer(Some("Front"));
    let a = shape(&mut doc, shapes::rectangle(Rect::new(10.0, 10.0, 40.0, 40.0)), red(), Paint::solid(Color::rgb(0.0, 0.0, 1.0)));
    let mut b = shape(&mut doc, shapes::rectangle(Rect::new(50.0, 10.0, 80.0, 40.0)), Paint::solid(Color::gray(0.5)), Paint::None);
    b.locked = true;
    let group = Node::group(doc.alloc_id(), vec![Arc::new(a), Arc::new(b)]);
    doc.insert(Some(back), usize::MAX, group).unwrap();
    let outer = shape(&mut doc, shapes::rectangle(Rect::new(100.0, 10.0, 140.0, 50.0)), Paint::None, Paint::None);
    let inner = shape(&mut doc, shapes::rectangle(Rect::new(110.0, 20.0, 130.0, 40.0)), Paint::None, Paint::None);
    let mut ring = Node::new(doc.alloc_id(), NodeKind::Compound { children: vec![Arc::new(outer), Arc::new(inner)], rule: FillRule::EvenOdd });
    ring.appearance = Appearance::basic(Paint::solid(Color::cmyk(0.0, 1.0, 1.0, 0.0)), Paint::None, 0.0).into();
    doc.insert(Some(back), usize::MAX, ring).unwrap();
    let clip = Node::new(
        doc.alloc_id(),
        NodeKind::Path {
            path: shapes::rectangle(Rect::new(160.0, 20.0, 180.0, 40.0)),
            rule: FillRule::NonZero,
            live: None,
            clipping: true,
            guide: false,
        },
    );
    let clipped = shape(&mut doc, shapes::rectangle(Rect::new(150.0, 10.0, 190.0, 50.0)), Paint::solid(Color::gray(0.25)), Paint::None);
    let window = Node::new(doc.alloc_id(), NodeKind::Group { children: vec![Arc::new(clip), Arc::new(clipped)], clip: true });
    doc.insert(Some(back), usize::MAX, window).unwrap();
    let g = Gradient {
        kind: GradientKind::Linear,
        stops: vec![GradientStop::new(0.0, Color::rgb(1.0, 1.0, 1.0)), GradientStop::new(1.0, Color::rgb(0.0, 0.0, 0.0))],
    };
    let shaded = shape(&mut doc, shapes::rectangle(Rect::new(10.0, 60.0, 50.0, 90.0)), Paint::Gradient(Box::new(GradientPaint::new(g))), Paint::None);
    doc.insert(Some(back), usize::MAX, shaded).unwrap();
    let ellipse = shape(&mut doc, shapes::ellipse(Rect::new(100.0, 60.0, 160.0, 90.0)), red(), Paint::None);
    doc.insert(Some(front), usize::MAX, ellipse).unwrap();
    let mut png = vec![];
    let img = image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255]).unwrap();
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let blob = ImageBlob::new("image/png", png);
    let key = blob.content_key();
    doc.images.insert(key.clone(), blob);
    let im =
        ImageObject { key, width: 2, height: 1, xf: Affine::new([10.0, 0.0, 0.0, 10.0, 170.0, 70.0]), link: None, placement: Default::default() };
    let im = Node::new(doc.alloc_id(), NodeKind::Image(Box::new(im)));
    doc.insert(Some(front), usize::MAX, im).unwrap();
    if let Some(l) = doc.layers.get_mut(1) {
        Arc::make_mut(l).visible = false;
    }
    doc
}

fn write(doc: &Document) -> LegacyAiOutput {
    export_legacy_ai(doc, &LegacyAiOptions { title: "Sample".into(), created: Some(0) }).unwrap()
}

fn children(n: &Node) -> &[Arc<Node>] {
    n.children().map_or(&[][..], Vec::as_slice)
}

#[test]
fn writes_the_documented_operators() {
    let out = write(&sample());
    let text = String::from_utf8(out.bytes).unwrap();
    for needle in [
        "%!PS-Adobe-3.0\n",
        "%AI5_FileFormat 3\n",
        "%%CreationDate: (1/1/1970) (00:00)\n",
        "%AI5_ArtSize: 200 100",
        "%AI3_TemplateBox: 100 50 100 50",
        "%AI5_NumLayers: 2",
        "(Back) Ln",
        "(Front) Ln",
        "%AI5_BeginGradient: (Gradient)",
        "\nu\n",
        "\n*u\n",
        "\nq\n",
        "\n1 A\n",
        "XI\n",
        "%%EOF\n",
    ] {
        assert!(text.contains(needle), "missing {needle:?} in\n{text}");
    }
    // The hidden layer is written hidden: visible 0.
    assert!(text.contains("%AI5_BeginLayer\n0 1 1 1 0 0 1 "), "{text}");
    // Art space is y up from the artboard's bottom-left: the red square (10..40 down from the top)
    // is 60..90 up. Its corners are corner points (upper case, AI7 spec §5.3.2); its fill RGB
    // (`r g b Xa`, §5.4.1).
    assert!(text.contains("1 0 0 Xa\n"), "{text}");
    assert!(text.contains("10 90 m\n40 90 L\n40 60 L\n10 60 L\n10 90 L\nb\n"), "{text}");
    // The ellipse's points are smooth (lower case).
    assert!(text.contains(" c\n") && !text.contains(" C\n"), "{text}");
    // The clipping path: closed by `h`, then `W n` (§5.6.2).
    assert!(text.contains(" L\nh\nW\nn\n"), "{text}");
    // Gradients: counted (`Bn`), each stop's operands before `%_Bs` (§3.5).
    assert!(text.contains("%%BeginSetup\n1 Bn\n%AI5_BeginGradient: (Gradient)\n(Gradient) 0 2 Bd\n[\n"), "{text}");
    assert!(text.contains(" 2 50 100 %_Bs\n") && !text.contains("Np\n"), "{text}");
    assert!(text.contains("1 0 0 1 0 0 Bg\nf\n0 BB\n"), "{text}");
    // The image: operands and `XI` on a line, its samples on comment lines (§8).
    assert!(text.contains("] 0 0 2 1 2 1 8 3 0 0 0 0 XI\n%FF00000000FF\n%AI5_EndRaster\n"), "{text}");
}

#[test]
fn a_gradient_fill_and_its_stroke_are_one_object() {
    let mut doc = Document::new(100.0, 100.0);
    let g = Gradient {
        kind: GradientKind::Radial,
        stops: vec![GradientStop::new(0.0, Color::rgb(1.0, 1.0, 1.0)), GradientStop::new(1.0, Color::cmyk(0.0, 0.0, 0.0, 1.0))],
    };
    let mut paint = GradientPaint::new(g);
    paint.geom = Some(vectorcraft_color::GradientGeom {
        start: vectorcraft_geom::Point::new(30.0, 30.0),
        end: vectorcraft_geom::Point::new(50.0, 30.0),
        aspect: 1.0,
        focal: Some(vectorcraft_geom::Point::new(35.0, 30.0)),
    });
    let n = shape(&mut doc, shapes::rectangle(Rect::new(10.0, 10.0, 50.0, 50.0)), Paint::Gradient(Box::new(paint)), red());
    let layer = doc.layers[0].id;
    doc.insert(Some(layer), usize::MAX, n).unwrap();
    let text = String::from_utf8(write(&doc).bytes).unwrap();
    // `2 BB` strokes the filled path (§3.5.3); the highlight is a quarter of the radius out (§3.5.5).
    assert!(text.contains("Bb\n5 0 0 0.25 Bh\n1 (Gradient) 30 70 0 20 1 0 0 1 0 0 Bg\nf\n2 BB\n"), "{text}");
    let doc = crate::import(text.as_bytes()).unwrap().document;
    let n = &children(&doc.layers[0])[0];
    let Paint::Gradient(g) = n.appearance.fill_paint() else { panic!("{:?}", n.appearance) };
    assert_eq!(g.gradient.kind, GradientKind::Radial);
    assert_eq!(n.appearance.stroke_paint().color(), Some(Color::rgb(1.0, 0.0, 0.0)));
}

#[test]
fn reads_back_with_layers_groups_and_paths() {
    let out = write(&sample());
    let imported = crate::import(&out.bytes).unwrap();
    let doc = imported.document.clone();
    let names: Vec<_> = doc.layers.iter().map(|l| l.name.clone().unwrap_or_default()).collect();
    assert_eq!(names, ["Back", "Front"], "{:?}", imported.warnings);
    assert!(doc.layers[0].visible);
    assert!(!doc.layers[1].visible);
    let back = children(&doc.layers[0]);
    assert_eq!(back.len(), 4, "{back:#?}");
    // The group of two squares, the second locked.
    let NodeKind::Group { children: pair, clip: false } = &back[0].kind else { panic!("{:?}", back[0].kind) };
    assert_eq!(pair.len(), 2);
    assert!(!pair[0].locked && pair[1].locked);
    let a = &pair[0];
    assert_eq!(a.appearance.fill_paint().color(), Some(Color::rgb(1.0, 0.0, 0.0)));
    assert_eq!(a.appearance.stroke_paint().color(), Some(Color::rgb(0.0, 0.0, 1.0)));
    // Four corners: the start point written again to close it isn't a fifth.
    let sq = a.path_data().unwrap();
    assert!(sq.subpaths[0].closed && sq.subpaths[0].anchors.len() == 4, "{sq:?}");
    assert!(sq.subpaths[0].anchors.iter().all(|p| p.kind == AnchorKind::Corner), "{sq:?}");
    let bounds = a.visual_bounds().unwrap();
    assert!((bounds.x0 - 9.0).abs() < 0.01 && (bounds.y0 - 9.0).abs() < 0.01, "{bounds:?}");
    // The ring: a compound path, even-odd.
    assert!(matches!(&back[1].kind, NodeKind::Compound { children, rule: FillRule::EvenOdd } if children.len() == 2), "{:?}", back[1].kind);
    // The clipping group: its clipping path first.
    let NodeKind::Group { children: window, clip: true } = &back[2].kind else { panic!("{:?}", back[2].kind) };
    assert!(matches!(window[0].kind, NodeKind::Path { clipping: true, .. }));
    // The gradient square.
    let Paint::Gradient(g) = back[3].appearance.fill_paint() else { panic!("{:?}", back[3].appearance) };
    assert_eq!(g.gradient.kind, GradientKind::Linear);
    assert_eq!(g.gradient.stops.len(), 2);
    assert_eq!(g.gradient.stops[0].color, Color::rgb(1.0, 1.0, 1.0));
    // The hidden layer: the ellipse with smooth points, and the image.
    let front = children(&doc.layers[1]);
    assert_eq!(front.len(), 2, "{:?} {:?}", front.iter().map(|n| &n.kind).collect::<Vec<_>>(), imported.warnings);
    let path = front[0].path_data().unwrap();
    assert_eq!(path.subpaths.len(), 1);
    assert!(path.subpaths[0].closed);
    assert!(path.subpaths[0].anchors.iter().all(|a| a.kind == AnchorKind::Smooth), "{path:?}");
    assert_eq!(path.subpaths[0].anchors.len(), 4, "{path:?}");
    let NodeKind::Image(im) = &front[1].kind else { panic!("{:?}", front[1].kind) };
    assert_eq!((im.width, im.height), (2, 1));
    let b = front[1].visual_bounds().unwrap();
    assert!((b.x0 - 170.0).abs() < 0.01 && (b.y0 - 70.0).abs() < 0.01 && (b.x1 - 190.0).abs() < 0.01, "{b:?}");
}

#[test]
fn spot_colours_are_custom_colours() {
    let mut doc = Document::new(100.0, 100.0);
    doc.swatches.push(vectorcraft_color::Swatch {
        name: "Spot Test".into(),
        paint: Paint::solid(Color::cmyk(0.0, 0.5, 1.0, 0.0)),
        global: true,
        spot: true,
    });
    let fill = Paint::Solid { color: Color::cmyk(0.0, 0.25, 0.5, 0.0), swatch: Some("Spot Test".into()), tint: 0.5 };
    let n = shape(&mut doc, shapes::rectangle(Rect::new(10.0, 10.0, 50.0, 50.0)), fill, Paint::None);
    let layer = doc.layers[0].id;
    doc.insert(Some(layer), usize::MAX, n).unwrap();
    let out = write(&doc);
    let text = String::from_utf8(out.bytes.clone()).unwrap();
    assert!(text.contains("%%DocumentCustomColors: (Spot Test)"), "{text}");
    assert!(text.contains("0 0.5 1 0 (Spot Test) 0.5 x"), "{text}");
    let doc = crate::import(&out.bytes).unwrap().document;
    let n = &children(&doc.layers[0])[0];
    let Paint::Solid { swatch, tint, .. } = n.appearance.fill_paint() else { panic!() };
    assert_eq!(swatch.as_deref(), Some("Spot Test"));
    assert!((tint - 0.5).abs() < 1e-6);
}

#[test]
fn losses_are_warned() {
    let mut doc = sample();
    // A sublayer, a hidden object, a named object, type and half opacity.
    let back = doc.layers[0].id;
    let sub = Node::layer(doc.alloc_id(), "Sub", vectorcraft_doc::LayerColor::Preset(2));
    let sub = doc.insert(Some(back), usize::MAX, sub).unwrap();
    let mut hidden = shape(&mut doc, shapes::rectangle(Rect::new(0.0, 0.0, 5.0, 5.0)), red(), Paint::None);
    hidden.visible = false;
    doc.insert(Some(sub), usize::MAX, hidden).unwrap();
    let mut named = shape(&mut doc, shapes::rectangle(Rect::new(0.0, 0.0, 5.0, 5.0)), red(), Paint::None);
    named.name = Some("Named".into());
    named.opacity = 0.5;
    doc.insert(Some(sub), usize::MAX, named).unwrap();
    let w = write(&doc).warnings;
    for expected in [SUBLAYERS, HIDDEN, NAMES, TRANSPARENCY] {
        assert!(w.iter().any(|x| x == expected), "{expected} not in {w:?}");
    }
    // The sublayer is a group in its layer.
    let doc = crate::import(&write(&doc).bytes).unwrap().document;
    let last = children(&doc.layers[0]).last().unwrap();
    assert!(matches!(&last.kind, NodeKind::Group { children, .. } if children.len() == 1), "{:?}", last.kind);
}

#[test]
fn several_fills_and_odd_strokes_become_objects() {
    let mut doc = Document::new(100.0, 100.0);
    let mut n = shape(&mut doc, shapes::rectangle(Rect::new(10.0, 10.0, 50.0, 50.0)), red(), Paint::solid(Color::gray(1.0)));
    let mut a = (*n.appearance).clone();
    if let Some(AppearanceItem::Stroke(s)) = a.items.iter_mut().find(|i| !i.is_fill()) {
        s.align = vectorcraft_doc::StrokeAlign::Inside;
    }
    n.appearance = a.into();
    let layer = doc.layers[0].id;
    doc.insert(Some(layer), usize::MAX, n).unwrap();
    let out = write(&doc);
    assert!(out.warnings.iter().any(|w| w.contains("filled outlines")), "{:?}", out.warnings);
    let doc = crate::import(&out.bytes).unwrap().document;
    let n = &children(&doc.layers[0])[0];
    // The fill, then the stroke's outline filled.
    assert!(matches!(&n.kind, NodeKind::Group { children, .. } if children.len() == 2), "{:?}", n.kind);
}

#[test]
fn art_stays_put_on_an_artboard_a_fraction_of_a_point_tall() {
    let mut doc = Document::new(100.0, 364.252);
    let n = shape(&mut doc, shapes::rectangle(Rect::new(10.0, 20.0, 30.0, 40.0)), red(), Paint::None);
    let layer = doc.layers[0].id;
    doc.insert(Some(layer), usize::MAX, n).unwrap();
    let text = String::from_utf8(write(&doc).bytes).unwrap();
    // The artboard's top is on a whole point (365); the template box's centre is the artboard's.
    assert!(text.contains("%AI3_TemplateBox: 50 182.874 50 182.874\n"), "{text}");
    assert!(text.contains("10 345 m\n"), "{text}");
    let doc = crate::import(text.as_bytes()).unwrap().document;
    let b = children(&doc.layers[0])[0].visual_bounds().unwrap();
    assert!((b.x0 - 10.0).abs() < 1e-3 && (b.y0 - 20.0).abs() < 1e-3 && (b.y1 - 40.0).abs() < 1e-3, "{b:?}");
}

#[test]
fn needs_an_artboard() {
    let mut doc = Document::new(100.0, 100.0);
    doc.artboards.clear();
    assert!(export_legacy_ai(&doc, &LegacyAiOptions::default()).is_err());
}
