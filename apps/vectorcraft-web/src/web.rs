//! The browser shell: web `Services`, drag-and-drop, and the eframe web runner.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use vectorcraft_engine::Session;
use vectorcraft_engine::cmd::fileio;
use vectorcraft_ui_egui::place::{DropTarget, PlaceArrival, PlaceInbox};
use vectorcraft_ui_egui::{Services, VectorcraftApp};
use wasm_bindgen::JsCast as _;

type Inbox = Arc<Mutex<Vec<(String, Vec<u8>)>>>;

/// Where the pointer was over the canvas during the last file drag (CSS px from its top-left) and
/// whether Shift was held: where dropped files land.
type DragPos = Rc<Cell<Option<(f32, f32, bool)>>>;

const CANVAS_ID: &str = "vectorcraft_canvas";
const LOADING_ID: &str = "vectorcraft_loading";

pub fn start() {
    eframe::WebLogger::init(log::LevelFilter::Info).ok();
    wasm_bindgen_futures::spawn_local(async {
        let Some(document) = web_sys::window().and_then(|w| w.document()) else {
            log::error!("no document");
            return;
        };
        let Some(canvas) = document.get_element_by_id(CANVAS_ID).and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok()) else {
            log::error!("missing <canvas id=\"{CANVAS_ID}\">");
            return;
        };
        let drag = track_drag(&canvas);
        let mut options = eframe::WebOptions::default();
        if query().contains("webgl")
            && let eframe::egui_wgpu::WgpuSetup::CreateNew(create) = &mut options.wgpu_options.wgpu_setup
        {
            create.instance_descriptor.backends = eframe::wgpu::Backends::GL;
        }
        let result = eframe::WebRunner::new()
            .start(
                canvas,
                options,
                Box::new(move |cc| {
                    if let Some(rs) = &cc.wgpu_render_state {
                        log::info!("vectorcraft-web: wgpu backend {:?}", rs.adapter.get_info().backend);
                    }
                    let inbox: Inbox = Arc::default();
                    let place_inbox: PlaceInbox = Arc::default();
                    let app = VectorcraftApp::new(Session::new(), services(inbox.clone(), place_inbox.clone(), cc.egui_ctx.clone()));
                    Ok(Box::new(WebShell { app, inbox, place_inbox, drag }))
                }),
            )
            .await;
        if let Some(el) = document.get_element_by_id(LOADING_ID) {
            match result {
                Ok(()) => el.remove(),
                Err(e) => el.set_inner_html(&format!("<p>VectorCraft failed to start: {e:?}</p><p>A browser with WebGPU or WebGL2 is required.</p>")),
            }
        }
    });
}

fn query() -> String {
    web_sys::window().and_then(|w| w.location().search().ok()).unwrap_or_default()
}

/// Follow file drags over the canvas: drag events carry the pointer position, which egui doesn't
/// get during a drag.
fn track_drag(canvas: &web_sys::HtmlCanvasElement) -> DragPos {
    let pos = DragPos::default();
    let p = pos.clone();
    let on_drag = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::DragEvent)>::new(move |e: web_sys::DragEvent| {
        p.set(Some((e.offset_x() as f32, e.offset_y() as f32, e.shift_key())));
    });
    for kind in ["dragover", "drop"] {
        if let Err(e) = canvas.add_event_listener_with_callback(kind, on_drag.as_ref().unchecked_ref()) {
            log::error!("couldn't follow {kind} events: {e:?}");
        }
    }
    // The listener lives as long as the page.
    on_drag.forget();
    pos
}

/// Wraps the app to read dropped files asynchronously (browsers can't read them synchronously)
/// and feed them through the inboxes: placed where they were dropped on the canvas, else opened.
struct WebShell {
    app: VectorcraftApp,
    inbox: Inbox,
    place_inbox: PlaceInbox,
    drag: DragPos,
}

