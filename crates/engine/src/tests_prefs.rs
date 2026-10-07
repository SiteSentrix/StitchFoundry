//! Preferences: `prefs.get` / `prefs.set` / `prefs.reset` / `prefs.list`.

use serde_json::json;

use super::*;
use crate::cmd::prefscmds::{PREF_CATEGORIES, PREF_GROUPS, PREF_SPECS, validate};

#[test]
fn every_spec_matches_a_prefs_field_and_back() {
    let all = Prefs::default().to_json();
    let obj = all.as_object().unwrap();
    for sp in PREF_SPECS {
        assert!(obj.contains_key(sp.key), "spec `{}` has no Prefs field", sp.key);
        assert!(PREF_CATEGORIES.contains(&sp.category), "spec `{}` has unknown category", sp.key);
        // Defaults validate against their own spec.
        assert!(validate(sp.key, &obj[sp.key]).is_ok(), "default of `{}` fails validation", sp.key);
    }
    for k in obj.keys() {
        assert!(PREF_SPECS.iter().any(|s| s.key == k) || PREF_GROUPS.contains(&k.as_str()), "Prefs field `{k}` has no spec");
    }
    for c in PREF_CATEGORIES {
        assert!(PREF_SPECS.iter().any(|s| s.category == *c), "category {c} is empty");
    }
}

#[test]
fn set_and_get_keyboard_increment_drives_nudge() {
    let mut s = Session::new();
    s.execute("prefs.set", &json!({"key": "keyboardIncrement", "value": 5})).unwrap();
    assert_eq!(s.execute("prefs.get", &json!({"key": "keyboardIncrement"})).unwrap(), json!(5.0));
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let r = s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 20, "height": 20})).unwrap();
    let id = NodeId(r["id"].as_u64().unwrap());
    s.execute("object.nudge", &json!({"dx": 1, "dy": 0})).unwrap();
    let b = s.doc().unwrap().doc.node(id).unwrap().geometric_bounds().unwrap();
    assert!((b.x0 - 15.0).abs() < 1e-9);
}

#[test]
fn set_rejects_out_of_range_and_wrong_types() {
    let mut s = Session::new();
    assert!(s.execute("prefs.set", &json!({"key": "keyboardIncrement", "value": -1})).is_err());
    assert!(s.execute("prefs.set", &json!({"key": "anchorSize", "value": 9})).is_err());
    assert!(s.execute("prefs.set", &json!({"key": "anchorSize", "value": 2.5})).is_err());
    assert!(s.execute("prefs.set", &json!({"key": "scaleStrokes", "value": 3})).is_err());
    assert!(s.execute("prefs.set", &json!({"key": "uiBrightness", "value": "purple"})).is_err());
    assert!(s.execute("prefs.set", &json!({"key": "gridColor", "value": "#12345"})).is_err());
    assert!(s.execute("prefs.set", &json!({"key": "nope", "value": 1})).is_err());
    assert!(s.execute("prefs.set", &json!({"key": "scaleStrokes"})).is_err());
    assert_eq!(s.prefs, Prefs::default());
}

#[test]
fn set_normalizes_strings_labels_and_colours() {
    let mut s = Session::new();
    let r = s
        .execute(
            "prefs.set",
            &json!({"values": {"uiBrightness": "Medium Light", "gridColor": "ABCDEF", "keyboardIncrement": "2.5 pt", "scaleStrokes": "false"}}),
        )
        .unwrap();
    assert_eq!(r["uiBrightness"], json!("mediumLight"));
    assert_eq!(s.prefs.grid_color, "#abcdef");
    assert_eq!(s.prefs.keyboard_increment, 2.5);
    assert!(!s.prefs.scale_strokes);
}

#[test]
fn batch_set_is_atomic() {
    let mut s = Session::new();
    assert!(s.execute("prefs.set", &json!({"values": {"keyboardIncrement": 3, "anchorSize": 99}})).is_err());
    assert_eq!(s.prefs.keyboard_increment, 1.0);
}

#[test]
fn grid_prefs_apply_to_open_documents() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    s.execute("prefs.set", &json!({"values": {"gridlineEvery": 36, "gridSubdivisions": 4}})).unwrap();
    let g = &s.doc().unwrap().doc.grid;
    assert_eq!((g.spacing, g.subdivisions), (36.0, 4));
}

