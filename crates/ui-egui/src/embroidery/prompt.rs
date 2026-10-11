//! Design from Prompt (Embroidery menu): describe a design and Claude draws flat-colour artwork
//! for it, which `embroidery.placeArt` places and digitizes. The request goes out with the user's
//! own Anthropic API key through the host's `http_post` service, on a thread of its own, while the
//! dialog shows it drawing; [`poll`] picks the answer up on a later frame.

use std::sync::mpsc::{Receiver, TryRecvError, channel};

use serde_json::{Value, json};
use vectorcraft_embroidery::sf_core::ai::{Art, ArtError, ArtRequest};

use crate::VectorcraftApp;
use crate::state::Dialog;

/// The dialog kind.
pub const KIND: &str = "designFromPrompt";
/// The secret the API key is kept as.
pub const KEY_NAME: &str = "anthropic-api-key";
/// Where to get a key.
pub const KEY_URL: &str = "https://console.anthropic.com/settings/keys";

/// Design from Prompt between frames. Not saved: the key is kept by the host's secret store.
#[derive(Default)]
pub struct PromptState {
    /// The API key as typed or loaded. Never in the dialog's fields, which agents can read.
    pub key: String,
    /// Whether the secret store was read for the key.
    loaded: bool,
    /// The request on its way.
    pending: Option<Pending>,
    /// Why the last request drew nothing, shown in the dialog.
    pub error: Option<String>,
    /// The time of the last frame drawn (egui time, seconds).
    pub now: f64,
}

struct Pending {
    rx: Receiver<Result<Art, ArtError>>,
    /// The key sent, kept once Anthropic takes it (Remember the key).
    keep: Option<String>,
    /// The placed art's name (the description) and size (mm).
    name: String,
    size: f64,
    /// When it was sent (egui time, seconds).
    since: f64,
}

impl PromptState {
    /// Seconds since the request on its way was sent.
    pub fn drawing_for(&self, now: f64) -> Option<f64> {
        self.pending.as_ref().map(|p| (now - p.since).max(0.0))
    }
}

/// Whether this build can send the request (the web can't yet).
pub fn available(app: &VectorcraftApp) -> bool {
    app.services.http_post.is_some()
}

/// The size new art is drawn at by default (mm): 90% of the hoop's shorter side, else 100.
fn default_size(app: &VectorcraftApp) -> f64 {
    let hoop = app.session.active().and_then(|st| st.doc.embroidery.as_deref().and_then(|e| e.hoop.clone()));
    hoop.map_or(100.0, |h| (0.9 * h.width.min(h.height)).round())
}

/// Open the dialog, its fields from `p` (`{prompt?, size?, colors?}`).
pub fn open(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    if !available(app) {
        return Err("Design from Prompt needs the desktop app for now".into());
    }
    if app.session.active().is_none() {
        return Err("no document".into());
    }
    let st = &mut app.embroidery.prompt;
    if !st.loaded {
        st.loaded = true;
        if let Some(k) = app.services.secrets.as_ref().and_then(|s| s.load(KEY_NAME)) {
            st.key = k;
        }
    }
    let size = p.get("size").and_then(Value::as_f64).unwrap_or_else(|| default_size(app));
    let fields = json!({
        "prompt": p.get("prompt").and_then(Value::as_str).unwrap_or(""),
        "size": size,
        "colors": p.get("colors").and_then(Value::as_u64).unwrap_or(6),
        "remember": true,
    });
    app.ui.dialog = Some(Dialog::new(KIND, fields));
    Ok(json!({ "open": true }))
}

/// Send the dialog's request (Generate). The dialog stays open, showing that it is drawing.
pub fn start(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let st = &mut app.embroidery.prompt;
    let now = st.now;
    if st.pending.is_some() {
        return Err("already drawing: wait for it, or Cancel".into());
    }
    let size = d.f64("size", 100.0);
    let request = ArtRequest::new(&d.str("prompt"), size, d.f64("colors", 6.0).round().clamp(0.0, 99.0) as u32);
    let http = request.http(&st.key).map_err(|e| {
        let m = e.to_string();
        st.error = Some(m.clone());
        m
    })?;
    let post = app.services.http_post.clone().ok_or("Design from Prompt needs the desktop app for now")?;
    // Remember off forgets a kept key now; on, the key is kept once Anthropic takes it ([`poll`]).
    let keep = d.bool("remember").then(|| st.key.trim().to_string());
    if keep.is_none() {
        keep_key(app.services.secrets.as_deref(), None);
    }
    let st = &mut app.embroidery.prompt;
    let (tx, rx) = channel();
    std::thread::Builder::new()
        .name("design-from-prompt".into())
        .spawn(move || {
            let r = vectorcraft_engine::guard::catch_panic(|| post(&http))
                .unwrap_or_else(|msg| Err(format!("internal error: {msg} (please report this bug)")))
                .map_err(ArtError::Network)
                .and_then(|(status, body)| vectorcraft_embroidery::sf_core::ai::parse_reply(status, &body));
            // Nothing waits any more once the dialog was cancelled.
            let _ = tx.send(r);
        })
        .map_err(|e| format!("can't send the request: {e}"))?;
    let name: String = request.prompt.chars().take(60).collect();
    st.error = None;
    st.pending = Some(Pending { rx, keep, name, size: request.size_mm, since: now });
    Ok(json!({ "drawing": true }))
}

/// Keep `key` as the API key (`None`: forget it). A store that can't only costs the convenience.
fn keep_key(store: Option<&dyn crate::SecretStore>, key: Option<&str>) {
    if let Some(Err(e)) = store.map(|s| s.save(KEY_NAME, key)) {
        log::warn!("Design from Prompt: the API key: {e}");
    }
}

/// Stop waiting for the request on its way (Cancel). The service may still bill it.
pub fn cancel(app: &mut VectorcraftApp) {
    app.embroidery.prompt.pending = None;
}

/// Each frame: place the art that arrived, or show why none did.
pub fn poll(app: &mut VectorcraftApp, ctx: &egui::Context) {
    app.embroidery.prompt.now = ctx.input(|i| i.time);
    let Some(p) = app.embroidery.prompt.pending.as_ref() else { return };
    let r = match p.rx.try_recv() {
        Ok(r) => r,
        Err(TryRecvError::Empty) => {
            // The thread doesn't wake the window: look again shortly.
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
            return;
        }
        Err(TryRecvError::Disconnected) => Err(ArtError::Network("the request stopped".into())),
    };
    let Some(p) = app.embroidery.prompt.pending.take() else { return };
    // Anthropic took the key unless it said otherwise.
    if let Some(key) = p.keep.as_deref().filter(|_| !matches!(r, Err(ArtError::Key(_)))) {
        keep_key(app.services.secrets.as_deref(), Some(key));
    }
    let open = app.ui.dialog.as_ref().is_some_and(|d| d.kind == KIND);
    let placed = r.map_err(|e| e.to_string()).and_then(|art| {
        if app.session.active().is_none() {
            return Err("no document".to_string());
        }
        app.run("embroidery.placeArt", json!({ "svg": art.svg, "size": p.size, "name": p.name }))
    });
    match placed {
        Ok(v) => {
            if open {
                app.ui.dialog = None;
            }
            let n = v.get("objects").and_then(Value::as_u64).unwrap_or(0);
            app.status(crate::i18n::tn(n, "Design placed: {n} stitched object", "Design placed: {n} stitched objects"));
        }
        Err(e) => {
            if !open {
                app.status(crate::i18n::msg(&e));
            }
            app.embroidery.prompt.error = Some(e);
        }
    }
}
