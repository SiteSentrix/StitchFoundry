//! Keyboard shortcuts: command shortcuts from the registry, single-key tool shortcuts, arrows,
//! and tool keys (Enter/Esc/↑/↓ while drawing).

use egui::{Key, KeyboardShortcut, Modifiers};
use serde_json::json;
use vectorcraft_tools::{Mods, ToolKey};

use crate::VectorcraftApp;

/// Parse "Cmd+Shift+]" into an egui shortcut. `Cmd` is Command on macOS and Ctrl elsewhere.
/// `+` and `=` name one chord key ([`Key::Equals`]): `+` is Shift+`=` on many layouts, so a
/// `Cmd+=` chord also answers `+` however it is typed (see [`consume`]).
pub fn parse(s: &str) -> Option<KeyboardShortcut> {
    if s.is_empty() {
        return None;
    }
    let (mods_part, key_part) =
        if let Some(stripped) = s.strip_suffix("++") { (stripped.trim_end_matches('+'), "+") } else { s.rsplit_once('+').unwrap_or(("", s)) };
    let mut m = Modifiers::NONE;
    for part in mods_part.split('+').filter(|p| !p.is_empty()) {
        match part {
            "Cmd" => m |= Modifiers::COMMAND,
            "Shift" => m |= Modifiers::SHIFT,
            "Alt" => m |= Modifiers::ALT,
            "Ctrl" => m |= Modifiers::CTRL,
            _ => return None,
        }
    }
    let key = match key_part {
        "]" => Key::CloseBracket,
        "[" => Key::OpenBracket,
        ";" => Key::Semicolon,
        "'" => Key::Quote,
        "/" => Key::Slash,
        "\\" => Key::Backslash,
        "=" | "+" => Key::Equals,
        "-" => Key::Minus,
        "Delete" => Key::Delete,
        "Backspace" => Key::Backspace,
        "Tab" => Key::Tab,
        "~" => Key::Backtick,
        k => Key::from_name(k)?,
    };
    Some(KeyboardShortcut::new(m, key))
}

/// Every command shortcut in effect (user overrides from Edit → Keyboard Shortcuts win), with the
/// command's params: `{}`, or `{panel}` for the panel shortcuts (`window.panel`).
pub(crate) fn all_shortcuts() -> Vec<(KeyboardShortcut, &'static str, serde_json::Value)> {
    let mut v = vec![];
    for c in vectorcraft_engine::command_specs() {
        if let Some(sc) = crate::menus::shortcut_of(c.id).and_then(parse) {
            v.push((sc, c.id, json!({})));
        }
    }
    for c in crate::menus::UI_COMMANDS {
        if let Some(sc) = crate::menus::shortcut_of(c.0).and_then(parse) {
            v.push((sc, c.0, json!({})));
        }
    }
    for (panel, _, _) in crate::state::ICON_PANELS {
        if let Some(sc) = crate::shortcut_editor::panel_shortcut(panel).and_then(parse) {
            v.push((sc, "window.panel", json!({ "panel": panel })));
        }
    }
    // Most specific (most modifiers) first so Cmd+Shift+Z isn't eaten by Cmd+Z.
    v.sort_by_key(|(sc, ..)| {
        std::cmp::Reverse(sc.modifiers.shift as u8 + sc.modifiers.alt as u8 + sc.modifiers.command as u8 + sc.modifiers.ctrl as u8)
    });
    v
}

/// Consume a press of `sc`. A `=` chord also takes [`Key::Plus`], which is how `+` arrives from
/// the numpad and from layouts where it has its own key (Shift+`=` arrives as either; extra Shift
/// is ignored). `native`: the system menu already handles the chord as written, so only that
/// alias is left to match here.
pub(crate) fn consume(i: &mut egui::InputState, sc: &KeyboardShortcut, native: bool) -> bool {
    (!native && i.consume_shortcut(sc))
        || (sc.logical_key == Key::Equals && i.consume_key(sc.modifiers, Key::Plus))
        || (sc.modifiers.shift && shifted(sc.logical_key).is_some_and(|k| i.consume_key(sc.modifiers, k)))
}

