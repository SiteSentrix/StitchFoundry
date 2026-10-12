//! Effects on every object kind: raster effects on type, images and symbol instances through an
//! offscreen layer of their art, geometry effects through their outlines, and one fill or
//! stroke's own raster effects.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::text::{CharStyle, TextObject};
use vectorcraft_doc::{Appearance, Effect, ImageBlob, ImageObject, Node, NodeKind, Symbol};
use vectorcraft_geom::{Point, shapes};

use super::*;

fn fx(id: &str, params: Value) -> Effect {
    Effect { id: id.into(), params, visible: true }
}

/// A 100×100 document with `n` (its id allocated) on the first layer.
fn doc_with(mut d: Document, mut n: Node) -> Document {
    n.id = d.alloc_id();
    let l = d.layers[0].id;
    d.insert(Some(l), 0, n).unwrap();
    d
}

fn render_threads(d: &Document, threads: u16) -> Rendered {
    let mut r = Renderer::new();
    r.threads = threads;
    r.render(d, 100, 100, Affine::IDENTITY, &RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() })
}

fn render(d: &Document) -> Rendered {
    render_threads(d, 0)
}

fn lum(p: [u8; 4]) -> u32 {
    p[0] as u32 + p[1] as u32 + p[2] as u32
}

const WHITE: u32 = 765;

#[test]
fn revolve_caps_cover_the_opening_and_match_export_and_visible_only_expansion() {
    let path = vectorcraft_geom::PathData::from_bezpath(&vectorcraft_geom::BezPath::from_svg("M70 30 L70 70").unwrap());
    let mut n = Node::path(NodeId(0), path, Appearance::basic(Paint::solid(Color::rgb(0.9, 0.2, 0.1)), Paint::None, 0.0));
    for (angle, opacity) in [(360.0, 1.0), (90.0, 1.0), (360.0, 0.5)] {
        n.opacity = opacity;
        n.appearance.effects = vec![fx(
            "threeD.revolve",
            json!({"offset":20,"rotationX":-35,"lightAzimuth":0,
            "angle":angle,"segments":32,"capEnds":false,"expandVisibleOnly":true}),
        )];
        let open = render(&doc_with(Document::new(100.0, 100.0), n.clone()));
        n.appearance.effects[0].params["capEnds"] = json!(true);
        let d = doc_with(Document::new(100.0, 100.0), n.clone());
        let live = render(&d);
        assert_ne!(live.pixels, open.pixels, "caps change the opening: {angle}, {opacity}");
        if angle == 360.0 && opacity == 1.0 {
            let pixel = (34 * 100 + 50) * 4;
            assert!(live.pixels[pixel] as u16 > open.pixels[pixel] as u16 + 30, "the lit cap covers the visible interior");
        }
        assert_eq!(live.pixels, render(&vectorcraft_effects::bake_document(&d).unwrap()).pixels);
        let mut expanded = d.clone();
        let id = expanded.layers[0].children().unwrap()[0].id;
        let source = expanded.node(id).unwrap().clone();
        let mut stroke = |_: &mut Document, _: &vectorcraft_geom::PathData, _: vectorcraft_geom::FillRule, _: &vectorcraft_doc::StrokeLayer| None;
        let art = vectorcraft_effects::expand_leaf(&mut expanded, &source, &mut stroke).unwrap();
        *expanded.node_mut(id).unwrap() = art;
        let flat = render(&expanded);
        let difference: u64 = live.pixels.iter().zip(&flat.pixels).map(|(a, b)| a.abs_diff(*b) as u64).sum();
        assert!(difference as f64 / (live.pixels.len() as f64) < 1.0, "visible-only expansion preserves cap appearance");
        if opacity < 1.0 {
            assert_eq!(live.pixels, flat.pixels);
        }
    }
}

#[test]
fn revolve_canvas_matches_baked_art_and_shades_its_surface() {
    let mut bp = vectorcraft_geom::BezPath::new();
    bp.move_to((50.0, 15.0));
    bp.line_to((75.0, 30.0));
    bp.line_to((75.0, 75.0));
    bp.line_to((50.0, 90.0));
    let mut n = Node::path(
        NodeId(0),
        vectorcraft_geom::PathData::from_bezpath(&bp),
        Appearance::basic(Paint::solid(Color::rgb(0.9, 0.2, 0.1)), Paint::None, 0.0),
    );
    n.appearance.effects.push(fx("threeD.revolve", json!({"rotationX":0,"rotationY":0})));
    let d = doc_with(Document::new(100.0, 100.0), n);
    let live = render(&d);
    let baked = vectorcraft_effects::bake_document(&d).unwrap();
    let exported = render(&baked);
    assert_eq!(live.pixels, exported.pixels, "canvas and baked/exported faces agree");
    let reds: Vec<_> =
        live.pixels.as_chunks::<4>().0.iter().filter(|p| p[0] as u16 > p[1] as u16 * 2 && p[0] as u16 > p[2] as u16 * 2).map(|p| p[0]).collect();
    assert!(reds.len() > 1500, "a filled surface, rather than the open source path");
    assert!(reds.iter().max().unwrap() - reds.iter().min().unwrap() > 80, "lighting varies across the surface");
    assert_eq!(live.pixels, render_threads(&d, 4).pixels);
}

