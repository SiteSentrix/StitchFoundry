//! MCP server properties (in-process, headless backend): tool schemas are valid, every tool
//! survives junk arguments, JSON-RPC framing always answers correctly, and random editing through
//! MCP keeps the document valid.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use proptest::prelude::*;
use serde_json::{Value, json};
use vectorcraft_doc::Document;
use vectorcraft_mcp::{Headless, Server, tool_definitions};
use vectorcraft_testkit::catch_quiet;
use vectorcraft_testkit::invariants::{check_document, doc_json};
use vectorcraft_testkit::strategies::{arb_ops, junk_values};

fn server() -> Server {
    Server::new(Box::new(Headless::with_document()))
}

fn rpc(s: &mut Server, id: u64, method: &str, params: Value) -> Value {
    let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}).to_string();
    let reply = s.handle_line(&line).expect("requests get a reply");
    let v: Value = serde_json::from_str(&reply).expect("reply is JSON");
    assert_eq!(v["jsonrpc"], "2.0");
    assert_eq!(v["id"], id);
    assert!(v.get("result").is_some() != v.get("error").is_some(), "exactly one of result/error: {v}");
    v
}

fn call(s: &mut Server, name: &str, args: Value) -> Value {
    rpc(s, 1, "tools/call", json!({"name": name, "arguments": args}))
}

fn document(s: &mut Server) -> Document {
    let v = rpc(s, 2, "resources/read", json!({"uri": "vectorcraft://document/json"}));
    let text = v["result"]["contents"][0]["text"].as_str().unwrap_or_else(|| panic!("{v}"));
    serde_json::from_str(text).expect("document JSON")
}

fn doc_uri_json(s: &mut Server) -> String {
    let list = rpc(s, 3, "resources/list", json!({}));
    list["result"]["resources"].as_array().unwrap().iter().find(|r| r["name"] == "document-json").unwrap()["uri"].as_str().unwrap().to_string()
}

/// Minimal JSON-schema sanity: an object schema with typed properties and consistent `required`.
fn validate_schema(name: &str, schema: &Value) {
    assert_eq!(schema["type"], "object", "{name}: inputSchema.type");
    let props = schema.get("properties").and_then(Value::as_object).unwrap_or_else(|| panic!("{name}: properties must be an object"));
    for (k, p) in props {
        let p = p.as_object().unwrap_or_else(|| panic!("{name}.{k}: property schema must be an object"));
        assert!(
            p.contains_key("type") || p.contains_key("anyOf") || p.contains_key("oneOf") || p.contains_key("enum") || p.contains_key("$ref"),
            "{name}.{k}: property has no type"
        );
        if let Some(t) = p.get("type") {
            let ok = |t: &str| ["object", "array", "string", "number", "integer", "boolean", "null"].contains(&t);
            match t {
                Value::String(t) => assert!(ok(t), "{name}.{k}: bad type {t}"),
                Value::Array(ts) => assert!(ts.iter().all(|t| t.as_str().is_some_and(ok)), "{name}.{k}: bad types"),
                _ => panic!("{name}.{k}: type must be a string"),
            }
            if t == "array" {
                assert!(p.get("items").is_none_or(Value::is_object), "{name}.{k}: items must be a schema");
            }
            if t == "object" {
                assert!(p.get("properties").is_none_or(Value::is_object), "{name}.{k}: nested properties");
            }
        }
        if let Some(e) = p.get("enum") {
            assert!(e.as_array().is_some_and(|a| !a.is_empty()), "{name}.{k}: empty enum");
        }
    }
    if let Some(req) = schema.get("required") {
        for r in req.as_array().unwrap_or_else(|| panic!("{name}: required must be an array")) {
            let r = r.as_str().unwrap();
            assert!(props.contains_key(r), "{name}: required `{r}` is not a property");
        }
    }
}

#[test]
fn tool_definitions_are_valid() {
    let tools = tool_definitions();
    assert!(tools.len() >= 15);
    let mut names = std::collections::HashSet::new();
    for t in &tools {
        let name = t["name"].as_str().expect("name");
        assert!(names.insert(name.to_string()), "duplicate tool {name}");
        assert!(name.chars().all(|c| c.is_ascii_lowercase() || c == '_'), "tool name {name}");
        assert!(t["description"].as_str().is_some_and(|d| d.len() >= 10), "{name}: description");
        assert!(t["annotations"]["readOnlyHint"].is_boolean(), "{name}: readOnlyHint");
        validate_schema(name, &t["inputSchema"]);
    }
    // tools/list over the protocol returns the same list.
    let mut s = server();
    let v = rpc(&mut s, 1, "tools/list", json!({}));
    assert_eq!(v["result"]["tools"].as_array().unwrap().len(), tools.len());
}

