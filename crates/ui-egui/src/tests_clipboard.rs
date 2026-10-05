//! Paste in the app: the Swatch Conflict dialog.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::NodeId;
use vectorcraft_engine::Session;

use crate::{VectorcraftApp, dialogs};

fn run(app: &mut VectorcraftApp, id: &str, p: Value) -> Value {
    app.run(id, p).unwrap_or_else(|e| panic!("{id}: {e}"))
}

fn rect(app: &mut VectorcraftApp) -> NodeId {
    NodeId(run(app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 40, "height": 20}))["id"].as_u64().unwrap())
}

fn copy(app: &mut VectorcraftApp, id: NodeId) {
    run(app, "select.set", json!({"ids": [id.0]}));
    run(app, "edit.copy", json!({}));
}

/// Objects on the active document's first layer.
fn objects(app: &VectorcraftApp) -> Vec<NodeId> {
    app.session.active().unwrap().doc.layers[0].children().unwrap().iter().map(|n| n.id).collect()
}

fn swatch(app: &mut VectorcraftApp, name: &str, hex: &str) {
    run(app, "swatch.new", json!({"name": name, "color": hex, "global": true}));
}

/// A rectangle filled with global swatch Brand (red) and stroked with Ink (black) copied from one
/// document, and a second document (active) whose Brand is blue and Ink green.
fn conflicting() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    run(&mut app, "file.new", json!({"width": 200, "height": 200}));
    swatch(&mut app, "Brand", "#ff0000");
    swatch(&mut app, "Ink", "#000000");
    let r = rect(&mut app);
    run(&mut app, "paint.setFill", json!({"ids": [r.0], "swatch": "Brand"}));
    run(&mut app, "paint.setStroke", json!({"ids": [r.0], "swatch": "Ink"}));
    copy(&mut app, r);
    run(&mut app, "file.new", json!({"width": 200, "height": 200}));
    swatch(&mut app, "Brand", "#0000ff");
    swatch(&mut app, "Ink", "#00ff00");
    app
}

fn paints(app: &VectorcraftApp) -> (Paint, Paint) {
    let st = app.session.active().unwrap();
    let n = st.doc.node(objects(app)[0]).unwrap();
    (n.appearance.fill_paint(), n.appearance.stroke().unwrap().paint.clone())
}

fn linked(hex: &str, name: &str) -> Paint {
    Paint::Solid { color: Color::from_hex(hex).unwrap(), swatch: Some(name.into()), tint: 1.0 }
}

fn set(app: &mut VectorcraftApp, k: &str, v: Value) {
    app.ui.dialog.as_mut().unwrap().fields.insert(k.into(), v);
}

#[test]
fn the_swatch_conflict_dialog_asks_about_each_swatch_then_pastes() {
    let mut app = conflicting();
    assert_eq!(run(&mut app, "edit.paste", json!({})), json!({"dialog": dialogs::swatch_conflict::KIND}));
    assert!(objects(&app).is_empty(), "nothing is pasted before the answer");
    // It draws headlessly, naming the swatch it asks about.
    let text = crate::tests_labels::painted_text(&mut app, |app, ui| dialogs::show(app, ui.ctx()));
    for label in ["Swatch Conflict", "\u{201c}Brand\u{201d}", "Conflict 1 of 2", "Merge Swatches", "Add Swatches", "Apply to All"] {
        assert!(text.contains(label), "{label} in {text}");
    }
    // Brand: add; Ink: merge.
    set(&mut app, "choice", json!("add"));
    dialogs::confirm(&mut app).unwrap();
    assert_eq!(app.ui.dialog.as_ref().unwrap().fields["index"], 1, "the next conflict");
    assert!(objects(&app).is_empty());
    set(&mut app, "choice", json!("merge"));
    dialogs::confirm(&mut app).unwrap();
    assert!(app.ui.dialog.is_none());
    assert_eq!(paints(&app), (linked("#ff0000", "Brand 2"), linked("#00ff00", "Ink")));
    // One undo step takes the object and the added swatch away.
    run(&mut app, "edit.undo", json!({}));
    assert!(objects(&app).is_empty() && app.session.active().unwrap().doc.swatch("Brand 2").is_none());
}

#[test]
fn apply_to_all_answers_the_rest_and_cancel_pastes_nothing() {
    let mut app = conflicting();
    run(&mut app, "edit.paste", json!({}));
    dialogs::cancel(&mut app);
    assert!(app.ui.dialog.is_none() && objects(&app).is_empty());
    run(&mut app, "edit.pasteInPlace", json!({}));
    set(&mut app, "choice", json!("add"));
    set(&mut app, "applyToAll", json!(true));
    dialogs::confirm(&mut app).unwrap();
    assert!(app.ui.dialog.is_none());
    assert_eq!(paints(&app), (linked("#ff0000", "Brand 2"), linked("#000000", "Ink 2")));
    // An answer given with the command needs no dialog (agents).
    let v = run(&mut app, "edit.pasteInPlace", json!({"swatchConflict": "merge"}));
    assert_eq!(v["merged"], 2);
    assert!(app.ui.dialog.is_none());
}
