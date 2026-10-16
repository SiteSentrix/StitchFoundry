//! The W/H link (Constrain Width and Height Proportions) in the Transform panel, the Properties
//! panel's Transform section and the Control bar: one preference, passed on as `proportional`.
//! Alt+Enter in their fields applies the value to a copy of the selection, passed on as `copy`.

use egui::{Event, Key, Modifiers, PointerButton};
use serde_json::json;

use super::tests_appearance::{frame_raw, run};
use super::*;

/// What draws a set of size fields.
type Draw = fn(&mut VectorcraftApp, &mut Ui);

/// A selected 100 × 50 pt rectangle.
fn rect_app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
    run(&mut app, "file.new", json!({"width": 400, "height": 400}));
    let id = run(&mut app, "shape.rectangle", json!({"x": 10, "y": 10, "width": 100, "height": 50}))["id"].clone();
    run(&mut app, "select.set", json!({ "ids": [id] }));
    app
}

fn size(app: &VectorcraftApp) -> (f64, f64) {
    let st = app.session.active().unwrap();
    let b = st.doc.bounds_of(&st.selection.objects, false).unwrap();
    (b.width().round(), b.height().round())
}

/// How a field's editing ends: Enter, or a press away from the field, with these modifiers held.
#[derive(Clone, Copy)]
enum End {
    Enter(Modifiers),
    ClickAway(Modifiers),
}

/// Type `value` into the field of what `draw` shows that shows `field` (the first one drawn), and
/// end its editing with `end`.
fn set_field(app: &mut VectorcraftApp, draw: Draw, field: &str, value: &str, end: End) {
    set_nth_field(app, draw, (field, 0), value, end);
}

/// [`set_field`] for the field drawn `nth` (from 0) among those that show `field`.
fn set_nth_field(app: &mut VectorcraftApp, draw: Draw, (field, nth): (&str, usize), value: &str, end: End) {
    let ctx = egui::Context::default();
    crate::theme::install_fonts(&ctx);
    let mut time = 0.0;
    let mut frame = |app: &mut VectorcraftApp, events: Vec<Event>| {
        time += 1.0;
        frame_raw(&ctx, app, egui::RawInput { events, time: Some(time), ..Default::default() }, draw)
    };
    let shown = frame(app, vec![]);
    let at = shown.iter().filter(|(s, _)| s == field).nth(nth).unwrap_or_else(|| panic!("no {field} field {nth} in {shown:?}")).1.center();
    let button = |pos, pressed, modifiers| Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers };
    frame(app, vec![Event::PointerMoved(at), button(at, true, Modifiers::NONE), button(at, false, Modifiers::NONE)]);
    // Focus selected the whole field: typing replaces it.
    frame(app, vec![]);
    frame(app, vec![Event::Text(value.into())]);
    // The modifiers go down first, as a keyboard reports them.
    match end {
        End::Enter(modifiers) => {
            let enter = Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers };
            frame(app, vec![Event::ModifiersChanged(modifiers), enter]);
        }
        End::ClickAway(modifiers) => {
            let away = at + egui::vec2(0.0, 300.0);
            let press = [button(away, true, modifiers), button(away, false, modifiers)];
            frame(app, [Event::ModifiersChanged(modifiers), Event::PointerMoved(away)].into_iter().chain(press).collect());
        }
    }
}

/// Type `value` into the W field (the one showing "100 pt") of what `draw` shows, and press Enter.
fn set_width(app: &mut VectorcraftApp, draw: Draw, value: &str) {
    set_field(app, draw, "100 pt", value, End::Enter(Modifiers::NONE));
}

/// The paths of the document, bottom to top: their size and angle (rounded), and whether each is
/// selected.
fn paths(app: &VectorcraftApp) -> Vec<((f64, f64), f64, bool)> {
    let st = app.session.active().unwrap();
    let mut out = vec![];
    st.doc.walk(|n| {
        if matches!(n.kind, vectorcraft_doc::NodeKind::Path { .. }) {
            let b = st.doc.oriented_bounds(&[n.id], false).unwrap();
            out.push(((b.rect.width().round(), b.rect.height().round()), b.angle.round(), st.selection.objects.contains(&n.id)));
        }
    });
    out
}

