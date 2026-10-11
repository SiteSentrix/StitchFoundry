//! The embroidery interface: Check Design and its findings.

use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app
}

#[test]
fn check_design_lists_findings_in_the_panel() {
    let mut app = app();
    assert_eq!(app.run("ui.embroideryCheck", json!({})).unwrap()["count"], 0);
    assert_eq!(app.embroidery.checks.as_ref().map(Vec::len), Some(0));
    for _ in 0..3 {
        app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 60, "height": 60})).unwrap();
        app.run("embroidery.set", json!({"type": "fill"})).unwrap();
    }
    let n = app.run("ui.embroideryCheck", json!({})).unwrap()["count"].as_u64().unwrap();
    assert!(n >= 1);
    let findings = app.embroidery.checks.clone().unwrap();
    assert!(findings.iter().any(|f| f["kind"] == "dense"));
    for f in &findings {
        assert!(!super::embroidery::panel::finding_text_for_test(f).is_empty(), "{f}");
    }
}

/// Every piece of text one headless frame of `show` paints.
fn painted_text(app: &mut VectorcraftApp, show: fn(&mut VectorcraftApp, &mut egui::Ui)) -> Vec<String> {
    let ctx = egui::Context::default();
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, 600.0))), ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| show(app, ui));
    out.textures_delta.clear();
    fn walk(s: &egui::Shape, into: &mut Vec<String>) {
        match s {
            egui::Shape::Text(t) => into.push(t.galley.text().to_string()),
            egui::Shape::Vec(v) => v.iter().for_each(|s| walk(s, into)),
            _ => {}
        }
    }
    let mut texts = Vec::new();
    out.shapes.iter().for_each(|c| walk(&c.shape, &mut texts));
    texts
}

#[test]
fn stitch_list_shows_every_record() {
    let mut app = app();
    assert!(painted_text(&mut app, crate::embroidery::stitch_list::show).iter().any(|t| t.contains("Nothing stitched yet")));
    app.run("shape.rectangle", json!({"x": 50, "y": 50, "width": 60, "height": 40})).unwrap();
    app.run("embroidery.set", json!({"type": "fill"})).unwrap();
    let texts = painted_text(&mut app, crate::embroidery::stitch_list::show);
    for want in ["Function", "Stitch", "0", "1"] {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
    // the first record is the jump to the start, then stitches with their lengths
    let plan = app.session.embroidery_plan().unwrap();
    let first = plan.pattern().stitches[0];
    assert!(texts.iter().any(|t| *t == format!("{:.2}", first.x)), "{texts:?}");
}

#[test]
fn styles_panel_lists_and_applies_styles() {
    let mut app = app();
    assert!(painted_text(&mut app, crate::embroidery::styles::show).iter().any(|t| t.starts_with("No styles yet")));
    app.run("shape.rectangle", json!({"x": 20, "y": 20, "width": 80, "height": 40})).unwrap();
    app.run("embroidery.set", json!({"type": "fill"})).unwrap();
    app.run("embroidery.style.new", json!({"name": "Dense"})).unwrap();
    let texts = painted_text(&mut app, crate::embroidery::styles::show);
    assert!(texts.iter().any(|t| t == "Dense") && texts.iter().any(|t| t == "Tatami Fill · 1"), "{texts:?}");
    // an object changed since it took its style is marked with a +, here and in the Embroidery panel
    app.run("embroidery.setParams", json!({"params": {"spacing": 0.6}})).unwrap();
    assert!(painted_text(&mut app, crate::embroidery::styles::show).iter().any(|t| t == "Dense+"));
    assert!(painted_text(&mut app, crate::embroidery::panel::show).iter().any(|t| t == "Dense+"));
    // with nothing selected, the Embroidery panel shows the properties new objects take
    app.run("select.none", json!({})).unwrap();
    let texts = painted_text(&mut app, crate::embroidery::panel::show);
    assert!(texts.iter().any(|t| t == "Properties for New Objects") && texts.iter().any(|t| t == "Density"), "{texts:?}");
}

#[test]
fn the_player_travels_by_object_colour_trim_and_stitch() {
    use crate::embroidery::{TravelBy, object_at, travel};
    use vectorcraft_embroidery::sf_core::Cmd;
    let mut app = app();
    let mut ids = Vec::new();
    for (x, color) in [(20.0, "#cc2233"), (150.0, "#2233cc"), (280.0, "#cc2233")] {
        let r = app.run("shape.rectangle", json!({"x": x, "y": 40, "width": 80, "height": 50})).unwrap();
        ids.push(vectorcraft_doc::NodeId(r["id"].as_u64().unwrap()));
        app.run("embroidery.set", json!({"type": "fill"})).unwrap();
        app.run("embroidery.setThread", json!({"color": color})).unwrap();
    }
    let plan = app.session.embroidery_plan().unwrap();
    let st = &plan.pattern().stitches;
    let n = st.len();
    // by object: the end of each object in turn, and back
    let first = travel(&plan, 0, TravelBy::Object, true);
    assert_eq!(first, plan.records_of(ids[0]).last().unwrap().end);
    assert_eq!(object_at(&plan, first), Some(ids[0]));
    let second = travel(&plan, first, TravelBy::Object, true);
    assert_eq!(object_at(&plan, second), Some(ids[1]));
    assert_eq!(travel(&plan, second, TravelBy::Object, false), first);
    assert_eq!(travel(&plan, n, TravelBy::Object, true), n);
    // by colour and by trim: just after each colour change or trim
    let c = travel(&plan, 0, TravelBy::Color, true);
    assert_eq!(st[c - 1].cmd, Cmd::ColorChange);
    let t = travel(&plan, 0, TravelBy::Trim, true);
    assert_eq!(st[t - 1].cmd, Cmd::Trim);
    let f = travel(&plan, 0, TravelBy::Function, true);
    assert!(st[f - 1].cmd != Cmd::Stitch && f <= t.min(c));
    // by stitch: one record at a time
    assert_eq!((travel(&plan, 10, TravelBy::Stitch, true), travel(&plan, 0, TravelBy::Stitch, false)), (11, 0));
}

#[test]
fn the_sequence_list_keeps_hidden_objects_to_show_them_again() {
    let mut app = app();
    for x in [20.0, 150.0] {
        app.run("shape.rectangle", json!({"x": x, "y": 40, "width": 80, "height": 50})).unwrap();
        app.run("embroidery.set", json!({"type": "fill"})).unwrap();
    }
    let id = app.session.active().unwrap().selection.objects[0];
    app.run("layer.setProps", json!({"ids": [id.0], "visible": false})).unwrap();
    let texts = painted_text(&mut app, crate::embroidery::sequence::show);
    for want in ["Hidden (not sewn)", "Travel by", "Object"] {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
}

#[test]
fn check_design_shows_its_findings_without_closing_the_panel() {
    let mut app = app();
    app.run("window.panel", json!({"panel": "embroidery"})).unwrap();
    assert_eq!(app.ui.open_panel.as_deref(), Some("embroidery"));
    // the panel's own Check Design button: the panel stays open
    app.run("ui.embroideryCheck", json!({})).unwrap();
    assert_eq!(app.ui.open_panel.as_deref(), Some("embroidery"));
    // from the menu with the panel closed: it opens to show the findings
    app.run("window.panel", json!({"panel": "embroidery"})).unwrap();
    assert_eq!(app.ui.open_panel, None);
    app.run("ui.embroideryCheck", json!({})).unwrap();
    assert_eq!(app.ui.open_panel.as_deref(), Some("embroidery"));
}
