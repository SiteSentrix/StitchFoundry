//! VectorCraft's egui frontend: an Illustrator-style UI over `vectorcraft-engine`.
//!
//! The UI is thin: every action goes through [`VectorcraftApp::run`], which dispatches UI commands
//! (view/window) here and everything else to the engine. The same entry point serves menus,
//! shortcuts, the ⌘K palette and the control channel ([`control`]).
#![forbid(unsafe_code)]

mod brand;
pub mod canvas;
pub mod chrome;
pub mod community;
pub mod control;
pub mod cursors;
pub mod dialogs;
pub mod dock;
pub mod find_font;
pub mod icon_data;
pub mod icons;
pub mod io;
pub mod menus;
pub mod palette;
pub mod panels;
pub mod prefs_dialog;
pub mod render_worker;
pub mod shortcut_editor;
pub mod shortcuts;
pub mod state;
pub mod theme;
pub mod titlebar;
pub mod toolbar;
pub mod unsaved;
pub mod widgets;
pub mod workspaces;

#[cfg(test)]
mod tests_docsetup;
#[cfg(test)]
mod tests_labels;
#[cfg(test)]
mod tests_overprint;
#[cfg(test)]
mod tests_paintchips;
#[cfg(test)]
mod tests_recolor;
#[cfg(test)]
mod tests_svg;
#[cfg(test)]
mod tests_transparencygrid;

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use vectorcraft_engine::{Session, ViewInfo};

pub use control::{ControlRequest, ControlResponse};
pub use state::{UiState, View};

pub type PickSave = Box<dyn FnMut(&str) -> Option<String>>;
pub type ReadFn = Box<dyn Fn(&str) -> Result<Vec<u8>, String>>;
pub type WriteFn = Box<dyn FnMut(&str, &[u8]) -> Result<(), String>>;
pub type Inbox = Arc<Mutex<Vec<(String, Vec<u8>)>>>;
pub type DownloadFn = Box<dyn FnMut(&str, &[u8])>;

/// Opens a URL in the system browser.
pub type OpenUrlFn = Box<dyn FnMut(&str)>;

/// Platform services injected by the host app (desktop or web).
#[derive(Default)]
pub struct Services {
    /// Show an open dialog; returns a path.
    pub pick_open: Option<Box<dyn FnMut() -> Option<String>>>,
    /// Show a save dialog with a suggested file name; returns a path.
    pub pick_save: Option<PickSave>,
    pub read: Option<ReadFn>,
    pub write: Option<WriteFn>,
    /// Files that arrived asynchronously (web open / drops): (name, bytes).
    pub inbox: Option<Inbox>,
    /// Web: trigger a browser download instead of writing a path.
    pub download: Option<DownloadFn>,
    /// Web: start an async open (bytes arrive via `inbox`).
    pub open_async: Option<Box<dyn FnMut()>>,
    /// Read the system clipboard's text (desktop). Without it, pasted text only arrives with
    /// egui's Paste event (web, and keyboard paste everywhere).
    pub clipboard_read: Option<Box<dyn FnMut() -> Option<String>>>,
    /// Open a URL in the system browser (desktop). Without it, egui opens it (a new tab on the web).
    pub open_url: Option<OpenUrlFn>,
}

