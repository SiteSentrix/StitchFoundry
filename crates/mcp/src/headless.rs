//! An in-process engine session that answers the control-channel methods itself.

use serde_json::{Value, json};
use vectorcraft_engine::{Session, UiRequest, ViewInfo};
use vectorcraft_geom::Point;
use vectorcraft_render::Renderer;
use vectorcraft_tools::{Mods, PointerEvent, PointerKind, TOOL_GROUPS, ToolKey};

use crate::backend::Backend;

/// Headless backend: a [`Session`] plus a CPU [`Renderer`] for screenshots and PNG export.
pub struct Headless {
    pub session: Session,
    pub renderer: Renderer,
    /// View state handed to tools (zoom 1, smart guides on).
    pub view: ViewInfo,
}

impl Default for Headless {
    fn default() -> Self {
        Self::new()
    }
}

/// Commands the desktop app adds on top of the engine; the headless backend implements them too so
/// `run_command` behaves the same in both modes.
const HOST_COMMANDS: &[(&str, &str, &str)] = &[
    ("file.open", "Open…", "{path} open a .vectorcraft or .svg file as a new document"),
    ("file.save", "Save", "{path?} save as .vectorcraft (default: the document's path)"),
    ("file.saveAs", "Save As…", "{path}"),
    ("file.export", "Export…", "{format?: svg|png|vectorcraft, path, scale?, artboard?}"),
    ("tool.select", "Select Tool", "{tool} e.g. selection, directSelection, pen, rectangle, ellipse, polygon, star, lineSegment"),
];

fn s<'a>(p: &'a Value, k: &str) -> Option<&'a str> {
    p.get(k).and_then(Value::as_str)
}

fn ext_of(path: &str) -> String {
    std::path::Path::new(path).extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

fn file_name(path: &str) -> String {
    std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| path.to_string())
}

pub(crate) fn pointer_kind(k: &str) -> Option<PointerKind> {
    Some(match k {
        "down" => PointerKind::Down,
        "drag" => PointerKind::Drag,
        "up" => PointerKind::Up,
        "move" => PointerKind::Move,
        "doubleclick" | "dblclick" => PointerKind::DoubleClick,
        _ => return None,
    })
}

fn tool_key(name: &str) -> Option<ToolKey> {
    Some(match name.to_ascii_lowercase().as_str() {
        "enter" | "return" => ToolKey::Enter,
        "esc" | "escape" => ToolKey::Escape,
        "backspace" => ToolKey::Backspace,
        "delete" => ToolKey::Delete,
        "up" | "arrowup" => ToolKey::Up,
        "down" | "arrowdown" => ToolKey::Down,
        "left" | "arrowleft" => ToolKey::Left,
        "right" | "arrowright" => ToolKey::Right,
        "[" | "bracketleft" | "openbracket" => ToolKey::BracketLeft,
        "]" | "bracketright" | "closebracket" => ToolKey::BracketRight,
        "tab" => ToolKey::Tab,
        _ => return None,
    })
}

/// Does a shortcut like `Cmd+Shift+]` match `key` + `mods`?
fn shortcut_matches(shortcut: &str, key: &str, mods: Mods) -> bool {
    let parts: Vec<&str> = shortcut.split('+').collect();
    let Some((k, ms)) = parts.split_last() else { return false };
    let has = |m: &str| ms.iter().any(|x| x.eq_ignore_ascii_case(m));
    k.eq_ignore_ascii_case(key)
        && has("cmd") == mods.cmd
        && has("shift") == mods.shift
        && (has("alt") || has("option")) == mods.alt
        && has("ctrl") == mods.ctrl
}

impl Headless {
    /// A session with no document (the first `file.new` / `app.open` creates one).
    pub fn new() -> Self {
        Self { session: Session::new(), renderer: Renderer::new(), view: ViewInfo::default() }
    }

    /// A session with a fresh default (Letter) document, ready to draw into.
    pub fn with_document() -> Self {
        let mut h = Self::new();
        h.ensure_document();
        h
    }

