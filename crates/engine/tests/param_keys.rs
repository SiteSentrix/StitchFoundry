//! A caller's params are checked against the command's description, but a journal replays as it
//! was recorded (#1442).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_engine::{Session, check_params};

/// `Session::execute` takes what it is given: the journal of an older version, an action recorded
/// with a key a command no longer reads, still plays. The check is the caller's.
#[test]
fn a_journal_with_a_key_no_longer_read_still_replays() {
    let old = [
        ("file.new", json!({"width": 300, "height": 200, "widht": 1})),
        ("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 40, "cornerStyle": "round"})),
    ];
    let mut s = Session::new();
    for (id, params) in &old {
        assert!(check_params(id, params).is_err(), "{id}");
        s.execute(id, params).unwrap();
    }
    let st = s.active().unwrap();
    assert_eq!((st.doc.artboards[0].rect.width(), st.doc.layers[0].children().unwrap().len()), (300.0, 1));
}

/// What a command adds to its own journal entry passes the check, so a recorded journal can be
/// sent back through the control channel or MCP.
#[test]
fn a_recorded_journal_passes_the_check() {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
    s.execute("select.all", &json!({})).unwrap();
    s.execute("object.scale", &json!({"sx": 150})).unwrap();
    s.execute("command.batch", &json!({"commands": [{"command": "object.move", "params": {"dx": 5, "dy": 0}}, {"command": "edit.copy"}]})).unwrap();
    assert!(s.journal.iter().count() >= 4);
    for (id, params) in s.journal.iter() {
        check_params(id, params).unwrap_or_else(|e| panic!("{id} {params}: {e}"));
    }
}
