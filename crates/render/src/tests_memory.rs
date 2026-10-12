//! What renderers keep between frames: decoded images shared by all of them, image mips, symbol
//! art placed for instances and inline graphics, and caches swept by every kind of render.

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
    d.insert(Some(layer), 0, Node::new(id, NodeKind::Image(Box::new(im)))).unwrap();
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

/// A document of `n` point type objects reading `word`, in rows.
fn text_doc(n: usize, word: &str) -> Document {
    use vectorcraft_doc::text::{CharStyle, TextObject};
    let mut d = Document::new(400.0, 400.0);
    let l = d.layers[0].id;
    for i in 0..n {
        let at = vectorcraft_geom::Point::new(10.0 + (i % 8) as f64 * 48.0, 30.0 + (i / 8) as f64 * 30.0);
        let style = CharStyle { size: 18.0, ..Default::default() };
        let mut t = Node::new(NodeId(0), NodeKind::Text(Box::new(TextObject::point(at, word, style))));
        t.id = d.alloc_id();
        d.insert(Some(l), 0, t).unwrap();
    }
    d
}

/// The cache keys of `d`'s type objects (their nodes' addresses).
fn text_keys(d: &Document) -> Vec<usize> {
    let mut keys = vec![];
    d.walk(|n| {
        if matches!(n.kind, NodeKind::Text(_)) {
            keys.push(n as *const Node as usize);
        }
    });
    keys
}

/// More type than the glyph cache's budget holds: the entries kept stay from frame to frame (the
/// rest is laid out again), where emptying the cache when full laid out every object every frame.
#[test]
fn type_over_the_text_budget_keeps_its_entries_between_frames() {
    let d = text_doc(40, "Type");
    let opts = RenderOptions::default();
    let mut r = Renderer::new();
    let want = r.render(&d, 400, 400, Affine::IDENTITY, &opts).pixels;
    assert_eq!(r.texts.len(), 40);
    let all = r.texts.bytes();
    let mut r = Renderer::new();
    r.texts = FrameCache::new(all / 2, 1024);
    let keys = text_keys(&d);
    let kept = |r: &Renderer| keys.iter().filter(|k| r.texts.contains_key(*k)).copied().collect::<Vec<_>>();
    assert_eq!(r.render(&d, 400, 400, Affine::IDENTITY, &opts).pixels, want);
    let first = kept(&r);
    assert!(!first.is_empty() && first.len() < 40 && r.texts.bytes() <= all / 2, "{} entries, {} bytes", first.len(), r.texts.bytes());
    for _ in 0..3 {
        assert_eq!(r.render(&d, 400, 400, Affine::IDENTITY, &opts).pixels, want);
        assert_eq!(kept(&r), first, "the same entries stay");
    }
    // Other type: its entries replace the earlier frames' ones, within the budget.
    let e = text_doc(40, "Other");
    r.render(&e, 400, 400, Affine::IDENTITY, &opts);
    assert!(kept(&r).is_empty(), "entries of earlier frames went first");
    assert!(!r.texts.is_empty() && r.texts.bytes() <= all / 2);
}

/// Type no longer drawn leaves the glyph cache after a few frames, and its bytes with it.
#[test]
fn swept_type_entries_give_back_their_bytes() {
    let d = text_doc(12, "Swept");
    let mut r = Renderer::new();
    r.texts = FrameCache::new(lru::TEXT_BUDGET, 0);
    r.render(&d, 400, 400, Affine::IDENTITY, &RenderOptions::default());
    let full = r.texts.bytes();
    assert!(full > 0 && r.texts.len() == 12);
    let empty = Document::new(10.0, 10.0);
    for _ in 0..5 {
        r.render(&empty, 10, 10, Affine::IDENTITY, &RenderOptions::default());
    }
    assert_eq!((r.texts.len(), r.texts.bytes()), (0, 0));
}

