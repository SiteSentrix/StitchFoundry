//! The journal replays to the same document whenever it runs: values a command takes from the
//! clock (File ▸ New's created date, a save's modified date) are recorded in its entry, and left out
//! of recorded actions.

use serde_json::json;

use super::*;

fn created(s: &Session) -> Option<i64> {
    s.doc().unwrap().doc.metadata.created
}

#[test]
fn file_new_records_its_created_date_in_the_journal() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200, "height": 100})).unwrap();
    let (id, p) = s.journal.last().unwrap().clone();
    assert_eq!(id, "file.new");
    assert!(created(&s).is_some(), "a new document is dated now");
    assert_eq!(p, json!({"width": 200, "height": 100, "created": created(&s)}));
}

#[test]
fn file_new_takes_a_given_created_date() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"created": 1_000_000_000})).unwrap();
    assert_eq!(created(&s), Some(1_000_000_000));
    s.execute("file.new", &json!({"created": null})).unwrap();
    assert_eq!(created(&s), None);
    // Given values are journaled as given.
    assert_eq!(s.journal.last().unwrap().1, json!({"created": null}));
    for bad in [json!("2026-01-01"), json!(1.5), json!(true)] {
        assert!(s.execute("file.new", &json!({"created": bad})).is_err(), "{bad}");
    }
    assert_eq!(s.documents().len(), 2, "a bad date makes no document");
}

#[test]
fn replay_reproduces_the_created_date_of_an_earlier_run() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 200})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
    // As if the original run happened long ago: a replay now must still land on its date.
    s.journal[0].1["created"] = json!(1_234_567_890);
    let mut r = Session::new();
    for (id, p) in &s.journal {
        r.execute(id, p).unwrap();
    }
    assert_eq!(created(&r), Some(1_234_567_890));
    assert_eq!(r.doc().unwrap().doc.node_count(), s.doc().unwrap().doc.node_count());
}

#[test]
fn recorded_actions_leave_out_replay_only_values() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 200})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 40})).unwrap();
    s.execute("object.scale", &json!({"sx": 200})).unwrap();
    let steps = s.journal_for_action(0);
    assert_eq!(steps.len(), s.journal.len());
    // Playing the action later dates its new document then…
    assert_eq!(steps[0], ("file.new".to_string(), json!({"width": 300, "height": 200})));
    // …while values resolved from the preferences stay recorded.
    let scale = steps.iter().find(|(id, _)| id == "object.scale").unwrap();
    assert!(scale.1.get("strokes").is_some(), "{scale:?}");
    assert_eq!(s.journal_for_action(1), s.journal[1..].to_vec());
}

#[test]
fn a_save_records_its_date_and_replays_it() {
    let path = vectorcraft_testkit::temp_dir("journal-save").join("a.vectorcraft").to_string_lossy().into_owned();
    let mut s = Session::new();
    s.execute("file.new", &json!({"created": null})).unwrap();
    s.execute("document.save", &json!({"path": path})).unwrap();
    let m = &s.doc().unwrap().doc.metadata;
    assert!(m.modified.is_some() && m.created == m.modified, "a save dates a document without one");
    let (id, p) = s.journal.last().unwrap().clone();
    assert_eq!((id.as_str(), &p["modified"]), ("document.save", &json!(m.modified)));
    // A given date is used as is; null leaves the dates alone.
    s.execute("document.save", &json!({"path": path, "modified": 2_000_000_000})).unwrap();
    assert_eq!(s.doc().unwrap().doc.metadata.modified, Some(2_000_000_000));
    s.execute("document.save", &json!({"path": path, "modified": null})).unwrap();
    assert_eq!(s.doc().unwrap().doc.metadata.modified, Some(2_000_000_000));
    assert!(s.execute("document.save", &json!({"path": path, "modified": "today"})).is_err());
    // Save As stamps the same way; Save a Copy leaves the document's dates (and records none).
    let dir = vectorcraft_testkit::temp_dir("journal-save");
    s.execute("file.saveAs", &json!({"path": dir.join("b.vectorcraft").to_string_lossy(), "modified": 2_000_000_100})).unwrap();
    assert_eq!(s.journal.last().unwrap().1["modified"], json!(2_000_000_100));
    s.execute("file.saveCopy", &json!({"path": dir.join("c.vectorcraft").to_string_lossy()})).unwrap();
    assert!(s.journal.last().unwrap().1.get("modified").is_none());
    assert_eq!(s.doc().unwrap().doc.metadata.modified, Some(2_000_000_100));
    // Replayed later, the saves stamp the recorded dates; an action stamps the time it plays.
    let mut r = Session::new();
    for (id, p) in &s.journal {
        r.execute(id, p).unwrap();
    }
    assert_eq!(r.doc().unwrap().doc.metadata, s.doc().unwrap().doc.metadata);
    assert!(s.journal_for_action(0).iter().all(|(_, p)| p.get("modified").is_none() && p.get("created").is_none()));
}

