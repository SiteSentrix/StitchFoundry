//! Stroke geometry in PDF export: arrowheads come from the shared `vectorcraft_effects::stroke`
//! geometry, the same the canvas draws.

use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, AppearanceItem, Arrowhead, Document, Node, NodeId, NodeKind, StrokeLayer};
use vectorcraft_effects::stroke::stroke_pieces;
use vectorcraft_geom::{Point, Rect, Shape, shapes};

use crate::*;

fn line_doc(st: StrokeLayer) -> (Document, Node) {
    let mut d = Document::new(100.0, 100.0);
    let mut n = Node::path(
        NodeId(0),
        shapes::line(Point::new(20.0, 50.0), Point::new(80.0, 50.0)),
        Appearance { items: vec![AppearanceItem::Stroke(st)], effects: vec![] },
    );
    n.id = d.alloc_id();
    let layer = d.default_layer().unwrap();
    d.insert(Some(layer), 0, n.clone()).unwrap();
    (d, n)
}

fn arrow_stroke(kind: Arrowhead) -> StrokeLayer {
    let mut st = StrokeLayer::new(Paint::solid(Color::BLACK), 4.0);
    st.end_arrow = Some(kind);
    st
}

fn close(a: Rect, b: Rect) -> bool {
    [(a.x0, b.x0), (a.y0, b.y0), (a.x1, b.x1), (a.y1, b.y1)].iter().all(|(p, q)| (p - q).abs() < 0.01)
}

/// Bounding boxes of the filled and the stroked paths of a PDF read back.
fn painted_boxes(d: &Document) -> (Vec<Rect>, Vec<Rect>) {
    let back = import(&export(d, &PdfOptions::default()).unwrap()).unwrap();
    let (mut fills, mut strokes) = (vec![], vec![]);
    back.walk(|m| {
        let Some(bb) = m.geometric_bounds().filter(|_| matches!(m.kind, NodeKind::Path { .. } | NodeKind::Compound { .. })) else { return };
        match m.appearance.items.first() {
            Some(AppearanceItem::Fill(_)) => fills.push(bb),
            Some(AppearanceItem::Stroke(_)) => strokes.push(bb),
            None => {}
        }
    });
    (fills, strokes)
}

#[test]
fn arrowheads_match_the_shared_geometry() {
    for kind in Arrowhead::ALL {
        let (d, n) = line_doc(arrow_stroke(kind));
        let bp = n.path_data().unwrap().to_bezpath();
        let pieces = stroke_pieces(&bp, n.appearance.stroke().unwrap());
        let (fills, strokes) = painted_boxes(&d);
        let head = pieces.heads[0].outline.bounding_box();
        assert_eq!(fills.len(), 1, "{kind:?}: one head");
        assert!(close(fills[0], head), "{kind:?}: {:?} vs {head:?}", fills[0]);
        assert_eq!(strokes.len(), 1, "{kind:?}: one line");
        assert!(close(strokes[0], bp.bounding_box()), "{kind:?}: {:?}", strokes[0]);
    }
}