/// A symbol's art inside the 20 × 20 square around the origin: a group of a square filled blue
/// with a 2 pt black stroke and a black line.
fn symbol_art(d: &mut Document) -> Node {
    use vectorcraft_doc::Appearance;
    let square = vectorcraft_geom::shapes::rectangle(Rect::new(-8.0, -8.0, 8.0, 8.0));
    let square = Node::path(d.alloc_id(), square, Appearance::basic(Paint::solid(Color::rgb(0.0, 0.0, 1.0)), Paint::solid(Color::BLACK), 2.0));
    let line = vectorcraft_geom::shapes::line((-8.0, 4.0).into(), (8.0, 4.0).into());
    let line = Node::path(d.alloc_id(), line, Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 1.5));
    Node::group(d.alloc_id(), vec![Arc::new(square), Arc::new(line)])
}

/// Three instances of symbol `S` ([`symbol_art`]): two in a row, the second stained red, and a
/// third turned and scaled by 1.5.
fn symbol_doc() -> Document {
    let mut d = Document::new(200.0, 100.0);
    let art = symbol_art(&mut d);
    d.symbols.push(vectorcraft_doc::Symbol { name: "S".into(), art: Arc::new(art) });
    let l = d.layers[0].id;
    let xfs = [
        Affine::translate((30.0, 30.0)),
        Affine::translate((80.0, 30.0)),
        Affine::translate((140.0, 60.0)) * Affine::rotate(0.5) * Affine::scale(1.5),
    ];
    for (i, xf) in xfs.into_iter().enumerate() {
        let mut inst = Node::new(d.alloc_id(), NodeKind::SymbolInstance { symbol: "S".into(), xf });
        if i == 1 {
            inst.appearance.set_fill(Paint::solid(Color::rgb(1.0, 0.0, 0.0)));
        }
        d.insert(Some(l), i, inst).unwrap();
    }
    d
}

/// The addresses of the nodes `r` keeps geometry for that are not the own nodes of any version
/// of the document drawn (`docs`): the symbol art it placed for instances and inline graphics.
fn placed_copies(r: &Renderer, docs: &[&Document]) -> std::collections::BTreeSet<usize> {
    let mut own = std::collections::HashSet::new();
    for d in docs {
        d.walk(|n| {
            own.insert(n as *const Node as usize);
        });
    }
    r.geom.values().map(|e| Arc::as_ptr(&e.node) as usize).filter(|p| !own.contains(p)).collect()
}

/// A second frame of an unchanged document draws the symbol art placed for its instances in the
/// first: it copies no art, and the geometry kept for the art's paths is used again. The renderer
/// holds the symbol's art itself only weakly.
#[test]
fn symbol_instances_keep_their_placed_art_between_frames() {
    let d = symbol_doc();
    let opts = RenderOptions::default();
    let mut r = Renderer::new();
    let first = r.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels;
    let placed = placed_copies(&r, &[&d]);
    assert_eq!(placed.len(), 6, "the square and the line of three instances");
    assert!(r.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels == first);
    assert_eq!(placed_copies(&r, &[&d]), placed, "the second frame draws the art the first placed");
    assert_eq!(Arc::strong_count(&d.symbols[0].art), 1, "only the document holds the symbol's art");
}