/// The document `s` ends on after replaying the journal of `s` in a new session.
fn replayed(s: &Session) -> Session {
    let mut r = Session::new();
    for (id, p) in &s.journal {
        r.execute(id, p).unwrap();
    }
    r
}

#[test]
fn a_save_in_a_batch_records_its_date_and_replays_it() {
    let path = vectorcraft_testkit::temp_dir("journal-batch").join("a.vectorcraft").to_string_lossy().into_owned();
    let mut s = Session::new();
    s.execute("file.new", &json!({"created": 1_000_000_000})).unwrap();
    let rect = json!({"x": 10, "y": 10, "width": 50, "height": 40});
    let steps = json!([{"command": "shape.rectangle", "params": rect}, {"command": "document.save", "params": {"path": path}}]);
    s.execute("command.batch", &json!({ "commands": steps })).unwrap();
    let modified = s.doc().unwrap().doc.metadata.modified;
    assert!(modified.is_some(), "the save in the batch dates the document");
    // The step's entry records the date it used; steps that take nothing from elsewhere stay as given.
    let (id, p) = s.journal.last().unwrap().clone();
    assert_eq!(id, "command.batch");
    assert_eq!(p["commands"][0], steps[0]);
    assert_eq!(p["commands"][1]["params"], json!({"path": path, "modified": modified}));
    // As if the original run happened at another time: the replay lands on the recorded date.
    s.journal.last_mut().unwrap().1["commands"][1]["params"]["modified"] = json!(1_234_567_890);
    let r = replayed(&s);
    assert_eq!(r.doc().unwrap().doc.metadata.modified, Some(1_234_567_890));
    assert_eq!(r.doc().unwrap().doc.metadata.created, Some(1_000_000_000));
    assert_eq!(r.doc().unwrap().doc.node_count(), s.doc().unwrap().doc.node_count());
    // An action leaves the step's date out, so playing it later dates the document then.
    let action = s.journal_for_action(0);
    assert_eq!(action[1].1["commands"][1]["params"], json!({ "path": path }));
    assert_eq!(action[1].1["commands"][0], steps[0]);
}

#[test]
fn a_batch_step_records_what_it_took_from_the_preferences() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"created": null})).unwrap();
    s.prefs.scale_strokes = true;
    let steps = json!([
        {"command": "shape.rectangle", "params": {"x": 10, "y": 10, "width": 50, "height": 40}},
        {"command": "stroke.set", "params": {"weight": 2}},
        {"command": "object.scale", "params": {"sx": 300}},
    ]);
    s.execute("command.batch", &json!({ "commands": steps })).unwrap();
    let p = &s.journal.last().unwrap().1;
    assert_eq!(p["commands"][2]["params"], json!({"sx": 300, "strokes": true, "corners": false}));
    // Replayed with other preferences, the scale still scales the stroke.
    let r = replayed(&s);
    assert!(!r.prefs.scale_strokes);
    let doc = |s: &Session| serde_json::to_value(&*s.doc().unwrap().doc).unwrap();
    assert_eq!(doc(&r), doc(&s));
}
