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
    painted_text_in(app, show, 600.0)
}

/// [`painted_text`] on a screen `height` points high.
fn painted_text_in(app: &mut VectorcraftApp, show: fn(&mut VectorcraftApp, &mut egui::Ui), height: f32) -> Vec<String> {
    let ctx = egui::Context::default();
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(420.0, height))), ..Default::default() };
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
fn the_panel_shows_a_fills_texture_and_split_lines() {
    let mut app = app();
    let id = app.run("shape.rectangle", json!({"x": 20, "y": 20, "width": 200, "height": 150})).unwrap()["id"].as_u64().unwrap();
    app.run("embroidery.set", json!({"type": "fill"})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    assert!(texts.iter().any(|t| t == "Texture") && !texts.iter().any(|t| t == "Texture size"), "{texts:?}");
    app.run("embroidery.setParams", json!({"params": {"texture": {"pattern": "scales"}}})).unwrap();
    app.run("embroidery.setSplits", json!({"id": id, "splits": [[[50, 0], [60, 200]]]})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    for want in ["Scales", "Texture size", "Texture angle", "Split lines: 1", "Clear Split Lines"] {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
    // Satin fills have neither.
    app.run("embroidery.set", json!({"type": "satinfill"})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    assert!(!texts.iter().any(|t| t == "Texture" || t == "Split lines: 1"), "{texts:?}");
}

#[test]
fn the_panel_offers_the_designs_own_motifs() {
    let mut app = app();
    let peak = app.run("path.create", json!({"d": "M 20 200 L 50 150 L 80 200"})).unwrap()["id"].as_u64().unwrap();
    app.run("select.set", json!({"ids": [peak]})).unwrap();
    app.run("embroidery.motif.make", json!({"name": "Peak"})).unwrap();
    let line = app.run("path.create", json!({"d": "M 20 100 L 300 100"})).unwrap()["id"].as_u64().unwrap();
    app.run("select.set", json!({"ids": [line]})).unwrap();
    app.run("embroidery.set", json!({"type": "motif"})).unwrap();
    // A built-in motif shows, and no Delete Motif.
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    assert!(texts.iter().any(|t| t == "Wave") && !texts.iter().any(|t| t == "Delete Motif"), "{texts:?}");
    // Sewing the design's motif: its name shows, with Delete Motif.
    app.run("embroidery.motif.apply", json!({"name": "Peak"})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    assert!(texts.iter().any(|t| t == "Peak") && texts.iter().any(|t| t == "Delete Motif"), "{texts:?}");
    // Deleted from the design, the run still shows the motif it sews, without the button.
    app.run("embroidery.motif.delete", json!({"name": "Peak"})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    assert!(texts.iter().any(|t| t == "Peak") && !texts.iter().any(|t| t == "Delete Motif"), "{texts:?}");
}

#[test]
fn the_panel_shows_sculpture_runs_and_cross_styles() {
    let mut app = app();
    let line = app.run("path.create", json!({"d": "M 20 100 L 300 100"})).unwrap()["id"].as_u64().unwrap();
    app.run("select.set", json!({"ids": [line]})).unwrap();
    app.run("embroidery.set", json!({"type": "run"})).unwrap();
    app.run("embroidery.setParams", json!({"params": {"kind": "Sculpture"}})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    for want in ["Sculpture run", "Triple stitch length", "Single stitch length", "Triple stitches", "Single stitches"] {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
    // a string stitch: its long stitches' chord length and spacing
    app.run("embroidery.setParams", json!({"params": {"kind": "String"}})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    for want in ["String stitch", "Chord length", "Spacing"] {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
    // a satin line sewn as motifs
    app.run("embroidery.set", json!({"type": "satin"})).unwrap();
    app.run("embroidery.setParams", json!({"params": {"motifColumn": {"motif": "Leaf"}}})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    for want in ["Sew as", "Motifs", "Leaf", "Motif length (× width)"] {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
    app.run("shape.rectangle", json!({"x": 20, "y": 150, "width": 100, "height": 100})).unwrap();
    // a 3D motif fill receding: its strength and direction
    app.run("embroidery.set", json!({"type": "motiffill"})).unwrap();
    app.run("embroidery.setParams", json!({"params": {"warp": {"kind": "Perspective"}}})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    for want in ["3D effect", "Perspective", "Strength", "Direction"] {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
    app.run("embroidery.set", json!({"type": "cross"})).unwrap();
    app.run("embroidery.setParams", json!({"params": {"cross": "Upright"}})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    assert!(texts.iter().any(|t| t == "Cross style") && texts.iter().any(|t| t == "Upright cross"), "{texts:?}");
}

#[test]
fn the_panel_edits_a_redwork_group_as_one() {
    let mut app = app();
    let a = app.run("path.create", json!({"d": "M 20 20 L 120 20 L 120 120 Z"})).unwrap()["id"].as_u64().unwrap();
    let b = app.run("path.create", json!({"d": "M 70 20 L 70 120"})).unwrap()["id"].as_u64().unwrap();
    app.run("select.set", json!({"ids": [a, b]})).unwrap();
    app.run("embroidery.redwork", json!({})).unwrap();
    let texts = painted_text_in(&mut app, crate::embroidery::panel::show, 2400.0);
    for want in ["Redwork", "Run style", "Stitch length", "Sewn as one line that ends where it starts; the way between lines is retraced along them."]
    {
        assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
    }
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

mod design_from_prompt {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use serde_json::{Value, json};
    use vectorcraft_engine::Session;

    use crate::embroidery::prompt;
    use crate::{HttpRequest, SecretStore, Services, VectorcraftApp};

    const ART: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 1000"><circle cx="500" cy="500" r="400" fill="#e07020"/><circle cx="380" cy="420" r="70" fill="#ffffff"/><circle cx="620" cy="420" r="70" fill="#ffffff"/></svg>"##;

    #[derive(Clone, Default)]
    struct Keys(Arc<Mutex<HashMap<String, String>>>);

    impl SecretStore for Keys {
        fn load(&self, name: &str) -> Option<String> {
            self.0.lock().unwrap().get(name).cloned()
        }
        fn save(&self, name: &str, value: Option<&str>) -> Result<(), String> {
            let mut m = self.0.lock().unwrap();
            match value {
                Some(v) => m.insert(name.to_string(), v.to_string()),
                None => m.remove(name),
            };
            Ok(())
        }
    }

    type Sent = Arc<Mutex<Vec<HttpRequest>>>;

    /// An app whose AI service answers `(status, body)` after `delay`, recording what it was sent.
    fn app(status: u16, body: String, delay: Duration) -> (VectorcraftApp, Sent, Keys) {
        let sent: Sent = Arc::default();
        let keys = Keys::default();
        let log = sent.clone();
        let services = Services {
            http_post: Some(Arc::new(move |r: &HttpRequest| {
                log.lock().unwrap().push(r.clone());
                std::thread::sleep(delay);
                Ok((status, body.clone()))
            })),
            secrets: Some(Box::new(keys.clone())),
            ..Default::default()
        };
        let mut app = VectorcraftApp::new(Session::new(), services);
        app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
        (app, sent, keys)
    }

    fn reply(svg: &str) -> String {
        json!({"type": "message", "model": "claude-opus-5-5", "content": [{"type": "text", "text": svg}], "stop_reason": "end_turn"}).to_string()
    }

    /// Poll as frames do until the answer is in.
    fn wait(app: &mut VectorcraftApp) {
        let ctx = egui::Context::default();
        for _ in 0..1000 {
            prompt::poll(app, &ctx);
            if app.embroidery.prompt.drawing_for(0.0).is_none() {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("no answer");
    }

    /// Every piece of text the open dialog paints, once it has measured and placed itself.
    fn dialog_text(app: &mut VectorcraftApp) -> Vec<String> {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        crate::theme::apply(&ctx, Default::default());
        let raw =
            || egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() };
        for _ in 0..4 {
            ctx.run_ui(raw(), |ui| crate::dialogs::show(app, &ui.ctx().clone())).textures_delta.clear();
        }
        let mut out = ctx.run_ui(raw(), |ui| crate::dialogs::show(app, &ui.ctx().clone()));
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

    fn objects(app: &VectorcraftApp) -> usize {
        app.session.active().unwrap().doc.layers.iter().map(|l| l.children().map_or(0, Vec::len)).sum()
    }

    #[test]
    fn the_art_is_drawn_placed_and_digitized_with_the_users_key() {
        let (mut app, sent, keys) = app(200, reply(ART), Duration::ZERO);
        assert!(crate::menus::enabled(&app, "ui.designFromPrompt"));
        app.run("ui.designFromPrompt", json!({"prompt": "a cartoon fox head", "size": 60, "colors": 4})).unwrap();
        let d = app.ui.dialog.clone().unwrap();
        assert_eq!(d.kind, prompt::KIND);
        assert_eq!((d.str("prompt").as_str(), d.f64("size", 0.0), d.f64("colors", 0.0)), ("a cartoon fox head", 60.0, 4.0));
        let texts = dialog_text(&mut app);
        for want in ["Design from Prompt", "Describe the design:", "Anthropic API key:", "Generate"] {
            assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
        }
        // the key is typed into the dialog but is never one of its fields (agents read those)
        app.embroidery.prompt.key = " sk-ant-test ".into();
        crate::dialogs::confirm(&mut app).unwrap();
        assert!(app.ui.dialog.as_ref().is_some_and(|d| !d.fields.values().any(|v| v.as_str().is_some_and(|s| s.contains("sk-ant")))));
        assert!(dialog_text(&mut app).iter().any(|t| t.starts_with("Drawing the design…")));
        wait(&mut app);
        assert!(app.ui.dialog.is_none(), "placed: the dialog closes");
        assert!(app.ui.status.starts_with("Design placed: 3 stitched objects"), "{}", app.ui.status);
        let sent = sent.lock().unwrap();
        let r = &sent[0];
        assert!(r.headers.iter().any(|(k, v)| *k == "x-api-key" && v == "sk-ant-test"));
        let body: Value = serde_json::from_str(&r.body).unwrap();
        assert!(body["messages"][0]["content"].as_str().unwrap().contains("a cartoon fox head"));
        assert!(body["system"].as_str().unwrap().contains("At most 4 different fill colours"));
        assert_eq!(keys.load(prompt::KEY_NAME).as_deref(), Some("sk-ant-test"), "remembered");
        // one group of stitched shapes, named by its description, gone with one undo
        let st = app.session.active().unwrap();
        let g = st.doc.node(st.selection.objects[0]).unwrap();
        assert_eq!(g.name.as_deref(), Some("a cartoon fox head"));
        assert!(g.children().unwrap().iter().all(|c| c.embroidery.is_some()));
        assert!(app.session.embroidery_plan().is_some_and(|p| !p.is_empty()));
        app.run("edit.undo", json!({})).unwrap();
        assert_eq!(objects(&app), 0);
    }

    #[test]
    fn what_goes_wrong_is_shown_in_the_dialog() {
        let err = json!({"type": "error", "error": {"type": "authentication_error", "message": "invalid x-api-key"}}).to_string();
        let (mut app, sent, keys) = app(401, err, Duration::ZERO);
        keys.save(prompt::KEY_NAME, Some("sk-ant-old")).unwrap();
        app.run("ui.designFromPrompt", json!({"prompt": "a star"})).unwrap();
        assert_eq!(app.embroidery.prompt.key, "sk-ant-old", "the kept key is filled in");
        // nothing is sent without a key or a description
        app.embroidery.prompt.key.clear();
        assert_eq!(crate::dialogs::confirm(&mut app).unwrap_err(), "Enter your Anthropic API key first.");
        app.embroidery.prompt.key = "sk-ant-bad".into();
        if let Some(d) = app.ui.dialog.as_mut() {
            d.fields.insert("prompt".into(), json!("  "));
        }
        assert_eq!(crate::dialogs::confirm(&mut app).unwrap_err(), "Describe the design first.");
        assert!(sent.lock().unwrap().is_empty());
        if let Some(d) = app.ui.dialog.as_mut() {
            d.fields.insert("prompt".into(), json!("a star"));
        }
        crate::dialogs::confirm(&mut app).unwrap();
        wait(&mut app);
        assert!(app.ui.dialog.is_some(), "the dialog stays open to try again");
        assert_eq!(app.embroidery.prompt.error.as_deref(), Some("Anthropic refused the API key: invalid x-api-key"));
        assert!(dialog_text(&mut app).iter().any(|t| t == "Anthropic refused the API key: invalid x-api-key"));
        assert_eq!(keys.load(prompt::KEY_NAME).as_deref(), Some("sk-ant-old"), "a refused key isn't remembered");
        assert_eq!(objects(&app), 0);
        // Remember off forgets the kept key
        if let Some(d) = app.ui.dialog.as_mut() {
            d.fields.insert("remember".into(), json!(false));
        }
        crate::dialogs::confirm(&mut app).unwrap();
        assert_eq!(keys.load(prompt::KEY_NAME), None);
        wait(&mut app);
    }

    #[test]
    fn cancel_stops_waiting_and_the_web_has_no_service() {
        let (mut app, sent, _) = app(200, reply(ART), Duration::from_millis(150));
        app.run("ui.designFromPrompt", json!({"prompt": "a star"})).unwrap();
        app.embroidery.prompt.key = "sk-ant-test".into();
        crate::dialogs::confirm(&mut app).unwrap();
        assert!(crate::dialogs::confirm(&mut app).is_err(), "one request at a time");
        crate::dialogs::cancel(&mut app);
        assert!(app.ui.dialog.is_none() && app.embroidery.prompt.drawing_for(0.0).is_none());
        std::thread::sleep(Duration::from_millis(300));
        prompt::poll(&mut app, &egui::Context::default());
        assert_eq!(objects(&app), 0, "a cancelled drawing is never placed");
        assert_eq!(sent.lock().unwrap().len(), 1);
        // without the service (the web) it can't open
        let mut web = VectorcraftApp::new(Session::new(), Services::default());
        web.run("file.new", json!({"width": 400, "height": 300})).unwrap();
        assert!(!crate::menus::enabled(&web, "ui.designFromPrompt"));
        assert!(web.run("ui.designFromPrompt", json!({})).is_err());
    }

    /// A 40 × 20 PNG: red on the left, blue on the right.
    const LOGO_PNG: &str =
        "iVBORw0KGgoAAAANSUhEUgAAACgAAAAUCAIAAABwJOjsAAAALElEQVR42mM8ISfHQC4Ik1tFtl4mhgECoxaPWjxq8ajFoxaPWjxq8ajF9AcAqiICSl834YsAAAAASUVORK5CYII=";

    #[test]
    fn an_image_is_redrawn_and_lands_on_it() {
        // a drawing on a page with the image's aspect: a circle on the left, a square on the right
        let art = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1000 500"><circle cx="250" cy="250" r="200" fill="#c81e1e"/><rect x="600" y="100" width="300" height="300" fill="#1e3cc8"/></svg>"##;
        let (mut app, sent, _) = app(200, reply(art), Duration::ZERO);
        assert!(!crate::menus::enabled(&app, "ui.redrawImage"), "no image selected");
        assert!(app.run("ui.redrawImage", json!({})).is_err());
        let placed = app.run("file.place", json!({"name": "logo.png", "dataBase64": LOGO_PNG, "at": [200, 150]})).unwrap();
        let image = vectorcraft_doc::NodeId(placed["ids"][0].as_u64().unwrap());
        assert!(crate::menus::enabled(&app, "ui.redrawImage"));
        app.run("ui.redrawImage", json!({"prompt": "thicker letters"})).unwrap();
        let d = app.ui.dialog.clone().unwrap();
        assert_eq!(d.fields["image"], json!(image.0));
        // the image is 40 × 20 pt (72 ppi), 14 mm across: brought up to the 20 mm least size
        assert_eq!(d.f64("size", 0.0), 20.0);
        let texts = dialog_text(&mut app);
        for want in ["Redraw Image with Claude", "What to change (optional):", "Only redraw artwork you own or have permission to use."] {
            assert!(texts.iter().any(|t| t == want), "{want} missing from {texts:?}");
        }
        app.embroidery.prompt.key = "sk-ant-test".into();
        crate::dialogs::confirm(&mut app).unwrap();
        wait(&mut app);
        assert!(app.ui.dialog.is_none(), "{:?}", app.embroidery.prompt.error);
        // the image went along as a PNG, before the instruction and the notes
        let body: Value = serde_json::from_str(&sent.lock().unwrap()[0].body).unwrap();
        let content = &body["messages"][0]["content"];
        assert_eq!(content[0]["source"]["media_type"], "image/png");
        assert!(content[0]["source"]["data"].as_str().unwrap().len() > 50);
        assert!(content[1]["text"].as_str().unwrap().ends_with("What to change: thicker letters"));
        // the art lands on the image: its page is the image's place, scaled about its centre to 20 mm
        let st = app.session.active().unwrap();
        let g = st.doc.node(st.selection.objects[0]).unwrap();
        assert_eq!(g.name.as_deref(), Some("Redrawn image"));
        assert!(g.children().unwrap().iter().all(|c| c.embroidery.is_some()));
        let img = st.doc.node(image).unwrap().geometric_bounds().unwrap();
        let k = 20.0 * vectorcraft_embroidery::PT_PER_MM / img.width();
        let (w, h) = (img.width() * k, img.height() * k);
        let (x0, y0) = (img.center().x - w / 2.0, img.center().y - h / 2.0);
        let b = g.geometric_bounds().unwrap();
        for (got, want) in [(b.x0, x0 + 0.05 * w), (b.x1, x0 + 0.9 * w), (b.y0, y0 + 0.1 * h), (b.y1, y0 + 0.9 * h)] {
            assert!((got - want).abs() < 0.5, "{b:?} in a frame at ({x0}, {y0}) {w} × {h}");
        }
    }
}
