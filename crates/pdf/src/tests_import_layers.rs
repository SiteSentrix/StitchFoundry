//! Optional content groups become layers: name, visibility, print state and lock; art that is
//! off comes in as a hidden layer.

use vectorcraft_color::Color;
use vectorcraft_doc::{Document, Node, NodeKind};
use vectorcraft_testkit::pdf::{PdfPage, first_extra, pdf_with_catalog};

use crate::tests_import_fidelity::stream;
use crate::*;

/// Groups "Shapes" and "Hidden" (not printed) as the first extra objects of a file of `pages`
/// pages, the catalog entries listing them with `config` (where `{A}` and `{B}` stand for them)
/// as the default configuration, and their object numbers.
fn groups(pages: usize, config: &str) -> ([String; 2], String, [usize; 2]) {
    let (a, b) = (first_extra(pages), first_extra(pages) + 1);
    let objs = [
        "<< /Type /OCG /Name (Shapes) >>".to_string(),
        "<< /Type /OCG /Name <FEFF00480069006400640065006E> /Usage << /Print << /PrintState /OFF >> >> >>".to_string(),
    ];
    let catalog = format!(
        "/OCProperties << /OCGs [{a} 0 R {b} 0 R] /D << /Order [{a} 0 R {b} 0 R] {} >> >> ",
        config.replace("{A}", &format!("{a} 0 R")).replace("{B}", &format!("{b} 0 R"))
    );
    (objs, catalog, [a, b])
}

/// A page drawing red in "Shapes", blue in "Hidden" and green outside any group.
fn page(ids: [usize; 2]) -> PdfPage {
    PdfPage {
        resources: format!("/Properties << /MC0 {} 0 R /MC1 {} 0 R >>", ids[0], ids[1]),
        ..PdfPage::new(100.0, 100.0, "/OC /MC0 BDC 1 0 0 rg 10 10 30 30 re f EMC /OC /MC1 BDC 0 0 1 rg 50 50 30 30 re f EMC 0 1 0 rg 0 0 5 5 re f")
    }
}

fn file(config: &str) -> Vec<u8> {
    let (objs, catalog, ids) = groups(1, config);
    pdf_with_catalog(&[page(ids)], &[&objs[0], &objs[1]], &catalog, None)
}

fn names(d: &Document) -> Vec<String> {
    d.layers.iter().map(|l| l.name.clone().unwrap_or_default()).collect()
}

/// The fill colours of a layer's art.
fn colors(l: &Node) -> Vec<Color> {
    l.children().unwrap().iter().filter_map(|n| n.appearance.fill().and_then(|f| f.paint.color())).collect()
}

fn printable(l: &Node) -> bool {
    matches!(l.kind, NodeKind::Layer { printable: true, .. })
}

#[test]
fn optional_content_groups_become_layers_and_off_art_a_hidden_layer() {
    let d = import(&file("/OFF [{B}]")).unwrap();
    // In paint order (bottom first): the groups, then the art outside them.
    assert_eq!(names(&d), ["Shapes", "Hidden", "Page 1"]);
    let [shapes, hidden, rest] = [0, 1, 2].map(|i| d.layers[i].clone());
    assert!(shapes.visible && printable(&shapes) && !shapes.locked);
    assert_eq!(colors(&shapes), [Color::rgb(1.0, 0.0, 0.0)]);
    // The group that is off is a hidden layer that still has its art; it doesn't print.
    assert!(!hidden.visible && !printable(&hidden));
    assert_eq!(colors(&hidden), [Color::rgb(0.0, 0.0, 1.0)]);
    assert_eq!(colors(&rest), [Color::rgb(0.0, 1.0, 0.0)]);
}

#[test]
fn base_state_on_lists_and_locks_are_read() {
    let d = import(&file("/BaseState /OFF /ON [{A}] /Locked [{A}]")).unwrap();
    assert_eq!(names(&d), ["Shapes", "Hidden", "Page 1"]);
    assert!(d.layers[0].visible && d.layers[0].locked);
    assert!(!d.layers[1].visible && !d.layers[1].locked);
}