/// Moving an instance places its art again, and editing a symbol's art places the art of its
/// instances again: the other instances keep theirs, and each frame draws what a new renderer
/// draws.
#[test]
fn editing_an_instance_or_a_symbol_places_only_its_art_again() {
    let mut d = symbol_doc();
    // Symbol `T` is a green square, and one instance of it stands below the others.
    let green = vectorcraft_doc::Appearance::basic(Paint::solid(Color::rgb(0.0, 1.0, 0.0)), Paint::None, 0.0);
    let square = Node::path(d.alloc_id(), vectorcraft_geom::shapes::rectangle(Rect::new(-5.0, -5.0, 5.0, 5.0)), green);
    let t = Node::group(d.alloc_id(), vec![Arc::new(square)]);
    d.symbols.push(vectorcraft_doc::Symbol { name: "T".into(), art: Arc::new(t) });
    let (l, id) = (d.layers[0].id, d.alloc_id());
    d.insert(Some(l), 0, Node::new(id, NodeKind::SymbolInstance { symbol: "T".into(), xf: Affine::translate((30.0, 80.0)) })).unwrap();
    let opts = RenderOptions::default();
    let frame = |r: &mut Renderer, d: &Document| r.render(d, 200, 100, Affine::IDENTITY, &opts).pixels;
    let mut r = Renderer::new();
    frame(&mut r, &d);
    let before = placed_copies(&r, &[&d]);
    assert_eq!(before.len(), 7);
    // The first instance of `S` moves 5 pt right.
    let unmoved = d.clone();
    let first = d.layers[0].children().unwrap()[1].id;
    if let Some(NodeKind::SymbolInstance { xf, .. }) = d.node_mut(first).map(|n| &mut n.kind) {
        *xf = Affine::translate((5.0, 0.0)) * *xf;
    }
    assert!(frame(&mut r, &d) == frame(&mut Renderer::new(), &d));
    let moved = placed_copies(&r, &[&unmoved, &d]);
    assert!(before.is_subset(&moved) && moved.len() == before.len() + 2, "{} copies, then {}", before.len(), moved.len());
    // `S`'s square is repainted red: its instances show it, and `T`'s instance keeps its art.
    let mut art = (*d.symbols[0].art).clone();
    if let Some(square) = art.children_mut().and_then(|c| c.first_mut()) {
        Arc::make_mut(square).appearance.set_fill(Paint::solid(Color::rgb(1.0, 0.0, 0.0)));
    }
    d.symbols[0].art = Arc::new(art);
    let red = frame(&mut r, &d);
    assert!(red == frame(&mut Renderer::new(), &d));
    let img = Rendered { width: 200, height: 100, pixels: red };
    assert_eq!(img.pixel(140, 60), [255, 0, 0, 255], "the turned instance's square");
    let edited = placed_copies(&r, &[&unmoved, &d]);
    assert!(moved.is_subset(&edited) && edited.len() == moved.len() + 6, "{} copies, then {}", moved.len(), edited.len());
}

/// Draws `d` (200 × 100 points, on white), applies `edit` and draws it again with the same
/// renderer: the frame changed, a new renderer draws the same, and only the document holds the
/// symbols' art.
fn edit_draws_as_new(mut d: Document, edit: impl FnOnce(&mut Document)) {
    let opts = RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() };
    let mut r = Renderer::new();
    let before = r.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels;
    edit(&mut d);
    let after = r.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels;
    assert!(after != before, "the edit changes the frame");
    assert!(after == Renderer::new().render(&d, 200, 100, Affine::IDENTITY, &opts).pixels, "a new renderer draws the same");
    assert!(d.symbols.iter().all(|s| Arc::strong_count(&s.art) == 1), "only the document holds the symbols' art");
}

/// `d` with symbol `Holder` (a group of `members`) shown by one instance at (100, 50).
fn holder_doc(mut d: Document, members: Vec<Node>) -> Document {
    let holder = Node::group(d.alloc_id(), members.into_iter().map(Arc::new).collect());
    d.symbols.push(vectorcraft_doc::Symbol { name: "Holder".into(), art: Arc::new(holder) });
    let (l, id) = (d.layers[0].id, d.alloc_id());
    d.insert(Some(l), 0, Node::new(id, NodeKind::SymbolInstance { symbol: "Holder".into(), xf: Affine::translate((100.0, 50.0)) })).unwrap();
    d
}