/// Extrude & Bevel, Inflate (also twisted, tapered and hollow) and Rotate draw on the canvas exactly
/// as they export, filling more than the flat square with faces of differing shades (Rotate: one
/// shade, narrower).
#[test]
fn solid_effects_canvas_matches_baked_art_and_shades_their_faces() {
    for (effect, params) in [
        ("threeD.extrude", json!({"depth": 30, "bevel": "rounded"})),
        ("threeD.inflate", json!({"depth": 10})),
        ("threeD.inflate", json!({"depth": 10, "twist": 90, "taper": 60, "cap": false, "rotationX": -20, "rotationY": 35})),
        ("threeD.rotate", json!({"rotationX": 0, "rotationY": 55, "rotationZ": 0})),
    ] {
        let mut n = Node::path(
            NodeId(0),
            shapes::rectangle(vectorcraft_geom::Rect::new(30.0, 30.0, 70.0, 70.0)),
            Appearance::basic(Paint::solid(Color::rgb(0.9, 0.2, 0.1)), Paint::None, 0.0),
        );
        n.appearance.effects.push(fx(effect, params));
        let d = doc_with(Document::new(100.0, 100.0), n);
        let live = render(&d);
        let exported = render(&vectorcraft_effects::bake_document(&d).unwrap());
        assert_eq!(live.pixels, exported.pixels, "{effect}: canvas and baked/exported faces agree");
        assert_eq!(live.pixels, render_threads(&d, 4).pixels, "{effect}");
        let reds: Vec<_> =
            live.pixels.as_chunks::<4>().0.iter().filter(|p| p[0] as u16 > p[1] as u16 * 2 && p[0] as u16 > p[2] as u16 * 2).map(|p| p[0]).collect();
        let shades = reds.iter().max().unwrap() - reds.iter().min().unwrap();
        if effect == "threeD.rotate" {
            assert!(reds.len() < 40 * 40 && reds.len() > 600, "{effect}: the square foreshortened, {} px", reds.len());
        } else {
            assert!(reds.len() > 40 * 40, "{effect}: a solid larger than its front, {} px", reds.len());
            assert!(shades > 60, "{effect}: lighting varies across the faces ({shades})");
        }
    }
}

#[test]
fn smooth_revolve_reduces_banding_without_changing_radial_resolution() {
    let path = vectorcraft_geom::PathData::from_bezpath(&vectorcraft_geom::BezPath::from_svg("M70 20 L70 80").unwrap());
    let mut n = Node::path(NodeId(0), path, Appearance::basic(Paint::solid(Color::rgb(0.2, 0.55, 0.9)), Paint::None, 0.0));
    n.appearance
        .effects
        .push(fx("threeD.revolve", json!({"offset":30,"segments":16,"lightAzimuth":0,"lightElevation":0,"ambient":10,"lightIntensity":80})));
    let d = doc_with(Document::new(100.0, 100.0), n);
    let smooth = render(&d);
    let levels = |r: &Rendered| (14..66).map(|x| r.pixels[(50 * 100 + x) * 4 + 2]).collect::<std::collections::BTreeSet<_>>().len();
    // More colour levels than radial facets proves interpolation inside the faces.
    assert!(levels(&smooth) > 16, "default rendering interpolates lighting: {} blue levels", levels(&smooth));
    let jump = |r: &Rendered| (15..66).map(|x| r.pixels[(50 * 100 + x) * 4 + 2].abs_diff(r.pixels[(50 * 100 + x - 1) * 4 + 2])).max().unwrap();
    assert!(jump(&smooth) < 20, "default lighting has no abrupt changes between face colours");
    assert_eq!(smooth.pixels, render(&vectorcraft_effects::bake_document(&d).unwrap()).pixels);
    assert_eq!(smooth.pixels, render_threads(&d, 4).pixels);
}