    /// Create a default document when none is open.
    pub fn ensure_document(&mut self) {
        if self.session.active().is_none()
            && let Err(e) = self.session.execute("file.new", &json!({}))
        {
            log::error!("file.new failed: {e}");
        }
    }

    fn exec(&mut self, id: &str, params: &Value) -> Result<Value, String> {
        match id {
            "file.open" => self.open(params),
            "file.save" | "file.saveAs" => self.save(params),
            "file.export" => self.export(params),
            "tool.select" => self.select_tool(params),
            _ => self.session.execute(id, params).map_err(|e| e.to_string()),
        }
    }

    fn commands(&self) -> Value {
        let mut v: Vec<Value> = self.session.commands().into_iter().map(|c| serde_json::to_value(c).unwrap_or_default()).collect();
        for (id, label, params) in HOST_COMMANDS {
            v.push(json!({"id": id, "label": label, "menu": [], "shortcut": null, "params": params, "enabled": true, "host": true}));
        }
        Value::Array(v)
    }

    fn select_tool(&mut self, p: &Value) -> Result<Value, String> {
        let t = s(p, "tool").ok_or("missing `tool`")?;
        if vectorcraft_tools::tool_info(t).is_none() {
            return Err(format!("unknown tool `{t}` (see ui.tool.list)"));
        }
        self.session.select_tool(t, self.view).map_err(|e| e.to_string())?;
        Ok(json!({"tool": self.session.tool_id()}))
    }

    fn apply_ui_requests(&mut self, reqs: Vec<UiRequest>, out: &mut Vec<Value>) -> Result<(), String> {
        for r in reqs {
            match r {
                UiRequest::SwitchTool(t) => {
                    self.session.select_tool(&t, self.view).map_err(|e| e.to_string())?;
                }
                UiRequest::Dialog(k, p) => out.push(json!({"dialog": k, "params": p})),
            }
        }
        Ok(())
    }

    fn pointer(&mut self, p: &Value) -> Result<Value, String> {
        let events = p.get("events").and_then(Value::as_array).ok_or("missing `events`")?;
        let base: Mods = p.get("mods").and_then(|m| serde_json::from_value(m.clone()).ok()).unwrap_or_default();
        self.ensure_document();
        let mut requests = vec![];
        for e in events {
            let kind = s(e, "kind").unwrap_or("");
            let kind = pointer_kind(kind).ok_or_else(|| format!("unknown pointer kind `{kind}` (down|drag|up|move|doubleclick)"))?;
            if s(e, "space") == Some("screen") {
                return Err("screen-space pointer events need the desktop app; use document coordinates".into());
            }
            let x = e.get("x").and_then(Value::as_f64).ok_or("pointer event needs numeric `x`")?;
            let y = e.get("y").and_then(Value::as_f64).ok_or("pointer event needs numeric `y`")?;
            let mods = e.get("mods").and_then(|m| serde_json::from_value(m.clone()).ok()).unwrap_or(base);
            let ev = PointerEvent { kind, pos: Point::new(x, y), mods, pressure: 1.0 };
            let reqs = self.session.pointer(&ev, self.view).map_err(|e| e.to_string())?;
            self.apply_ui_requests(reqs, &mut requests)?;
        }
        let sel = self.session.active().map(|d| d.selection.objects.iter().map(|i| i.0).collect::<Vec<_>>()).unwrap_or_default();
        Ok(json!({"selection": sel, "tool": self.session.tool_id(), "requests": requests}))
    }

