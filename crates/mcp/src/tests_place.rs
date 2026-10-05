//! File → Place over MCP, headless: `file.place` through `run_command`.

use serde_json::{Value, json};

use crate::{Backend, Headless, call_tool};

fn text(r: &crate::ToolResult) -> Value {
    assert!(!r.is_error, "{r:?}");
    serde_json::from_str(r.content[0]["text"].as_str().unwrap()).unwrap()
}

/// A headless session with a 400×300 document, and a 30×20 PNG as base64.
fn setup() -> (Headless, String) {
    let mut h = Headless::new();
    h.call("engine.execute", json!({"command": "file.new", "params": {"width": 30, "height": 20}})).unwrap();
    let png = h.call("engine.execute", json!({"command": "document.serialize", "params": {"format": "png"}})).unwrap();
    h.call("engine.execute", json!({"command": "file.new", "params": {"width": 400, "height": 300}})).unwrap();
    (h, png["dataBase64"].as_str().unwrap().to_string())
}

fn images(h: &Headless) -> usize {
    let mut n = 0;
    h.session.doc().unwrap().doc.walk(|x| n += usize::from(matches!(x.kind, vectorcraft_doc::NodeKind::Image(_))));
    n
}

#[test]
fn headless_run_command_file_place() {
    let (mut h, png) = setup();
    let r = text(&call_tool(
        &mut h,
        "run_command",
        &json!({"command": "file.place", "params": {"name": "tile.png", "dataBase64": png, "at": [100, 50]}}),
    ));
    assert_eq!((r["format"].as_str(), r["width"].as_f64(), r["height"].as_f64()), (Some("png"), Some(30.0), Some(20.0)), "{r}");
    assert_eq!(images(&h), 1);
    let sel = h.session.doc().unwrap().selection.objects.clone();
    assert_eq!(sel.len(), 1);
    assert_eq!(sel[0].0, r["ids"][0].as_u64().unwrap());
}