/// The key a shifted punctuation key arrives as: egui reports the character typed, so Cmd+Shift+[
/// comes in as Cmd+Shift+`{` (US-style layouts).
fn shifted(k: Key) -> Option<Key> {
    Some(match k {
        Key::OpenBracket => Key::OpenCurlyBracket,
        Key::CloseBracket => Key::CloseCurlyBracket,
        Key::Slash => Key::Questionmark,
        Key::Semicolon => Key::Colon,
        Key::Backslash => Key::Pipe,
        _ => return None,
    })
}

/// Keys that paste with Cmd, or alone. (Shift+Insert is left out: Ctrl+Insert copies.)
const PASTE_KEYS: [Key; 2] = [Key::V, Key::Paste];

/// The paste command whose V chord `held` holds (Paste in Place for Cmd+Shift+V by default), else
/// Paste: egui reports every Cmd+V chord as a paste, never as its key.
pub(crate) fn paste_command(held: Modifiers) -> &'static str {
    all_shortcuts()
        .into_iter()
        .find(|(sc, id, _)| sc.logical_key == Key::V && id.starts_with("edit.paste") && held.matches_logically(sc.modifiers))
        .map_or("edit.paste", |(_, id, _)| id)
}

/// Keyboard pastes of something other than text. egui swallows a paste chord's key press and
/// sends a Paste event only when the clipboard holds text, so a paste key released without its
/// press (or a Paste event) reported was a paste of a bitmap or a PDF.
#[derive(Default)]
pub(crate) struct PasteChord {
    /// Paste keys whose press was reported (typed, not pasting).
    pressed: Vec<Key>,
    /// A Paste event came since the last paste key was released.
    pasted: bool,
}

impl PasteChord {
    /// Follow one frame's `events` → the modifiers of a paste egui sent no event for (the chord's,
    /// as its key is released).
    pub(crate) fn textless_paste(&mut self, events: &[egui::Event]) -> Option<Modifiers> {
        let mut fire = None;
        for e in events {
            match e {
                egui::Event::Paste(_) => self.pasted = true,
                // A release that comes while the window is away is never seen.
                egui::Event::WindowFocused(false) => *self = Self::default(),
                egui::Event::Key { key, pressed: true, .. } if PASTE_KEYS.contains(key) && !self.pressed.contains(key) => self.pressed.push(*key),
                egui::Event::Key { key, pressed: false, modifiers, .. } if PASTE_KEYS.contains(key) => {
                    match self.pressed.iter().position(|k| k == key) {
                        Some(i) => {
                            self.pressed.swap_remove(i);
                        }
                        // Unless a Paste event came (the guard takes it either way).
                        None if !std::mem::take(&mut self.pasted) => fire = Some(*modifiers),
                        None => {}
                    }
                }
                _ => {}
            }
        }
        fire
    }
}

/// End an IME composition outside the IME (a click away, the IME going away): the
/// marked text stays as typed, and the system IME is told to drop it.
pub(crate) fn keep_marked_text(app: &mut VectorcraftApp) {
    let Some(m) = app.ime_marked.take() else { return };
    app.ime_discard = true;
    if app.session.tool_composing() {
        // Errors are already reported by the engine's interaction.
        let _ = app.session.tool_text(&m, app.view_info());
    }
}