#[test]
fn revolve_rim_has_no_spikes_at_high_zoom_and_keeps_seams_covered() {
    use vectorcraft_doc::AppearanceItem;
    let path = vectorcraft_geom::PathData::from_bezpath(&vectorcraft_geom::BezPath::from_svg("M70 30 L70 70").unwrap());
    let mut n = Node::path(NodeId(0), path, Appearance::basic(Paint::solid(Color::rgb(0.2, 0.55, 0.9)), Paint::None, 0.0));
    n.appearance.effects.push(fx("threeD.revolve", json!({"offset":20,"rotationX":-18,"segments":24,"capEnds":true})));
    let d = doc_with(Document::new(100.0, 100.0), n);
    let baked = vectorcraft_effects::bake_document(&d).unwrap();
    let mut fills = baked.clone();
    fn remove_edges(n: &mut Node) {
        n.appearance.items.retain(|i| matches!(i, AppearanceItem::Fill(_)));
        for child in n.children_mut().into_iter().flatten() {
            remove_edges(Arc::make_mut(child));
        }
    }
    for layer in &mut fills.layers {
        remove_edges(Arc::make_mut(layer));
    }
    let bounds = fills.layers[0].geometric_bounds().unwrap();
    let zoom = 24.0;
    let xf = Affine::translate((600.0 - 50.0 * zoom, 50.0 - bounds.y0 * zoom)) * Affine::scale(zoom);
    let draw =
        |doc: &Document| Renderer::new().render(doc, 1200, 400, xf, &RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() });
    let reference = draw(&fills);
    let live = draw(&d);
    // The seam stroke may extend half its width, plus antialiasing, past the fill
    // silhouette. Miter spikes previously reached several times that distance.
    let reach = (0.375 * zoom) as u32 + 2;
    let top = |img: &Rendered| (0..1200).map(|x| (0..400).find(|y| lum(img.pixel(x, *y)) < 600).unwrap_or(400)).collect::<Vec<_>>();
    let fill_top = top(&reference);
    for (x, y) in top(&live).into_iter().enumerate().filter(|(_, y)| *y < 400) {
        let near = *fill_top[x.saturating_sub(reach as usize)..=(x + reach as usize).min(1199)].iter().min().unwrap();
        assert!(y + reach >= near, "rim spike at x {x}: ink starts at {y}, nearest fill at {near}");
    }
    assert!((200..1000).all(|x| lum(live.pixel(x, 200)) < 600), "seam coverage leaves no white gaps across the cap");
    assert_eq!(live.pixels, draw(&baked).pixels, "live and exported geometry agree at high zoom");
}

#[test]
fn continuous_revolve_caps_keep_one_colour_along_the_curved_rim_at_high_zoom() {
    let path = vectorcraft_geom::PathData::from_bezpath(&vectorcraft_geom::BezPath::from_svg("M70 30 L70 70").unwrap());
    for rotation in [-18.0f64, 18.0] {
        for shade in [false, true] {
            let mut n = Node::path(NodeId(0), path.clone(), Appearance::basic(Paint::solid(Color::rgb(0.2, 0.55, 0.9)), Paint::None, 0.0));
            n.appearance.effects.push(fx("threeD.revolve", json!({"offset":20,"rotationX":rotation,"segments":8,"capEnds":true,"shade":shade})));
            let d = doc_with(Document::new(100.0, 100.0), n);
            let center_y = 50.0 + if rotation < 0.0 { -20.0 } else { 20.0 } * rotation.to_radians().cos();
            let zoom = 24.0;
            let xf = Affine::translate((600.0 - 50.0 * zoom, 220.0 - center_y * zoom)) * Affine::scale(zoom);
            let draw = |doc: &Document| {
                Renderer::new().render(doc, 1200, 440, xf, &RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() })
            };
            let live = draw(&d);
            let color = live.pixel(600, 220);
            assert!(lum(color) < 600, "the visible cap is filled");
            // Sample 0.15 pt inside the projected ellipse. A triangle fan with
            // seam strokes lets body colours intrude here between mesh corners.
            for step in 1..80 {
                let theta = std::f64::consts::PI * step as f64 / 80.0;
                let p = xf * Point::new(50.0 + 19.85 * theta.cos(), center_y + 19.85 * rotation.to_radians().abs().sin() * theta.sin());
                let pixel = live.pixel(p.x.round() as u32, p.y.round() as u32);
                assert!(
                    pixel.iter().zip(color).all(|(a, b)| a.abs_diff(b) <= 1),
                    "cap rim is continuous: rotation {rotation}, shade {shade}, theta {theta}, {pixel:?} vs {color:?}"
                );
            }
            assert_eq!(live.pixels, draw(&vectorcraft_effects::bake_document(&d).unwrap()).pixels);
        }
    }
}