/// Symbol art that shows another symbol (type with an inline graphic of it, an instance of it with
/// a Transform effect) is placed again in every frame: when that symbol's art changes, the next
/// frame draws what a new renderer draws.
#[test]
fn art_showing_another_symbol_draws_its_edits() {
    use vectorcraft_doc::text::{CharStyle, InlineArt, TextObject, TextRun};
    let mut d = Document::new(200.0, 100.0);
    let art = symbol_art(&mut d);
    d.symbols.push(vectorcraft_doc::Symbol { name: "S".into(), art: Arc::new(art) });
    let style = CharStyle { size: 24.0, ..Default::default() };
    let mut t = TextObject::point(vectorcraft_geom::Point::new(-60.0, 10.0), "A", style.clone());
    t.runs.push(TextRun::inline(InlineArt::new("S"), style.clone()));
    t.runs.push(TextRun::new("B", style));
    let text = Node::new(d.alloc_id(), NodeKind::Text(Box::new(t)));
    let mut moved = Node::new(d.alloc_id(), NodeKind::SymbolInstance { symbol: "S".into(), xf: Affine::translate((40.0, 0.0)) });
    moved.appearance.effects.push(vectorcraft_doc::Effect {
        id: "distort.transform".into(),
        params: serde_json::json!({"moveH": 4.0}),
        visible: true,
    });
    let d = holder_doc(d, vec![text, moved]);
    // `S` becomes a wide red bar: the inline graphic grows, and the type after it moves.
    edit_draws_as_new(d, |d| {
        let bar = vectorcraft_geom::shapes::rectangle(Rect::new(-20.0, -4.0, 20.0, 4.0));
        let red = vectorcraft_doc::Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::None, 0.0);
        d.symbols[0].art = Arc::new(Node::path(d.alloc_id(), bar, red));
    });
}

/// Symbol art with raster effects is placed again in every frame: when a pattern under a blur or
/// the document's raster resolution under De-Interlace changes, the next frame draws what a new
/// renderer draws.
#[test]
fn art_with_raster_effects_draws_pattern_and_resolution_edits() {
    use vectorcraft_doc::pattern::{PatternDef, pattern_paint};
    let effect = |id: &str, params: serde_json::Value| vectorcraft_doc::Effect { id: id.into(), params, visible: true };
    let mut d = Document::new(200.0, 100.0);
    d.raster_effects_ppi = 72.0;
    // Pattern `Checks` is a red 10 × 10 square in a 20 × 20 tile.
    let tile = |c: Color| {
        let square = vectorcraft_geom::shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0));
        vec![Arc::new(Node::path(NodeId(900), square, vectorcraft_doc::Appearance::basic(Paint::solid(c), Paint::None, 0.0)))]
    };
    let mut def = PatternDef::new("Checks", tile(Color::rgb(1.0, 0.0, 0.0)));
    def.tile = Rect::new(0.0, 0.0, 20.0, 20.0);
    d.patterns.push(def);
    let band = vectorcraft_geom::shapes::rectangle(Rect::new(-90.0, -45.0, -20.0, -25.0));
    let mut blurred = Node::path(d.alloc_id(), band, vectorcraft_doc::Appearance::basic(pattern_paint("Checks"), Paint::None, 0.0));
    blurred.appearance.effects.push(effect("blur.gaussian", serde_json::json!({"radius": 2.0})));
    // The path holds ten bars, 1 pt high and 2 pt apart.
    let mut bars = vectorcraft_geom::shapes::rectangle(Rect::new(20.0, -40.0, 90.0, -39.0));
    for k in 1..10 {
        let y = -40.0 + 2.0 * k as f64;
        bars.subpaths.extend(vectorcraft_geom::shapes::rectangle(Rect::new(20.0, y, 90.0, y + 1.0)).subpaths);
    }
    let mut striped = Node::path(d.alloc_id(), bars, vectorcraft_doc::Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0));
    striped.appearance.effects.push(effect("video.deinterlace", serde_json::json!({})));
    let d = holder_doc(d, vec![blurred, striped]);
    edit_draws_as_new(d, |d| {
        if let Some(def) = d.pattern_mut("Checks") {
            def.art = tile(Color::rgb(0.0, 0.0, 1.0));
        }
        d.raster_effects_ppi = 24.0;
    });
}

