//! Pen and touch input (an iPad with an Apple Pencil): the shell reads Pointer Events itself, in
//! the capture phase, before eframe's own touch and pointer handlers, which can't tell a pen from
//! a finger and see each touch twice (as a pointer and as a touch event).
//!
//! - A pen drives egui's pointer and is a touch contact with the pen's pressure as its force (the
//!   canvas reads it), so holding it still opens the context menu (egui's long touch). It hovers
//!   where the pen can (Apple Pencil hover).
//! - Fingers are touch contacts: two pinch and pan the canvas, and tap to undo or redo. The first
//!   finger also drives the pointer, so it taps the panels and menus; on the canvas it navigates
//!   only once a pen has been seen ([`touch::set_source`]), which the page remembers.
//! - Palm rejection: while a pen is down, fingers are ignored; a pen landing cancels the fingers
//!   already down (a palm resting first).
//!
//! Mouse and trackpad events (a Magic Keyboard) are left to eframe.

use std::cell::RefCell;
use std::rc::Rc;

use egui::{Event, PointerButton, Pos2, TouchDeviceId, TouchId, TouchPhase};
use vectorcraft_ui_egui::touch::{self, Source};
use wasm_bindgen::JsCast as _;

/// Where the page remembers that a pen has been seen (`localStorage`).
const PEN_SEEN_KEY: &str = "vectorcraft-pen-seen";
/// The pen's touch device (fingers use another, so egui doesn't pinch a pen with a finger).
const PEN_DEVICE: TouchDeviceId = TouchDeviceId(1);
const FINGER_DEVICE: TouchDeviceId = TouchDeviceId(2);

/// The pen and finger contacts followed, and the egui events they made since the last frame.
#[derive(Default)]
pub struct Pointers {
    events: Vec<Event>,
    /// The pen pressing, by pointer id.
    pen: Option<i32>,
    /// The fingers down, by pointer id.
    fingers: Vec<i32>,
    /// The contact driving egui's pointer.
    primary: Option<i32>,
}

pub type SharedPointers = Rc<RefCell<Pointers>>;

impl Pointers {
    /// The events since the last frame, for the frame's raw input.
    pub fn take(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    fn touch(&mut self, device: TouchDeviceId, id: i32, phase: TouchPhase, pos: Pos2, force: Option<f32>) {
        self.events.push(Event::Touch { device_id: device, id: TouchId::from(id as u64), phase, pos, force });
    }

    fn press(&mut self, id: i32, pos: Pos2) {
        self.primary = Some(id);
        self.events.push(Event::PointerMoved(pos));
        self.events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::default() });
    }

    fn release(&mut self, pos: Pos2) {
        self.primary = None;
        self.events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::default() });
    }

    /// Cancel every finger down (a palm under a pen landing).
    fn cancel_fingers(&mut self, pos: Pos2) {
        for id in std::mem::take(&mut self.fingers) {
            self.touch(FINGER_DEVICE, id, TouchPhase::Cancel, pos, None);
            if self.primary == Some(id) {
                self.release(pos);
                self.events.push(Event::PointerGone);
            }
        }
    }

    /// Follow one pointer event; `true` if it was the pen's or a finger's (taken from eframe).
    fn feed(&mut self, kind: &str, e: &web_sys::PointerEvent, pos: Pos2, coalesced: &[(Pos2, f32)], on_canvas: bool) -> bool {
        let id = e.pointer_id();
        match e.pointer_type().as_str() {
            "pen" => {
                let force = Some(e.pressure().clamp(0.0, 1.0));
                match kind {
                    "pointerdown" => {
                        if !on_canvas && self.pen.is_none() {
                            return false;
                        }
                        set_pen_seen();
                        touch::set_source(Source::Pen);
                        self.cancel_fingers(pos);
                        self.pen = Some(id);
                        self.touch(PEN_DEVICE, id, TouchPhase::Start, pos, force);
                        self.press(id, pos);
                    }
                    "pointermove" if self.pen == Some(id) => {
                        // The positions the pen reported between two events (up to 240 a second
                        // on an iPad), each with its pressure, so fast strokes keep their curve.
                        for c in coalesced.iter().take(coalesced.len().saturating_sub(1)) {
                            self.touch(PEN_DEVICE, id, TouchPhase::Move, c.0, Some(c.1));
                            self.events.push(Event::PointerMoved(c.0));
                        }
                        self.touch(PEN_DEVICE, id, TouchPhase::Move, pos, force);
                        self.events.push(Event::PointerMoved(pos));
                    }
                    // Hovering (Apple Pencil hover): the pointer follows without pressing.
                    "pointermove" => {
                        if self.primary.is_some() {
                            return true;
                        }
                        if !on_canvas {
                            return false;
                        }
                        self.events.push(Event::PointerMoved(pos));
                    }
                    "pointerup" | "pointercancel" if self.pen == Some(id) => {
                        let phase = if kind == "pointerup" { TouchPhase::End } else { TouchPhase::Cancel };
                        self.pen = None;
                        self.touch(PEN_DEVICE, id, phase, pos, None);
                        self.release(pos);
                    }
                    // A pen leaving the hover range, or an event of a press we didn't take.
                    _ => {
                        if kind == "pointerleave" && self.primary.is_none() && on_canvas {
                            self.events.push(Event::PointerGone);
                        }
                        return self.pen.is_some();
                    }
                }
                true
            }
            "touch" => {
                match kind {
                    "pointerdown" => {
                        // Palm rejection: nothing presses while the pen does.
                        if self.pen.is_some() {
                            return true;
                        }
                        if !on_canvas && self.fingers.is_empty() {
                            return false;
                        }
                        self.fingers.push(id);
                        self.touch(FINGER_DEVICE, id, TouchPhase::Start, pos, None);
                        if self.primary.is_none() && self.fingers.len() == 1 {
                            touch::set_source(Source::Finger);
                            self.press(id, pos);
                        }
                    }
                    "pointermove" if self.fingers.contains(&id) => {
                        self.touch(FINGER_DEVICE, id, TouchPhase::Move, pos, None);
                        if self.primary == Some(id) {
                            self.events.push(Event::PointerMoved(pos));
                        }
                    }
                    "pointerup" | "pointercancel" if self.fingers.contains(&id) => {
                        let phase = if kind == "pointerup" { TouchPhase::End } else { TouchPhase::Cancel };
                        self.fingers.retain(|f| *f != id);
                        self.touch(FINGER_DEVICE, id, phase, pos, None);
                        if self.primary == Some(id) {
                            self.release(pos);
                            self.events.push(Event::PointerGone);
                        }
                    }
                    // A rejected palm's later events, or one we never took.
                    _ => return self.pen.is_some() || !self.fingers.is_empty() || on_canvas,
                }
                true
            }
            _ => false,
        }
    }
}

