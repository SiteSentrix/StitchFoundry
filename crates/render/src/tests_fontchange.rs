//! Type redraws in a font that becomes available while it is on screen (fonts added, Refresh Font
//! List).

use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

use vectorcraft_doc::text::{CharStyle, TextObject};
use vectorcraft_doc::{Node, NodeKind};
use vectorcraft_geom::Point;

use super::*;

/// Taken to add fonts. Renderers lay type out again when the fonts change, so tests of what they
/// keep between frames hold it shared ([`fonts_steady`]) and the tests here hold it alone.
static FONTS: RwLock<()> = RwLock::new(());

/// No fonts are added by these tests until the guard is dropped.
pub(crate) fn fonts_steady() -> RwLockReadGuard<'static, ()> {
    // A test that failed holding it leaves nothing to undo.
    FONTS.read().unwrap_or_else(PoisonError::into_inner)
}

/// No test holding [`fonts_steady`] runs until the guard is dropped.
fn adding_fonts() -> RwLockWriteGuard<'static, ()> {
    FONTS.write().unwrap_or_else(PoisonError::into_inner)
}

/// The bundled Source Serif 4 renamed `family` (as long as the original name).
fn serif_as(family: &str) -> Vec<u8> {
    let mut data = include_bytes!("../../../assets/fonts/SourceSerif4-Regular.ttf").to_vec();
    let utf16 = |s: &str| s.encode_utf16().flat_map(u16::to_be_bytes).collect::<Vec<u8>>();
    for (from, to) in [(utf16("Source Serif 4"), utf16(family)), (b"Source Serif 4".to_vec(), family.as_bytes().to_vec())] {
        assert_eq!(from.len(), to.len());
        let mut i = 0;
        while let Some(at) = data[i..].windows(from.len()).position(|w| w == from) {
            data[i + at..i + at + to.len()].copy_from_slice(&to);
            i += at + to.len();
        }
    }
    data
}

#[test]
fn type_redraws_when_its_font_becomes_available() {
    let _adding = adding_fonts();
    const FAMILY: &str = "Arrival Serif4";
    let mut d = Document::new(300.0, 150.0);
    let style = CharStyle { size: 60.0, font_family: FAMILY.into(), ..Default::default() };
    let mut n = Node::new(NodeId(0), NodeKind::Text(Box::new(TextObject::point(Point::new(10.0, 100.0), "Hamburg", style))));
    n.id = d.alloc_id();
    let l = d.layers[0].id;
    d.insert(Some(l), 0, n).unwrap();
    let opts = RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() };
    let mut r = Renderer::new();
    let before = r.render(&d, 300, 150, Affine::IDENTITY, &opts).pixels;
    assert!(vectorcraft_text::FontDb::global().add_font(serif_as(FAMILY)) > 0);
    let after = r.render(&d, 300, 150, Affine::IDENTITY, &opts).pixels;
    assert_ne!(before, after, "drawn in the font that arrived");
    assert_eq!(after, Renderer::new().render(&d, 300, 150, Affine::IDENTITY, &opts).pixels, "as a new renderer draws it");
}

/// Type with raster effects redraws in a font that becomes available: its drop shadow and its
/// blurred content are made again.
#[test]
fn type_with_raster_effects_redraws_when_its_font_becomes_available() {
    let _adding = adding_fonts();
    const FAMILY: &str = "Raster Serif 4";
    let mut d = Document::new(300.0, 150.0);
    let l = d.layers[0].id;
    let effects =
        [("stylize.dropShadow", serde_json::json!({"x": 4, "y": 4, "blur": 1, "opacity": 100})), ("blur.gaussian", serde_json::json!({"radius": 1}))];
    for (i, (id, params)) in effects.into_iter().enumerate() {
        let style = CharStyle { size: 50.0, font_family: FAMILY.into(), ..Default::default() };
        let mut n = Node::new(d.alloc_id(), NodeKind::Text(Box::new(TextObject::point(Point::new(10.0, 60.0 + 70.0 * i as f64), "Hamburg", style))));
        n.appearance.effects.push(vectorcraft_doc::Effect { id: id.into(), params, visible: true });
        d.insert(Some(l), i, n).unwrap();
    }
    let opts = RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() };
    let mut r = Renderer::new();
    let before = r.render(&d, 300, 150, Affine::IDENTITY, &opts).pixels;
    assert!(vectorcraft_text::FontDb::global().add_font(serif_as(FAMILY)) > 0);
    let after = r.render(&d, 300, 150, Affine::IDENTITY, &opts).pixels;
    assert_ne!(before, after, "drawn in the font that arrived");
    assert_eq!(after, Renderer::new().render(&d, 300, 150, Affine::IDENTITY, &opts).pixels, "as a new renderer draws it");
}

/// Type with a geometry effect in a symbol's art redraws in a font that becomes available: the art
/// kept for the instance is placed again when the fonts change.
#[test]
fn symbol_type_redraws_when_its_font_becomes_available() {
    let _adding = adding_fonts();
    const FAMILY: &str = "Placed Serif 4";
    let mut d = Document::new(300.0, 150.0);
    let style = CharStyle { size: 60.0, font_family: FAMILY.into(), ..Default::default() };
    let mut t = Node::new(d.alloc_id(), NodeKind::Text(Box::new(TextObject::point(Point::new(-140.0, 25.0), "Hamburg", style))));
    t.appearance.effects.push(vectorcraft_doc::Effect { id: "distort.transform".into(), params: serde_json::json!({"moveH": 0.0}), visible: true });
    let art = Node::group(d.alloc_id(), vec![Arc::new(t)]);
    d.symbols.push(vectorcraft_doc::Symbol { name: "Word".into(), art: Arc::new(art) });
    let (l, id) = (d.layers[0].id, d.alloc_id());
    d.insert(Some(l), 0, Node::new(id, NodeKind::SymbolInstance { symbol: "Word".into(), xf: Affine::translate((150.0, 75.0)) })).unwrap();
    let opts = RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() };
    let mut r = Renderer::new();
    let before = r.render(&d, 300, 150, Affine::IDENTITY, &opts).pixels;
    assert!(vectorcraft_text::FontDb::global().add_font(serif_as(FAMILY)) > 0);
    let after = r.render(&d, 300, 150, Affine::IDENTITY, &opts).pixels;
    assert_ne!(before, after, "drawn in the font that arrived");
    assert_eq!(after, Renderer::new().render(&d, 300, 150, Affine::IDENTITY, &opts).pixels, "as a new renderer draws it");
}
