//! Save PDF › Create PDF layers: an optional content group per top-level layer, with its
//! visibility, print state and lock.

use hayro_syntax::object::{Array, Dict, Name, ObjectIdentifier};
use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Document, LayerColor, Node, NodeKind};
use vectorcraft_geom::{Rect, shapes};

use crate::*;

const RED: Color = Color::Rgb { r: 1.0, g: 0.0, b: 0.0 };
const GREEN: Color = Color::Rgb { r: 0.0, g: 1.0, b: 0.0 };
const BLUE: Color = Color::Rgb { r: 0.0, g: 0.0, b: 1.0 };

fn rect(d: &mut Document, x: f64, c: Color) -> Node {
    Node::path(d.alloc_id(), shapes::rectangle(Rect::new(x, 10.0, x + 30.0, 40.0)), Appearance::basic(Paint::solid(c), Paint::None, 0.0))
}

/// Layers, bottom first: "Back" (red), "Hidden" (green; hidden and locked), "Notes" (blue; not
/// printed) and a template layer (grey).
fn layered() -> Document {
    let mut d = Document::new(200.0, 100.0);
    d.layers.clear();
    for (i, (name, c)) in [("Back", RED), ("Hidden", GREEN), ("Notes", BLUE), ("Template", Color::gray(0.5))].into_iter().enumerate() {
        let art = rect(&mut d, 10.0 + 40.0 * i as f64, c);
        let mut l = Node::layer(d.alloc_id(), name, LayerColor::Preset(i as u8));
        (l.visible, l.locked) = (name != "Hidden", name == "Hidden");
        if let NodeKind::Layer { children, printable, template, .. } = &mut l.kind {
            children.push(art.into());
            *printable = name != "Notes";
            *template = name == "Template";
        }
        d.layers.push(l.into());
    }
    d
}

fn export(d: &Document, v: Value) -> ExportReport {
    let settings: PdfSettings = serde_json::from_value(v).unwrap();
    export_with_report(d, &PdfOptions { settings, ..Default::default() }).unwrap()
}

