//! The font menus list the installed fonts without asking for them (#36), and the font dropdown
//! searches them.

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_text::{FontDb, system_font_dirs};

use crate::tests_labels::shapes_text;
use crate::{VectorcraftApp, menus, widgets};

#[test]
fn type_font_menu_lists_every_available_family() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({})).unwrap();
    let fonts = |app: &VectorcraftApp| -> Vec<String> {
        menus::menu_entries(app).into_iter().filter(|e| e.path == ["Type", "Font"]).map(|e| e.label).collect()
    };
    let labels = fonts(&app);
    assert_eq!(labels, *FontDb::global().menu_family_list());
    let installed = FontDb::with_font_dirs(system_font_dirs());
    installed.load_system_fonts();
    let missing: Vec<String> = installed.menu_family_list().iter().filter(|f| !labels.contains(f)).cloned().collect();
    assert!(missing.is_empty(), "installed but not in Type › Font: {missing:?}");
    // The menu is built every frame, from a list built once.
    assert_eq!(fonts(&app), labels);
}

#[test]
fn the_font_dropdown_searches_the_families_and_enter_picks_the_first_match() {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    // Each frame: what it painted, where the dropdown is and what it picked.
    let frame = |events: Vec<egui::Event>| {
        let input =
            egui::RawInput { events, screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 800.0))), ..Default::default() };
        let (mut at, mut picked) = (egui::Pos2::ZERO, None);
        let mut out = ctx.run_ui(input, |ui| {
            at = ui.next_widget_position();
            picked = widgets::font_dropdown(ui, "test-font", "Inter", 220.0);
        });
        out.textures_delta.clear();
        (shapes_text(&out), at, picked)
    };
    let (closed, at, _) = frame(vec![]);
    assert_eq!(closed.trim(), "Inter");
    let click = at + egui::vec2(40.0, 10.0);
    let button = |pressed| egui::Event::PointerButton { pos: click, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(vec![egui::Event::PointerMoved(click), button(true)]);
    frame(vec![button(false)]);
    // The popup sizes itself invisibly in its first frame, then the list scrolls (animated).
    for _ in 0..30 {
        frame(vec![]);
    }
    // The families painted in the list (the button shows the current one, Inter).
    let families = FontDb::global().families();
    let listed = |text: &str| -> Vec<String> { text.lines().skip(1).filter(|l| families.iter().any(|f| f == l)).map(str::to_string).collect() };
    let (open, ..) = frame(vec![]);
    assert!(listed(&open).len() >= families.len().min(5), "{open}");
    assert!(listed(&open).iter().any(|f| f == "Inter"), "the current font in view: {open}");
    // The search field has the focus: typing filters the list.
    frame(vec![egui::Event::Text("source serif".into())]);
    let (filtered, ..) = frame(vec![]);
    let shown = listed(&filtered);
    assert!(shown.iter().any(|f| f == "Source Serif 4"), "{filtered}");
    assert!(shown.iter().all(|f| f.to_lowercase().contains("source serif")), "{shown:?}");
    let enter = |pressed| egui::Event::Key { key: egui::Key::Enter, physical_key: None, pressed, repeat: false, modifiers: Default::default() };
    let (_, _, picked) = frame(vec![enter(true), enter(false)]);
    assert_eq!(picked.as_deref(), Some("Source Serif 4"));
    let (closed, ..) = frame(vec![]);
    assert_eq!(closed.trim(), "Inter", "the list closes");
}

#[test]
fn the_character_panel_menu_refreshes_the_font_list() {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({})).unwrap();
    // The installed fonts are always listed: the menu offers to look for new ones instead.
    let text = crate::tests_labels::painted_text(&mut app, crate::panels::character::menu);
    assert!(text.contains("Refresh Font List") && !text.contains("System Fonts"), "{text}");
}