#[test]
fn revolve_visible_surface_expansion_preserves_the_view_and_transparent_art() {
    let mut bp = vectorcraft_geom::BezPath::new();
    bp.move_to((50.0, 15.0));
    bp.curve_to((80.0, 25.0), (80.0, 75.0), (50.0, 90.0));
    let mut n = Node::path(
        NodeId(0),
        vectorcraft_geom::PathData::from_bezpath(&bp),
        Appearance::basic(Paint::solid(Color::rgb(0.2, 0.55, 0.9)), Paint::None, 0.0),
    );
    for (angle, rotation, perspective, opacity, shade) in [
        (360.0, 0.0, 0.0, 1.0, false),
        (360.0, -35.0, 55.0, 1.0, false),
        (150.0, -28.0, 40.0, 1.0, false),
        (360.0, -35.0, 55.0, 0.5, false),
        (360.0, -35.0, 55.0, 1.0, true),
        (150.0, -28.0, 40.0, 1.0, true),
        (360.0, -35.0, 55.0, 0.5, true),
    ] {
        n.opacity = opacity;
        n.appearance.effects = vec![fx(
            "threeD.revolve",
            json!({"angle":angle,"rotationX":rotation,"rotationY":25,"perspective":perspective,"segments":32,"expandVisibleOnly":true,"shade":shade}),
        )];
        let d = doc_with(Document::new(100.0, 100.0), n.clone());
        let live = render(&d);
        let mut expanded = d.clone();
        let id = expanded.layers[0].children().unwrap()[0].id;
        let source = expanded.node(id).unwrap().clone();
        let mut stroke = |_: &mut Document, _: &vectorcraft_geom::PathData, _: vectorcraft_geom::FillRule, _: &vectorcraft_doc::StrokeLayer| None;
        let art = vectorcraft_effects::expand_leaf(&mut expanded, &source, &mut stroke).unwrap();
        *expanded.node_mut(id).unwrap() = art;
        let flat = render(&expanded);
        let difference: u64 = live.pixels.iter().zip(&flat.pixels).map(|(a, b)| a.abs_diff(*b) as u64).sum();
        let average = difference as f64 / live.pixels.len() as f64;
        assert!(average < 1.0, "appearance differs by {average} channel levels: angle {angle}, rotation {rotation}, opacity {opacity}");
        if opacity < 1.0 {
            assert_eq!(live.pixels, flat.pixels);
        }
        // Export baking keeps every face, even when the expansion checkbox is checked.
        assert_eq!(live.pixels, render(&vectorcraft_effects::bake_document(&d).unwrap()).pixels);
    }
}

/// A 30×30 red image placed at (20, 20).
fn image_doc(effects: Vec<Effect>) -> Document {
    let mut d = Document::new(100.0, 100.0);
    let img = image::RgbaImage::from_pixel(30, 30, image::Rgba([255, 0, 0, 255]));
    let mut png = vec![];
    img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    d.images.insert("red".into(), ImageBlob::new("image/png", png));
    let mut n = Node::new(
        NodeId(0),
        NodeKind::Image(Box::new(ImageObject {
            key: "red".into(),
            width: 30,
            height: 30,
            xf: Affine::translate((20.0, 20.0)),
            link: None,
            placement: Default::default(),
        })),
    );
    n.appearance.effects = effects;
    doc_with(d, n)
}

/// An instance at (50, 50) of a symbol whose art is a black 20×20 square centred on its origin.
fn symbol_doc(effects: Vec<Effect>) -> Document {
    let mut d = Document::new(100.0, 100.0);
    let art = Node::path(
        d.alloc_id(),
        shapes::rectangle(Rect::new(-10.0, -10.0, 10.0, 10.0)),
        Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0),
    );
    d.symbols.push(Symbol { name: "Square".into(), art: Arc::new(art) });
    let mut n = Node::new(NodeId(0), NodeKind::SymbolInstance { symbol: "Square".into(), xf: Affine::translate((50.0, 50.0)) });
    n.appearance.effects = effects;
    doc_with(d, n)
}

