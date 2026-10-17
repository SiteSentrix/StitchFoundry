//! Ask About Embroidery (Help and Embroidery menus): a window where the user asks Claude about
//! machine embroidery or about StitchFoundry, with their own Anthropic API key (the one Design
//! from Prompt keeps). The conversation goes along with each question, and, unless the user turns
//! it off, a short summary of the design open now ([`design_summary`]). The request goes out on a
//! thread of its own through the host's `http_post` service; [`poll`] picks the answer up.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use egui::RichText;
use serde_json::{Value, json};
use vectorcraft_embroidery::MM_PER_PT;
use vectorcraft_embroidery::sf_core::ai::ArtError;
use vectorcraft_embroidery::sf_core::ai::ask::{self, Answer, AskRequest, Turn};

use super::prompt;
use crate::VectorcraftApp;
use crate::theme::Tokens;

/// The most selected objects the summary lists.
const MAX_LISTED: usize = 12;
/// The most Check Design findings the summary lists.
const MAX_FINDINGS: usize = 12;

/// Ask About Embroidery between frames. Not saved: a conversation ends with the app.
pub struct AskState {
    pub open: bool,
    /// The question being typed.
    pub input: String,
    /// The questions asked and their answers, oldest first.
    pub turns: Vec<Turn>,
    /// Send a summary of the design open now with each question.
    pub include_design: bool,
    /// Keep the API key once Anthropic takes it.
    pub remember: bool,
    /// Show the API key row (it shows by itself while there is no key).
    pub show_key: bool,
    pending: Option<Pending>,
    /// Why the last question got no answer.
    pub error: Option<String>,
}

impl Default for AskState {
    fn default() -> Self {
        Self {
            open: false,
            input: String::new(),
            turns: Vec::new(),
            include_design: true,
            remember: true,
            show_key: false,
            pending: None,
            error: None,
        }
    }
}

struct Pending {
    rx: Receiver<Result<Answer, ArtError>>,
    question: String,
    /// The key sent, kept once Anthropic takes it.
    keep: Option<String>,
    /// When it was sent (egui time, seconds).
    since: f64,
}

impl AskState {
    /// Whether a question is on its way.
    pub fn asking(&self) -> bool {
        self.pending.is_some()
    }
}

/// Open the window (`ui.askAboutEmbroidery`); with `{question}`, ask it at once.
pub fn open(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    if !prompt::available(app) {
        return Err("Ask About Embroidery needs the desktop app for now".into());
    }
    prompt::load_key(app);
    app.embroidery.ask.open = true;
    let mut asking = false;
    if let Some(q) = p.get("question").and_then(Value::as_str).filter(|q| !q.trim().is_empty()) {
        app.embroidery.ask.input = q.to_string();
        send(app)?;
        asking = true;
    }
    Ok(json!({ "open": true, "asking": asking }))
}