impl eframe::App for WebShell {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let dropped = ctx.input_mut(|i| std::mem::take(&mut i.raw.dropped_files));
        if !dropped.is_empty() {
            let at = self.drag.take();
            let z = ctx.zoom_factor();
            let target = self.app.drop_target(at.map(|(x, y, _)| egui::pos2(x / z, y / z)), at.is_some_and(|a| a.2));
            for f in dropped {
                let (inbox, place_inbox, ctx) = (self.inbox.clone(), self.place_inbox.clone(), ctx.clone());
                wasm_bindgen_futures::spawn_local(async move {
                    let name = f.path().file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "dropped".into());
                    match f.bytes_async().await {
                        Ok(bytes) => {
                            match target {
                                DropTarget::Place { at, embed } => {
                                    place_inbox.lock().unwrap_or_else(|e| e.into_inner()).push(PlaceArrival { name, bytes, drop: Some((at, embed)) })
                                }
                                DropTarget::Open => inbox.lock().unwrap_or_else(|e| e.into_inner()).push((name, bytes)),
                            }
                            ctx.request_repaint();
                        }
                        Err(e) => log::error!("couldn't read dropped file {name}: {e}"),
                    }
                });
            }
        }
        self.app.logic(ctx);
    }

    fn raw_input_hook(&mut self, _ctx: &egui::Context, raw: &mut egui::RawInput) {
        self.app.raw_input_hook(raw);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.app.ui(ui);
    }
}

fn services(inbox: Inbox, place_inbox: PlaceInbox, ctx: egui::Context) -> Services {
    let open_inbox = inbox.clone();
    let picked = place_inbox.clone();
    let place_ctx = ctx.clone();
    Services {
        // File → Place…: the picked files go to the Place dialog.
        place_async: Some(Box::new(move || {
            let inbox = picked.clone();
            let ctx = place_ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let dialog = fileio::place_filters().fold(rfd::AsyncFileDialog::new().set_title("Place"), |d, (name, exts)| d.add_filter(name, exts));
                let Some(files) = dialog.pick_files().await else {
                    return;
                };
                let mut arrived = vec![];
                for f in files {
                    arrived.push(PlaceArrival { name: f.file_name(), bytes: f.read().await, drop: None });
                }
                inbox.lock().unwrap_or_else(|e| e.into_inner()).extend(arrived);
                ctx.request_repaint();
            });
        })),
        place_inbox: Some(place_inbox),
        open_async: Some(Box::new(move || {
            let inbox = open_inbox.clone();
            let ctx = ctx.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let dialog = fileio::open_filters().fold(rfd::AsyncFileDialog::new(), |d, (name, exts)| d.add_filter(name, exts));
                let Some(file) = dialog.pick_file().await else {
                    return;
                };
                let bytes = file.read().await;
                inbox.lock().unwrap_or_else(|e| e.into_inner()).push((file.file_name(), bytes));
                ctx.request_repaint();
            });
        })),
        download: Some(Box::new(|name: &str, bytes: &[u8]| {
            if let Err(e) = download(name, bytes) {
                log::error!("download of {name} failed: {e}");
            }
        })),
        inbox: Some(inbox),
        ..Default::default()
    }
}

/// Trigger a browser download of `bytes` named after the last component of `path`.
fn download(path: &str, bytes: &[u8]) -> Result<(), String> {
    let js = |e: wasm_bindgen::JsValue| format!("{e:?}");
    let name = std::path::Path::new(path).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "vectorcraft".into());
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(bytes));
    let opts = web_sys::BlobPropertyBag::new();
    opts.set_type(fileio::format_for_name(&name).map_or("application/octet-stream", |f| f.mime));
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &opts).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
    let a: web_sys::HtmlAnchorElement = document.create_element("a").map_err(js)?.dyn_into().map_err(|_| "not an anchor")?;
    a.set_href(&url);
    a.set_download(&name);
    a.style().set_property("display", "none").map_err(js)?;
    let body = document.body().ok_or("no body")?;
    body.append_child(&a).map_err(js)?;
    a.click();
    a.remove();
    // Revoke after the click has been dispatched; the download keeps its own reference.
    let revoke = wasm_bindgen::closure::Closure::once_into_js(move || {
        web_sys::Url::revoke_object_url(&url).ok();
    });
    window.set_timeout_with_callback_and_timeout_and_arguments_0(revoke.unchecked_ref(), 10_000).map_err(js)?;
    Ok(())
}