/// Typed text and IME events, in the order they came, to the Type tool.
fn type_text(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let view = app.view_info();
    // Switching apps mid-composition needs nothing: the macOS IME keeps its marked text and goes
    // on with it when the window comes back (measured with Kotoeri), as the tool does.
    let events: Vec<egui::Event> =
        ctx.input(|i| i.events.iter().filter(|e| matches!(e, egui::Event::Text(_) | egui::Event::Ime(_))).cloned().collect());
    for e in events {
        // Errors are already reported by the engine's interaction (a failed preview keeps it).
        let _ = match e {
            // Keys the IME passed through come as text only when nothing is marked.
            egui::Event::Text(t) if !app.session.tool_composing() => app.session.tool_text(&t, view),
            egui::Event::Ime(egui::ImeEvent::Preedit { text, active_range_chars }) => {
                app.ime_marked = Some(text.clone()).filter(|t| !t.is_empty());
                app.session.tool_preedit(&text, active_range_chars, view)
            }
            // A bare line break confirms the composition (Enter): the marked text, not a newline.
            egui::Event::Ime(egui::ImeEvent::Commit(t)) if t == "\n" || t == "\r" => {
                keep_marked_text(app);
                Ok(vec![])
            }
            egui::Event::Ime(egui::ImeEvent::Commit(t)) => {
                app.ime_marked = None;
                app.session.tool_text(&t, view)
            }
            // `DeleteSurrounding` isn't sent by egui-winit (desktop); the web host has no IME
            // path to the canvas yet.
            _ => Ok(vec![]),
        };
    }
}