#[test]
fn without_layers_each_page_is_one_layer_of_what_shows() {
    let r = import_with_report(&file("/OFF [{B}]"), &ImportOptions { layers: false, ..Default::default() }).unwrap();
    assert_eq!(names(&r.document), ["Page 1"]);
    assert_eq!(colors(&r.document.layers[0]), [Color::rgb(1.0, 0.0, 0.0), Color::rgb(0.0, 1.0, 0.0)]);
}

#[test]
fn pages_share_the_layers_of_their_groups() {
    let (objs, catalog, ids) = groups(2, "/OFF [{B}]");
    let bytes = pdf_with_catalog(&[page(ids), page(ids)], &[&objs[0], &objs[1]], &catalog, None);
    let d = import(&bytes).unwrap();
    assert_eq!(names(&d), ["Shapes", "Hidden", "Page 1", "Page 2"]);
    assert_eq!(colors(&d.layers[0]).len(), 2, "both pages' art");
    assert_eq!(colors(&d.layers[1]).len(), 2);
    // The second page's art sits on its artboard.
    let x = d.layers[0].children().unwrap()[1].geometric_bounds().unwrap().x0;
    assert!(x > d.artboards[1].rect.x0, "{x}");
}

#[test]
fn groups_marked_inside_forms_and_clips_reach_their_layers() {
    let (objs, catalog, ids) = groups(1, "/OFF [{B}]");
    let form_no = first_extra(1) + 2;
    let form = stream(
        &format!("/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /Properties << /MC1 {} 0 R >> >>", ids[1]),
        "/OC /MC1 BDC 0 0 1 rg 50 50 30 30 re f EMC",
    );
    let page = PdfPage {
        resources: format!("/Properties << /MC0 {} 0 R >> /XObject << /X0 {form_no} 0 R >>", ids[0]),
        ..PdfPage::new(100.0, 100.0, "q 0 0 25 25 re W n /OC /MC0 BDC 1 0 0 rg 10 10 30 30 re f EMC Q /X0 Do")
    };
    let d = import(&pdf_with_catalog(&[page], &[&objs[0], &objs[1], &form], &catalog, None)).unwrap();
    assert_eq!(names(&d), ["Shapes", "Hidden"]);
    let shapes = &d.layers[0].children().unwrap()[0];
    assert!(matches!(shapes.kind, NodeKind::Group { clip: true, .. }), "the clip group goes with its art");
    assert!(!d.layers[1].visible);
    assert_eq!(colors(&d.layers[1]), [Color::rgb(0.0, 0.0, 1.0)]);
}

#[test]
fn a_form_hidden_by_its_own_group_stays_hidden() {
    let (objs, catalog, ids) = groups(1, "/OFF [{B}]");
    let form_no = first_extra(1) + 2;
    let form = stream(&format!("/Type /XObject /Subtype /Form /BBox [0 0 100 100] /OC {} 0 R", ids[1]), "0 0 1 rg 50 50 30 30 re f");
    let page = PdfPage { resources: format!("/XObject << /X0 {form_no} 0 R >>"), ..PdfPage::new(100.0, 100.0, "1 0 0 rg 10 10 30 30 re f /X0 Do") };
    let r = import_with_report(&pdf_with_catalog(&[page], &[&objs[0], &objs[1], &form], &catalog, None), &ImportOptions::default()).unwrap();
    assert_eq!(names(&r.document), ["Page 1"]);
    assert_eq!(colors(&r.document.layers[0]), [Color::rgb(1.0, 0.0, 0.0)], "the hidden form isn't shown");
    assert!(r.warnings.iter().any(|w| w.contains("hidden layers")), "{:?}", r.warnings);
}

#[test]
fn an_encrypted_file_keeps_its_hidden_layers() {
    let (objs, catalog, ids) = groups(1, "/OFF [{B}]");
    let bytes = pdf_with_catalog(&[page(ids)], &[&objs[0], &objs[1]], &catalog, Some("pw"));
    let r = import_with_report(&bytes, &ImportOptions { password: Some("pw".into()), ..Default::default() }).unwrap();
    let d = r.document;
    assert_eq!(d.layers.len(), 3, "{:?}", r.warnings);
    assert!(d.layers[0].visible && !d.layers[1].visible);
    assert_eq!(colors(&d.layers[1]), [Color::rgb(0.0, 0.0, 1.0)]);
}