    fn key(&mut self, p: &Value) -> Result<Value, String> {
        let key = s(p, "key").ok_or("missing `key`")?;
        let b = |n: &str| p.get(n).and_then(Value::as_bool).unwrap_or(false);
        let mods = Mods { shift: b("shift"), alt: b("alt"), cmd: b("cmd"), ctrl: b("ctrl"), ..Mods::default() };
        let tk = tool_key(key);
        // A busy tool (pen path in progress, shape being dragged) gets its keys first.
        if let Some(k) = tk
            && self.session.tool_busy()
        {
            let mut out = vec![];
            let reqs = self.session.tool_key(k, mods, self.view).map_err(|e| e.to_string())?;
            self.apply_ui_requests(reqs, &mut out)?;
            return Ok(json!({"handledBy": "tool", "requests": out}));
        }
        if let Some(c) = vectorcraft_engine::command_specs().iter().find(|c| c.shortcut.is_some_and(|sc| shortcut_matches(sc, key, mods))) {
            let r = self.exec(c.id, &json!({}))?;
            return Ok(json!({"handledBy": "command", "command": c.id, "result": r}));
        }
        if let Some(t) = TOOL_GROUPS.iter().flat_map(|g| g.iter()).find(|t| t.shortcut.is_some_and(|sc| shortcut_matches(sc, key, mods))) {
            self.session.select_tool(t.id, self.view).map_err(|e| e.to_string())?;
            return Ok(json!({"handledBy": "tool.select", "tool": t.id}));
        }
        if let Some(k) = tk {
            let mut out = vec![];
            let reqs = self.session.tool_key(k, mods, self.view).map_err(|e| e.to_string())?;
            self.apply_ui_requests(reqs, &mut out)?;
            return Ok(json!({"handledBy": "tool", "requests": out}));
        }
        Err(format!("no headless binding for key `{key}`"))
    }

    fn render(&mut self, p: &Value) -> Result<Value, String> {
        let st = self.session.active().ok_or("no document")?;
        let doc = st.doc.clone();
        let idx = p.get("artboard").and_then(Value::as_u64).unwrap_or(0) as usize;
        let r = doc.artboards.get(idx).map(|a| a.rect).ok_or("no such artboard")?;
        let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.01, 16.0);
        let img = self.renderer.render_region(&doc, r, scale, true);
        let png = img.to_png();
        match s(p, "path") {
            Some(path) => {
                std::fs::write(path, &png).map_err(|e| format!("write {path}: {e}"))?;
                Ok(json!({"path": path, "width": img.width, "height": img.height}))
            }
            None => Ok(json!({"width": img.width, "height": img.height, "pngBase64": vectorcraft_format::base64_encode(&png)})),
        }
    }

    /// `app.open {path}`: `.vectorcraft`, `.svg`, `.pdf` or PDF-compatible `.ai` as a new active document.
    pub fn open(&mut self, p: &Value) -> Result<Value, String> {
        let path = s(p, "path").ok_or("missing `path`")?;
        let bytes = std::fs::read(path).map_err(|e| format!("read {path}: {e}"))?;
        let name = file_name(path);
        let e = ext_of(path);
        let mut warnings = vec![];
        let (doc, keep_path) = if vectorcraft_format::is_native_ext(&e) || vectorcraft_format::sniff(&bytes) {
            let mut d = vectorcraft_format::load(&bytes).map_err(|e| e.to_string())?;
            if d.title.is_empty() {
                d.title = name.clone();
            }
            (d, true)
        } else if e == "svg" || bytes.starts_with(b"<?xml") || bytes.starts_with(b"<svg") {
            let text = std::str::from_utf8(&bytes).map_err(|_| "SVG is not UTF-8".to_string())?;
            let (mut d, w) = vectorcraft_svg::import_with_report(text).map_err(|e| e.to_string())?;
            d.title = name.clone();
            warnings = w;
            (d, false)
        } else if e == "pdf" || e == "ai" || bytes.starts_with(b"%PDF") {
            // PDF, or the PDF-compatible part of an Illustrator `.ai`, as in the app's File ▸ Open.
            let r = vectorcraft_pdf::import_with_report(&bytes, &vectorcraft_pdf::ImportOptions::default()).map_err(|e| e.to_string())?;
            let mut d = r.document;
            d.title = name.clone();
            warnings = r.warnings;
            (d, false)
        } else {
            return Err(format!("headless mode can open .vectorcraft, .svg, .pdf and .ai files, not .{e}"));
        };
        let index = self.session.add_document(doc, keep_path.then(|| path.to_string()));
        Ok(json!({"index": index, "title": name, "warnings": warnings}))
    }

    /// `app.save {path?}`: native format; remembers the path and clears the dirty flag.
    pub fn save(&mut self, p: &Value) -> Result<Value, String> {
        let st = self.session.active().ok_or("no document")?;
        let path = s(p, "path").map(str::to_string).or_else(|| st.path.clone()).ok_or("missing `path` (document was never saved)")?;
        let bytes = vectorcraft_format::save_file(&st.doc);
        std::fs::write(&path, bytes).map_err(|e| format!("write {path}: {e}"))?;
        if let Some(st) = self.session.active_mut() {
            st.path = Some(path.clone());
            st.mark_saved();
        }
        Ok(json!({"path": path}))
    }

    /// `app.export {format?, path, scale?, artboard?}`: svg | png | pdf | jpg | webp | vectorcraft (format defaults to the extension).
    pub fn export(&mut self, p: &Value) -> Result<Value, String> {
        let path = s(p, "path").ok_or("missing `path`")?;
        let fmt = s(p, "format").map(str::to_ascii_lowercase).unwrap_or_else(|| ext_of(path));
        let st = self.session.active().ok_or("no document")?;
        let doc = st.doc.clone();
        let bytes = match fmt.as_str() {
            "svg" => {
                let ab = p.get("artboard").and_then(Value::as_u64).unwrap_or(0) as usize;
                vectorcraft_svg::export(&doc, &vectorcraft_svg::ExportOptions { artboard: Some(ab), ..Default::default() }).into_bytes()
            }
            "png" => {
                let idx = p.get("artboard").and_then(Value::as_u64).unwrap_or(0) as usize;
                let r = doc.artboards.get(idx).map(|a| a.rect).ok_or("no such artboard")?;
                let scale = p.get("scale").and_then(Value::as_f64).unwrap_or(1.0).clamp(0.01, 16.0);
                self.renderer.render_region(&doc, r, scale, false).to_png()
            }
            "vectorcraft" => vectorcraft_format::save_file(&doc),
            // pdf, jpg, webp…: the engine's exporter (same bytes as the app).
            other => {
                let params = json!({"path": path, "format": other, "scale": p.get("scale"), "artboard": p.get("artboard")});
                let r = self.session.execute("document.export", &params).map_err(|e| e.to_string())?;
                return Ok(json!({"path": path, "format": fmt, "bytes": r["bytes"]}));
            }
        };
        std::fs::write(path, &bytes).map_err(|e| format!("write {path}: {e}"))?;
        Ok(json!({"path": path, "format": fmt, "bytes": bytes.len()}))
    }
}

