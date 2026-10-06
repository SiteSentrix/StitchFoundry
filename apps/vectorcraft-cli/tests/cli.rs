// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::{Value, json};

const BIN: &str = env!("CARGO_BIN_EXE_vectorcraft-cli");

fn tmp(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("vectorcraft-cli-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

#[test]
fn commands_prints_catalogue() {
    let out = Command::new(BIN).arg("commands").output().unwrap();
    assert!(out.status.success());
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    let ids: Vec<&str> = v.as_array().unwrap().iter().filter_map(|c| c["id"].as_str()).collect();
    assert!(ids.contains(&"shape.rectangle") && ids.contains(&"object.group") && ids.contains(&"file.export"));
}

#[test]
fn run_batch_exports() {
    let svg = tmp("batch.svg");
    let png = tmp("batch.png");
    let dc = tmp("batch.vectorcraft");
    let out = Command::new(BIN)
        .args(["run", "--cmd", "file.new", "--params", r#"{"width":200,"height":100}"#])
        .args(["--cmd", "shape.ellipse", "--params", r#"{"x":10,"y":10,"width":80,"height":60}"#])
        .args(["--cmd", "paint.setFill", "--params", r##"{"color":"#3366ff"}"##])
        .args(["--export", svg.to_str().unwrap(), "--export", png.to_str().unwrap(), "--export", dc.to_str().unwrap(), "--scale", "2"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let lines: Vec<Value> = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(lines.len(), 6);
    assert!(std::fs::read_to_string(&svg).unwrap().contains("3366ff"));
    let png_bytes = std::fs::read(&png).unwrap();
    assert_eq!(&png_bytes[..4], b"\x89PNG");
    // 200×100 pt at scale 2 → 400×200 px (IHDR width/height).
    assert_eq!(u32::from_be_bytes(png_bytes[16..20].try_into().unwrap()), 400);

    // Re-open the native file and inspect.
    let out = Command::new(BIN).args(["run", "--in", dc.to_str().unwrap(), "--cmd", "document.inspect"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let last: Value = serde_json::from_str(String::from_utf8(out.stdout).unwrap().lines().last().unwrap()).unwrap();
    assert_eq!(last["result"]["artboards"][0]["width"], 200.0);
}

#[test]
fn run_reports_errors() {
    let out = Command::new(BIN).args(["run", "--cmd", "nope.nothing"]).output().unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("nope.nothing"));
    let out = Command::new(BIN).args(["run", "--params", "{}"]).output().unwrap();
    assert!(!out.status.success());
}

#[test]
fn mcp_headless_over_stdio() {
    let mut child = Command::new(BIN).args(["mcp", "--headless"]).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    {
        let mut stdin = child.stdin.take().unwrap();
        for m in [
            json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}),
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}),
            json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"draw_shape","arguments":{"shape":"polygon","cx":100,"cy":100,"radius":50,"sides":5,"fill":"#00aa00"}}}),
            json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"screenshot","arguments":{"scale":0.25}}}),
        ] {
            writeln!(stdin, "{m}").unwrap();
        }
    }
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success());
    let replies: Vec<Value> = String::from_utf8(out.stdout).unwrap().lines().map(|l| serde_json::from_str(l).unwrap()).collect();
    assert_eq!(replies.iter().map(|r| r["id"].as_u64().unwrap()).collect::<Vec<_>>(), [1, 2, 3, 4]);
    assert_eq!(replies[0]["result"]["serverInfo"]["name"], "vectorcraft");
    assert_eq!(replies[2]["result"]["isError"], false);
    assert_eq!(replies[3]["result"]["content"][0]["type"], "image");
}

/// `vectorcraft-cli commands | head -1` panicked with "failed printing to
/// stdout: Broken pipe (os error 32)" and exit status 101.
#[test]
fn closed_stdout_ends_quietly() {
    use std::io::Read;
    let mut child = Command::new(BIN).arg("commands").stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let mut first = [0u8; 16];
    child.stdout.as_mut().unwrap().read_exact(&mut first).unwrap();
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success() && !err.contains("panicked"), "{:?}: {err}", out.status);
}
