//! The document canvas: rendering, rulers, navigation, pointer routing to tools and on-canvas
//! selection visuals (bounding box, anchors, handles, smart-guide style labels).

use egui::{Color32, CornerRadius, Pos2, Sense, Shape, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_doc::{Node, NodeKind};
use vectorcraft_geom::{Affine, BezPath, PathEl, Point, Rect};
use vectorcraft_tools::{Cursor, Mods, Overlay, PointerEvent, PointerKind};

use crate::state::View;
use crate::theme::{self, Tokens};
use crate::{CacheKey, VectorcraftApp, now_ms, widgets};

const RULER: f32 = 16.0;

/// Screen ↔ document mapping for one frame.
#[derive(Clone, Copy, Debug)]
pub struct Xf {
    pub rect: egui::Rect,
    pub zoom: f64,
    pub center: Point,
    /// View rotation in radians (Rotate View), clockwise on screen.
    pub rot: f64,
}

impl Xf {
    pub fn new(rect: egui::Rect, v: &View) -> Self {
        Self { rect, zoom: v.zoom, center: v.center, rot: v.rotation.to_radians() }
    }
    pub fn to_screen(&self, p: Point) -> Pos2 {
        let c = self.rect.center();
        let (dx, dy) = ((p.x - self.center.x) * self.zoom, (p.y - self.center.y) * self.zoom);
        let (sn, cs) = self.rot.sin_cos();
        pos2(c.x + (dx * cs - dy * sn) as f32, c.y + (dx * sn + dy * cs) as f32)
    }
    pub fn to_doc(&self, p: Pos2) -> Point {
        let c = self.rect.center();
        let (dx, dy) = ((p.x - c.x) as f64, (p.y - c.y) as f64);
        let (sn, cs) = self.rot.sin_cos();
        let (ux, uy) = (dx * cs + dy * sn, -dx * sn + dy * cs);
        Point::new(self.center.x + ux / self.zoom, self.center.y + uy / self.zoom)
    }
    /// Screen-space corners of a document rect (a rotated quad when the view is rotated).
    pub fn quad(&self, r: Rect) -> Vec<Pos2> {
        [Point::new(r.x0, r.y0), Point::new(r.x1, r.y0), Point::new(r.x1, r.y1), Point::new(r.x0, r.y1)].iter().map(|p| self.to_screen(*p)).collect()
    }
    /// Convert a screen-space delta to a document delta.
    pub fn delta_to_doc(&self, d: egui::Vec2) -> vectorcraft_geom::Vec2 {
        let (sn, cs) = self.rot.sin_cos();
        let (dx, dy) = (d.x as f64, d.y as f64);
        vectorcraft_geom::Vec2::new((dx * cs + dy * sn) / self.zoom, (-dx * sn + dy * cs) / self.zoom)
    }
    pub fn rect_to_screen(&self, r: Rect) -> egui::Rect {
        egui::Rect::from_two_pos(self.to_screen(Point::new(r.x0, r.y0)), self.to_screen(Point::new(r.x1, r.y1)))
    }
    /// Affine mapping document points to screen points.
    pub fn affine(&self) -> Affine {
        let c = self.rect.center();
        Affine::translate((c.x as f64, c.y as f64)) * Affine::rotate(self.rot) * Affine::scale(self.zoom) * Affine::translate(-self.center.to_vec2())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Drag {
    Tool,
    /// Hand tool / Space drag, or a middle-button drag with any tool (`middle`).
    Pan {
        start: Pos2,
        center: Point,
        middle: bool,
    },
    ZoomBox {
        start: Pos2,
    },
    RotateView {
        start_angle: f64,
        start_rot: f64,
    },
    /// Cmd held: temporary selection tool; restore this tool on release.
    TempSelect,
    /// A Selection tool move dragged off the canvas: the panels get the art
    /// ([`widgets::PanelDrag::Art`]); `temp` restores the tool Cmd switched from on release.
    Art {
        temp: bool,
    },
}

fn drag_id() -> egui::Id {
    egui::Id::new("canvas-drag")
}
fn temp_tool_id() -> egui::Id {
    egui::Id::new("canvas-temp-tool")
}

pub fn mods(m: egui::Modifiers, space: bool) -> Mods {
    Mods { shift: m.shift, alt: m.alt, cmd: m.command, ctrl: m.ctrl, space }
}

/// Fit the view (View → Fit Artboard / Fit All / Actual Size).
pub fn fit(app: &mut VectorcraftApp, how: &str) {
    let Some(rect) = app.canvas_rect else {
        if let Some(v) = app.view_mut() {
            v.fitted = false;
        }
        return;
    };
    let Some(st) = app.session.active() else { return };
    let target = match how {
        "view.fitAll" => {
            st.doc.art_bounds().map(|a| st.doc.artboards.iter().fold(a, |r, ab| r.union(ab.rect))).or(st.doc.artboards.first().map(|a| a.rect))
        }
        _ => st.doc.artboards.first().map(|a| a.rect),
    };
    let Some(target) = target else { return };
    let v = app.view_mut().unwrap();
    v.center = target.center();
    v.fitted = true;
    if how == "view.actualSize" {
        v.zoom = 1.0;
    } else {
        let zx = (rect.width() as f64 - 60.0) / target.width().max(1.0);
        let zy = (rect.height() as f64 - 60.0) / target.height().max(1.0);
        v.zoom = zx.min(zy).clamp(0.0313, 640.0);
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let full = ui.available_rect_before_wrap();
    if app.session.active().is_none() {
        home(app, ui, full);
        return;
    }
    let rect = if app.ui.view.rulers && app.ui.screen_mode < 3 { egui::Rect::from_min_max(full.min + vec2(RULER, RULER), full.max) } else { full };
    app.canvas_rect = Some(rect);
    let fitted = app.view().is_some_and(|v| v.fitted);
    if !fitted {
        fit(app, "view.fitArtboard");
    }
    let resp = ui.interact(rect, egui::Id::new("canvas"), Sense::click_and_drag());
    handle_input(app, ui, &resp, rect);
    let v = *app.view().unwrap_or(&View::default());
    let xf = Xf::new(rect, &v);
    panel_drop(app, ui, &resp, &xf);
    let painter = ui.painter_at(rect);
    let Some(st) = app.session.active() else { return };
    let doc = st.doc.clone();
    let mask_view = st.shown_mask();

    // Pasteboard, artboard shadows and paper.
    painter.rect_filled(rect, 0.0, t.pasteboard);
    if app.ui.view.artboards && !app.ui.view.outline {
        for ab in &doc.artboards {
            let r = xf.rect_to_screen(ab.rect);
            let q = xf.quad(ab.rect);
            // Hard 2 pt drop shadow, right and bottom (measured: #4d4d4d then #565656 on #606060).
            let shift = |d: f32| q.iter().map(|p| *p + vec2(d, d)).collect::<Vec<_>>();
            painter.add(Shape::convex_polygon(shift(2.0), Color32::from_black_alpha(26), Stroke::NONE));
            painter.add(Shape::convex_polygon(shift(1.0), Color32::from_black_alpha(52), Stroke::NONE));
            if st.transparency_grid && xf.rot == 0.0 {
                checker(&painter, r);
            } else {
                painter.add(Shape::convex_polygon(q, Color32::WHITE, Stroke::NONE));
            }
        }
    } else if app.ui.view.outline {
        for ab in &doc.artboards {
            painter.add(Shape::convex_polygon(xf.quad(ab.rect), Color32::WHITE, Stroke::NONE));
        }
    }
    if app.ui.view.grid {
        grid(&painter, &xf, doc.grid.spacing, doc.grid.subdivisions);
    }

    // Artwork raster.
    let ppp = ui.ctx().pixels_per_point();
    let (w, h) = ((rect.width() * ppp).round().max(1.0) as u32, (rect.height() * ppp).round().max(1.0) as u32);
    let key = CacheKey {
        doc: st.uid as usize,
        revision: st.revision,
        zoom: v.zoom,
        cx: v.center.x,
        cy: v.center.y,
        w,
        h,
        outline: app.ui.view.outline,
        trim: app.ui.view.trim_view,
        ppp,
        hidden: vec![],
        rot: v.rotation,
    };
    if !app.canvas.worker_started {
        app.canvas.worker_started = true;
        if std::env::var_os("VECTORCRAFT_SYNC_RENDER").is_none() {
            app.canvas.worker = crate::render_worker::Worker::spawn(ui.ctx().clone());
        }
    }
    // Upload finished background renders.
    if let Some(done) = app.canvas.worker.as_mut().and_then(|w| w.poll())
        && done.key.doc == key.doc
    {
        upload(app, ui.ctx(), &done.img);
        app.canvas.key = Some(done.key);
        app.perf.render_ms = done.ms;
        app.canvas.last_ms = done.ms;
    }
    if app.canvas.key.as_ref() != Some(&key) || app.canvas.texture.is_none() {
        let view = Affine::translate((w as f64 / 2.0, h as f64 / 2.0))
            * Affine::rotate(v.rotation.to_radians())
            * Affine::scale(v.zoom * ppp as f64)
            * Affine::translate(-v.center.to_vec2());
        let opts = vectorcraft_render::RenderOptions { outline: app.ui.view.outline, background: None, artboards: false, ..Default::default() };
        let opts = vectorcraft_render::RenderOptions {
            proof: vectorcraft_render::proof::active_proof(),
            overprint_preview: vectorcraft_render::proof::overprint_preview_on(),
            trim: app.ui.view.trim_view,
            tile_edge: vectorcraft_color::Color::from_hex(&app.session.prefs.pattern_tile_edge_color).map_or(opts.tile_edge, |c| {
                let [r, g, b, _] = c.to_rgba8(1.0);
                [r, g, b]
            }),
            mask_view,
            ..opts
        };
        // Light documents render synchronously (no lag vs overlays); heavy ones go to the worker.
        // A document switch renders synchronously so another document's frame is never shown.
        let same_doc = app.canvas.key.as_ref().is_some_and(|k| k.doc == key.doc);
        let heavy = app.canvas.last_ms > 8.0 && app.canvas.texture.is_some() && same_doc;
        match (&mut app.canvas.worker, heavy) {
            (Some(worker), true) => worker.submit(crate::render_worker::Job { key: key.clone(), doc: doc.clone(), w, h, view, opts }),
            _ => {
                let t0 = now_ms();
                let img = app.canvas.renderer.render(&doc, w, h, view, &opts);
                upload(app, ui.ctx(), &img);
                app.canvas.key = Some(key.clone());
                app.perf.render_ms = now_ms() - t0;
                app.canvas.last_ms = app.perf.render_ms;
            }
        }
    }
    if let (Some(tex), Some(k)) = (&app.canvas.texture, &app.canvas.key)
        && k.doc == key.doc
        && (k.rot - v.rotation).abs() < 1e-9
    {
        // Reproject the last frame if it was rendered for a different view.
        let old = Xf { rect, zoom: k.zoom, center: Point::new(k.cx, k.cy), rot: xf.rot };
        let _ = k.rot;
        let a = xf.to_screen(old.to_doc(rect.min));
        let b = xf.to_screen(old.to_doc(rect.max));
        painter.image(tex.id(), egui::Rect::from_min_max(a, b), egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    }
    // Artboard edges and names.
    let active_ab = 0;
    for (i, ab) in doc.artboards.iter().enumerate() {
        let r = xf.rect_to_screen(ab.rect);
        let c = if i == active_ab { Color32::from_gray(0) } else { Color32::from_gray(120) };
        painter.add(Shape::closed_line(xf.quad(ab.rect), Stroke::new(if i == active_ab { 1.0 } else { 0.6 }, c)));
        if app.session.tool_id() == "artboard" || doc.artboards.len() > 1 {
            painter.text(
                r.left_top() - vec2(0.0, 4.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{:02} - {}", i + 1, ab.name),
                egui::FontId::proportional(11.0),
                t.text_dim,
            );
        }
    }
    if app.ui.view.guides {
        for g in &doc.guides {
            let (a, b) = if g.vertical {
                let x = xf.to_screen(Point::new(g.pos, 0.0)).x;
                (pos2(x, rect.top()), pos2(x, rect.bottom()))
            } else {
                let y = xf.to_screen(Point::new(0.0, g.pos)).y;
                (pos2(rect.left(), y), pos2(rect.right(), y))
            };
            painter.line_segment([a, b], Stroke::new(1.0, t.guide));
        }
    }

    // Selection visuals and tool overlays.
    if app.ui.view.edges {
        hover_highlight(app, &painter, &xf);
        selection_overlay(app, &painter, &xf);
        if app.ui.view.text_threads {
            thread_overlay(app, &painter, &xf);
        }
        if app.ui.view.hidden_chars {
            hidden_chars_overlay(app, &painter, &xf);
        }
    }
    let view_info = app.view_info();
    // View → Hide Gradient Annotator hides the Gradient tool's annotator.
    if app.session.tool_id() != "gradient" || app.ui.view.gradient_annotator {
        let overlays = app.session.overlays(view_info);
        draw_overlays(&painter, &xf, &overlays, &t);
    }

    if app.ui.view.rulers && app.ui.screen_mode < 3 {
        rulers(ui, full, &xf, app.hover_doc, &t);
    }
    if app.ui.task_bar && !app.session.tool_busy() && app.ui.screen_mode < 3 {
        task_bar(app, ui, &xf);
    }
    // Cursor.
    if resp.hovered() {
        let m = ui.input(|i| i.modifiers);
        let space = ui.input(|i| i.key_down(egui::Key::Space));
        let panning = matches!(ui.data(|d| d.get_temp::<Drag>(drag_id())), Some(Drag::Pan { .. }));
        let cur = if panning {
            egui::CursorIcon::Grabbing
        } else if space || app.session.tool_id() == "hand" {
            egui::CursorIcon::Grab
        } else if app.session.tool_id() == "zoom" {
            if m.alt { egui::CursorIcon::ZoomOut } else { egui::CursorIcon::ZoomIn }
        } else if let Some(p) = app.hover_doc {
            let c = app.session.cursor(p, mods(m, space), view_info);
            let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("tool-cursor")));
            // Caps Lock gives precise (crosshair) cursors, like Illustrator; env opt-out for system cursors.
            let custom = std::env::var_os("VECTORCRAFT_SYSTEM_CURSORS").is_none();
            match ui.input(|i| i.pointer.hover_pos()) {
                Some(hp) if custom && crate::cursors::paint(&painter, c, hp) => egui::CursorIcon::None,
                _ => cursor_icon(c),
            }
        } else {
            egui::CursorIcon::Default
        };
        ui.ctx().set_cursor_icon(cur);
    }
}

fn cursor_icon(c: Cursor) -> egui::CursorIcon {
    use egui::CursorIcon as C;
    match c {
        Cursor::Arrow | Cursor::ArrowHollow | Cursor::CornerRadius => C::Default,
        Cursor::Move => C::Move,
        Cursor::Crosshair => C::Crosshair,
        Cursor::ResizeH => C::ResizeHorizontal,
        Cursor::ResizeV => C::ResizeVertical,
        Cursor::ResizeNwSe => C::ResizeNwSe,
        Cursor::ResizeNeSw => C::ResizeNeSw,
        Cursor::Rotate => C::Alias,
        Cursor::Pen | Cursor::PenAdd | Cursor::PenDelete | Cursor::PenClose | Cursor::PenContinue => C::Crosshair,
        Cursor::Text => C::Text,
        Cursor::Hand => C::Grab,
        Cursor::HandGrab => C::Grabbing,
        Cursor::ZoomIn => C::ZoomIn,
        Cursor::ZoomOut => C::ZoomOut,
        Cursor::Eyedropper => C::Crosshair,
        Cursor::NotAllowed => C::NotAllowed,
        Cursor::AddStop => C::Copy,
        Cursor::RemoveStop => C::NotAllowed,
    }
}

fn handle_input(app: &mut VectorcraftApp, ui: &Ui, resp: &egui::Response, rect: egui::Rect) {
    let (pointer, m, space, scroll, zoom_delta) =
        ui.input(|i| (i.pointer.clone(), i.modifiers, i.key_down(egui::Key::Space), i.smooth_scroll_delta, i.zoom_delta()));
    let v = *app.view().unwrap_or(&View::default());
    let xf = Xf::new(rect, &v);
    let hover = pointer.hover_pos().filter(|p| rect.contains(*p));
    app.hover_doc = hover.map(|p| xf.to_doc(p));
    let view = app.view_info();
    let drag: Option<Drag> = ui.data(|d| d.get_temp(drag_id()));

    // Zoom: pinch / Cmd-scroll / Alt-scroll around the pointer. Plain scroll pans.
    if resp.hovered() {
        let mut factor = zoom_delta as f64;
        if m.alt && scroll.y != 0.0 {
            factor *= (scroll.y as f64 * 0.01).exp();
        }
        if (factor - 1.0).abs() > 1e-6 {
            if let (Some(p), Some(vm)) = (hover, app.view_mut()) {
                let before = xf.to_doc(p);
                vm.zoom = (vm.zoom * factor).clamp(0.0313, 640.0);
                let nx = Xf { rect, zoom: vm.zoom, center: vm.center, rot: vm.rotation.to_radians() };
                let after = nx.to_doc(p);
                vm.center += before - after;
            }
        } else if (scroll.x != 0.0 || scroll.y != 0.0)
            && !m.alt
            && let Some(vm) = app.view_mut()
        {
            let d = Xf { rect, zoom: vm.zoom, center: vm.center, rot: vm.rotation.to_radians() }.delta_to_doc(scroll);
            vm.center -= d;
        }
    }

    let tool = app.session.tool_id();
    let pan_mode = space || tool == "hand";
    let middle_pan = matches!(drag, Some(Drag::Pan { middle: true, .. }));
    if pointer.primary_pressed() && resp.hovered() && !middle_pan {
        ui.ctx().memory_mut(|mem| mem.stop_text_input());
        app.ui.flyout = None;
        let p = hover.unwrap_or(rect.center());
        let d = if pan_mode {
            Drag::Pan { start: p, center: v.center, middle: false }
        } else if tool == "zoom" {
            Drag::ZoomBox { start: p }
        } else if tool == "rotateView" {
            let c = rect.center();
            Drag::RotateView { start_angle: (p.y - c.y).atan2(p.x - c.x) as f64, start_rot: v.rotation }
        } else {
            let selection_family = matches!(tool, "selection" | "directSelection" | "groupSelection");
            let mut kind = Drag::Tool;
            if m.command && !selection_family {
                ui.data_mut(|d| d.insert_temp(temp_tool_id(), tool.to_string()));
                app.select_tool("selection");
                kind = Drag::TempSelect;
            }
            let ev = PointerEvent { kind: PointerKind::Down, pos: xf.to_doc(p), mods: mods(m, space), pressure: 1.0 };
            dispatch(app, &ev, view);
            kind
        };
        ui.data_mut(|dd| dd.insert_temp(drag_id(), d));
    } else if drag.is_none() && resp.hovered() && pointer.button_pressed(egui::PointerButton::Middle) {
        // Middle-button drag pans the view whatever the tool.
        let start = hover.unwrap_or(rect.center());
        ui.data_mut(|dd| dd.insert_temp(drag_id(), Drag::Pan { start, center: v.center, middle: true }));
    } else if let Some(d) = drag {
        let p = pointer.interact_pos().unwrap_or(rect.center());
        let held = if middle_pan { pointer.button_down(egui::PointerButton::Middle) } else { pointer.primary_down() };
        if held {
            match d {
                Drag::Pan { start, center, .. } => {
                    let d = xf.delta_to_doc(p - start);
                    if let Some(vm) = app.view_mut() {
                        vm.center = center - d;
                    }
                }
                Drag::RotateView { start_angle, start_rot } => {
                    let c = rect.center();
                    let a = (p.y - c.y).atan2(p.x - c.x) as f64;
                    let mut deg = start_rot + (a - start_angle).to_degrees();
                    if m.shift {
                        deg = (deg / 15.0).round() * 15.0;
                    }
                    if let Some(vm) = app.view_mut() {
                        vm.rotation = vectorcraft_geom::normalize_deg(deg);
                    }
                }
                Drag::ZoomBox { .. } | Drag::Art { .. } => {}
                Drag::Tool | Drag::TempSelect if drag_art_out(app, ui, resp, p, view) => {
                    ui.data_mut(|dd| dd.insert_temp(drag_id(), Drag::Art { temp: d == Drag::TempSelect }));
                }
                Drag::Tool | Drag::TempSelect => {
                    if pointer.delta() != egui::Vec2::ZERO {
                        let ev = PointerEvent { kind: PointerKind::Drag, pos: xf.to_doc(p), mods: mods(m, space), pressure: 1.0 };
                        dispatch(app, &ev, view);
                    }
                }
            }
        } else {
            ui.data_mut(|dd| dd.remove::<Drag>(drag_id()));
            match d {
                Drag::ZoomBox { start } => {
                    let r = egui::Rect::from_two_pos(start, p);
                    if let Some(vm) = app.view_mut() {
                        if r.width() > 8.0 && r.height() > 8.0 {
                            let a = xf.to_doc(r.min);
                            let b = xf.to_doc(r.max);
                            vm.center = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
                            vm.zoom = (rect.width() as f64 / (b.x - a.x)).min(rect.height() as f64 / (b.y - a.y)).clamp(0.0313, 640.0);
                        } else {
                            let before = xf.to_doc(p);
                            vm.zoom = crate::state::next_zoom(vm.zoom, !m.alt);
                            let nx = Xf { rect, zoom: vm.zoom, center: vm.center, rot: vm.rotation.to_radians() };
                            vm.center += before - nx.to_doc(p);
                        }
                    }
                }
                Drag::Tool | Drag::TempSelect | Drag::Art { .. } => {
                    if !matches!(d, Drag::Art { .. }) {
                        let ev = PointerEvent { kind: PointerKind::Up, pos: xf.to_doc(p), mods: mods(m, space), pressure: 1.0 };
                        dispatch(app, &ev, view);
                    }
                    if matches!(d, Drag::TempSelect | Drag::Art { temp: true })
                        && let Some(prev) = ui.data(|dd| dd.get_temp::<String>(temp_tool_id()))
                    {
                        app.select_tool(&prev);
                    }
                }
                Drag::Pan { .. } | Drag::RotateView { .. } => {}
            }
        }
    } else if let Some(p) = hover
        && pointer.is_moving()
    {
        let ev = PointerEvent { kind: PointerKind::Move, pos: xf.to_doc(p), mods: mods(m, space), pressure: 1.0 };
        dispatch(app, &ev, view);
    }
    if resp.double_clicked()
        && let Some(p) = hover
    {
        let ev = PointerEvent { kind: PointerKind::DoubleClick, pos: xf.to_doc(p), mods: mods(m, space), pressure: 1.0 };
        dispatch(app, &ev, view);
    }
    if drag.is_some() || pointer.is_moving() {
        ui.ctx().request_repaint();
    }
}

/// A Selection tool move dragged off the canvas (to `p`, over a panel) turns into a panel drag of
/// the selected art ([`widgets::PanelDrag::Art`]): the move is dropped, so the art stays where it
/// was, and the panel it is released on takes it (the Graphic Styles panel makes a style of it).
fn drag_art_out(app: &mut VectorcraftApp, ui: &Ui, resp: &egui::Response, p: Pos2, view: vectorcraft_engine::ViewInfo) -> bool {
    let off = !ui.clip_rect().contains(p) || ui.ctx().layer_id_at(p).is_some_and(|l| l != resp.layer_id);
    let Some(st) = app.session.active().filter(|_| off && app.session.tool_id() == "selection") else { return false };
    // The Selection tool's interaction while it moves (or Alt-copies) the selection.
    if !st.interaction.as_ref().is_some_and(|i| i.label == "Move" || i.label == "Copy") {
        return false;
    }
    let ids = st.selection.objects.clone();
    if app.session.cancel_interaction().is_err() {
        return false;
    }
    // With nothing left to commit, the tool's mouse-up just returns it to rest.
    dispatch(app, &PointerEvent { kind: PointerKind::Up, pos: Point::ZERO, mods: Mods::default(), pressure: 1.0 }, view);
    egui::DragAndDrop::set_payload(ui.ctx(), widgets::PanelDrag::Art(ids));
    true
}

/// Send a pointer event to the active tool and act on UI requests (dialogs, tool switches).
pub fn dispatch(app: &mut VectorcraftApp, ev: &PointerEvent, view: vectorcraft_engine::ViewInfo) {
    match app.session.pointer(ev, view) {
        Ok(reqs) => {
            for r in reqs {
                match r {
                    vectorcraft_engine::UiRequest::Dialog(kind, p) => crate::dialogs::open_tool_dialog(app, &kind, p),
                    vectorcraft_engine::UiRequest::SwitchTool(t) => app.select_tool(&t),
                }
            }
        }
        Err(e) => app.status(e.to_string()),
    }
}

fn checker(p: &egui::Painter, r: egui::Rect) {
    p.rect_filled(r, 0.0, Color32::WHITE);
    let s = 8.0;
    let clip = p.with_clip_rect(r.intersect(p.clip_rect()));
    let (nx, ny) = (((r.width() / s).ceil() as i32).min(400), ((r.height() / s).ceil() as i32).min(400));
    for y in 0..ny {
        for x in 0..nx {
            if (x + y) % 2 == 1 {
                clip.rect_filled(egui::Rect::from_min_size(r.min + vec2(x as f32 * s, y as f32 * s), vec2(s, s)), 0.0, Color32::from_gray(204));
            }
        }
    }
}

fn grid(p: &egui::Painter, xf: &Xf, spacing: f64, subdiv: u32) {
    let r = xf.rect;
    let a = xf.to_doc(r.min);
    let b = xf.to_doc(r.max);
    let sub = spacing / subdiv.max(1) as f64;
    let step = if sub * xf.zoom >= 6.0 { sub } else { spacing };
    if step * xf.zoom < 4.0 {
        return;
    }
    let mut x = (a.x / step).floor() * step;
    while x <= b.x {
        let major = (x / spacing).round() * spacing == x || (x / spacing - (x / spacing).round()).abs() < 1e-6;
        let sx = xf.to_screen(Point::new(x, 0.0)).x;
        p.line_segment([pos2(sx, r.top()), pos2(sx, r.bottom())], Stroke::new(1.0, Color32::from_black_alpha(if major { 60 } else { 22 })));
        x += step;
    }
    let mut y = (a.y / step).floor() * step;
    while y <= b.y {
        let major = (y / spacing - (y / spacing).round()).abs() < 1e-6;
        let sy = xf.to_screen(Point::new(0.0, y)).y;
        p.line_segment([pos2(r.left(), sy), pos2(r.right(), sy)], Stroke::new(1.0, Color32::from_black_alpha(if major { 60 } else { 22 })));
        y += step;
    }
}

fn rulers(ui: &Ui, full: egui::Rect, xf: &Xf, hover: Option<Point>, t: &Tokens) {
    let p = ui.painter();
    let top = egui::Rect::from_min_max(pos2(full.left() + RULER, full.top()), pos2(full.right(), full.top() + RULER));
    let left = egui::Rect::from_min_max(pos2(full.left(), full.top() + RULER), pos2(full.left() + RULER, full.bottom()));
    let corner = egui::Rect::from_min_size(full.min, vec2(RULER, RULER));
    for r in [top, left, corner] {
        p.rect_filled(r, 0.0, t.ruler);
    }
    p.line_segment([top.left_bottom(), top.right_bottom()], Stroke::new(1.0, t.border));
    p.line_segment([left.right_top(), left.right_bottom()], Stroke::new(1.0, t.border));
    // Crosshair in the origin box.
    p.line_segment([corner.center() - vec2(4.0, 0.0), corner.center() + vec2(4.0, 0.0)], Stroke::new(1.0, t.ruler_tick));
    p.line_segment([corner.center() - vec2(0.0, 4.0), corner.center() + vec2(0.0, 4.0)], Stroke::new(1.0, t.ruler_tick));
    // Pick a label step that gives ≥ 50 px between labels.
    let steps = [1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0];
    let step = steps.iter().copied().find(|s| s * xf.zoom >= 50.0).unwrap_or(10000.0);
    let minor = step / 10.0;
    let font = egui::FontId::proportional(9.5);
    let a = xf.to_doc(top.left_top());
    let b = xf.to_doc(top.right_top());
    let clip_top = p.with_clip_rect(top);
    let mut x = (a.x / minor).floor() * minor;
    while x <= b.x {
        let sx = xf.to_screen(Point::new(x, 0.0)).x;
        let is_major = ((x / step).round() * step - x).abs() < minor * 0.01;
        let is_mid = ((x / (step / 2.0)).round() * (step / 2.0) - x).abs() < minor * 0.01;
        let len = if is_major {
            RULER
        } else if is_mid {
            7.0
        } else {
            4.0
        };
        clip_top.line_segment([pos2(sx, top.bottom() - len), pos2(sx, top.bottom())], Stroke::new(1.0, t.ruler_tick));
        if is_major {
            clip_top.text(pos2(sx + 2.0, top.top() + 1.0), egui::Align2::LEFT_TOP, format!("{}", x.round() as i64), font.clone(), t.ruler_tick);
        }
        x += minor;
    }
    let a = xf.to_doc(left.left_top());
    let b = xf.to_doc(left.left_bottom());
    let clip_left = p.with_clip_rect(left);
    let mut y = (a.y / minor).floor() * minor;
    while y <= b.y {
        let sy = xf.to_screen(Point::new(0.0, y)).y;
        let is_major = ((y / step).round() * step - y).abs() < minor * 0.01;
        let is_mid = ((y / (step / 2.0)).round() * (step / 2.0) - y).abs() < minor * 0.01;
        let len = if is_major {
            RULER
        } else if is_mid {
            7.0
        } else {
            4.0
        };
        clip_left.line_segment([pos2(left.right() - len, sy), pos2(left.right(), sy)], Stroke::new(1.0, t.ruler_tick));
        if is_major {
            // Vertical labels read top-to-bottom, one digit per line like Illustrator.
            let s = format!("{}", y.round() as i64);
            for (k, ch) in s.chars().enumerate() {
                clip_left.text(
                    pos2(left.left() + 4.0, sy + 2.0 + k as f32 * 8.5),
                    egui::Align2::LEFT_TOP,
                    ch.to_string(),
                    font.clone(),
                    t.ruler_tick,
                );
            }
        }
        y += minor;
    }
    if let Some(h) = hover {
        let s = xf.to_screen(h);
        clip_top.line_segment([pos2(s.x, top.top()), pos2(s.x, top.bottom())], Stroke::new(1.0, t.text));
        clip_left.line_segment([pos2(left.left(), s.y), pos2(left.right(), s.y)], Stroke::new(1.0, t.text));
    }
}

fn to_screen_path(bp: &BezPath, xf: &Xf) -> Vec<Vec<Pos2>> {
    // Flatten for drawing: a polyline per subpath (tolerance ~0.25 screen px).
    let mut out: Vec<Vec<Pos2>> = vec![];
    let a = xf.affine();
    let mut t = bp.clone();
    t.apply_affine(a);
    let mut cur: Vec<Pos2> = vec![];
    kurbo_flatten(&t, 0.25, &mut |el| match el {
        PathEl::MoveTo(p) => {
            if cur.len() > 1 {
                out.push(std::mem::take(&mut cur));
            }
            cur.clear();
            cur.push(pos2(p.x as f32, p.y as f32));
        }
        PathEl::LineTo(p) => cur.push(pos2(p.x as f32, p.y as f32)),
        PathEl::ClosePath => {
            if let Some(f) = cur.first().copied() {
                cur.push(f);
            }
        }
        _ => {}
    });
    if cur.len() > 1 {
        out.push(cur);
    }
    out
}

fn stroke_path(p: &egui::Painter, bp: &BezPath, xf: &Xf, s: Stroke) {
    for line in to_screen_path(bp, xf) {
        p.add(Shape::line(line, s));
    }
}

fn c32(rgb: [u8; 3]) -> Color32 {
    Color32::from_rgb(rgb[0], rgb[1], rgb[2])
}

/// Outline of a node for highlighting (paths, compound children, text/image bounds).
fn node_outline(n: &Node) -> BezPath {
    let mut bp = BezPath::new();
    n.walk(&mut |c| match &c.kind {
        NodeKind::Path { path, .. } => bp.extend(path.to_bezpath()),
        NodeKind::Text(_) | NodeKind::Image(_) | NodeKind::SymbolInstance { .. } => {
            if let Some(b) = c.geometric_bounds() {
                bp.extend(vectorcraft_geom::shapes::rectangle(b).to_bezpath());
            }
        }
        _ => {}
    });
    bp
}

/// The topmost editable object under document point `p` at `zoom` (3 px tolerance).
fn hit_at(app: &VectorcraftApp, p: Point, zoom: f64) -> Option<vectorcraft_doc::hit::Hit> {
    let opt = vectorcraft_doc::hit::HitOptions { tol: 3.0 / zoom, outline: app.ui.view.outline, path_only: false };
    vectorcraft_doc::hit::hit_test(&app.session.active()?.doc, p, opt)
}

/// A panel drag ([`widgets::PanelDrag`]) dropped on art acts on the object under the pointer,
/// selected or not: a paint (swatches, a Fill/Stroke proxy, the Gradient panel's thumbnail) goes
/// to its active proxy (`paint.setFill`/`paint.setStroke` with its `ids`; a gradient fits it), the
/// Appearance panel's thumbnail gives the object (the topmost one hit) the appearance it carries
/// (`appearance.copyFrom`), a graphic style is applied to it (`graphicStyle.apply`; with Alt, on
/// top of its appearance). A chip follows the pointer meanwhile.
fn panel_drop(app: &mut VectorcraftApp, ui: &Ui, resp: &egui::Response, xf: &Xf) {
    crate::panels::swatches::drag_preview(app, ui.ctx());
    let Some(pos) = ui.input(|i| i.pointer.interact_pos()) else { return };
    // Not through a floating panel over the canvas.
    if ui.ctx().layer_id_at(pos).is_some_and(|l| l != resp.layer_id) {
        return;
    }
    let Some(d) = resp.dnd_release_payload::<widgets::PanelDrag>() else { return };
    let Some(hit) = hit_at(app, xf.to_doc(pos), xf.zoom) else { return };
    let Some(st) = app.session.active() else { return };
    let (cmd, params) = match &*d {
        widgets::PanelDrag::Paint { params, .. } => {
            // Colour groups paint nothing.
            if params.is_null() {
                return;
            }
            let mut params = params.clone();
            params["ids"] = json!([vectorcraft_tools::xform::paint_owner(&st.doc, hit.leaf).0]);
            params["focus"] = json!(false);
            (crate::panels::proxy_cmd(app, false), params)
        }
        widgets::PanelDrag::Appearance(source) => {
            let target = hit.top_object(st.isolation);
            if target == *source {
                return;
            }
            ("appearance.copyFrom", json!({"source": source.0, "ids": [target.0]}))
        }
        widgets::PanelDrag::GraphicStyle(name) => {
            let add = ui.input(|i| i.modifiers.alt);
            ("graphicStyle.apply", json!({"name": name, "ids": [hit.top_object(st.isolation).0], "add": add}))
        }
        // Art dragged back onto the canvas: its move was already dropped.
        widgets::PanelDrag::Art(_) => return,
    };
    if let Err(e) = app.run(cmd, params) {
        app.status(e);
    }
}

fn hover_highlight(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(h) = app.hover_doc else { return };
    if app.session.tool_busy() || !matches!(app.session.tool_id(), "selection" | "directSelection" | "groupSelection") {
        return;
    }
    let Some(st) = app.session.active() else { return };
    let Some(hit) = hit_at(app, h, xf.zoom) else { return };
    let id = if app.session.tool_id() == "selection" { hit.top_object(st.isolation) } else { hit.leaf };
    if st.selection.contains(id) {
        return;
    }
    if let Some(n) = st.doc.node(id) {
        let color = c32(st.doc.layer_color(id));
        stroke_path(p, &node_outline(n), xf, Stroke::new(1.5, color));
    }
}

/// Selected anchors are drawn slightly deeper than the layer colour (#4f80ff → #3d82ff for Layer 1).
fn selected_anchor(c: Color32) -> Color32 {
    if c == Color32::from_rgb(0x4f, 0x80, 0xff) { Color32::from_rgb(0x3d, 0x82, 0xff) } else { c }
}

fn anchor_square(p: &egui::Painter, c: Pos2, color: Color32, filled: bool, size: f32) {
    let r = egui::Rect::from_center_size(c, vec2(size, size));
    if filled {
        p.rect_filled(r, 0.0, color);
    } else {
        p.rect_filled(r, 0.0, Color32::WHITE);
        p.rect_stroke(r, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
    }
}

/// Type → Show Hidden Characters: spaces as dots, ¶ at paragraph ends, # at the end of a story.
fn hidden_chars_overlay(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(st) = app.session.active() else { return };
    let clip = p.clip_rect();
    let color = Color32::from_rgb(0x4f, 0x9d, 0xff);
    let font = egui::FontId::proportional(((10.0 * xf.zoom) as f32).clamp(7.0, 18.0));
    st.doc.walk(|n| {
        let NodeKind::Text(tx) = &n.kind else { return };
        if !n.visible || n.geometric_bounds().is_none_or(|b| !xf.rect_to_screen(b).intersects(clip)) {
            return;
        }
        let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), tx);
        if lay.on_path {
            return;
        }
        let text = tx.plain_text();
        let to = |q: Point| xf.to_screen(tx.xf * q);
        for g in &lay.glyphs {
            if text.get(g.byte..).is_some_and(|s| s.starts_with(' ')) {
                p.circle_filled(to(Point::new(g.origin.x + g.advance / 2.0, g.origin.y - 3.0)), 1.2, color);
            }
        }
        for (i, line) in lay.lines.iter().enumerate() {
            let last = i + 1 == lay.lines.len();
            let breaks = text.get(line.end..).is_some_and(|s| s.starts_with('\n'));
            let (glyph, at) = if last && line.end >= text.len() {
                ("#", true)
            } else if breaks {
                ("¶", true)
            } else {
                ("", false)
            };
            if at {
                p.text(to(Point::new(line.x1 + 1.0, line.baseline)), egui::Align2::LEFT_BOTTOM, glyph, font.clone(), color);
            }
        }
    });
}

/// Text threads of the selected frames: a line from each frame's out port (bottom right) to the
/// next frame's in port (top left), like Illustrator's thread indicators.
fn thread_overlay(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(st) = app.session.active() else { return };
    for thread in &st.doc.text_threads {
        if !thread.iter().any(|id| st.selection.objects.contains(id)) {
            continue;
        }
        let color = c32(st.doc.layer_color(thread[0]));
        let frames: Vec<vectorcraft_geom::Rect> = thread.iter().filter_map(|id| st.doc.node(*id).and_then(|n| n.geometric_bounds())).collect();
        for w in frames.windows(2) {
            let (out, inp) = (xf.to_screen(Point::new(w[0].x1, w[0].y1)), xf.to_screen(Point::new(w[1].x0, w[1].y0)));
            p.line_segment([out, inp], Stroke::new(1.0, color));
            for port in [out, inp] {
                p.rect_filled(egui::Rect::from_center_size(port, egui::vec2(7.0, 7.0)), 0.0, Color32::WHITE);
                p.rect_stroke(egui::Rect::from_center_size(port, egui::vec2(7.0, 7.0)), 0.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
                p.line_segment([port - egui::vec2(2.0, 0.0), port + egui::vec2(2.0, 0.0)], Stroke::new(1.0, color));
            }
        }
    }
}

fn selection_overlay(app: &VectorcraftApp, p: &egui::Painter, xf: &Xf) {
    let Some(st) = app.session.active() else { return };
    let tool = app.session.tool_id();
    let direct = matches!(tool, "directSelection" | "pen" | "addAnchor" | "deleteAnchor" | "anchorPoint" | "curvature");
    for id in &st.selection.objects {
        let Some(n) = st.doc.node(*id) else { continue };
        let color = c32(st.doc.layer_color(*id));
        let partial = st.selection.partial(*id);
        // Path outlines.
        stroke_path(p, &node_outline(n), xf, Stroke::new(1.0, color));
        // Anchors (and handles for selected anchors in direct mode).
        n.walk(&mut |c| {
            let NodeKind::Path { path, .. } = &c.kind else { return };
            for (si, ai, a) in path.anchors() {
                let sel = match partial {
                    Some(set) => set.contains(&(si, ai)),
                    None => !direct || c.id == *id,
                };
                let sp = xf.to_screen(a.p);
                if sel && (direct || partial.is_some()) {
                    for h in [a.h_in, a.h_out] {
                        if h.distance(a.p) > 1e-6 {
                            let hp = xf.to_screen(h);
                            p.line_segment([sp, hp], Stroke::new(1.0, color));
                            p.circle_filled(hp, 2.75, color);
                        }
                    }
                }
                anchor_square(
                    p,
                    sp,
                    if sel && partial.is_some() { selected_anchor(color) } else { color },
                    sel,
                    if partial.is_some() || direct { 5.0 } else { 4.0 },
                );
            }
        });
        // Text: baseline marker.
        if let NodeKind::Text(tx) = &n.kind {
            let o = xf.to_screen(tx.xf * Point::ZERO);
            let b = n.geometric_bounds().unwrap_or_default();
            let e = xf.to_screen(Point::new(b.x1, (tx.xf * Point::ZERO).y));
            p.line_segment([o, e], Stroke::new(1.0, color));
            p.circle_filled(o, 2.5, color);
        }
    }
    // Live Corners widgets (Selection / Direct Selection on a single live rectangle).
    if matches!(tool, "selection" | "directSelection")
        && app.ui.view.corner_widgets
        && let Some(w) = vectorcraft_tools::corners::CornerWidgets::of(&st.doc, &st.selection, xf.zoom)
    {
        let color = c32(st.doc.layer_color(w.id));
        for sp in w.points.map(|q| xf.to_screen(q)) {
            p.circle_filled(sp, 3.0, Color32::WHITE);
            p.circle_stroke(sp, 3.0, Stroke::new(1.0, color));
            p.circle_filled(sp, 1.0, color);
        }
    }
    // Bounding box with handles (Selection tool).
    if tool == "selection" && app.ui.view.bounding_box && !st.selection.is_empty() && st.selection.anchors.is_empty() {
        let Some(b) = st.doc.bounds_of(&st.selection.objects, false) else { return };
        let color = c32(st.doc.layer_color(st.selection.objects[0]));
        p.add(Shape::closed_line(xf.quad(b), Stroke::new(1.0, color)));
        for h in vectorcraft_tools::bbox::Handle::ALL {
            let c = xf.to_screen(h.pos(b));
            let hr = egui::Rect::from_center_size(c, vec2(6.0, 6.0));
            p.rect_filled(hr, 0.0, Color32::WHITE);
            p.rect_stroke(hr, 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
        }
    }
}

fn draw_overlays(p: &egui::Painter, xf: &Xf, overlays: &[Overlay], t: &Tokens) {
    for o in overlays {
        match o {
            Overlay::Marquee(r) => {
                let sr = xf.rect_to_screen(*r);
                let pts = [sr.left_top(), sr.right_top(), sr.right_bottom(), sr.left_bottom(), sr.left_top()];
                for w in pts.windows(2) {
                    p.extend(Shape::dashed_line(&[w[0], w[1]], Stroke::new(1.0, Color32::from_gray(90)), 3.0, 3.0));
                }
            }
            Overlay::Path { path, color, width, dashed } => {
                let s = Stroke::new(*width, c32(*color));
                if *dashed {
                    for line in to_screen_path(path, xf) {
                        p.extend(Shape::dashed_line(&line, s, 4.0, 3.0));
                    }
                } else {
                    stroke_path(p, path, xf, s);
                }
            }
            Overlay::Line { a, b, color, dashed } => {
                let s = Stroke::new(1.0, c32(*color));
                let (a, b) = (xf.to_screen(*a), xf.to_screen(*b));
                if *dashed {
                    p.extend(Shape::dashed_line(&[a, b], s, 4.0, 3.0));
                } else {
                    p.line_segment([a, b], s);
                }
            }
            Overlay::Anchor { p: pt, color, filled, size } => anchor_square(p, xf.to_screen(*pt), c32(*color), *filled, *size),
            Overlay::Handle { p: pt, color } => {
                p.circle_filled(xf.to_screen(*pt), 2.8, c32(*color));
            }
            Overlay::Label { p: pt, text, color } => {
                let sp = xf.to_screen(*pt) + vec2(8.0, -14.0);
                p.text(sp, egui::Align2::LEFT_TOP, text, egui::FontId::proportional(11.0), c32(*color));
            }
            Overlay::Highlight { quad, color } => {
                let c = Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]);
                p.add(Shape::convex_polygon(quad.iter().map(|q| xf.to_screen(*q)).collect(), c, Stroke::NONE));
            }
            Overlay::Measure { p: pt, text } => {
                let sp = xf.to_screen(*pt) + vec2(14.0, 14.0);
                let galley = p.layout(text.clone(), egui::FontId::proportional(11.0), Color32::WHITE, 200.0);
                let r = egui::Rect::from_min_size(sp, galley.size() + vec2(12.0, 8.0));
                p.rect_filled(r, CornerRadius::same(3), t.measure_bg);
                p.galley(sp + vec2(6.0, 4.0), galley, Color32::WHITE);
            }
            Overlay::Swatch { p: pt, color, selected } => {
                // A white disc under the colour shows its opacity; a dark rim keeps it readable on
                // any art, and the accent ring marks the selected stop.
                let c = xf.to_screen(*pt);
                p.circle_filled(c, 6.0, Color32::WHITE);
                p.circle_filled(c, 5.0, Color32::from_rgba_unmultiplied(color[0], color[1], color[2], color[3]));
                p.circle_stroke(c, 6.0, Stroke::new(1.0, Color32::from_gray(32)));
                if *selected {
                    p.circle_stroke(c, 8.0, Stroke::new(2.0, t.accent));
                }
            }
        }
    }
}

/// The Home screen shown when no document is open.
fn home(app: &mut VectorcraftApp, ui: &mut Ui, rect: egui::Rect) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(rect, 0.0, t.panel_darker);
    let inner = rect.shrink2(vec2((rect.width() - 820.0).max(40.0) / 2.0, 60.0));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner).layout(egui::Layout::top_down(egui::Align::Min)));
    let ui = &mut child;
    ui.label(egui::RichText::new("Welcome to VectorCraft").font(theme::semibold(26.0)).color(t.text));
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Vector illustration — fast, open, scriptable.").size(14.0).color(t.text_dim));
    ui.add_space(22.0);
    ui.horizontal(|ui| {
        if widgets::primary_button(ui, "New file").clicked() {
            app.run("file.newDialog", json!({})).ok();
        }
        ui.add_space(8.0);
        if widgets::secondary_button(ui, "Open").clicked() {
            app.run("file.open", json!({})).ok();
        }
    });
    ui.add_space(28.0);
    ui.label(egui::RichText::new("Quickly start a new file").font(theme::semibold(14.0)).color(t.text));
    ui.add_space(10.0);
    let presets: [(&str, &str, f64, f64); 6] = [
        ("Letter", "612 × 792 pt", 612.0, 792.0),
        ("A4", "595.28 × 841.89 pt", 595.28, 841.89),
        ("Web 1920", "1920 × 1080 px", 1920.0, 1080.0),
        ("Mobile", "390 × 844 px", 390.0, 844.0),
        ("Postcard", "288 × 432 pt", 432.0, 288.0),
        ("Square", "1080 × 1080 px", 1080.0, 1080.0),
    ];
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(14.0, 14.0);
        for (name, size, w, h) in presets {
            let (r, resp) = ui.allocate_exact_size(vec2(120.0, 132.0), Sense::click());
            ui.painter().rect_filled(r, CornerRadius::same(8), if resp.hovered() { t.hover } else { t.panel });
            let s = (70.0 / w.max(h)) as f32;
            let pr = egui::Rect::from_center_size(r.center_top() + vec2(0.0, 50.0), vec2(w as f32 * s, h as f32 * s));
            ui.painter().rect_filled(pr.translate(vec2(2.0, 2.0)), 0.0, Color32::from_black_alpha(80));
            ui.painter().rect_filled(pr, 0.0, Color32::WHITE);
            ui.painter().text(r.center_bottom() - vec2(0.0, 30.0), egui::Align2::CENTER_CENTER, name, theme::semibold(12.5), t.text);
            ui.painter().text(r.center_bottom() - vec2(0.0, 14.0), egui::Align2::CENTER_CENTER, size, egui::FontId::proportional(11.0), t.text_dim);
            if resp.clicked() {
                app.run("file.new", json!({"width": w, "height": h, "units": if size.ends_with("px") { "Pixels" } else { "Points" }})).ok();
            }
        }
    });
    ui.add_space(28.0);
    ui.label(egui::RichText::new("Community").font(theme::semibold(14.0)).color(t.text));
    ui.add_space(10.0);
    crate::community::links(app, ui);
}

