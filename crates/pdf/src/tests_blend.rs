//! Blend modes and transparency groups in PDF export.

use vectorcraft_color::{BlendMode, Color, Paint};
use vectorcraft_doc::{Appearance, Document, Node, NodeId};
use vectorcraft_geom::{Rect, shapes};

use crate::*;

fn rect(r: Rect, rgb: (f32, f32, f32)) -> Node {
    Node::path(NodeId(0), shapes::rectangle(r), Appearance::basic(Paint::solid(Color::rgb(rgb.0, rgb.1, rgb.2)), Paint::None, 0.0))
}

fn add(d: &mut Document, mut n: Node) {
    n.id = d.alloc_id();
    let l = d.layers[0].id;
    d.insert(Some(l), usize::MAX, n).unwrap();
}

/// The file's text with spaces removed (content streams uncompressed).
fn text(d: &Document) -> String {
    let bytes = export(d, &PdfOptions { compress: false, ..Default::default() }).unwrap();
    String::from_utf8_lossy(&bytes).replace(' ', "")
}

#[test]
fn every_blend_mode_is_written_under_its_pdf_name_and_read_back() {
    for mode in BlendMode::ALL.into_iter().filter(|m| *m != BlendMode::Normal) {
        let mut d = Document::new(100.0, 100.0);
        add(&mut d, rect(Rect::new(0.0, 0.0, 100.0, 100.0), (0.2, 0.5, 0.9)));
        let mut top = rect(Rect::new(20.0, 20.0, 80.0, 80.0), (0.6, 0.25, 0.5));
        top.blend = mode;
        add(&mut d, top);
        let name = mode.label().replace(' ', "");
        assert!(text(&d).contains(&format!("/BM/{name}")), "{mode:?}");
        let back = import(&export(&d, &PdfOptions::default()).unwrap()).unwrap();
        let mut found = BlendMode::Normal;
        back.walk(|n| {
            if n.blend != BlendMode::Normal {
                found = n.blend;
            }
        });
        assert_eq!(found, mode);
    }
}
