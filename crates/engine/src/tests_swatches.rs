//! Swatches: colour groups as first-class swatch homes (colour mode, spot plates, PDF, Document
//! Info), naming and kinds, Swatch Options edits reaching linked art, deleting and new groups.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};

use super::*;

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 100, "height": 100})).unwrap();
    s
}

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

fn rect(s: &mut Session) -> NodeId {
    NodeId(run(s, "shape.rectangle", json!({"x": 0, "y": 0, "width": 10, "height": 10}))["id"].as_u64().unwrap())
}

fn doc(s: &Session) -> &vectorcraft_doc::Document {
    &s.doc().unwrap().doc
}

/// Put a solid swatch into colour group `group` directly in the model (no command creates one in a
/// group before M3.13).
fn grouped_swatch(s: &mut Session, group: &str, name: &str, color: Color) {
    let d = std::sync::Arc::make_mut(&mut s.doc_mut().unwrap().doc);
    let g = d.swatch_groups.iter_mut().find(|g| g.name == group).unwrap();
    g.swatches.push(vectorcraft_color::Swatch { name: name.into(), paint: Paint::solid(color), global: true, spot: false });
}

#[test]
fn document_color_mode_converts_grouped_swatches() {
    let mut s = session();
    grouped_swatch(&mut s, "Grays", "Ink", Color::rgb(0.2, 0.4, 0.6));
    run(&mut s, "object.convertDocumentColorMode", json!({"mode": "cmyk"}));
    for g in &doc(&s).swatch_groups {
        for sw in &g.swatches {
            assert!(!matches!(sw.paint.color(), Some(Color::Rgb { .. })), "{} in {} is still RGB", sw.name, g.name);
        }
    }
    assert!(matches!(doc(&s).swatch("Ink").unwrap().paint.color(), Some(Color::Cmyk { .. })));
    // The File menu's mode switch converts them as well.
    let mut s = session();
    run(&mut s, "file.documentColorMode", json!({"mode": "cmyk"}));
    assert!(matches!(doc(&s).swatch("Bright Red").unwrap().paint.color(), Some(Color::Cmyk { .. })));
}

#[test]
fn grouped_spot_swatches_print_on_their_own_plate() {
    let mut s = session();
    grouped_swatch(&mut s, "Brights", "Signal Orange", Color::cmyk(0.0, 0.6, 1.0, 0.0));
    assert_eq!(run(&mut s, "swatch.setSpot", json!({"name": "Signal Orange"})), json!({"name": "Signal Orange", "spot": true}));
    assert!(doc(&s).swatch("Signal Orange").is_some_and(|w| w.spot && w.global));
    let plates = run(&mut s, "color.plates", json!({}));
    assert!(plates["plates"].as_array().unwrap().iter().any(|p| p["name"] == "Signal Orange" && p["spot"] == true), "{plates}");
    let a = rect(&mut s);
    run(&mut s, "paint.setFill", json!({"ids": [a.0], "swatch": "Signal Orange"}));
    let bytes = vectorcraft_pdf::export(doc(&s), &vectorcraft_pdf::PdfOptions { compress: false, created: Some(0), ..Default::default() }).unwrap();
    let pdf = String::from_utf8_lossy(&bytes);
    assert!(pdf.contains("/Separation") && pdf.contains("Signal#20Orange"), "a grouped spot swatch is a Separation");
    let info = run(&mut s, "document.info", json!({}));
    assert_eq!(info["spotColors"], json!(["Signal Orange"]));
}

#[test]
fn document_info_counts_grouped_swatches() {
    let mut s = session();
    let d = doc(&s);
    let all = d.swatches.len() + d.swatch_groups.iter().map(|g| g.swatches.len()).sum::<usize>();
    assert!(all > d.swatches.len());
    assert_eq!(run(&mut s, "document.info", json!({}))["swatches"], json!(all));
}