pub fn handle(app: &mut VectorcraftApp, ctx: &egui::Context) {
    // Followed before anything returns, so a key typed in a field isn't taken for a paste.
    let textless_paste = ctx.input(|i| app.paste_chord.textless_paste(&i.events));
    if app.ui.dialog.is_some() || app.ui.palette_open {
        if crate::shortcut_editor::is_recording(app) {
            return;
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            app.ui.dialog = None;
            app.ui.palette_open = false;
        }
        return;
    }
    let typing = ctx.egui_wants_keyboard_input();
    let view = app.view_info();
    // While an IME composes, Enter and Escape are its own (confirm / cancel).
    let composing = app.session.tool_composing();
    // Tool keys first (Enter/Escape end paths; arrows change polygon sides while dragging).
    let busy = app.session.tool_busy();
    for (k, tk) in [(Key::Enter, ToolKey::Enter), (Key::Escape, ToolKey::Escape)] {
        if !typing && !composing && ctx.input(|i| i.key_pressed(k)) {
            // A key the tool claims (Esc with a loaded place cursor) is only the tool's.
            let claimed = app.session.tool_claims_key(tk, view);
            let r = app.session.tool_key(tk, Mods::default(), view);
            if claimed {
                crate::canvas::apply_requests(app, r);
            }
            if k == Key::Escape && !busy && !claimed {
                if app.session.active().is_some_and(|d| d.doc.pattern_edit.is_some()) {
                    let _ = app.run("object.pattern.done", json!({}));
                } else if app.session.active().is_some_and(|d| d.isolation.is_some()) {
                    let _ = app.run("object.exitIsolation", json!({}));
                } else if app.ui.flyout.is_some() {
                    app.ui.flyout = None;
                }
            }
        }
    }
    if typing {
        return;
    }
    // Type tool editing: text, IME composition and editing keys go to the tool.
    if app.session.tool_wants_text() {
        type_text(app, ctx);
        // Keys of a frame that began composing are the IME's too (a Backspace that empties the
        // marked text must not delete the committed character before it).
        if composing || app.session.tool_composing() {
            // The IME owns the keyboard until it commits: no editing keys, clipboard or shortcuts.
            ctx.input_mut(|i| {
                i.events.retain(|e| !matches!(e, egui::Event::Key { .. } | egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)))
            });
            return;
        }
        // Editing keys with modifiers, clipboard and Cmd+A.
        crate::panels::character::route_type_input(app, ctx);
        // Enter was already delivered above as ToolKey::Enter (newline).
        let fire = all_shortcuts().into_iter().filter(|(sc, ..)| sc.modifiers.command).find(|(sc, ..)| ctx.input_mut(|i| consume(i, sc, false)));
        if let Some((_, id, p)) = fire {
            crate::menus::invoke(app, id, p);
        }
        return;
    }
    // Clipboard keys arrive as events, not key presses (except where the native menu has them);
    // the chord held says which paste.
    let mut clip = vec![];
    ctx.input_mut(|i| {
        let held = i.modifiers;
        i.events.retain(|e| {
            let (id, text) = match e {
                egui::Event::Copy => ("edit.copy", None),
                egui::Event::Cut => ("edit.cut", None),
                egui::Event::Paste(t) => (paste_command(held), Some(t.clone())),
                _ => return true,
            };
            if app.native_shortcuts.contains(id) {
                return true;
            }
            clip.push((id, text));
            false
        })
    });
    // Only the system clipboard service reads what isn't text (the native menu has its own Paste).
    if let Some(id) = textless_paste.map(paste_command)
        && app.services.system_clipboard.is_some()
        && !app.native_shortcuts.contains(id)
    {
        clip.push((id, None));
    }
    for (id, text) in clip {
        app.clipboard_in = text;
        crate::menus::invoke(app, id, json!({}));
    }
    if busy {
        for (k, tk) in [(Key::ArrowUp, ToolKey::Up), (Key::ArrowDown, ToolKey::Down)] {
            if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, k)) {
                let _ = app.session.tool_key(tk, Mods::default(), view);
            }
        }
        // Digits (5 while dragging with the Perspective Selection tool), once per press.
        let digits: Vec<u8> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, repeat: false, .. } => digit_of(*key),
                    _ => None,
                })
                .collect()
        });
        for d in digits {
            let _ = app.session.tool_key(ToolKey::Digit(d), Mods::default(), view);
        }
        return;
    }
    // Keys the active tool claims ahead of their shortcuts (the Gradient tool's selected stop:
    // Delete/Backspace remove it, ←/→ nudge it; the loaded place cursor: the arrows cycle files).
    let m = ctx.input(|i| i.modifiers);
    for (k, tk) in [
        (Key::Delete, ToolKey::Delete),
        (Key::Backspace, ToolKey::Backspace),
        (Key::ArrowLeft, ToolKey::Left),
        (Key::ArrowRight, ToolKey::Right),
        (Key::ArrowUp, ToolKey::Up),
        (Key::ArrowDown, ToolKey::Down),
    ] {
        if ctx.input(|i| i.key_pressed(k)) && app.session.tool_claims_key(tk, view) && ctx.input_mut(|i| i.consume_key(m, k)) {
            let r = app.session.tool_key(tk, crate::canvas::mods(m, false), view);
            crate::canvas::apply_requests(app, r);
            return;
        }
    }
    // Digits the active tool or the perspective grid takes (1–4 pick the plane while it shows).
    if !(m.command || m.alt || m.ctrl) {
        let digits: Vec<(u8, Key)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, .. } => digit_of(*key).map(|d| (d, *key)),
                    _ => None,
                })
                .collect()
        });
        for (d, k) in digits {
            if app.session.tool_claims_key(ToolKey::Digit(d), view) && ctx.input_mut(|i| i.consume_key(m, k)) {
                // The digit's text is the key's too: no single-key shortcut sees it.
                let text = d.to_string();
                ctx.input_mut(|i| i.events.retain(|e| !matches!(e, egui::Event::Text(t) if *t == text)));
                let r = app.session.tool_key(ToolKey::Digit(d), crate::canvas::mods(m, false), view);
                crate::canvas::apply_requests(app, r);
                return;
            }
        }
    }
    // Command shortcuts.
    let mut fire = None;
    for (sc, id, p) in all_shortcuts() {
        // Letter / punctuation keys without Cmd/Alt/Ctrl are handled below as text (tool shortcuts,
        // X, D, /, `,` and `.`).
        let plain = !(sc.modifiers.command || sc.modifiers.alt || sc.modifiers.ctrl);
        if plain && (sc.logical_key.name().len() == 1 || matches!(sc.logical_key, Key::Slash | Key::Comma | Key::Period)) {
            continue;
        }
        let native = app.native_shortcuts.contains(id);
        if ctx.input_mut(|i| consume(i, &sc, native)) {
            fire = Some((id, p));
            break;
        }
    }
    if let Some((id, p)) = fire {
        crate::menus::invoke(app, id, p);
        return;
    }
    // Arrow nudges (Shift = ×10, Alt = copy).
    let arrows = [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)];
    for (k, dx, dy) in arrows {
        let m = ctx.input(|i| i.modifiers);
        if ctx.input_mut(|i| i.consume_key(m, k)) && app.session.active().is_some_and(|d| !d.selection.is_empty()) {
            let _ = app.run("object.nudge", json!({"dx": dx, "dy": dy, "big": m.shift, "copy": m.alt}));
        }
    }
    // Delete / Backspace clear the selection.
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Backspace)) && app.session.active().is_some_and(|d| !d.selection.is_empty()) {
        let _ = app.run("edit.clear", json!({}));
    }
    // Single-key tool shortcuts (no Cmd/Ctrl/Alt).
    let events: Vec<(String, Modifiers)> = ctx.input(|i| {
        i.events
            .iter()
            .filter_map(|e| match e {
                egui::Event::Text(t) => Some((t.clone(), i.modifiers)),
                _ => None,
            })
            .collect()
    });
    for (text, m) in events {
        if m.command || m.alt || m.ctrl {
            continue;
        }
        let upper = text.to_uppercase();
        let key = if m.shift && text.chars().all(|c| c.is_alphabetic()) { format!("Shift+{upper}") } else { upper.clone() };
        // A shifted punctuation character can also be written with its Shift (the Curvature tool's
        // Shift+~ arrives as `~`).
        let shifted = m.shift.then(|| format!("Shift+{text}"));
        let lookup = |find: fn(&str) -> Option<&'static str>| find(&key).or_else(|| find(&text)).or_else(|| shifted.as_deref().and_then(find));
        // Single-key command shortcuts (X, Shift+X, D, /, Shift+D, F, Shift+F by default).
        if let Some(id) = lookup(crate::shortcut_editor::command_for_key) {
            let _ = app.run(id, json!({}));
            continue;
        }
        if let Some(t) = lookup(crate::shortcut_editor::tool_for_key) {
            app.select_tool(t);
        }
    }
}