#[test]
fn every_tool_survives_empty_and_junk_arguments() {
    let dir = vectorcraft_testkit::temp_dir("mcp-junk");
    let safe = dir.join("junk.vectorcraft").to_string_lossy().to_string();
    let mut failures = vec![];
    for t in tool_definitions() {
        let name = t["name"].as_str().unwrap().to_string();
        let props: Vec<String> = t["inputSchema"]["properties"].as_object().unwrap().keys().cloned().collect();
        let mut cases = vec![json!({}), Value::Null, json!([]), json!("x"), json!(3)];
        for k in &props {
            for v in junk_values() {
                // Never let fuzzed strings name files; keep renders small.
                let v = if k == "path" {
                    json!(safe)
                } else if k == "scale" && v.as_f64().is_some_and(|x| x.abs() > 4.0) {
                    json!(4)
                } else {
                    v
                };
                cases.push(json!({ k.clone(): v }));
            }
        }
        for args in cases {
            let mut s = server();
            let r = catch_quiet(|| {
                let v = call(&mut s, &name, args.clone());
                if let Some(res) = v.get("result") {
                    assert!(res["content"].is_array(), "{name}: result has no content array: {v}");
                }
                check_document(&document(&mut s))
            });
            match r {
                Err(p) => failures.push(format!("PANIC {name} {args}: {p}")),
                Ok(Err(e)) => failures.push(format!("{name} {args}: {e}")),
                Ok(Ok(())) => {}
            }
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures.join("\n"));
}

#[test]
fn framing_never_panics_on_malformed_messages() {
    let mut s = server();
    for line in [
        "",
        "null",
        "[]",
        "[1,2,3]",
        "{}",
        "{\"jsonrpc\":\"2.0\",\"id\":{},\"method\":\"ping\"}",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":5}",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\"}",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"nope\"}}",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"run_command\",\"arguments\":[]}}",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"resources/read\",\"params\":{\"uri\":\"nope://x\"}}",
        "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"1900-01-01\"}}",
        "\u{0}\u{1}",
        "{\"jsonrpc\":\"2.0\",\"id\":1e400,\"method\":\"ping\"}",
    ] {
        let r = catch_quiet(|| s.handle_line(line));
        assert!(r.is_ok(), "panic on {line:?}");
        if let Ok(Some(reply)) = r {
            let v: Value = serde_json::from_str(&reply).unwrap_or_else(|e| panic!("{line:?} → invalid JSON reply {reply}: {e}"));
            assert!(v.is_object() || v.is_array());
        }
    }
    // Still alive afterwards.
    rpc(&mut s, 7, "ping", json!({}));
}

#[test]
fn resources_are_json() {
    let mut s = server();
    let uri = doc_uri_json(&mut s);
    let v = rpc(&mut s, 1, "resources/read", json!({"uri": uri}));
    let d: Document = serde_json::from_str(v["result"]["contents"][0]["text"].as_str().unwrap()).unwrap();
    check_document(&d).unwrap();
    let list = rpc(&mut s, 2, "resources/list", json!({}));
    for r in list["result"]["resources"].as_array().unwrap() {
        let v = rpc(&mut s, 3, "resources/read", json!({"uri": r["uri"]}));
        let text = v["result"]["contents"][0]["text"].as_str().unwrap();
        serde_json::from_str::<Value>(text).unwrap();
    }
}

#[test]
fn undo_redo_tools_restore_documents() {
    let mut s = server();
    let d0 = doc_json(&document(&mut s));
    let v = call(&mut s, "draw_shape", json!({"shape": "rectangle", "x": 10, "y": 10, "width": 50, "height": 40, "fill": "#ff0000"}));
    assert!(v["result"]["isError"] != json!(true), "{v}");
    call(&mut s, "draw_shape", json!({"shape": "star", "cx": 100, "cy": 100, "radius1": 40, "radius2": 20}));
    let d2 = doc_json(&document(&mut s));
    // draw + paint are separate undo steps; undo until the original document is back.
    let mut n = 0;
    while doc_json(&document(&mut s)) != d0 {
        let v = call(&mut s, "undo", json!({}));
        assert!(v["result"]["isError"] != json!(true), "undo failed early: {v}");
        n += 1;
        assert!(n < 20);
    }
    for _ in 0..n {
        call(&mut s, "redo", json!({}));
    }
    assert_eq!(doc_json(&document(&mut s)), d2);
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 24, failure_persistence: None, ..ProptestConfig::default() })]

    /// Random engine command sequences through `run_command` keep the document valid and the
    /// server responsive; the result equals running the same commands on a local session.
    #[test]
    fn run_command_sequences_match_local_session(ops in arb_ops(5..40)) {
        let mut srv = server();
        // Match the headless default document, dated as it is (File Info dates come from the clock).
        let d_remote = doc_json(&document(&mut srv));
        let mut local = vectorcraft_engine::Session::new();
        let created = &d_remote["metadata"]["created"];
        vectorcraft_testkit::fixtures::exec(&mut local, "file.new", json!({"width": 612, "height": 792, "created": created}));
        let d_local = doc_json(&local.doc().unwrap().doc);
        prop_assume!(d_remote == d_local);
        for op in &ops {
            let (id, params) = op.command(&local);
            let a = local.execute(&id, &params).is_ok();
            let v = call(&mut srv, "run_command", json!({"command": id, "params": params}));
            let b = v["result"]["isError"] != json!(true);
            prop_assert_eq!(a, b, "`{}` {}: local ok={} mcp ok={} ({})", id, params, a, b, v);
        }
        let got = document(&mut srv);
        check_document(&got).map_err(TestCaseError::fail)?;
        { let (g, l) = (doc_json(&got), doc_json(&local.doc().unwrap().doc)); // 1e-12: JSON text round trips aren't bit-exact (serde_json without float_roundtrip; see
        // crates/format/tests/prop_format.rs::bug_f64_bit_exact_roundtrip).
        prop_assert!(vectorcraft_testkit::invariants::json_approx_eq(&g, &l, 1e-12), "MCP and local sessions diverged: {}", vectorcraft_testkit::invariants::first_diff(&l, &g, "$")); }
    }
}

