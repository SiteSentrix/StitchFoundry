//! Perspective Grid dialogs: Define Grid, the presets manager and the tool options.

use serde_json::{Value, json};
use vectorcraft_engine::Session;
use vectorcraft_tools::distort::perspective::PerspectiveGrid;

use crate::{VectorcraftApp, theme};

fn frame(app: &mut VectorcraftApp) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    let mut out = ctx.run_ui(egui::RawInput::default(), |ui| super::show(app, ui.ctx()));
    out.textures_delta.clear();
}

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 800, "height": 600})).unwrap();
    app
}

fn kind(app: &VectorcraftApp) -> Option<&str> {
    app.ui.dialog.as_ref().map(|d| d.kind.as_str())
}

fn field(app: &VectorcraftApp, k: &str) -> Value {
    app.ui.dialog.as_ref().and_then(|d| d.fields.get(k).cloned()).unwrap_or_default()
}

fn set(app: &mut VectorcraftApp, k: &str, v: Value) {
    app.ui.dialog.as_mut().expect("a dialog is open").fields.insert(k.into(), v);
}

fn grid(app: &VectorcraftApp) -> PerspectiveGrid {
    PerspectiveGrid::effective(&app.session.active().unwrap().doc)
}

#[test]
fn define_grid_opens_on_the_grid_and_applies_its_fields() {
    let mut app = app();
    app.run("perspective.grid.preset", json!({"kind": 3})).unwrap();
    let g = grid(&app);
    app.run("ui.perspectiveGridDialog", json!({})).unwrap();
    assert_eq!(kind(&app), Some(super::perspective_grid::KIND));
    assert_eq!(field(&app, "kind"), json!(3), "prefilled from the grid, not a fixed type");
    assert!((field(&app, "gridline").as_f64().unwrap() - g.cell).abs() < 1e-9);
    frame(&mut app);
    // A new unit converts the lengths shown.
    set(&mut app, "units", json!("inches"));
    frame(&mut app);
    assert!((field(&app, "gridline").as_f64().unwrap() - g.cell / 72.0).abs() < 1e-9);
    // OK with nothing changed leaves the grid as it was.
    super::confirm(&mut app).unwrap();
    assert_eq!(kind(&app), None);
    assert_eq!((grid(&app).cell, grid(&app).vp_left, grid(&app).kind), (g.cell, g.vp_left, 3));
    // A new viewing angle and colour apply in one undo step.
    let undo = app.session.active().unwrap().history.undo.len();
    app.run("ui.perspectiveGridDialog", json!({})).unwrap();
    set(&mut app, "angle", json!(30));
    set(&mut app, "rightColor", json!("#112233"));
    frame(&mut app);
    super::confirm(&mut app).unwrap();
    let n = grid(&app);
    assert!((n.viewing_angle() - 30.0).abs() < 1e-9);
    assert_eq!(n.right_color.hex(), "#112233");
    assert_eq!(app.session.active().unwrap().history.undo.len(), undo + 1);
    // A refused value keeps the dialog open.
    app.run("ui.perspectiveGridDialog", json!({})).unwrap();
    set(&mut app, "distance", json!(-5));
    assert!(super::confirm(&mut app).is_err());
    assert_eq!(kind(&app), Some(super::perspective_grid::KIND));
}

#[test]
fn define_grid_is_a_view_menu_item() {
    let app = app();
    let entries = crate::menus::menu_entries(&app);
    let e = entries.iter().find(|e| e.command.as_deref() == Some("ui.perspectiveGridDialog")).expect("View › Perspective Grid › Define Grid…");
    assert!(e.path.iter().any(|p| p == "Perspective Grid"), "{:?}", e.path);
    assert!(e.enabled);
}