/// A short plain-text summary of the design open now, for the model: its size, stitches, threads,
/// hoop, machine, what Check Design finds and what is selected. `None` without a document.
pub fn design_summary(app: &mut VectorcraftApp) -> Option<String> {
    app.session.active()?;
    let info = app.session.execute("embroidery.info", &json!({})).ok()?;
    let num = |k: &str| info.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let text = |k: &str| info.get(k).and_then(Value::as_str).map(str::to_string);
    let mut lines = Vec::new();
    if num("stitches") > 0.0 {
        lines.push(format!(
            "Size {:.0} × {:.0} mm; {} stitched objects; {} stitches; {} trims, {} jumps, {} colour changes, {} stops; about {} to sew.",
            num("widthMm"),
            num("heightMm"),
            num("objects"),
            num("stitches"),
            num("trims"),
            num("jumps"),
            num("colorChanges"),
            num("stops"),
            text("time").unwrap_or_default(),
        ));
        let threads: Vec<String> = info
            .get("colors")
            .and_then(Value::as_array)
            .map(|cs| {
                cs.iter()
                    .map(|c| {
                        let name = c.get("name").and_then(Value::as_str).filter(|n| !n.is_empty());
                        let hex = c.get("color").and_then(Value::as_str).unwrap_or("?");
                        name.map_or_else(|| hex.to_string(), |n| format!("{n} ({hex})"))
                    })
                    .collect()
            })
            .unwrap_or_default();
        if !threads.is_empty() {
            lines.push(format!("Threads in sewing order: {}.", threads.join(", ")));
        }
    } else {
        lines.push("Nothing is stitched yet.".to_string());
    }
    let mut setup = Vec::new();
    if let Some(h) = text("hoop") {
        let fits = match info.get("fits").and_then(Value::as_bool) {
            Some(false) => ", and the design does not fit it",
            _ => "",
        };
        setup.push(format!("hoop {h}{fits}"));
    }
    if let Some(f) = text("fabric") {
        setup.push(format!("fabric {f}"));
    }
    if let Some(m) = text("machine") {
        setup.push(format!("machine {m}"));
    }
    if let Some(f) = text("machineFormat") {
        setup.push(format!("saves as {f}"));
    }
    if !setup.is_empty() {
        lines.push(format!("Set up: {}.", setup.join("; ")));
    }
    if let Ok(check) = app.session.execute("embroidery.check", &json!({})) {
        let findings: Vec<String> = check
            .get("findings")
            .and_then(Value::as_array)
            .map(|fs| fs.iter().filter_map(|f| f.get("message").and_then(Value::as_str)).take(MAX_FINDINGS).map(|m| format!("- {m}")).collect())
            .unwrap_or_default();
        if findings.is_empty() {
            lines.push("Check Design finds nothing.".to_string());
        } else {
            lines.push("Check Design finds:".to_string());
            lines.extend(findings);
        }
    }
    let st = app.session.active()?;
    let selected: Vec<String> = st
        .selection
        .objects
        .iter()
        .filter_map(|id| st.doc.node(*id))
        .take(MAX_LISTED)
        .map(|n| {
            let stitch = n.embroidery.as_deref().map_or("no stitches", |e| e.stitch.label());
            let size = n.geometric_bounds().map(|b| format!(", {:.0} × {:.0} mm", b.width() * MM_PER_PT, b.height() * MM_PER_PT)).unwrap_or_default();
            format!("- {}: {stitch}{size}", n.display_name())
        })
        .collect();
    if !selected.is_empty() {
        lines.push(format!("Selected ({}):", st.selection.objects.len()));
        lines.extend(selected);
    }
    Some(lines.join("\n"))
}

/// Send the question typed (Ask). It stays in the box until the answer comes.
pub fn send(app: &mut VectorcraftApp) -> Result<(), String> {
    if app.embroidery.ask.pending.is_some() {
        return Err("already asking: wait for the answer".into());
    }
    let design = if app.embroidery.ask.include_design { design_summary(app) } else { None };
    let st = &app.embroidery.ask;
    let request = AskRequest::new(&st.input, &st.turns, design);
    let http = match request.http(&app.embroidery.prompt.key) {
        Ok(h) => h,
        Err(e) => {
            let m = e.to_string();
            app.embroidery.ask.show_key |= matches!(e, ArtError::NoKey | ArtError::BadKey);
            app.embroidery.ask.error = Some(m.clone());
            return Err(m);
        }
    };
    let post = app.services.http_post.clone().ok_or("Ask About Embroidery needs the desktop app for now")?;
    let keep = app.embroidery.ask.remember.then(|| app.embroidery.prompt.key.trim().to_string());
    if keep.is_none() {
        prompt::keep_key(app.services.secrets.as_deref(), None);
    }
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("ask-about-embroidery".into())
        .spawn(move || {
            let r = vectorcraft_engine::guard::catch_panic(|| post(&http))
                .unwrap_or_else(|msg| Err(format!("internal error: {msg} (please report this bug)")))
                .map_err(ArtError::Network)
                .and_then(|(status, body)| ask::parse_answer(status, &body));
            // Nothing waits any more once the conversation was cleared.
            let _ = tx.send(r);
        })
        .map_err(|e| format!("can't send the question: {e}"))?;
    let since = app.embroidery.prompt.now;
    let st = &mut app.embroidery.ask;
    st.error = None;
    st.pending = Some(Pending { rx, question: request.question, keep, since });
    Ok(())
}