#[test]
fn new_swatches_are_named_by_their_colour_model() {
    let mut s = session();
    let name = |s: &mut Session, p: Value| run(s, "swatch.new", p)["name"].as_str().unwrap().to_string();
    assert_eq!(name(&mut s, json!({"color": {"c": 10, "m": 20, "y": 30, "k": 0}})), "C=10 M=20 Y=30 K=0");
    assert_eq!(name(&mut s, json!({"color": "#ff8000"})), "R=255 G=128 B=0");
    assert_eq!(name(&mut s, json!({"color": {"gray": 40}})), "Gray K=40");
    // The same colour again (or an explicit name in use) gets the next free number, document-wide.
    assert_eq!(name(&mut s, json!({"color": {"c": 10, "m": 20, "y": 30, "k": 0}})), "C=10 M=20 Y=30 K=0 2");
    assert_eq!(name(&mut s, json!({"name": "Grays", "color": "#000000"})), "Grays 2");
    assert_eq!(name(&mut s, json!({"name": "Bright Red", "color": "#000000"})), "Bright Red 2");
    // A spot swatch is always global; saving a linked colour stores the colour, not the link.
    let spot = name(&mut s, json!({"name": "Ink", "color": "#336699", "spot": true}));
    assert!(doc(&s).swatch(&spot).is_some_and(|w| w.spot && w.global));
    let copy = name(&mut s, json!({"swatch": "Ink"}));
    assert_eq!(doc(&s).swatch(&copy).unwrap().paint, Paint::solid(Color::from_hex("#336699").unwrap()));
    assert!(s.execute("swatch.new", &json!({"none": true})).is_err(), "None is not a swatch");
}

#[test]
fn new_swatch_saves_the_active_pattern_or_gradient() {
    let mut s = session();
    let a = rect(&mut s);
    run(&mut s, "select.set", json!({"ids": [a.0]}));
    run(&mut s, "object.pattern.make", json!({}));
    run(&mut s, "object.pattern.done", json!({}));
    let pattern = doc(&s).patterns[0].name.clone();
    let name = run(&mut s, "swatch.new", json!({"pattern": pattern}))["name"].as_str().unwrap().to_string();
    assert_eq!(name, "New Pattern Swatch 1");
    assert!(matches!(&doc(&s).swatch(&name).unwrap().paint, Paint::Pattern { pattern: p, .. } if *p == pattern));
    assert!(s.execute("swatch.new", &json!({"pattern": "Nope"})).is_err());
    let g = json!({"gradient": {"kind": "radial", "stops": [{"offset": 0, "color": "#ffffff"}, {"offset": 1, "color": "#000000"}]}});
    let name = run(&mut s, "swatch.new", g)["name"].as_str().unwrap().to_string();
    assert_eq!(name, "New Gradient Swatch 1");
    assert!(matches!(doc(&s).swatch(&name).unwrap().paint, Paint::Gradient(_)));
    assert!(s.execute("swatch.new", &json!({"pattern": pattern, "spot": true})).is_err(), "only colours are spot");
}

#[test]
fn colour_groups_hold_solid_colours_only() {
    let mut s = session();
    let g = run(&mut s, "swatch.newGroup", json!({"name": "Mix", "swatches": ["Sunset", "Red", "[None]", "Bright Blue"], "colors": ["#00ff00"]}));
    let d = doc(&s);
    let group = d.swatch_groups.iter().find(|x| x.name == g["name"]).unwrap();
    let names: Vec<&str> = group.swatches.iter().map(|w| w.name.as_str()).collect();
    assert_eq!(names, ["Red", "Bright Blue", "R=0 G=255 B=0"]);
    assert!(d.swatches.iter().any(|w| w.name == "Sunset") && d.swatches.iter().any(|w| w.name == "[None]"), "gradients and None stay put");
    assert!(d.swatch_groups.iter().find(|x| x.name == "Brights").unwrap().swatches.iter().all(|w| w.name != "Bright Blue"), "moved, not copied");
}