/// Whether the page remembers a pen, for [`touch::set_pen_seen`] before the first frame.
pub fn restore_pen_seen() {
    let seen = local_storage().and_then(|s| s.get_item(PEN_SEEN_KEY).ok().flatten()).is_some_and(|v| v == "1");
    touch::set_pen_seen(seen);
}

fn set_pen_seen() {
    if !touch::pen_seen()
        && let Some(s) = local_storage()
    {
        // Private browsing can refuse it: fingers then draw again on the next visit, no more.
        let _ = s.set_item(PEN_SEEN_KEY, "1");
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

/// Take pen and finger Pointer Events on the page from eframe into `pointers` (and the touch
/// events that follow them), for the runner's frames. Mouse events stay eframe's.
pub fn install(runner: &eframe::WebRunner, canvas: &web_sys::HtmlCanvasElement, pointers: &SharedPointers) -> Result<(), wasm_bindgen::JsValue> {
    let window: web_sys::EventTarget = web_sys::window().ok_or("no window")?.into();
    let options = web_sys::AddEventListenerOptions::new();
    options.set_capture(true);
    options.set_passive(false);
    for kind in ["pointerdown", "pointermove", "pointerup", "pointercancel", "pointerleave"] {
        let (pointers, canvas) = (pointers.clone(), canvas.clone());
        runner.add_event_listener_ex(&window, kind, &options, move |e: web_sys::PointerEvent, runner, _| {
            let on_canvas = e.target().is_some_and(|t| &t == canvas.unchecked_ref::<web_sys::EventTarget>());
            let rect = canvas.get_bounding_client_rect();
            let z = runner.egui_ctx().zoom_factor();
            let pos = egui::pos2((e.client_x() as f32 - rect.left() as f32) / z, (e.client_y() as f32 - rect.top() as f32) / z);
            let at = |x: i32, y: i32| egui::pos2((x as f32 - rect.left() as f32) / z, (y as f32 - rect.top() as f32) / z);
            let coalesced: Vec<(Pos2, f32)> = if kind == "pointermove"
                && e.pointer_type() == "pen"
                // Safari before 17 hasn't it, and calling a missing method would abort the app.
                && js_sys::Reflect::has(&e, &"getCoalescedEvents".into()).unwrap_or(false)
            {
                e.get_coalesced_events()
                    .iter()
                    .filter_map(|c| c.dyn_into::<web_sys::PointerEvent>().ok())
                    .map(|c| (at(c.client_x(), c.client_y()), c.pressure().clamp(0.0, 1.0)))
                    .collect()
            } else {
                vec![]
            };
            if !pointers.borrow_mut().feed(kind, &e, pos, &coalesced, on_canvas) {
                return;
            }
            e.stop_immediate_propagation();
            // No compatibility mouse events, no text selection or magnifier.
            e.prevent_default();
            if kind == "pointerup" {
                // Keys typed next (a Magic Keyboard) go to the app.
                canvas.focus().ok();
                // Safari lets a page open the share sheet, a download or the on-screen keyboard
                // only during the user's gesture: handle the release now.
                runner.logic();
            }
            runner.egui_ctx().request_repaint();
        })?;
    }
    // The touch events behind those pointer events: eframe would see each touch a second time,
    // and Safari would scroll, zoom or select.
    for kind in ["touchstart", "touchmove", "touchend", "touchcancel"] {
        let (pointers, canvas) = (pointers.clone(), canvas.clone());
        runner.add_event_listener_ex(&window, kind, &options, move |e: web_sys::TouchEvent, _runner, _| {
            let on_canvas = e.target().is_some_and(|t| &t == canvas.unchecked_ref::<web_sys::EventTarget>());
            let p = pointers.borrow();
            if on_canvas || p.pen.is_some() || !p.fingers.is_empty() {
                e.stop_immediate_propagation();
                if e.cancelable() {
                    e.prevent_default();
                }
            }
        })?;
    }
    Ok(())
}