#[test]
fn history_states_limit_undo_depth() {
    let mut s = Session::new();
    s.execute("prefs.set", &json!({"key": "historyStates", "value": 5})).unwrap();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    for i in 0..9 {
        s.execute("shape.rectangle", &json!({"x": i * 10, "y": 0, "width": 5, "height": 5})).unwrap();
    }
    assert!(s.doc().unwrap().history.undo.len() <= 5);
}

#[test]
fn reset_category_and_all() {
    let mut s = Session::new();
    s.execute("prefs.set", &json!({"values": {"keyboardIncrement": 7, "gridColor": "#000000"}})).unwrap();
    s.execute("prefs.reset", &json!({"category": "General"})).unwrap();
    assert_eq!(s.prefs.keyboard_increment, 1.0);
    assert_eq!(s.prefs.grid_color, "#000000");
    s.execute("prefs.reset", &json!({})).unwrap();
    assert_eq!(s.prefs, Prefs::default());
    assert!(s.execute("prefs.reset", &json!({"category": "Bogus"})).is_err());
}

#[test]
fn list_describes_every_pref() {
    let mut s = Session::new();
    let l = s.execute("prefs.list", &json!({})).unwrap();
    let a = l.as_array().unwrap();
    assert_eq!(a.len(), PREF_SPECS.len());
    assert!(a.iter().any(|e| e["key"] == "renderThreads" && e["kind"] == "integer" && e["min"] == -1));
}

#[test]
fn prefs_serde_round_trip_and_tolerates_missing_fields() {
    let p = Prefs { ui_scaling: 1.25, units_general: "millimeters".into(), ..Default::default() };
    let back: Prefs = serde_json::from_value(p.to_json()).unwrap();
    assert_eq!(back, p);
    let partial: Prefs = serde_json::from_value(json!({"keyboardIncrement": 4})).unwrap();
    assert_eq!(partial.keyboard_increment, 4.0);
    assert_eq!(partial.corner_radius, 12.0);
}

/// Performance › Graphics Processor (#306): power saving by default, set by value or label, kept
/// through a save/load round trip, and a bad value is an error that changes nothing.
#[test]
fn gpu_preference_defaults_to_power_saving_and_validates() {
    let mut s = Session::new();
    assert_eq!(s.prefs.gpu_preference, "powerSaving");
    assert_eq!(s.execute("prefs.get", &json!({"key": "gpuPreference"})).unwrap(), json!("powerSaving"));
    s.execute("prefs.set", &json!({"key": "gpuPreference", "value": "highPerformance"})).unwrap();
    assert_eq!(s.prefs.gpu_preference, "highPerformance");
    s.execute("prefs.set", &json!({"key": "gpuPreference", "value": "Power Saving (integrated)"})).unwrap();
    assert_eq!(s.prefs.gpu_preference, "powerSaving");
    for bad in [json!("turbo"), json!(""), json!(1), json!(null), json!(["highPerformance"])] {
        assert!(s.execute("prefs.set", &json!({"key": "gpuPreference", "value": bad})).is_err(), "{bad}");
        assert_eq!(s.prefs.gpu_preference, "powerSaving");
    }
    let p = Prefs { gpu_preference: "highPerformance".into(), ..Default::default() };
    let back: Prefs = serde_json::from_value(p.to_json()).unwrap();
    assert_eq!(back.gpu_preference, "highPerformance");
    // Preference files written before the preference existed get the default.
    let old: Prefs = serde_json::from_value(json!({"gpuPerformance": true})).unwrap();
    assert_eq!(old.gpu_preference, "powerSaving");
    let l = s.execute("prefs.list", &json!({})).unwrap();
    let row = l.as_array().unwrap().iter().find(|e| e["key"] == "gpuPreference").unwrap().clone();
    assert_eq!(row["category"], "Performance");
    assert_eq!(row["options"], json!(["powerSaving", "highPerformance"]));
    s.execute("prefs.set", &json!({"key": "gpuPreference", "value": "highPerformance"})).unwrap();
    s.execute("prefs.reset", &json!({"category": "Performance"})).unwrap();
    assert_eq!(s.prefs.gpu_preference, "powerSaving");
}