/// The groups of `refs` (an array of the catalog's optional content properties) by name.
fn names(xref: &hayro_syntax::xref::XRef, refs: Option<Array<'_>>) -> Vec<String> {
    let ids = refs.map(|a| a.raw_iter().filter_map(|i| i.as_obj_ref()).map(ObjectIdentifier::from).collect::<Vec<_>>()).unwrap_or_default();
    ids.into_iter()
        .map(|id| {
            let g = xref.get::<Dict<'_>>(id).unwrap();
            String::from_utf8(g.get::<hayro_syntax::object::String<'_>>(b"Name").unwrap().as_bytes().to_vec()).unwrap()
        })
        .collect()
}

/// The catalog's `/OCProperties` of `bytes` (opened with `password`), if any.
fn oc_properties(bytes: &[u8], password: Option<&str>, f: impl FnOnce(Option<Dict<'_>>, &hayro_syntax::xref::XRef)) {
    let file = crate::pages::open(bytes, password).unwrap();
    let xref = file.xref();
    let catalog = xref.get::<Dict<'_>>(xref.root_id()).unwrap();
    f(catalog.get::<Dict<'_>>(b"OCProperties"), xref);
}

/// The layers of an import: name, visible, printable, locked and fill colours of their art.
fn layers(d: &Document) -> Vec<(String, bool, bool, bool, Vec<Color>)> {
    d.layers
        .iter()
        .map(|l| {
            let printable = matches!(l.kind, NodeKind::Layer { printable: true, .. });
            let colors = l.children().unwrap().iter().filter_map(|n| n.appearance.fill().and_then(|f| f.paint.color())).collect();
            (l.name.clone().unwrap_or_default(), l.visible, printable, l.locked, colors)
        })
        .collect()
}

#[test]
fn create_layers_writes_a_group_per_layer_with_its_states() {
    let d = layered();
    let r = export(&d, json!({"createLayers": true, "compression": {"compressText": false}}));
    assert!(r.warnings.is_empty(), "{:?}", r.warnings);
    oc_properties(&r.bytes, None, |props, xref| {
        let props = props.expect("/OCProperties");
        // Template layers aren't exported; the groups are in paint order, listed top first.
        assert_eq!(names(xref, props.get(b"OCGs")), ["Back", "Hidden", "Notes"]);
        let config = props.get::<Dict<'_>>(b"D").unwrap();
        assert_eq!(names(xref, config.get(b"Order")), ["Notes", "Hidden", "Back"]);
        assert_eq!(names(xref, config.get(b"OFF")), ["Hidden"], "hidden layers are off");
        assert_eq!(names(xref, config.get(b"Locked")), ["Hidden"]);
        assert!(config.get::<hayro_syntax::object::String<'_>>(b"Name").is_some());
        let auto = config.get::<Array<'_>>(b"AS").unwrap().iter::<Dict<'_>>().next().unwrap();
        assert_eq!(auto.get::<Name<'_>>(b"Event").unwrap().as_ref(), b"Print");
        let print_state = |name: &str| {
            let ids =
                props.get::<Array<'_>>(b"OCGs").unwrap().raw_iter().filter_map(|i| i.as_obj_ref()).map(ObjectIdentifier::from).collect::<Vec<_>>();
            let g = ids
                .into_iter()
                .map(|id| xref.get::<Dict<'_>>(id).unwrap())
                .find(|g| g.get::<hayro_syntax::object::String<'_>>(b"Name").unwrap().as_bytes() == name.as_bytes());
            let usage = g.unwrap().get::<Dict<'_>>(b"Usage").unwrap();
            usage.get::<Dict<'_>>(b"Print").unwrap().get::<Name<'_>>(b"PrintState").unwrap().as_ref().to_vec()
        };
        assert_eq!(print_state("Notes"), b"OFF", "non-printing layers aren't printed");
        assert_eq!(print_state("Back"), b"ON");
    });
    // Each layer's art is its group's optional content.
    let text = String::from_utf8_lossy(&r.bytes);
    assert_eq!(text.matches(" BDC ").count(), 3, "{text}");
    assert!(text.contains("/OC /VCoc0 BDC /x") && text.contains("/Properties<<"));
}

#[test]
fn pdf_layers_import_back_as_layers() {
    let d = layered();
    for password in [None, Some("secret")] {
        let mut set = json!({"createLayers": true});
        if let Some(p) = password {
            set["security"] = json!({"openPassword": p});
        }
        let bytes = export(&d, set).bytes;
        let opts = ImportOptions { password: password.map(str::to_string), ..Default::default() };
        let back = import_with_report(&bytes, &opts).unwrap().document;
        assert_eq!(
            layers(&back),
            [
                ("Back".to_string(), true, true, false, vec![RED]),
                ("Hidden".to_string(), false, true, true, vec![GREEN]),
                ("Notes".to_string(), true, false, false, vec![BLUE]),
            ],
            "password {password:?}"
        );
    }
}

#[test]
fn identical_layers_stay_apart() {
    let mut d = Document::new(200.0, 100.0);
    d.layers.clear();
    for name in ["One", "Two"] {
        let art = rect(&mut d, 10.0, RED);
        let mut l = Node::layer(d.alloc_id(), name, LayerColor::Preset(0));
        if let NodeKind::Layer { children, .. } = &mut l.kind {
            children.push(art.into());
        }
        d.layers.push(l.into());
    }
    let back = import(&export(&d, json!({"createLayers": true})).bytes).unwrap();
    let got: Vec<(String, usize)> = back.layers.iter().map(|l| (l.name.clone().unwrap_or_default(), l.children().unwrap().len())).collect();
    assert_eq!(got, [("One".to_string(), 1), ("Two".to_string(), 1)]);
}

#[test]
fn without_pdf_layers_the_layers_are_plain_page_content() {
    let mut d = layered();
    for (v, warning) in [
        (json!({}), None),
        (json!({"createLayers": true, "compatibility": "1.4"}), Some("1.5")),
        (json!({"createLayers": true, "compatibility": "1.5"}), None),
    ] {
        let r = export(&d, v.clone());
        let mut has = false;
        oc_properties(&r.bytes, None, |props, _| has = props.is_some());
        assert_eq!(has, v["createLayers"] == json!(true) && warning.is_none(), "{v}");
        match warning {
            Some(w) => assert!(r.warnings.iter().any(|x| x.contains(w)), "{v}: {:?}", r.warnings),
            None => assert!(r.warnings.is_empty(), "{v}: {:?}", r.warnings),
        }
    }
    // A knockout page group draws objects, not layers.
    d.page_knockout = true;
    let r = export(&d, json!({"createLayers": true}));
    oc_properties(&r.bytes, None, |props, _| assert!(props.is_none()));
    assert!(r.warnings.iter().any(|w| w.contains("knockout page group")), "{:?}", r.warnings);
}

#[test]
fn pdf_a_layers_have_no_automatic_states() {
    let r = export(&layered(), json!({"standard": "pdfA2b", "createLayers": true}));
    oc_properties(&r.bytes, None, |props, _| {
        let config = props.unwrap().get::<Dict<'_>>(b"D").unwrap();
        assert!(config.get::<Array<'_>>(b"AS").is_none() && config.get::<hayro_syntax::object::String<'_>>(b"Name").is_some());
    });
}

#[test]
fn pdf_x4_keeps_pdf_layers_and_x1a_refuses_them() {
    let d = layered();
    let r = export(&d, json!({"standard": "pdfX4", "compatibility": "1.6", "createLayers": true}));
    assert!(r.bytes.starts_with(b"%PDF-1.6"));
    oc_properties(&r.bytes, None, |props, xref| {
        let props = props.expect("/OCProperties");
        assert_eq!(names(xref, props.get(b"OCGs")), ["Back", "Hidden", "Notes"]);
        assert!(props.get::<Dict<'_>>(b"D").unwrap().get::<Array<'_>>(b"AS").is_none());
    });
    let set: PdfSettings = serde_json::from_value(json!({"standard": "pdfX1a", "compatibility": "1.4", "createLayers": true})).unwrap();
    assert!(matches!(export_with_report(&d, &PdfOptions { settings: set, ..Default::default() }), Err(PdfError::BadSetting(_))));
}

/// The painted paths of `d`: their bounds' corner and colour, in order.
fn leaves(d: &Document) -> Vec<(i32, i32, Option<[u8; 4]>)> {
    let mut out = vec![];
    d.walk(|n| {
        if let Some(b) = n.path_data().and_then(|p| p.bounds()) {
            let paint = n.appearance.fill().map(|f| f.paint.clone()).or_else(|| n.appearance.stroke().map(|s| s.paint.clone()));
            out.push((b.x0.round() as i32, b.y0.round() as i32, paint.and_then(|p| p.color()).map(|c| c.to_rgba8(1.0))));
        }
    });
    out.sort_by_key(|l| (l.0, l.1));
    out
}

#[test]
fn layered_art_imports_as_it_was_drawn() {
    let mut d = layered();
    d.layers = d.layers.iter().map(|l| Node { visible: true, ..(**l).clone() }.into()).collect();
    let plain = leaves(&import(&export(&d, json!({"includeNonPrinting": true})).bytes).unwrap());
    assert_eq!(plain.len(), 3);
    assert_eq!(leaves(&import(&export(&d, json!({"createLayers": true})).bytes).unwrap()), plain, "the marks draw nothing");
}
