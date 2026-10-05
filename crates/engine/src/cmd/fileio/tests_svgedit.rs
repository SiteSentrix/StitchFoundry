//! SVG files that reopen as they were: Save keeps hidden layers and linked images stay linked.

use serde_json::{Value, json};
use vectorcraft_doc::NodeKind;

use super::tests_svg::{image_session, svg};
use super::*;

fn new_id(v: &Value) -> u64 {
    v["id"].as_u64().or_else(|| v["ids"][0].as_u64()).unwrap_or_else(|| panic!("no id in {v}"))
}

fn rect(s: &mut Session, x: f64, y: f64) -> u64 {
    new_id(&s.execute("shape.rectangle", &json!({"x": x, "y": y, "width": 20, "height": 20})).unwrap())
}

fn open(s: &mut Session, name: &str, text: &str) -> Value {
    s.execute("document.open", &json!({"name": name, "dataBase64": vectorcraft_format::base64_encode(text.as_bytes())})).unwrap()
}

#[test]
fn save_keeps_hidden_layers_and_export_leaves_them_out() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    rect(&mut s, 10.0, 10.0);
    let notes = new_id(&s.execute("layer.new", &json!({"name": "Notes"})).unwrap());
    rect(&mut s, 50.0, 50.0);
    s.execute("layer.setProps", &json!({"id": notes, "visible": false})).unwrap();

    let exported = svg(&mut s, json!({}));
    assert!(!exported.contains("Notes"), "{exported}");
    let saved = s.execute("document.save", &json!({"format": "svg"})).unwrap();
    let saved = String::from_utf8(vectorcraft_format::base64_decode(saved["dataBase64"].as_str().unwrap()).unwrap()).unwrap();
    assert!(saved.contains("id=\"Notes\"") && saved.contains("display=\"none\""), "{saved}");
    // Asked not to, Save leaves them out too.
    let r = s.execute("document.save", &json!({"format": "svg", "svg": {"hiddenLayers": false}})).unwrap();
    assert!(!String::from_utf8(vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap()).unwrap().contains("Notes"));

    open(&mut s, "saved.svg", &saved);
    let d = &s.doc().unwrap().doc;
    let layers: Vec<(Option<&str>, bool, usize)> =
        d.layers.iter().map(|l| (l.name.as_deref(), l.visible, l.children().map_or(0, Vec::len))).collect();
    assert_eq!(layers, [(Some("Layer 1"), true, 1), (Some("Notes"), false, 1)]);
}

#[test]
fn link_mode_writes_no_data_uri() {
    let mut s = image_session();
    // An image placed linked: it has both its bytes and its file.
    s.edit("Link", |d, _| {
        let id = d.layers[0].children().and_then(|c| c.first()).map(|n| n.id).unwrap();
        if let Some(NodeKind::Image(im)) = d.node_mut(id).map(|n| &mut n.kind) {
            im.link = Some("art/dot.png".into());
        }
        Ok(())
    })
    .unwrap();
    let linked = svg(&mut s, json!({"images": "link"}));
    assert!(linked.contains("xlink:href=\"art/dot.png\"") && !linked.contains("data:"), "{linked}");
    let embedded = svg(&mut s, json!({"images": "embed"}));
    assert!(embedded.contains("href=\"data:image/png;base64,") && !embedded.contains("art/dot.png"), "{embedded}");
}