/// Black 40 pt "IIII" with its baseline at (5, 60).
fn text_doc(effects: Vec<Effect>) -> Document {
    let style = CharStyle { size: 40.0, fill: Paint::solid(Color::BLACK), ..Default::default() };
    let mut n = Node::new(NodeId(0), NodeKind::Text(Box::new(TextObject::point(Point::new(5.0, 60.0), "IIII", style))));
    n.appearance.effects = effects;
    doc_with(Document::new(100.0, 100.0), n)
}

/// Columns (x) holding any dark pixel.
fn inked_columns(img: &Rendered) -> Vec<u32> {
    (0..100).filter(|x| (0..100).any(|y| lum(img.pixel(*x, y)) < 300)).collect()
}

#[test]
fn a_shadow_on_an_image_paints_offset_pixels() {
    let plain = render(&image_doc(vec![]));
    assert_eq!(lum(plain.pixel(55, 55)), WHITE);
    let d = image_doc(vec![fx("stylize.dropShadow", json!({"x": 10, "y": 10, "blur": 0, "opacity": 100, "mode": "normal"}))]);
    let img = render(&d);
    assert!(lum(img.pixel(55, 55)) < 100, "shadow below-right: {:?}", img.pixel(55, 55));
    assert_eq!(img.pixel(35, 35), [255, 0, 0, 255], "the image stays on top");
    assert_eq!(lum(img.pixel(15, 15)), WHITE);
    // The multithreaded pipeline filters offscreen and matches.
    let mt = render_threads(&d, 3);
    let worst = img.pixels.iter().zip(&mt.pixels).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 4, "max channel difference {worst}");
}

#[test]
fn blur_on_a_symbol_instance_softens_its_edges() {
    let plain = render(&symbol_doc(vec![]));
    assert_eq!(lum(plain.pixel(38, 50)), WHITE);
    assert_eq!(lum(plain.pixel(50, 50)), 0);
    let img = render(&symbol_doc(vec![fx("blur.gaussian", json!({"radius": 8}))]));
    let edge = lum(img.pixel(38, 50));
    assert!(edge > 0 && edge < WHITE, "the blur spreads past the edge: {edge}");
    assert!(lum(img.pixel(50, 50)) < 200, "the centre stays dark");
}

#[test]
fn geometry_effects_reshape_type_through_its_outlines() {
    let plain = inked_columns(&render(&text_doc(vec![])));
    assert!(!plain.is_empty() && plain[0] < 30, "{plain:?}");
    // Transform moves the outlines as one piece.
    let moved = inked_columns(&render(&text_doc(vec![fx("distort.transform", json!({"moveH": 30}))])));
    assert_eq!(moved.first().copied(), plain.first().map(|x| x + 30));
    // Roughen redraws the glyphs as rough outlines.
    let d = text_doc(vec![fx("distort.roughen", json!({"size": 4, "relative": false, "detail": 30, "points": "corner"}))]);
    let rough = render(&d);
    let smooth = render(&text_doc(vec![]));
    let changed = rough.pixels.chunks(4).zip(smooth.pixels.chunks(4)).filter(|(a, b)| a != b).count();
    assert!(changed > 100, "{changed} pixels changed");
    // The reshaped art is plain paths painted with the text's fill.
    let n = &d.layers[0].children().unwrap()[0];
    let art = vectorcraft_effects::reshape(n, None).unwrap();
    let (mut text, mut black) = (0, 0);
    art.walk(&mut |m| {
        text += usize::from(matches!(m.kind, NodeKind::Text(_)));
        black += usize::from(m.appearance.fill_paint() == Paint::solid(Color::BLACK));
    });
    assert!(text == 0 && black > 0, "{art:?}");
}

#[test]
fn a_strokes_own_glow_paints_around_that_stroke_only() {
    let mut d = Document::new(100.0, 100.0);
    let mut n = Node::path(
        NodeId(0),
        shapes::rectangle(Rect::new(30.0, 30.0, 70.0, 70.0)),
        Appearance::basic(Paint::solid(Color::WHITE), Paint::solid(Color::BLACK), 2.0),
    );
    n.appearance.items[1].effects_mut().push(fx("stylize.outerGlow", json!({"color": "#00ff00", "mode": "normal", "opacity": 100, "blur": 6})));
    d = doc_with(d, n);
    let img = render(&d);
    let g = img.pixel(27, 50);
    assert!(g[1] > g[0] + 20 && g[1] > g[2] + 20, "green glow outside the stroke: {g:?}");
    assert_eq!(img.pixel(50, 50), [255, 255, 255, 255], "the fill has no glow");
    let mt = render_threads(&d, 3);
    let worst = img.pixels.iter().zip(&mt.pixels).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
    assert!(worst <= 4, "max channel difference {worst}");
}
