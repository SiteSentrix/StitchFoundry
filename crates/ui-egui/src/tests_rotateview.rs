//! View › Rotate View and `view.rotate`: the view turns to one of the status bar's preset angles
//! from the menu, or to any angle from the command, and the menu checks the angle the view is turned to.

use serde_json::json;
use vectorcraft_engine::Session;

use crate::{VectorcraftApp, menus};

/// These are the status bar's angles and their labels, in the status bar's order.
const ANGLES: [(&str, f64); 12] = [
    ("0°", 0.0),
    ("15°", 15.0),
    ("30°", 30.0),
    ("45°", 45.0),
    ("60°", 60.0),
    ("90°", 90.0),
    ("180°", 180.0),
    ("-15°", -15.0),
    ("-30°", -30.0),
    ("-45°", -45.0),
    ("-60°", -60.0),
    ("-90°", -90.0),
];

fn with_document() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), crate::Services::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app
}

/// Returns the labels of the View › Rotate View items that are checked.
fn checked(app: &VectorcraftApp) -> Vec<&'static str> {
    ANGLES.iter().filter(|(_, a)| menus::checked(app, "view.rotate", &json!({ "angle": a })) == Some(true)).map(|(l, _)| *l).collect()
}

#[test]
fn view_rotate_turns_the_view_to_any_angle() {
    let mut app = with_document();
    let center = app.view().unwrap().center;
    assert_eq!(app.run("view.rotate", json!({"angle": 30})).unwrap(), json!({"angle": 30.0}));
    let v = app.view().unwrap();
    assert_eq!((v.rotation, v.center), (30.0, center), "it turns about the center of the canvas");
    // Angles are normalized to (-180, 180], as a Rotate View drag normalizes them.
    for (angle, want) in [(390.0, 30.0), (-190.0, 170.0), (-180.0, 180.0), (12.5, 12.5)] {
        assert_eq!(app.run("view.rotate", json!({ "angle": angle })).unwrap(), json!({ "angle": want }), "{angle}");
        assert_eq!(app.view().unwrap().rotation, want, "{angle}");
    }
    for p in [json!({"angle": "x"}), json!({}), json!({"angle": null})] {
        assert_eq!(app.run("view.rotate", p.clone()).unwrap_err(), "`angle` must be a number", "{p}");
    }
    assert_eq!(app.view().unwrap().rotation, 12.5, "an error leaves the view as it was");
    let mut empty = VectorcraftApp::new(Session::new(), crate::Services::default());
    assert_eq!(empty.run("view.rotate", json!({"angle": 30})).unwrap_err(), "no document");
}

#[test]
fn the_view_menu_lists_the_status_bar_angles_and_checks_the_current_one() {
    let mut app = with_document();
    let entries = menus::menu_entries(&app);
    let at = entries.iter().position(|e| e.path == ["View", "Rotate View"]).expect("View › Rotate View");
    let (items, after) = entries[at..].split_at(ANGLES.len());
    let shown: Vec<_> = items.iter().map(|e| (e.path.join(" › "), e.label.as_str(), e.command.as_deref(), e.params.clone(), e.enabled)).collect();
    let want: Vec<_> = ANGLES.iter().map(|(l, a)| ("View › Rotate View".to_string(), *l, Some("view.rotate"), json!({ "angle": a }), true)).collect();
    assert_eq!(shown, want);
    assert_eq!((after[0].path.join(" › "), after[0].label.as_str()), ("View".to_string(), "Reset Rotate View"), "the submenu comes right before it");

    app.run("view.rotate", json!({"angle": 30})).unwrap();
    assert_eq!(checked(&app), ["30°"]);
    app.run("view.rotate", json!({"angle": 17})).unwrap();
    assert!(checked(&app).is_empty());
    // Every item stays a check item, unchecked, while the view is at another angle.
    assert!(ANGLES.iter().all(|(_, a)| menus::checked(&app, "view.rotate", &json!({ "angle": a })) == Some(false)));
    // A click on 180° runs its command.
    let item = &items[6];
    let (id, p) = menus::click_target(&item.label, item.command.as_deref().unwrap(), &item.params);
    menus::invoke(&mut app, &id, p);
    assert_eq!(app.view().unwrap().rotation, 180.0);
    assert_eq!(checked(&app), ["180°"]);
}

/// Rotate View with no angle, as the command palette runs it, opens a dialog with the view's angle
/// filled in, and OK turns the view to the angle entered.
#[test]
fn rotate_view_without_an_angle_asks_for_one() {
    assert!(crate::palette::items().iter().any(|(label, id, _)| (label.as_str(), id.as_str()) == ("Rotate View", "view.rotate")));
    let mut app = with_document();
    app.run("view.rotate", json!({"angle": 30})).unwrap();
    menus::invoke(&mut app, "view.rotate", json!({}));
    let mut d = app.ui.dialog.clone().expect("Rotate View dialog");
    assert_eq!((d.kind.as_str(), d.str("__command"), d.str("__label")), ("command", "view.rotate".into(), "Rotate View".into()));
    assert_eq!(d.fields.get("angle"), Some(&json!(30.0)));
    d.fields.insert("angle".into(), json!(-45));
    app.ui.dialog = Some(d);
    crate::dialogs::confirm(&mut app).unwrap();
    assert!(app.ui.dialog.is_none());
    assert_eq!(app.view().unwrap().rotation, -45.0);
    // Without a document no dialog opens, and the status bar shows "no document".
    let mut empty = VectorcraftApp::new(Session::new(), crate::Services::default());
    menus::invoke(&mut empty, "view.rotate", json!({}));
    assert!(empty.ui.dialog.is_none());
    assert_eq!(empty.ui.status, "no document");
}

#[test]
fn rotate_view_needs_a_document() {
    let empty = VectorcraftApp::new(Session::new(), crate::Services::default());
    assert!(!menus::enabled(&empty, "view.rotate"));
    assert_eq!(menus::checked(&empty, "view.rotate", &json!({"angle": 0})), Some(false), "a check item without a document");
    assert!(menus::enabled(&with_document(), "view.rotate"));
}

/// The strings the catalogs cover include the submenu's name and leave out the angle labels, as they
/// leave out Type › Size's point sizes.
#[test]
fn only_the_submenu_name_needs_translating() {
    let strings = menus::menu_strings();
    assert!(strings.contains("Rotate View"));
    for (label, _) in ANGLES {
        assert!(!strings.contains(label), "{label}");
    }
}
