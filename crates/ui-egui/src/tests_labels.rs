//! Neutral wording in menus and panels: what users read names VectorCraft's own features.

use egui::epaint::Shape;
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;
use crate::menus::{self, Item};

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    app
}

/// Every string painted by `f` in one headless frame, one per line.
fn painted_text(app: &mut VectorcraftApp, mut f: impl FnMut(&mut VectorcraftApp, &mut egui::Ui)) -> String {
    fn collect(s: &Shape, out: &mut String) {
        match s {
            Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let ctx = egui::Context::default();
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| f(app, ui));
    out.textures_delta.clear();
    let mut text = String::new();
    for c in &out.shapes {
        collect(&c.shape, &mut text);
    }
    text
}

#[test]
fn effect_menu_has_a_vector_effects_section() {
    let tree = menus::menu_tree();
    let (_, effect) = tree.iter().find(|(t, _)| *t == "Effect").expect("Effect menu");
    assert!(effect.iter().any(|i| matches!(i, Item::Header("Vector Effects"))));
}

#[test]
fn library_submenus_are_disabled_placeholders() {
    let app = app();
    let entries = menus::menu_entries(&app);
    for lib in ["Brush Libraries", "Graphic Style Libraries", "Swatch Libraries", "Symbol Libraries"] {
        let items: Vec<_> = entries.iter().filter(|e| e.path == ["Window", lib]).collect();
        let labels: Vec<&str> = items.iter().map(|e| e.label.as_str()).collect();
        assert_eq!(labels, ["Built-in Libraries", "User Defined", "Other Library…"], "{lib}");
        assert!(items.iter().all(|e| !e.enabled && e.command.is_none()), "{lib}");
    }
}

#[test]
fn stroke_profiles_have_descriptive_names() {
    let labels: Vec<&str> = crate::panels::stroke::PROFILES.iter().map(|p| p.1).collect();
    assert_eq!(labels, ["Uniform", "Lens", "Taper Start", "Taper End"]);
}

#[test]
fn swatches_menu_offers_save_swatch_library() {
    let mut app = app();
    let text = painted_text(&mut app, crate::panels::swatches::menu);
    assert!(text.contains("Save Swatch Library…"), "{text}");
    assert!(!text.contains("ASE"), "{text}");
}
