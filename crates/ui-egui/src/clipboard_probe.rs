//! Checks the system clipboard for Paste off the UI thread. `has()` can block for seconds when the
//! clipboard owner never answers (an unresponsive X11 client), so it must not run on the thread that
//! draws the window: a worker owns a second clipboard handle, publishes its verdict, and the UI only
//! reads the latest one. Without a worker the check stays in line, as before (the web, and hosts
//! that don't install a factory).

use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::Duration;

use vectorcraft_engine::cmd::clipboard::PASTE_ORDER;

use crate::sysclip::ClipboardProbeFactory;

/// How often the worker looks at the clipboard.
const PROBE_INTERVAL: Duration = Duration::from_millis(250);

/// The background clipboard check ([`crate::VectorcraftApp`]'s `clipboard_probe`).
pub(crate) struct Probe {
    rx: Option<Receiver<bool>>,
    /// The last verdict the worker sent; `None` until the first one arrives.
    last: Option<bool>,
}

impl Probe {
    /// Start the worker. `None` on wasm, or when the thread can't be spawned: the caller then falls
    /// back to the in-line check.
    pub(crate) fn start(make: ClipboardProbeFactory, ctx: egui::Context) -> Option<Self> {
        if cfg!(target_arch = "wasm32") {
            return None;
        }
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::Builder::new()
            .name("vectorcraft-clip".into())
            .spawn(move || {
                let mut clipboard = make();
                let mut sent: Option<bool> = None;
                loop {
                    // The worker's thread may wait here for a slow owner; the UI's never does.
                    let verdict = clipboard.has(&PASTE_ORDER);
                    if tx.send(verdict).is_err() {
                        return; // the app is gone: the receiver was dropped
                    }
                    // Wake the UI only when the answer changed (it reads the verdict every frame).
                    if sent != Some(verdict) {
                        ctx.request_repaint();
                    }
                    sent = Some(verdict);
                    std::thread::sleep(PROBE_INTERVAL);
                }
            })
            .ok()?;
        Some(Self { rx: Some(rx), last: None })
    }

    /// The latest verdict the worker sent, if one has arrived. Never blocks.
    pub(crate) fn poll(&mut self) -> Option<bool> {
        loop {
            match self.rx.as_ref().map(Receiver::try_recv) {
                Some(Ok(verdict)) => self.last = Some(verdict),
                Some(Err(TryRecvError::Empty)) => return self.last,
                Some(Err(TryRecvError::Disconnected)) | None => {
                    self.rx = None;
                    return self.last;
                }
            }
        }
    }
}