#[test]
fn the_link_keeps_proportions_in_every_size_field() {
    let surfaces: [(&str, Draw); 3] =
        [("Transform panel", transform::show), ("Properties", properties::transform_section), ("Control bar", crate::chrome::control_bar)];
    for (name, draw) in surfaces {
        // Off: W alone.
        let mut app = rect_app();
        set_width(&mut app, draw, "200");
        assert_eq!(size(&app), (200.0, 50.0), "{name}, unlinked");
        // On: H follows.
        let mut app = rect_app();
        run(&mut app, "prefs.set", json!({"key": "constrainProportions", "value": true}));
        set_width(&mut app, draw, "200");
        assert_eq!(size(&app), (200.0, 100.0), "{name}, linked");
    }
}

/// Alt+Enter (Option+Return) applies a field's value to a copy of the selection and selects the
/// copy, in one undo step; a press elsewhere with Alt held applies it to the selection.
#[test]
fn alt_enter_applies_the_value_to_a_copy_in_every_size_field() {
    let surfaces: [(&str, Draw); 3] =
        [("Transform panel", transform::show), ("Properties", properties::transform_section), ("Control bar", crate::chrome::control_bar)];
    for (name, draw) in surfaces {
        let mut app = rect_app();
        set_field(&mut app, draw, "100 pt", "200", End::Enter(Modifiers::ALT));
        assert_eq!(paths(&app), [((100.0, 50.0), 0.0, false), ((200.0, 50.0), 0.0, true)], "{name}");
        run(&mut app, "edit.undo", json!({}));
        assert_eq!(paths(&app), [((100.0, 50.0), 0.0, true)], "{name}, undone");
        let mut app = rect_app();
        set_field(&mut app, draw, "100 pt", "200", End::ClickAway(Modifiers::ALT));
        assert_eq!(paths(&app), [((200.0, 50.0), 0.0, true)], "{name}, a press away");
    }
}

/// Alt+Enter in Rotate turns a copy to the angle and leaves the original, in the Transform panel
/// and the Properties panel; in the Transform panel, Alt+Enter in Shear shears a copy.
#[test]
fn alt_enter_in_rotate_and_shear_transforms_a_copy() {
    for (name, draw) in [("Transform panel", transform::show as Draw), ("Properties", properties::transform_section)] {
        let mut app = rect_app();
        // Rotate is the first field showing 0° (the Transform panel's Shear, after it, rests at 0° too).
        set_field(&mut app, draw, "0°", "45", End::Enter(Modifiers::ALT));
        assert_eq!(paths(&app), [((100.0, 50.0), 0.0, false), ((100.0, 50.0), 45.0, true)], "{name}");
    }
    let mut app = rect_app();
    // The second field showing 0° is Shear.
    set_nth_field(&mut app, transform::show, ("0°", 1), "30", End::Enter(Modifiers::ALT));
    assert_eq!(paths(&app), [((100.0, 50.0), 0.0, false), ((129.0, 50.0), 0.0, true)]);
}

/// With anchors selected with Direct Selection, the fields show those anchors' box, and Alt+Enter
/// applies the value to them as Enter does.
#[test]
fn alt_enter_moves_selected_anchors_without_a_copy() {
    let mut app = rect_app();
    let id = app.session.active().unwrap().selection.objects[0].0;
    run(&mut app, "select.anchors", json!({"id": id, "anchors": [[0, 1]]}));
    let before = paths(&app).len();
    // The anchor's X (the top-right corner of the rectangle at 10, 10).
    set_field(&mut app, transform::show, "110 pt", "150", End::Enter(Modifiers::ALT));
    assert_eq!(paths(&app).len(), before);
    let st = app.session.active().unwrap();
    assert_eq!(st.doc.bounds_of(&st.selection.objects, false).unwrap().width().round(), 140.0);
}

#[test]
fn the_link_is_a_preference_agents_can_read_and_set() {
    let mut app = rect_app();
    assert_eq!(run(&mut app, "prefs.get", json!({"key": "constrainProportions"})), json!(false));
    run(&mut app, "prefs.set", json!({"key": "constrainProportions", "value": true}));
    assert!(app.session.prefs.constrain_proportions);
    // What the fields send with the link on.
    run(&mut app, "object.setBounds", json!({"height": 100, "reference": 4, "proportional": true}));
    assert_eq!(size(&app), (200.0, 100.0));
}