/// Symbol art with a live object (an envelope that distorts a pattern fill) is placed again in
/// every frame: when the pattern's tile type changes, the next frame draws what a new renderer
/// draws.
#[test]
fn art_with_live_objects_draws_pattern_layout_edits() {
    use vectorcraft_doc::live::EnvelopeOptions;
    use vectorcraft_doc::pattern::{PatternDef, TileType, pattern_paint};
    let mut d = Document::new(200.0, 100.0);
    // Pattern `Checks` is a red 10 × 10 square in a 20 × 20 tile.
    let square = vectorcraft_geom::shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0));
    let red = vectorcraft_doc::Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::None, 0.0);
    let mut def = PatternDef::new("Checks", vec![Arc::new(Node::path(NodeId(900), square, red))]);
    def.tile = Rect::new(0.0, 0.0, 20.0, 20.0);
    d.patterns.push(def);
    let band = vectorcraft_geom::shapes::rectangle(Rect::new(-80.0, -40.0, 80.0, 40.0));
    let filled = Node::path(d.alloc_id(), band, vectorcraft_doc::Appearance::basic(pattern_paint("Checks"), Paint::None, 0.0));
    let kind = vectorcraft_doc::EnvelopeKind::Warp { style: "arc".into(), bend: 30.0, h: 0.0, v: 0.0, horizontal: true };
    let options = EnvelopeOptions { distort_appearance: true, distort_pattern_fills: true, ..Default::default() };
    let envelope = Node::new(
        d.alloc_id(),
        NodeKind::Envelope {
            content: vec![Arc::new(filled)],
            kind: Box::new(kind),
            fidelity: 50.0,
            editing: false,
            options,
            frame: Box::new(Affine::IDENTITY),
        },
    );
    let d = holder_doc(d, vec![envelope]);
    edit_draws_as_new(d, |d| {
        if let Some(def) = d.pattern_mut("Checks") {
            def.tile_type = TileType::BrickByRow { offset: 0.5 };
        }
    });
}

/// Symbol instances draw what groups of their art, stained and placed, draw: in every frame, when
/// two instances placed alike are stained differently, when the art is a single path, and with a
/// cache too small for all of the art. Art no longer drawn leaves the cache after a few frames.
#[test]
fn symbol_instances_draw_as_groups_of_their_placed_art() {
    let mut d = symbol_doc();
    let l = d.layers[0].id;
    // An instance placed like the first one lies over it, stained green.
    let mut over = Node::new(d.alloc_id(), NodeKind::SymbolInstance { symbol: "S".into(), xf: Affine::translate((30.0, 30.0)) });
    over.appearance.set_fill(Paint::solid(Color::rgb(0.0, 1.0, 0.0)));
    d.insert(Some(l), 1, over).unwrap();
    // Symbol `U` is a single path, a circle with a black stroke.
    let circle = vectorcraft_geom::shapes::ellipse(Rect::new(-6.0, -6.0, 6.0, 6.0));
    let ap = vectorcraft_doc::Appearance::basic(Paint::solid(Color::rgb(1.0, 0.8, 0.0)), Paint::solid(Color::BLACK), 1.0);
    let circle = Node::path(d.alloc_id(), circle, ap);
    d.symbols.push(vectorcraft_doc::Symbol { name: "U".into(), art: Arc::new(circle) });
    let id = d.alloc_id();
    d.insert(Some(l), usize::MAX, Node::new(id, NodeKind::SymbolInstance { symbol: "U".into(), xf: Affine::translate((175.0, 25.0)) })).unwrap();
    // `groups` holds the same art in place of the instances: groups, and the circle as a path.
    let mut groups = d.clone();
    let placed = d.layers[0].children().unwrap().iter().map(|c| match &c.kind {
        NodeKind::SymbolInstance { symbol, xf } => {
            let sym = d.symbols.iter().find(|s| s.name == *symbol).unwrap();
            let mut art = instance_art(&sym.art, c);
            art.transform(*xf, false);
            Arc::new(art)
        }
        _ => c.clone(),
    });
    *groups.children_mut(Some(l)).unwrap() = placed.collect();
    let opts = RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() };
    let want = Renderer::new().render(&groups, 200, 100, Affine::IDENTITY, &opts).pixels;
    let mut r = Renderer::new();
    r.placed_arts = FrameCache::new(lru::PLACED_ART_BUDGET, 0);
    for frame in 0..3 {
        assert!(r.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels == want, "frame {frame}");
    }
    // A cache that holds half the bytes of that art keeps what fits and places the rest every frame.
    let all = r.placed_arts.bytes();
    let mut small = Renderer::new();
    small.placed_arts = FrameCache::new(all / 2, 0);
    for frame in 0..3 {
        assert!(small.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels == want, "frame {frame} over the budget");
        assert!(!small.placed_arts.is_empty() && small.placed_arts.bytes() <= all / 2, "{} of {all} bytes", small.placed_arts.bytes());
    }
    let empty = Document::new(10.0, 10.0);
    for _ in 0..5 {
        r.render(&empty, 10, 10, Affine::IDENTITY, &opts);
    }
    assert_eq!((r.placed_arts.len(), r.placed_arts.bytes()), (0, 0));
}