fn kurbo_flatten(p: &BezPath, tol: f64, f: &mut impl FnMut(PathEl)) {
    vectorcraft_geom::kurbo::flatten(p.elements().iter().copied(), tol, f);
}

fn upload(app: &mut VectorcraftApp, ctx: &egui::Context, img: &vectorcraft_render::Rendered) {
    let color = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
    match &mut app.canvas.texture {
        Some(tex) => tex.set(color, egui::TextureOptions::LINEAR),
        None => app.canvas.texture = Some(ctx.load_texture("canvas", color, egui::TextureOptions::LINEAR)),
    }
}

/// The Contextual Task Bar: a floating pill under the selection with the most likely next actions.
fn task_bar(app: &mut VectorcraftApp, ui: &mut Ui, xf: &Xf) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else { return };
    if st.selection.is_empty() || !matches!(app.session.tool_id(), "selection" | "directSelection" | "groupSelection") {
        return;
    }
    let Some(b) = st.doc.bounds_of(&st.selection.objects, true) else { return };
    let n = st.selection.len();
    let first = st.selection.objects.first().and_then(|id| st.doc.node(*id)).cloned();
    let is_group = first.as_ref().is_some_and(|f| matches!(f.kind, NodeKind::Group { .. }));
    let is_text = first.as_ref().is_some_and(|f| matches!(f.kind, NodeKind::Text(_)));
    let mut items: Vec<(&str, &str, &str)> = vec![]; // (label, icon, command)
    if n > 1 {
        items.push(("Group", "group", "object.group"));
        items.push(("Unite", "squares-unite", "object.pathfinder.unite"));
    } else if is_group {
        items.push(("Ungroup", "ungroup", "object.ungroup"));
        items.push(("Isolate", "square-dashed", "object.isolate"));
    } else if is_text {
        items.push(("Create Outlines", "type", "type.createOutlines"));
    } else {
        items.push(("Offset Path", "square-dashed", "object.path.offsetPath"));
        items.push(("Simplify", "spline", "object.path.simplify"));
    }
    items.push(("Duplicate", "copy", "edit.duplicate"));
    let fill = first.as_ref().map(|f| f.appearance.fill_paint()).unwrap_or_default();
    let anchor = xf.to_screen(Point::new(b.center().x, b.y1));
    let est_w = 118.0 + items.iter().map(|(l, _, _)| l.len() as f32 * 7.2 + 44.0).sum::<f32>();
    let x = (anchor.x - est_w / 2.0).clamp(xf.rect.left() + 8.0, (xf.rect.right() - est_w - 8.0).max(xf.rect.left() + 8.0));
    let y = (anchor.y + 28.0).min(xf.rect.bottom() - 56.0);
    let mut run: Option<String> = None;
    egui::Area::new(egui::Id::new("task-bar")).order(egui::Order::Middle).fixed_pos(pos2(x, y)).show(ui.ctx(), |ui| {
        egui::Frame::NONE
            .fill(t.panel)
            .stroke(Stroke::new(1.0, t.tool_active))
            .corner_radius(CornerRadius::same(5))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .shadow(egui::epaint::Shadow { offset: [0, 3], blur: 10, spread: 0, color: Color32::from_black_alpha(70) })
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let (g, _) = ui.allocate_exact_size(vec2(4.0, 28.0), Sense::hover());
                    ui.painter().rect_filled(g.shrink2(vec2(0.5, 4.0)), CornerRadius::same(2), t.button_border);
                    for (label, icon, cmd) in &items {
                        let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(13.0), t.text_strong);
                        let (r, resp) = ui.allocate_exact_size(vec2(galley.size().x + 38.0, 30.0), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(3), t.hover);
                        }
                        ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0, t.button_border), StrokeKind::Inside);
                        crate::icons::paint(ui, icon, egui::Rect::from_min_size(r.min + vec2(8.0, 7.0), vec2(16.0, 16.0)), t.icon);
                        ui.painter().galley(pos2(r.left() + 30.0, r.center().y - galley.size().y / 2.0), galley, t.text_strong);
                        if resp.clicked() {
                            run = Some(cmd.to_string());
                        }
                    }
                    let (r, resp) = ui.allocate_exact_size(vec2(26.0, 30.0), Sense::click());
                    widgets::paint_chip(ui, egui::Rect::from_center_size(r.center(), vec2(16.0, 16.0)), &fill);
                    ui.painter().rect_stroke(
                        egui::Rect::from_center_size(r.center(), vec2(16.0, 16.0)),
                        0.0,
                        Stroke::new(1.0, t.button_border),
                        StrokeKind::Outside,
                    );
                    if resp.on_hover_text("Fill").clicked() {
                        app.session.fill_active = true;
                        app.ui.open_panel = Some("swatches".into());
                    }
                    if widgets::icon_button(ui, "lock", "Lock (⌘2)", false, 30.0).clicked() {
                        run = Some("object.lock".into());
                    }
                    if widgets::icon_button(ui, "ellipsis", "Hide Contextual Task Bar", false, 30.0).clicked() {
                        run = Some("window.taskBar".into());
                    }
                });
            });
    });
    if let Some(c) = run {
        crate::menus::invoke(app, &c, json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;

    /// One headless canvas frame on an 800 × 600 window.
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), events, ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| show(app, ui));
        out.textures_delta.clear();
    }

    fn middle(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Middle, pressed, modifiers: Default::default() }
    }

    #[test]
    fn middle_drag_pans_the_view_with_any_tool() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        // A drawing tool: the middle button must pan, never draw.
        app.select_tool("rectangle");
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos2(400.0, 300.0))]);
        let before = *app.view().unwrap();
        frame(&mut app, &ctx, vec![middle(pos2(400.0, 300.0), true)]);
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos2(450.0, 330.0))]);
        frame(&mut app, &ctx, vec![middle(pos2(450.0, 330.0), false)]);
        let after = *app.view().unwrap();
        assert_eq!(after.zoom, before.zoom);
        assert!((after.center.x - (before.center.x - 50.0 / before.zoom)).abs() < 1e-6);
        assert!((after.center.y - (before.center.y - 30.0 / before.zoom)).abs() < 1e-6);
        // Released: moving no longer pans, and nothing was drawn.
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos2(300.0, 200.0))]);
        assert_eq!(app.view().unwrap().center, after.center);
        assert_eq!(app.session.active().unwrap().doc.art_bounds(), None);
    }

    #[test]
    fn swatches_and_proxy_paints_dropped_on_art_fill_the_object_hit() {
        use vectorcraft_color::{Color, GradientGeom, GradientPaint, Paint};
        use widgets::{PanelDrag, SwatchRows};
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        let id = app.session.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap()["id"].as_u64().unwrap();
        app.session.execute("select.none", &json!({})).unwrap();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        let xf = Xf::new(app.canvas_rect.unwrap(), app.view().unwrap());
        let drop = |app: &mut VectorcraftApp, d: PanelDrag, at: Point| {
            let at = xf.to_screen(at);
            egui::DragAndDrop::set_payload(&ctx, d);
            let up = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() };
            frame(app, &ctx, vec![egui::Event::PointerMoved(at), up]);
        };
        let fill = |app: &VectorcraftApp| app.session.active().unwrap().doc.node(vectorcraft_doc::NodeId(id)).unwrap().appearance.fill_paint();
        let red = Paint::solid(Color::from_hex("#ed1c24").unwrap());
        let rows = SwatchRows { grabbed: "Red".into(), names: vec!["Red".into()], groups: false };
        let swatch = PanelDrag::Paint { paint: red.clone(), params: json!({"swatch": "Red"}), rows: Some(rows) };
        // Off the art nothing happens.
        drop(&mut app, swatch.clone(), Point::new(300.0, 250.0));
        assert_eq!(fill(&app), Paint::solid(Color::WHITE));
        drop(&mut app, swatch, Point::new(100.0, 100.0));
        assert_eq!(fill(&app), red, "paint.setFill with the hit id");
        assert!(app.session.active().unwrap().selection.is_empty(), "the selection stays as it was");
        // A proxy's paint works the same.
        drop(&mut app, PanelDrag::paint(Paint::solid(Color::rgb(0.0, 0.0, 1.0))), Point::new(60.0, 60.0));
        assert_eq!(fill(&app), Paint::solid(Color::rgb(0.0, 0.0, 1.0)));
        // A dragged gradient (the Gradient panel's thumbnail, a proxy) fits the object it lands on.
        let mut g = GradientPaint::new(Default::default());
        g.geom = Some(GradientGeom { start: Point::new(0.0, 0.0), end: Point::new(10.0, 0.0), aspect: 1.0 });
        drop(&mut app, PanelDrag::paint(Paint::Gradient(Box::new(g))), Point::new(100.0, 100.0));
        assert!(matches!(fill(&app), Paint::Gradient(g) if g.geom.is_none()));
    }

    #[test]
    fn dropping_the_appearance_thumbnail_on_art_copies_the_appearance() {
        use vectorcraft_doc::NodeId;
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: serde_json::Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 400, "height": 300}));
        let a = run(&mut app, "shape.rectangle", json!({"x": 20, "y": 20, "width": 100, "height": 100}))["id"].as_u64().unwrap();
        run(&mut app, "effect.apply", json!({"effect": "distort.twist"}));
        run(&mut app, "transparency.set", json!({"opacity": 40}));
        let b = run(&mut app, "shape.rectangle", json!({"x": 200, "y": 20, "width": 100, "height": 100}))["id"].as_u64().unwrap();
        run(&mut app, "select.set", json!({ "ids": [a] }));
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        // The Appearance panel's thumbnail (its drag payload) released over the second rectangle.
        let xf = Xf::new(app.canvas_rect.unwrap(), app.view().unwrap());
        let drop = |app: &mut VectorcraftApp, at: Point| {
            let at = xf.to_screen(at);
            egui::DragAndDrop::set_payload(&ctx, widgets::PanelDrag::Appearance(NodeId(a)));
            frame(app, &ctx, vec![egui::Event::PointerMoved(at)]);
            let up = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() };
            frame(app, &ctx, vec![up]);
        };
        drop(&mut app, Point::new(250.0, 70.0));
        let doc = &app.session.active().unwrap().doc;
        let (na, nb) = (doc.node(NodeId(a)).unwrap(), doc.node(NodeId(b)).unwrap());
        assert_eq!((&nb.appearance, nb.opacity), (&na.appearance, na.opacity));
        assert_eq!(nb.appearance.effects[0].id, "distort.twist");
        // Dropped on empty canvas or on the source itself: nothing changes.
        let undo_len = app.session.active().unwrap().history.undo.len();
        drop(&mut app, Point::new(350.0, 250.0));
        drop(&mut app, Point::new(70.0, 70.0));
        assert_eq!(app.session.active().unwrap().history.undo.len(), undo_len);
    }

    #[test]
    fn a_graphic_style_dropped_on_art_applies_to_the_object_hit() {
        use vectorcraft_doc::NodeId;
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        let run = |app: &mut VectorcraftApp, id: &str, p: serde_json::Value| app.session.execute(id, &p).unwrap();
        run(&mut app, "file.new", json!({"width": 400, "height": 300}));
        let a = NodeId(run(&mut app, "shape.rectangle", json!({"x": 20, "y": 20, "width": 100, "height": 100}))["id"].as_u64().unwrap());
        let b = NodeId(run(&mut app, "shape.rectangle", json!({"x": 200, "y": 20, "width": 100, "height": 100}))["id"].as_u64().unwrap());
        run(&mut app, "select.set", json!({ "ids": [a.0] }));
        let name = app.session.active().unwrap().doc.graphic_styles[2].name.clone();
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        let xf = Xf::new(app.canvas_rect.unwrap(), app.view().unwrap());
        let at = xf.to_screen(Point::new(250.0, 70.0));
        egui::DragAndDrop::set_payload(&ctx, widgets::PanelDrag::GraphicStyle(name.clone()));
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(at)]);
        let up = egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() };
        frame(&mut app, &ctx, vec![up]);
        let st = app.session.active().unwrap();
        let g = st.doc.graphic_style(&name).unwrap();
        // `graphicStyle.apply` with the hit object's id: linked, the selection left alone.
        assert_eq!(st.doc.node(b).unwrap().graphic_style, Some(g.id));
        assert_eq!(st.doc.node(b).unwrap().appearance, g.appearance);
        assert_eq!(st.doc.node(a).unwrap().graphic_style, None);
        assert_eq!(st.selection.objects, [a]);
    }

    #[test]
    fn art_moved_off_the_canvas_becomes_a_panel_drag_and_stays_put() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
        let id = app.session.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 100, "height": 100})).unwrap()["id"].as_u64().unwrap();
        let id = vectorcraft_doc::NodeId(id);
        app.select_tool("selection");
        let ctx = egui::Context::default();
        frame(&mut app, &ctx, vec![]);
        let xf = Xf::new(app.canvas_rect.unwrap(), app.view().unwrap());
        let before = app.session.active().unwrap().doc.clone();
        let undo = app.session.active().unwrap().history.undo.len();
        let button = |pos: Pos2, pressed: bool| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        };
        let start = xf.to_screen(Point::new(100.0, 100.0));
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, true)]);
        // Moving on the canvas moves the art...
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start + vec2(40.0, 0.0))]);
        assert!(app.session.active().unwrap().interaction.is_some());
        // ...until the pointer leaves it: the move is dropped and the panels get the art.
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(pos2(850.0, 100.0))]);
        assert!(app.session.active().unwrap().interaction.is_none());
        assert_eq!(*app.session.active().unwrap().doc, *before, "the art is back where it was");
        assert_eq!(egui::DragAndDrop::payload::<widgets::PanelDrag>(&ctx).as_deref(), Some(&widgets::PanelDrag::Art(vec![id])));
        assert!(!app.session.tool_busy());
        // Released anywhere, nothing moves and nothing is recorded.
        frame(&mut app, &ctx, vec![egui::Event::PointerMoved(start), button(start, false)]);
        let st = app.session.active().unwrap();
        assert_eq!(*st.doc, *before);
        assert_eq!(st.history.undo.len(), undo);
    }
}