impl Backend for Headless {
    fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let p = &params;
        match method {
            "engine.execute" | "ui.menu.invoke" | "command" => {
                let id = s(p, "command").or(s(p, "id")).ok_or("missing `command`")?.to_string();
                let params = match p.get("params") {
                    None | Some(Value::Null) => json!({}),
                    Some(v) => v.clone(),
                };
                self.exec(&id, &params)
            }
            "engine.commands" => Ok(self.commands()),
            "document.inspect" => self.exec("document.inspect", &json!({})),
            "document.node" => self.exec("document.node", p),
            "document.json" => self.exec("document.json", &json!({})),
            "ui.tool.select" => self.select_tool(p),
            "ui.tool.list" => Ok(serde_json::to_value(TOOL_GROUPS).unwrap_or_default()),
            "ui.pointer" => self.pointer(p),
            "ui.key" => self.key(p),
            "ui.render" | "ui.screenshot" => self.render(p),
            "app.open" => self.open(p),
            "app.save" => self.save(p),
            "app.export" => self.export(p),
            "app.quit" => Ok(Value::Null),
            m if m.starts_with("ui.") => {
                Err(format!("`{m}` needs the desktop app (start `vectorcraft --control 7979` and use `vectorcraft-cli mcp --connect`)"))
            }
            other => Err(format!("unknown method `{other}`")),
        }
    }

    fn has_ui(&self) -> bool {
        false
    }

    fn describe(&self) -> String {
        "headless".into()
    }
}
