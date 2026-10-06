//! The Liquify tools through MCP: their options (`tool.setOption`, kept across tool switches) and
//! brush strokes with `pointer_gesture`.

use serde_json::{Value, json};

use crate::{Headless, call_tool};

fn ok(h: &mut Headless, name: &str, args: Value) -> Value {
    let r = call_tool(h, name, &args);
    assert!(!r.is_error, "{name} {args}: {r:?}");
    serde_json::from_str(r.content[0]["text"].as_str().unwrap()).unwrap_or(Value::Null)
}

fn command(h: &mut Headless, command: &str, params: Value) -> Value {
    ok(h, "run_command", json!({"command": command, "params": params}))
}

#[test]
fn tool_options_are_set_headless_and_kept_across_tool_switches() {
    let mut h = Headless::with_document();
    command(&mut h, "tool.select", json!({"tool": "warp"}));
    let o = command(&mut h, "tool.setOption", json!({"values": {"width": 64, "detail": 6}}));
    assert_eq!((o["width"].as_f64(), o["detail"].as_f64()), (Some(64.0), Some(6.0)));
    command(&mut h, "tool.select", json!({"tool": "bloat"}));
    let o = command(&mut h, "tool.setOption", json!({}));
    assert_eq!((o["tool"].as_str(), o["width"].as_f64(), o["detail"].as_f64()), (Some("bloat"), Some(64.0), Some(2.0)));
    command(&mut h, "tool.select", json!({"tool": "warp"}));
    assert_eq!(command(&mut h, "tool.setOption", json!({}))["detail"], json!(6.0));
}