/// Each frame: take in the answer that arrived, or why none did.
pub fn poll(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let Some(p) = app.embroidery.ask.pending.as_ref() else { return };
    let r = match p.rx.try_recv() {
        Ok(r) => r,
        Err(TryRecvError::Empty) => {
            // The thread doesn't wake the window: look again shortly.
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }
        Err(TryRecvError::Disconnected) => Err(ArtError::Network("the request stopped".into())),
    };
    let Some(p) = app.embroidery.ask.pending.take() else { return };
    if let Some(key) = p.keep.as_deref().filter(|_| !matches!(r, Err(ArtError::Key(_)))) {
        prompt::keep_key(app.services.secrets.as_deref(), Some(key));
    }
    let st = &mut app.embroidery.ask;
    match r {
        Ok(a) => {
            st.turns.push(Turn { question: p.question, answer: a.text });
            st.input.clear();
        }
        Err(e) => {
            st.show_key |= matches!(e, ArtError::Key(_));
            st.error = Some(e.to_string());
        }
    }
}

/// Start a new conversation (New Conversation): the answers so far, and any question on its way,
/// are dropped.
pub fn clear(app: &mut VectorcraftApp) {
    let st = &mut app.embroidery.ask;
    st.turns.clear();
    st.pending = None;
    st.error = None;
}

/// The window, while open.
pub fn window(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !app.embroidery.ask.open {
        return;
    }
    let t = Tokens::get(ctx);
    let now = ctx.input(|i| i.time);
    app.embroidery.prompt.now = now;
    let mut open = true;
    let mut ask_now = false;
    let mut new_conversation = false;
    egui::Window::new(tl!("Ask About Embroidery"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([460.0, 520.0])
        .show(ctx, |ui| {
            let st = &app.embroidery.ask;
            let footer = 190.0;
            egui::ScrollArea::vertical()
                .id_salt("ask-conversation")
                .max_height((ui.available_height() - footer).max(120.0))
                .stick_to_bottom(true)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if st.turns.is_empty() && st.pending.is_none() {
                        ui.label(RichText::new(tl!("Ask anything about machine embroidery or StitchFoundry: which stabilizer to use, why thread breaks, how to make letters sew cleaner, where a feature is.")).color(t.text_dim));
                    }
                    for turn in &st.turns {
                        ui.add_space(6.0);
                        ui.label(RichText::new(&turn.question).strong());
                        ui.add_space(2.0);
                        ui.label(&turn.answer);
                    }
                    if let Some(p) = &st.pending {
                        ui.add_space(6.0);
                        ui.label(RichText::new(&p.question).strong());
                        ui.horizontal(|ui| {
                            ui.add(egui::Spinner::new());
                            ui.label(format!("{} {:.0} s", tl!("Thinking…"), (now - p.since).max(0.0)));
                        });
                    }
                });
            ui.separator();
            if let Some(e) = &st.error {
                ui.label(RichText::new(crate::i18n::msg(e)).color(t.error));
            }
            let st = &mut app.embroidery.ask;
            let r = ui.add(
                egui::TextEdit::multiline(&mut st.input)
                    .desired_rows(2)
                    .desired_width(f32::INFINITY)
                    .hint_text(tl!("Type your question, then press Enter")),
            );
            // Enter asks; Shift+Enter starts a new line.
            if r.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift) {
                st.input = st.input.trim_end_matches('\n').to_string();
                ask_now = true;
            }
            ui.horizontal(|ui| {
                if crate::widgets::primary_button(ui, tl!("Ask")).clicked() {
                    ask_now = true;
                }
                if ui.button(tl!("New Conversation")).clicked() {
                    new_conversation = true;
                }
            });
            ui.checkbox(&mut st.include_design, tl!("Include my design"));
            let show_key = st.show_key || app.embroidery.prompt.key.trim().is_empty();
            if show_key {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(tl!("Anthropic API key:")).color(t.text_dim));
                    ui.add(egui::TextEdit::singleline(&mut app.embroidery.prompt.key).password(true).desired_width(220.0).hint_text("sk-ant-…"));
                });
                ui.checkbox(&mut app.embroidery.ask.remember, tl!("Remember the key on this computer"));
            }
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(tl!("Answers come from Claude with your own Anthropic API key, billed to your Anthropic account. They can be wrong: try a test sew before an important piece.")).size(11.0).color(t.text_dim));
                if show_key {
                    if ui.link(RichText::new(tl!("Get a key")).size(11.0)).clicked() {
                        app.open_url(prompt::KEY_URL);
                    }
                } else if ui.link(RichText::new(tl!("Change key")).size(11.0)).clicked() {
                    app.embroidery.ask.show_key = true;
                }
            });
        });
    if new_conversation {
        clear(app);
    }
    if ask_now {
        // (a failure is shown in the window)
        let _ = send(app);
    }
    if !open {
        app.embroidery.ask.open = false;
    }
}