/// Cached canvas raster.
pub struct CanvasCache {
    pub renderer: vectorcraft_render::Renderer,
    pub texture: Option<egui::TextureHandle>,
    pub key: Option<CacheKey>,
    pub last_ms: f64,
    pub worker: Option<render_worker::Worker>,
    pub worker_started: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CacheKey {
    pub doc: usize,
    pub revision: u64,
    pub zoom: f64,
    pub cx: f64,
    pub cy: f64,
    pub w: u32,
    pub h: u32,
    pub outline: bool,
    pub trim: bool,
    pub ppp: f32,
    pub hidden: Vec<u64>,
    pub rot: f64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Perf {
    pub frame_ms: f64,
    pub render_ms: f64,
    pub fps: f64,
}

pub struct VectorcraftApp {
    pub session: Session,
    pub ui: UiState,
    pub views: Vec<View>,
    pub services: Services,
    pub canvas: CanvasCache,
    pub perf: Perf,
    /// macOS: draw our own title strip under the traffic lights.
    pub integrated_titlebar: bool,
    /// The host installed a native menu bar (macOS): don't draw in-window menus.
    pub native_menu: bool,
    /// Last applied effect (Effect → Apply Last Effect).
    pub last_effect: Option<(String, serde_json::Value)>,
    /// Commands whose shortcuts the native menu handles (skip them in egui to avoid double firing).
    pub native_shortcuts: std::collections::HashSet<String>,
    control_rx: Option<Receiver<ControlRequest>>,
    /// (token, path, reply, deadline ms): a window that isn't presented never delivers its frame.
    pending_screenshots: Vec<(u64, Option<String>, Sender<ControlResponse>, f64)>,
    queued_screenshots: Vec<(u64, f64, u32)>,
    screenshot_token: u64,
    /// Synthetic input events (from the control channel) injected one step per frame.
    pub synthetic: Vec<egui::Event>,
    styled: bool,
    fonts_ready: bool,
    frame: u64,
    last_time: f64,
    /// Canvas rect of the last frame (screen points), for control-channel coordinate mapping.
    pub canvas_rect: Option<egui::Rect>,
    /// Hover position in document coordinates.
    pub hover_doc: Option<vectorcraft_geom::Point>,
    /// System clipboard: SVG to publish next frame, the last SVG we published (so pasting it back
    /// uses the lossless internal clipboard) and text that arrived with a Paste event.
    clipboard_out: Option<String>,
    clipboard_published: Option<String>,
    pub(crate) clipboard_in: Option<String>,
    /// A URL to open through egui next frame (when the host has no `open_url` service).
    pending_url: Option<String>,
    /// Windows and Linux: the window has no OS decorations, so the app bar is the title bar (drag,
    /// double-click to maximize, caption buttons) and invisible edge zones resize the window.
    pub custom_titlebar: bool,
}

impl VectorcraftApp {
    pub fn new(session: Session, services: Services) -> Self {
        let views = session.documents().iter().map(|_| View::default()).collect();
        Self {
            session,
            ui: UiState::default(),
            views,
            services,
            canvas: CanvasCache {
                renderer: vectorcraft_render::Renderer::new(),
                texture: None,
                key: None,
                last_ms: 0.0,
                worker: None,
                worker_started: false,
            },
            perf: Perf::default(),
            integrated_titlebar: false,
            native_menu: false,
            last_effect: None,
            native_shortcuts: Default::default(),
            clipboard_out: None,
            clipboard_published: None,
            clipboard_in: None,
            pending_url: None,
            control_rx: None,
            pending_screenshots: vec![],
            queued_screenshots: vec![],
            screenshot_token: 0,
            synthetic: vec![],
            styled: false,
            fonts_ready: false,
            frame: 0,
            last_time: 0.0,
            canvas_rect: None,
            hover_doc: None,
            custom_titlebar: false,
        }
    }

    pub fn with_control(mut self, rx: Receiver<ControlRequest>) -> Self {
        self.control_rx = Some(rx);
        self
    }

    /// Keep `views` aligned with the session's documents.
    pub fn sync_views(&mut self) {
        let n = self.session.documents().len();
        while self.views.len() < n {
            self.views.push(View::default());
        }
        self.views.truncate(n);
    }

    pub fn view(&self) -> Option<&View> {
        self.session.active_index().and_then(|i| self.views.get(i))
    }
    pub fn view_mut(&mut self) -> Option<&mut View> {
        self.sync_views();
        let i = self.session.active_index()?;
        self.views.get_mut(i)
    }

    pub fn view_info(&self) -> ViewInfo {
        ViewInfo {
            zoom: self.view().map(|v| v.zoom).unwrap_or(1.0),
            outline: self.ui.view.outline,
            smart_guides: self.ui.view.smart_guides,
            snap_to_grid: self.ui.view.snap_to_grid,
            snap_to_pixel: self.ui.view.snap_to_pixel,
            show_bbox: self.ui.view.bounding_box,
            snap_to_point: self.ui.view.snap_to_point,
            corner_widgets: self.ui.view.corner_widgets,
        }
    }

    /// Run a UI or engine command by id. The single entry point for every frontend path.
    pub fn run(&mut self, id: &str, params: Value) -> Result<Value, String> {
        if let Some(r) = menus::run_ui_command(self, id, &params) {
            return r;
        }
        if id.starts_with("edit.paste") {
            self.adopt_system_clipboard();
        }
        let r = self.session.execute(id, &params).map_err(|e| e.to_string());
        if r.is_ok() && matches!(id, "edit.copy" | "edit.cut") && self.session.prefs.copy_as_svg {
            self.clipboard_out = self.session.clipboard_svg();
            self.clipboard_published = self.clipboard_out.clone();
        }
        self.sync_views();
        match &r {
            Err(e) => self.ui.status = e.clone(),
            Ok(_) => {
                if id == "file.new" {
                    self.ui.status.clear();
                }
                if id == "text.setStyle"
                    && let Some(font) = params.get("font").and_then(Value::as_str)
                {
                    let r = &mut self.ui.recent_fonts;
                    r.retain(|f| f != font);
                    r.insert(0, font.to_string());
                    r.truncate(10);
                }
            }
        }
        r
    }

    /// Before a paste: SVG that another app put on the system clipboard replaces the internal
    /// clipboard (centred in the view). Our own published SVG keeps the lossless internal copy.
    fn adopt_system_clipboard(&mut self) {
        let text = self.clipboard_in.take().or_else(|| self.services.clipboard_read.as_mut().and_then(|f| f()));
        let Some(text) = text.filter(|t| vectorcraft_engine::cmd::clipboard::looks_like_svg(t)) else { return };
        if self.clipboard_published.as_deref() == Some(text.as_str()) {
            return;
        }
        let center = self.view().map(|v| [v.center.x, v.center.y]);
        match self.session.execute("clipboard.importSvg", &serde_json::json!({ "svg": text, "center": center })) {
            Ok(_) => self.clipboard_published = Some(text),
            Err(e) => self.ui.status = format!("Couldn't paste SVG: {e}"),
        }
    }

    /// Open a link in the browser (Help → Discord, website, GitHub…).
    pub fn open_url(&mut self, url: &str) {
        match self.services.open_url.as_mut() {
            Some(open) => open(url),
            None => self.pending_url = Some(url.to_string()),
        }
        self.ui.status = format!("Opened {url}");
    }

    /// Run a Help link command and open the URL it returns.
    pub fn open_link(&mut self, id: &str) {
        if let Ok(v) = self.run(id, serde_json::json!({}))
            && let Some(u) = v["url"].as_str()
        {
            let u = u.to_string();
            self.open_url(&u);
        }
    }

    /// Select a tool (also used by the toolbar and shortcuts).
    pub fn select_tool(&mut self, id: &str) {
        let v = self.view_info();
        if let Err(e) = self.session.select_tool(id, v) {
            self.ui.status = e.to_string();
        }
        if let Some(g) = vectorcraft_tools::catalog::group_of(id)
            && let Some(slot) = self.ui.group_tool.get_mut(g)
        {
            *slot = id.to_string();
        }
        toolbar::remember(self, id);
        self.ui.flyout = None;
    }

    fn drain_control(&mut self, ctx: &egui::Context) {
        let Some(rx) = self.control_rx.take() else { return };
        while let Ok(req) = rx.try_recv() {
            let reply = req.reply.clone();
            // Guarded per request: a panic must not drop the channel (taken out of `self` above).
            let outcome = vectorcraft_engine::guard::catch_panic(|| control::handle(self, ctx, &req))
                .unwrap_or_else(|msg| control::err(format!("internal error: {msg} (please report this bug)")));
            match outcome {
                control::Outcome::Done(v) => {
                    let _ = reply.send(v);
                }
                control::Outcome::Screenshot { path } => {
                    self.screenshot_token += 1;
                    let token = self.screenshot_token;
                    let settle = ctx.global_style().animation_time as f64 * 2000.0 + 80.0;
                    self.queued_screenshots.push((token, now_ms() + settle, 0));
                    self.pending_screenshots.push((token, path, reply, now_ms() + settle + 8000.0));
                }
            }
        }
        self.control_rx = Some(rx);
    }

    fn issue_screenshots(&mut self, ctx: &egui::Context) {
        let now = now_ms();
        self.queued_screenshots.retain_mut(|(token, at, frames)| {
            *frames += 1;
            if now >= *at && *frames >= 3 {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::new(*token)));
                false
            } else {
                true
            }
        });
        if !self.queued_screenshots.is_empty() || !self.pending_screenshots.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }

    fn collect_screenshots(&mut self, ctx: &egui::Context) {
        if self.pending_screenshots.is_empty() {
            return;
        }
        let events: Vec<_> = ctx.input(|i| {
            i.raw
                .events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Screenshot { user_data, image, .. } => {
                        let token = user_data.data.as_ref().and_then(|d| d.downcast_ref::<u64>()).copied()?;
                        Some((token, image.clone()))
                    }
                    _ => None,
                })
                .collect()
        });
        for (token, image) in events {
            if let Some(i) = self.pending_screenshots.iter().position(|(t, ..)| *t == token) {
                let (_, path, reply, _) = self.pending_screenshots.remove(i);
                let _ = reply.send(control::save_screenshot(self, &image, path.as_deref()));
            }
        }
        let now = now_ms();
        self.pending_screenshots.retain(|(_, _, reply, deadline)| {
            if now < *deadline {
                return true;
            }
            let _ = reply.send(serde_json::json!({
                "ok": false,
                "error": "no frame was presented (screen locked, window minimized or fully covered); ui.render still renders the artboard"
            }));
            false
        });
    }

    /// Show a transient status message.
    pub fn status(&mut self, s: impl Into<String>) {
        self.ui.status = s.into();
    }

    fn drain_inbox(&mut self) {
        let arrived: Vec<(String, Vec<u8>)> =
            self.services.inbox.as_ref().map(|q| std::mem::take(&mut *q.lock().unwrap_or_else(|e| e.into_inner()))).unwrap_or_default();
        for (name, bytes) in arrived {
            if let Err(e) = io::open_bytes(self, &name, &bytes, None) {
                self.status(format!("Couldn't open {name}: {e}"));
            }
        }
    }
}