/// The digit a number-row or keypad key types.
fn digit_of(k: Key) -> Option<u8> {
    const DIGITS: [Key; 10] = [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    DIGITS.iter().position(|d| *d == k).map(|i| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One headless frame delivering `events` to the shortcut handler.
    fn frame(app: &mut VectorcraftApp, events: Vec<egui::Event>) -> egui::FullOutput {
        let ctx = egui::Context::default();
        let mut out = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
            handle(app, ui.ctx());
            app.logic(ui.ctx());
        });
        out.textures_delta.clear();
        out
    }

    #[test]
    fn copy_and_paste_events_use_the_system_clipboard() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        let id = app.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap()["id"].clone();
        app.session.execute("select.set", &json!({"ids": [id]})).unwrap();
        // Copy publishes SVG to the system clipboard (egui's CopyText output command).
        let out = frame(&mut app, vec![egui::Event::Copy]);
        let copied = out.platform_output.commands.iter().find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        });
        let svg = copied.expect("copy publishes SVG");
        assert!(svg.contains("<svg"));
        // Pasting our own SVG back uses the internal clipboard; foreign SVG replaces it.
        frame(&mut app, vec![egui::Event::Paste(svg)]);
        assert_eq!(app.session.doc().unwrap().doc.layers[0].children().unwrap().len(), 2);
        let foreign = r##"<svg xmlns="http://www.w3.org/2000/svg"><circle cx="5" cy="5" r="5"/><circle cx="20" cy="5" r="5"/><circle cx="35" cy="5" r="5"/></svg>"##;
        frame(&mut app, vec![egui::Event::Paste(foreign.into())]);
        assert_eq!(app.session.doc().unwrap().doc.layers[0].children().unwrap().len(), 5);
        // Plain text is not art: nothing is pasted from it (the internal clipboard is reused).
        frame(&mut app, vec![egui::Event::Paste("hello".into())]);
        assert_eq!(app.session.doc().unwrap().doc.layers[0].children().unwrap().len(), 8);
    }

    #[test]
    fn comma_and_period_reapply_the_last_colour_and_gradient() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        app.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 10, "height": 10})).unwrap();
        app.session.execute("paint.setFill", &json!({"gradient": {"kind": "linear"}})).unwrap();
        app.session.execute("paint.setFill", &json!({"color": "#336699"})).unwrap();
        let fill = |app: &VectorcraftApp| crate::panels::current_paints(app).0;
        frame(&mut app, vec![egui::Event::Text(".".into())]);
        assert!(matches!(fill(&app), vectorcraft_color::Paint::Gradient(_)), "`.` applies the last gradient");
        frame(&mut app, vec![egui::Event::Text(",".into())]);
        assert_eq!(fill(&app).color().unwrap().to_hex(), "#336699", "`,` applies the last colour");
    }

    #[test]
    fn shifted_punctuation_shortcuts_work_as_typed() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        let a = app.session.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 50, "height": 50})).unwrap()["id"].clone();
        let b = app.session.execute("shape.ellipse", &json!({"x": 20, "y": 20, "width": 50, "height": 50})).unwrap()["id"].clone();
        app.session.execute("select.set", &json!({"ids": [a]})).unwrap();
        let order =
            |app: &VectorcraftApp| -> Vec<u64> { app.session.doc().unwrap().doc.layers[0].children().unwrap().iter().map(|n| n.id.0).collect() };
        let (a, b) = (a.as_u64().unwrap(), b.as_u64().unwrap());
        assert_eq!(order(&app), [a, b]);
        let press = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        // Recorded in the Keyboard Shortcuts editor, they read as the keys (and so conflict).
        assert_eq!(crate::shortcut_editor::chord_from_event(Key::CloseCurlyBracket, cmd_shift).as_deref(), Some("Cmd+Shift+]"));
        assert_eq!(crate::shortcut_editor::chord_from_event(Key::Questionmark, cmd_shift).as_deref(), Some("Cmd+Shift+/"));
        // Cmd+Shift+] arrives as Cmd+Shift+`}`, Cmd+Shift+[ as Cmd+Shift+`{`.
        frame(&mut app, vec![press(Key::CloseCurlyBracket, cmd_shift)]);
        assert_eq!(order(&app), [b, a], "Bring to Front");
        frame(&mut app, vec![press(Key::OpenCurlyBracket, cmd_shift)]);
        assert_eq!(order(&app), [a, b], "Send to Back");
        // Cmd+Shift+/ arrives as Cmd+Shift+`?`: Search Commands.
        frame(&mut app, vec![press(Key::Questionmark, cmd_shift)]);
        assert!(app.ui.palette_open, "Search Commands");
        app.ui.palette_open = false;
        // The Curvature tool's Shift+~ arrives as the text `~` with Shift.
        frame(&mut app, vec![egui::Event::ModifiersChanged(Modifiers::SHIFT), egui::Event::Text("~".into())]);
        assert_eq!(app.session.tool_id(), "curvature");
    }

    #[test]
    fn plus_typed_any_way_zooms_in() {
        assert_eq!(parse("Cmd++"), parse("Cmd+="), "`+` and `=` are one chord key");
        assert_eq!(crate::shortcut_editor::chord_from_event(Key::Plus, Modifiers::COMMAND).as_deref(), Some("Cmd+="), "a recorded `+` too");
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
        let zoom = |app: &mut VectorcraftApp| app.view_mut().unwrap().zoom;
        let press = |key, modifiers| egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers };
        let cmd_shift = Modifiers::COMMAND | Modifiers::SHIFT;
        // `=`, Shift+`=` (reported as `=` or as `+`), and the numpad `+` / a layout's own `+` key.
        for (key, m) in [(Key::Equals, Modifiers::COMMAND), (Key::Equals, cmd_shift), (Key::Plus, cmd_shift), (Key::Plus, Modifiers::COMMAND)] {
            let before = zoom(&mut app);
            frame(&mut app, vec![press(key, m)]);
            assert!(zoom(&mut app) > before, "{key:?} with {m:?} zooms in");
        }
        let before = zoom(&mut app);
        frame(&mut app, vec![press(Key::Minus, Modifiers::COMMAND)]);
        assert!(zoom(&mut app) < before, "Cmd+- zooms out");
        // The system menu handles `Cmd+=` itself; the `+` key is still matched here.
        app.native_shortcuts.insert("view.zoomIn".into());
        let before = zoom(&mut app);
        frame(&mut app, vec![press(Key::Equals, Modifiers::COMMAND)]);
        assert_eq!(zoom(&mut app), before, "left to the system menu");
        frame(&mut app, vec![press(Key::Plus, Modifiers::COMMAND)]);
        assert!(zoom(&mut app) > before);
    }

    #[test]
    fn digit_keys_reach_a_dragging_tool_once_per_press() {
        use vectorcraft_tools::{PointerEvent, PointerKind};
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 800, "height": 600})).unwrap();
        app.session.execute("perspective.grid.preset", &json!({"kind": 2})).unwrap();
        let id = app.session.execute("shape.rectangle", &json!({"x": 450, "y": 380, "width": 60, "height": 60})).unwrap()["id"].clone();
        app.session.execute("perspective.attach", &json!({"ids": [id], "plane": "right"})).unwrap();
        app.select_tool("perspectiveSelection");
        let v = app.view_info();
        let c =
            app.session.doc().unwrap().doc.node(vectorcraft_engine::doc::NodeId(id.as_u64().unwrap())).unwrap().geometric_bounds().unwrap().center();
        app.session.pointer(&PointerEvent::new(PointerKind::Down, c.x, c.y), v).unwrap();
        app.session.pointer(&PointerEvent::new(PointerKind::Drag, c.x + 30.0, c.y), v).unwrap();
        let key = |repeat| egui::Event::Key { key: Key::Num5, physical_key: None, pressed: true, repeat, modifiers: Modifiers::NONE };
        // A held key repeats: only its first press toggles.
        frame(&mut app, vec![key(false), key(true)]);
        let preview = app.session.doc().unwrap().interaction.as_ref().unwrap().preview.clone().unwrap();
        assert_eq!((preview.0.as_str(), &preview.1["perpendicular"]), ("perspective.move", &json!(true)));
        assert_eq!(digit_of(Key::Num0), Some(0));
        assert_eq!(digit_of(Key::A), None);
    }

    #[test]
    fn parses() {
        let s = parse("Cmd+Shift+]").unwrap();
        assert_eq!(s.logical_key, Key::CloseBracket);
        assert!(s.modifiers.shift && s.modifiers.command);
        assert_eq!(parse("Cmd+=").unwrap().logical_key, Key::Equals);
        assert_eq!(parse("Cmd+Alt+2").unwrap().logical_key, Key::Num2);
        assert_eq!(parse("F12").unwrap().logical_key, Key::F12);
        assert!(parse("").is_none());
    }

    #[test]
    fn all_registered_shortcuts_parse() {
        for c in vectorcraft_engine::command_specs() {
            if let Some(s) = c.shortcut
                && s != "D"
                && s != "X"
                && s != "Shift+X"
                && s != "/"
            {
                assert!(parse(s).is_some(), "{} has unparsable shortcut {s}", c.id);
            }
        }
        for c in crate::menus::UI_COMMANDS {
            if !c.2.is_empty() && c.2 != "F" && c.2 != "Shift+F" {
                assert!(parse(c.2).is_some(), "{} has unparsable shortcut {}", c.0, c.2);
            }
        }
    }
}
