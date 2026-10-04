//! File I/O through MCP: every readable format opens headless.

use serde_json::{Value, json};

use crate::{Backend, Headless, call_tool, tool_definitions};

fn tmp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("vectorcraft-mcp-fileio-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().to_string()
}

fn text(r: &crate::ToolResult) -> Value {
    assert!(!r.is_error, "{r:?}");
    serde_json::from_str(r.content[0]["text"].as_str().unwrap()).unwrap()
}

/// A headless session with two artboards of different widths (100 and 40 pt) and a rectangle.
fn two_boards() -> Headless {
    let mut h = Headless::new();
    h.call("engine.execute", json!({"command": "file.new", "params": {"width": 100, "height": 50, "artboards": 2}})).unwrap();
    h.call("engine.execute", json!({"command": "artboard.setProps", "params": {"index": 1, "width": 40}})).unwrap();
    h.call("engine.execute", json!({"command": "shape.rectangle", "params": {"x": 10, "y": 10, "width": 20, "height": 20}})).unwrap();
    h
}

#[test]
fn headless_opens_pdf_ai_and_templates() {
    let mut h = two_boards();
    let pdf = h.call("engine.execute", json!({"command": "document.serialize", "params": {"format": "pdf"}})).unwrap();
    let pdf = vectorcraft_format::base64_decode(pdf["dataBase64"].as_str().unwrap()).unwrap();
    for name in ["x.pdf", "x.ai"] {
        let path = tmp(name);
        std::fs::write(&path, &pdf).unwrap();
        let r = text(&call_tool(&mut h, "open_file", &json!({"path": path})));
        assert_eq!(r["title"], name);
        assert_eq!(h.session.doc().unwrap().doc.artboards.len(), 2, "one artboard per page");
    }
    let ait = tmp("x.ait");
    std::fs::write(&ait, &pdf).unwrap();
    let r = text(&call_tool(&mut h, "open_file", &json!({"path": ait})));
    assert!(r["title"].as_str().unwrap().starts_with("Untitled-"), "{r}");
    let tpl = tmp("t.vectorcraft");
    h.call("engine.execute", json!({"command": "file.saveAsTemplate", "params": {"path": tpl}})).unwrap();
    let r = text(&call_tool(&mut h, "open_file", &json!({"path": tpl})));
    assert!(r["title"].as_str().unwrap().starts_with("Untitled-"), "{r}");
    assert_eq!(h.session.doc().unwrap().path, None, "Save asks for a new name");
}

#[test]
fn open_file_lists_the_engine_formats() {
    let tools = tool_definitions();
    let open = tools.iter().find(|t| t["name"] == "open_file").unwrap();
    assert!(open["description"].as_str().unwrap().contains(".ait"));
}
