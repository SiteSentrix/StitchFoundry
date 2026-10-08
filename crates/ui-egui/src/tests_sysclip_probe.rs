//! The system clipboard is checked for Paste off the UI thread (the regression the app used to
//! freeze on): an X11 clipboard owner that never answers `get_text()` blocked the frame loop for
//! seconds, so the window stopped drawing and could not even be closed. These tests use a clipboard
//! that blocks on demand and prove the UI frame is not the thread that waits on it.
//!
//! With no probe factory (the web, tests that predate this) the check stays on the UI thread, so the
//! old path is kept and its tests still cover it.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;
use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::clipboard::Flavour;

use crate::{ClipboardProbeFactory, Services, SystemClipboard, VectorcraftApp, menus};

/// How long a blocking clipboard holds a call before giving up: bounded so a regression fails a
/// test instead of hanging the whole suite.
const GATE_MAX: Duration = Duration::from_millis(1000);

/// Lets `has()` through when opened, and otherwise at most [`GATE_MAX`].
#[derive(Clone, Default)]
struct Gate(Arc<std::sync::atomic::AtomicBool>);

impl Gate {
    fn open(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    fn wait(&self) {
        let end = Instant::now() + GATE_MAX;
        while !self.0.load(Ordering::SeqCst) && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

/// What the two clipboard handles recorded: the thread each `has()` ran on, and how many handles
/// are alive.
#[derive(Default)]
struct Recorder {
    threads: Mutex<Vec<Option<String>>>,
    alive: AtomicUsize,
}

/// A system clipboard whose `has()` blocks on a [`Gate`] (like an owner that never answers).
struct Blocking {
    rec: Arc<Recorder>,
    gate: Gate,
    verdict: bool,
}

impl Blocking {
    fn new(rec: Arc<Recorder>, gate: Gate, verdict: bool) -> Self {
        rec.alive.fetch_add(1, Ordering::SeqCst);
        Self { rec, gate, verdict }
    }
}

impl Drop for Blocking {
    fn drop(&mut self) {
        self.rec.alive.fetch_sub(1, Ordering::SeqCst);
    }
}

impl SystemClipboard for Blocking {
    fn write(&mut self, _flavours: &[Flavour]) -> Result<(), String> {
        Ok(())
    }
    fn holds_ours(&mut self) -> bool {
        false
    }
    fn read(&mut self, _mimes: &[&'static str]) -> Option<Flavour> {
        None
    }
    fn has(&mut self, _mimes: &[&'static str]) -> bool {
        self.rec.threads.lock().unwrap().push(std::thread::current().name().map(str::to_owned));
        self.gate.wait();
        self.verdict
    }
}

/// An app with a document, a blocking `system_clipboard` (what the UI would call in line) and a
/// probe factory whose own handle shares the recorder.
fn app(system_verdict: bool, probe_verdict: bool, gate: &Gate) -> (VectorcraftApp, Arc<Recorder>) {
    let rec = Arc::new(Recorder::default());
    let system = Blocking::new(Arc::clone(&rec), gate.clone(), system_verdict);
    let make = {
        let rec = Arc::clone(&rec);
        let gate = gate.clone();
        move || -> Box<dyn SystemClipboard> { Box::new(Blocking::new(rec, gate, probe_verdict)) }
    };
    let probe: ClipboardProbeFactory = Box::new(make);
    let services = Services { system_clipboard: Some(Box::new(system)), clipboard_probe: Some(probe), ..Default::default() };
    let mut app = VectorcraftApp::new(Session::new(), services);
    app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    (app, rec)
}

/// One headless frame of app logic, in the test's own context (a new one would restart the fonts).
/// Time advances by more than the clipboard poll interval each frame, so every frame re-checks (a
/// headless context otherwise keeps `time` fixed and only the first frame would).
fn frame(app: &mut VectorcraftApp) {
    thread_local! {
        static CTX: egui::Context = egui::Context::default();
        static NOW: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
    }
    let ctx = CTX.with(Clone::clone);
    let time = NOW.with(|t| {
        let v = t.get() + 0.5;
        t.set(v);
        v
    });
    let input = egui::RawInput { time: Some(time), ..Default::default() };
    ctx.run_ui(input, |ui| app.logic(ui.ctx())).textures_delta.clear();
}

/// A regression guard is only meaningful if the frame really does not wait: a blocking clipboard
/// must not delay the frame, and the check must run on the probe thread.
#[test]
fn a_blocking_clipboard_never_blocks_the_ui_frame() {
    let gate = Gate::default();
    let (mut app, rec) = app(false, false, &gate);
    // Warm up: the first frames install the UI fonts.
    for _ in 0..3 {
        frame(&mut app);
    }
    let t0 = Instant::now();
    frame(&mut app);
    let elapsed = t0.elapsed();
    assert!(elapsed < Duration::from_millis(300), "the frame waited on the clipboard: {elapsed:?}");
    // The check ran on the probe thread, never on the test's.
    let end = Instant::now() + Duration::from_millis(1000);
    while rec.threads.lock().unwrap().is_empty() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    let threads = rec.threads.lock().unwrap().clone();
    assert!(!threads.is_empty(), "the clipboard was never checked");
    assert!(threads.iter().all(|n| n.as_deref() == Some("vectorcraft-clip")), "checked on: {threads:?}");
    assert!(!app.system_paste);
}

#[test]
fn the_probe_reports_a_pasteable_clipboard_to_the_menu() {
    let gate = Gate::default();
    gate.open(); // both handles answer at once
    // The in-line clipboard reports nothing, only the probe's does: the verdict must come from the probe.
    let (mut app, _) = app(false, true, &gate);
    for _ in 0..3 {
        frame(&mut app);
    }
    let mut seen = false;
    for _ in 0..200 {
        frame(&mut app);
        if app.system_paste {
            seen = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(seen, "the probe's verdict never reached `system_paste`");
    assert!(menus::enabled(&app, "edit.paste"));
}

#[test]
fn a_probe_that_reports_nothing_leaves_paste_disabled() {
    let gate = Gate::default();
    gate.open();
    let (mut app, _) = app(false, false, &gate);
    for _ in 0..20 {
        frame(&mut app);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(!app.system_paste && !menus::enabled(&app, "edit.paste"));
}

#[test]
fn dropping_the_app_stops_the_probe() {
    let gate = Gate::default();
    gate.open();
    let (mut app, rec) = app(false, true, &gate);
    for _ in 0..3 {
        frame(&mut app);
    }
    assert!(rec.alive.load(Ordering::SeqCst) > 0, "no clipboard handle alive");
    drop(app);
    let end = Instant::now() + Duration::from_millis(2000);
    while rec.alive.load(Ordering::SeqCst) != 0 && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(rec.alive.load(Ordering::SeqCst), 0, "the probe thread did not stop");
}

#[test]
fn without_a_probe_factory_the_inline_check_still_works() {
    // The web and older hosts: no probe, the UI thread checks the clipboard itself (fast when the
    // owner answers, which is the case here).
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><rect width="5" height="5"/></svg>"##;
    let services = Services { clipboard_read: Some(Box::new(move || Some(svg.to_string()))), ..Default::default() };
    let mut app = VectorcraftApp::new(Session::new(), services);
    app.run("file.new", json!({"width": 100, "height": 100})).unwrap();
    frame(&mut app);
    assert!(menus::enabled(&app, "edit.paste"));
}