pub fn now_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        use std::time::{SystemTime, UNIX_EPOCH};
        SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        0.0
    }
}

/// eframe isn't a dependency of this crate (the host owns the event loop); these entry points are
/// called from the host's `eframe::App` impl.
impl VectorcraftApp {
    /// Per-frame logic before layout (control channel, shortcuts, inbox). A bug that panics costs
    /// one frame and shows an error, instead of closing the app with unsaved work.
    pub fn logic(&mut self, ctx: &egui::Context) {
        if let Err(msg) = vectorcraft_engine::guard::catch_panic(|| self.logic_frame(ctx)) {
            self.status(format!("Internal error: {msg} (please report this bug)"));
        }
    }

    fn logic_frame(&mut self, ctx: &egui::Context) {
        if !self.styled {
            theme::install_fonts(ctx);
            theme::apply(ctx, self.ui.brightness);
            egui_extras::install_image_loaders(ctx);
            self.styled = true;
        } else {
            self.fonts_ready = true;
        }
        self.frame += 1;
        let now = ctx.input(|i| i.time);
        let dt = now - self.last_time;
        if dt > 0.0 {
            self.perf.fps = self.perf.fps * 0.9 + (1.0 / dt).min(240.0) * 0.1;
        }
        self.last_time = now;
        self.sync_views();
        // The window's close button (or the system quitting the app) asks about unsaved documents.
        if ctx.input(|i| i.viewport().close_requested()) && unsaved::any_dirty(self) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            if let Err(e) = unsaved::close_all(self, "quit") {
                self.status(e);
            }
        }
        shortcut_editor::sync(&self.ui);
        prefs_dialog::apply_runtime(self, ctx);
        self.drain_control(ctx);
        if !self.synthetic.is_empty() {
            ctx.request_repaint();
        }
        self.collect_screenshots(ctx);
        self.issue_screenshots(ctx);
        if let Some(u) = self.pending_url.take() {
            ctx.open_url(egui::OpenUrl::new_tab(u));
        }
        if let Some(t) = self.clipboard_out.take() {
            ctx.copy_text(t);
        }
        self.drain_inbox();
        if self.fonts_ready {
            shortcuts::handle(self, ctx);
        }
        // Native only: the web host reads dropped files asynchronously and feeds `Services::inbox`.
        #[cfg(not(target_arch = "wasm32"))]
        for f in ctx.input(|i| i.raw.dropped_files.clone()) {
            let path = f.path().to_path_buf();
            let name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped".into());
            match f.bytes() {
                Ok(b) => {
                    let p = Some(path.to_string_lossy().to_string()).filter(|s| !s.is_empty());
                    if let Err(e) = io::open_bytes(self, &name, &b, p) {
                        self.status(format!("Couldn't open {name}: {e}"));
                    }
                }
                Err(e) => self.status(format!("Couldn't read {name}: {e}")),
            }
        }
    }

    /// Inject synthetic events (one press/release step per frame).
    pub fn raw_input_hook(&mut self, raw: &mut egui::RawInput) {
        if self.synthetic.is_empty() {
            return;
        }
        // Pointer events go one per frame so egui sees presses, drags and releases as real input;
        // keyboard sequences go up to the key release.
        let n = match self.synthetic[0] {
            egui::Event::PointerMoved(_) | egui::Event::PointerButton { .. } => 1,
            _ => self.synthetic.iter().position(|e| matches!(e, egui::Event::Key { pressed: false, .. })).map_or(self.synthetic.len(), |i| i + 1),
        };
        if let Some(egui::Event::PointerMoved(p) | egui::Event::PointerButton { pos: p, .. }) = self.synthetic.first() {
            raw.events.push(egui::Event::PointerMoved(*p));
        }
        raw.events.extend(self.synthetic.drain(..n));
    }

    /// Lay out the whole window.
    pub fn ui(&mut self, ui: &mut egui::Ui) {
        if let Err(msg) = vectorcraft_engine::guard::catch_panic(|| self.ui_frame(ui)) {
            self.status(format!("Internal error: {msg} (please report this bug)"));
        }
    }

    fn ui_frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.fonts_ready {
            ctx.request_repaint();
            return;
        }
        let t0 = now_ms();
        let t = theme::Tokens::get(&ctx);
        if self.ui.screen_mode < 2 {
            chrome::app_bar(self, ui);
            if self.ui.control_bar {
                chrome::control_bar(self, ui);
            }
        }
        if self.ui.status_bar && self.ui.screen_mode < 3 {
            chrome::status_bar(self, ui);
            chrome::hint_bar(self, ui);
        }
        if self.ui.toolbar && self.ui.screen_mode < 3 {
            toolbar::show(self, ui);
        }
        if self.ui.dock && self.ui.screen_mode < 3 {
            dock::show(self, ui);
        }
        egui::CentralPanel::default().frame(egui::Frame::NONE.fill(t.pasteboard)).show(ui, |ui| {
            if self.ui.screen_mode < 3 {
                chrome::doc_tabs(self, ui);
            }
            canvas::show(self, ui);
        });
        dock::floating_panel(self, &ctx);
        panels::library_panel::show_window(self, &ctx);
        dialogs::show(self, &ctx);
        palette::show(self, &ctx);
        if self.custom_titlebar {
            titlebar::resize_zones(ui);
        }
        self.perf.frame_ms = now_ms() - t0;
        let _ = json!(null);
    }
}