/// Tools driven directly against a Headless backend never leave an interaction open or corrupt
/// the session, whatever the arguments. (A gesture that stops after `drag` legitimately stays open
/// so an agent can continue it in the next call; that case is not listed.)
#[test]
fn tools_never_leave_interactions_open() {
    use vectorcraft_mcp::call_tool;
    use vectorcraft_testkit::invariants::check_session;
    let cases = [
        ("pointer_gesture", json!({"tool": "rectangle", "events": [{"kind": "down", "x": 10, "y": 10}, {"kind": "bogus", "x": 50, "y": 50}]})),
        ("pointer_gesture", json!({"tool": "pen", "events": [{"kind": "down", "x": 10, "y": 10}, {"kind": "drag", "x": "a", "y": 50}]})),
        (
            "pointer_gesture",
            json!({"tool": "selection", "events": [{"kind": "down", "x": 1e308, "y": -1e308}, {"kind": "drag", "x": 1e308, "y": 0}, {"kind": "up", "x": 0, "y": 0}]}),
        ),
        ("pointer_gesture", json!({"tool": "ellipse", "events": [{"kind": "up", "x": 0, "y": 0}]})),
        ("press_key", json!({"key": "Escape"})),
        (
            "run_command",
            json!({"command": "command.batch", "params": {"commands": [{"command": "shape.rectangle", "params": {"x": 0, "y": 0, "width": 5, "height": 5}}, {"command": "nope"}]}}),
        ),
    ];
    let mut failures = vec![];
    for (name, args) in cases {
        let mut h = Headless::with_document();
        let r = catch_quiet(|| call_tool(&mut h, name, &args));
        if r.is_err() {
            failures.push(format!("PANIC {name} {args}"));
            continue;
        }
        // A finished gesture sequence must not leave a drag half-applied.
        if h.session.in_interaction() {
            failures.push(format!("{name} {args}: interaction left open"));
        }
        if let Err(e) = check_session(&h.session) {
            failures.push(format!("{name} {args}: {e}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