/// Inline graphics in type draw the art placed for them in an earlier frame, as instances do.
#[test]
fn inline_graphics_keep_their_placed_art_between_frames() {
    use vectorcraft_doc::text::{CharStyle, InlineArt, TextObject, TextRun};
    let mut d = Document::new(200.0, 100.0);
    let art = symbol_art(&mut d);
    d.symbols.push(vectorcraft_doc::Symbol { name: "S".into(), art: Arc::new(art) });
    let style = CharStyle { size: 24.0, ..Default::default() };
    let mut t = TextObject::point(vectorcraft_geom::Point::new(10.0, 50.0), "A", style.clone());
    t.runs.push(TextRun::inline(InlineArt::new("S"), style.clone()));
    t.runs.push(TextRun::new("B", style));
    let (l, id) = (d.layers[0].id, d.alloc_id());
    d.insert(Some(l), 0, Node::new(id, NodeKind::Text(Box::new(t)))).unwrap();
    let opts = RenderOptions::default();
    let mut r = Renderer::new();
    let first = r.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels;
    let placed = placed_copies(&r, &[&d]);
    assert_eq!(placed.len(), 2, "the square and the line of the inline graphic");
    assert!(r.render(&d, 200, 100, Affine::IDENTITY, &opts).pixels == first);
    assert_eq!(placed_copies(&r, &[&d]), placed, "the second frame draws the art the first placed");
}

/// Each run draws exactly the outlines it drew when kept as a path of its own: its glyphs and
/// decorations in layout order (the whole text's path is kept once, the runs as spans of it).
#[test]
fn text_runs_draw_their_own_glyphs_from_the_shared_outlines() {
    use vectorcraft_doc::text::{CharStyle, TextObject, TextRun};
    let plain = CharStyle { size: 24.0, ..Default::default() };
    let under = CharStyle { underline: true, ..plain.clone() };
    let mut t = TextObject::point(vectorcraft_geom::Point::new(10.0, 40.0), "Ab", plain.clone());
    t.runs.push(TextRun::new("cd", under));
    t.runs.push(TextRun::new("ef", plain));
    let g = text_geom(&t);
    let db = vectorcraft_text::FontDb::global();
    let layout = vectorcraft_text::layout(db, &t);
    let mut want = vec![BezPath::new(); t.runs.len()];
    for gl in &layout.glyphs {
        want[gl.run].extend(gl.outline.iter());
    }
    for (run, bar) in vectorcraft_text::decorations(&layout, db, &t) {
        want[run].extend(bar.iter());
    }
    for (i, w) in want.iter().enumerate() {
        assert_eq!(g.run(i).elements(), w.elements(), "run {i}");
    }
    assert!(g.run(7).elements().is_empty());
}
