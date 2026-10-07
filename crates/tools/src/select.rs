//! The Selection tool (V): click/shift-click, marquee, move (Alt copies, Shift constrains),
//! bounding-box scale (Shift proportional, Alt from centre) and rotate (outside corners, Shift 45°),
//! drag a live rectangle's corner widget to round its corners, double-click to enter isolation mode.
//! The bounding box stands at the selection's own angle after a rotation, so its handles scale
//! along the objects' axes. A handle drag resizes area type's frame (the type area) instead of
//! scaling its type: the text reflows at its size.

use serde_json::{Value, json};
use vectorcraft_doc::hit::{hit_test, marquee};
use vectorcraft_doc::{NodeId, OrientedBox};
use vectorcraft_geom::{Affine, Point, Rect};

use crate::bbox::{Handle, hit_handle, in_rotate_zone, move_delta, rotate_for_drag, scale_for_drag};
use crate::corners::{CornerDrag, over_widget};
use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, json_ids};

#[derive(Clone, Debug, Default)]
enum State {
    #[default]
    Idle,
    /// Pressed on an object; becomes Moving after the drag threshold.
    Moving {
        start: Point,
        began: bool,
        /// Shift-pressed on this selected object: released without a drag, it leaves the
        /// selection.
        deselect: Option<NodeId>,
    },
    Scaling {
        handle: Handle,
        bx: OrientedBox,
        /// Area type is selected: its frame resizes (`typeAreas`), its type keeps its size.
        areas: bool,
    },
    Rotating {
        center: Point,
        start: Point,
    },
    Marquee {
        start: Point,
        cur: Point,
        add: bool,
    },
    /// Dragging a Live Corners widget.
    Corner(CornerDrag),
}

#[derive(Default)]
pub struct SelectionTool {
    state: State,
    measure: Option<(Point, String)>,
    guides: Vec<Overlay>,
    targets: Option<crate::guides::Targets>,
    start_bounds: Option<Rect>,
}

pub fn matrix_json(a: Affine) -> Value {
    let c = a.as_coeffs();
    json!([c[0], c[1], c[2], c[3], c[4], c[5]])
}

/// Selection bounds used for the bounding box: visual bounds with Use Preview Bounds, else
/// geometric bounds.
pub fn selection_bounds(cx: &ToolContext) -> Option<Rect> {
    cx.doc.bounds_of(&cx.selection.objects, cx.preview_bounds)
}

/// [`selection_bounds`] square to the selection's own angle: the bounding box the Selection tool
/// shows and drags (rotated after a rotation).
pub fn selection_box(cx: &ToolContext) -> Option<OrientedBox> {
    cx.doc.oriented_bounds(&cx.selection.objects, cx.preview_bounds)
}

/// What of the bounding box is under the pointer.
enum BoxHit {
    Handle(Handle),
    /// Just outside a corner.
    Rotate,
}

/// The bounding-box handle or rotate zone under `p`.
fn box_hit(cx: &ToolContext, b: &OrientedBox, p: Point) -> Option<BoxHit> {
    let (tol, lp) = (cx.tol(5.0), b.to_local(p));
    if let Some(h) = hit_handle(b.rect, lp, tol) {
        return Some(BoxHit::Handle(h));
    }
    in_rotate_zone(b.rect, lp, tol, cx.tol(18.0)).map(|_| BoxHit::Rotate)
}

impl SelectionTool {
    fn drag_threshold(cx: &ToolContext) -> f64 {
        cx.tol(3.0)
    }
}

impl Tool for SelectionTool {
    fn id(&self) -> &'static str {
        "selection"
    }

    fn busy(&self) -> bool {
        !matches!(self.state, State::Idle)
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        let m = ev.mods;
        match (ev.kind, self.state.clone()) {
            (PointerKind::DoubleClick, _) => {
                self.state = State::Idle;
                if let Some(h) = hit_test(cx.doc, p, cx.hit_options()) {
                    let top = h.top_object(cx.isolation);
                    if cx.doc.node(top).is_some_and(|n| matches!(n.kind, vectorcraft_doc::NodeKind::Group { .. })) {
                        return vec![Action::Exec("object.isolate".into(), json!({ "id": top.0 }))];
                    }
                    if cx.doc.node(top).is_some_and(|n| matches!(n.kind, vectorcraft_doc::NodeKind::Text(_))) {
                        return vec![Action::SwitchTool("type".into())];
                    }
                } else if cx.isolation.is_some() {
                    return vec![Action::Exec("object.exitIsolation".into(), json!({}))];
                }
                vec![]
            }
            (PointerKind::Down, _) => {
                // 1. Live Corners widgets, then the bounding-box handles of the current selection.
                if let Some(c) = CornerDrag::hit(cx, p) {
                    self.state = State::Corner(c);
                    return vec![];
                }
                if cx.show_bbox
                    && let Some(bx) = selection_box(cx)
                {
                    match box_hit(cx, &bx, p) {
                        Some(BoxHit::Handle(handle)) => {
                            let is_area = |id: &NodeId| cx.doc.node(*id).is_some_and(is_area_type);
                            let areas = cx.selection.objects.iter().any(is_area);
                            // Only area type: the step resizes type areas; anything else scales.
                            let label = if areas && cx.selection.objects.iter().all(is_area) { "Resize Type Area" } else { "Scale" };
                            self.state = State::Scaling { handle, bx, areas };
                            return vec![Action::Begin(label.into())];
                        }
                        Some(BoxHit::Rotate) => {
                            self.state = State::Rotating { center: bx.center(), start: p };
                            return vec![Action::Begin("Rotate".into())];
                        }
                        None => {}
                    }
                }
                // 2. Objects.
                match hit_test(cx.doc, p, cx.hit_options()) {
                    Some(h) => {
                        let top = h.top_object(cx.isolation);
                        let mut out = vec![];
                        let mut deselect = None;
                        if m.shift {
                            // A Shift-click takes a selected object out of the selection when it
                            // is released; a Shift-drag moves the selection, constrained.
                            if cx.selection.contains(top) {
                                deselect = Some(top);
                            } else {
                                out.push(Action::Exec("select.toggle".into(), json!({ "id": top.0 })));
                            }
                        } else if !cx.selection.contains(top) {
                            out.push(Action::Exec("select.set".into(), json!({ "ids": [top.0] })));
                        }
                        self.state = State::Moving { start: p, began: false, deselect };
                        out
                    }
                    None => {
                        self.state = State::Marquee { start: p, cur: p, add: m.shift };
                        vec![]
                    }
                }
            }
            (PointerKind::Drag, State::Moving { start, began, .. }) => {
                let mut out = vec![];
                if !began {
                    if p.distance(start) < Self::drag_threshold(cx) {
                        return out;
                    }
                    out.push(Action::Begin(if m.alt { "Copy".into() } else { "Move".into() }));
                }
                let mut d = move_delta(start, p, m.shift);
                if !began {
                    self.start_bounds = selection_bounds(cx);
                    self.targets = cx.smart_guides.then(|| crate::guides::Targets::collect(cx.doc, &cx.selection.objects, None));
                }
                self.guides.clear();
                if let (Some(t), Some(b)) = (&self.targets, self.start_bounds) {
                    let (adj, ov) = t.snap_rect(b + d, cx.tol(5.0));
                    d += adj;
                    self.guides = ov;
                }
                // Snap to Pixel: the moved selection's top-left lands on whole pixels.
                if cx.snap_to_pixel
                    && let Some(b) = self.start_bounds
                {
                    d = vectorcraft_geom::Vec2::new((b.x0 + d.x).round() - b.x0, (b.y0 + d.y).round() - b.y0);
                    self.guides.clear();
                }
                self.state = State::Moving { start, began: true, deselect: None };
                self.measure = Some((p, cx.offset_label(d.x, d.y)));
                out.push(Action::Preview("object.transform".into(), json!({ "matrix": matrix_json(Affine::translate(d)), "copy": m.alt })));
                out
            }
            (PointerKind::Drag, State::Scaling { handle, bx, areas }) => {
                // Scale in the box's own frame: along the objects' axes when it is rotated.
                let a = scale_for_drag(bx.rect, handle, bx.to_local(p), m.shift, m.alt);
                let nr = a.transform_rect_bbox(bx.rect);
                self.measure = Some((p, cx.size_label(nr.width(), nr.height())));
                let mut params = json!({ "matrix": matrix_json(bx.conjugate(a)), "copy": false });
                if areas {
                    params["typeAreas"] = json!(true);
                }
                vec![Action::Preview("object.transform".into(), params)]
            }
            (PointerKind::Drag, State::Rotating { center, start, .. }) => {
                let (a, deg) = rotate_for_drag(center, start, p, m.shift);
                self.measure = Some((p, format!("{:.1}°", -deg)));
                vec![Action::Preview("object.transform".into(), json!({ "matrix": matrix_json(a), "copy": false }))]
            }
            (PointerKind::Drag, State::Marquee { start, add, .. }) => {
                self.state = State::Marquee { start, cur: p, add };
                vec![]
            }
            (PointerKind::Drag, State::Corner(mut c)) => {
                let out = c.drag(cx, p);
                self.state = State::Corner(c);
                out
            }
            (PointerKind::Up, State::Corner(c)) => {
                self.state = State::Idle;
                c.finish()
            }
            (PointerKind::Up, State::Moving { began, deselect, .. }) => {
                self.state = State::Idle;
                self.measure = None;
                self.guides.clear();
                self.targets = None;
                match (began, deselect) {
                    (true, _) => vec![Action::Commit],
                    (false, Some(id)) => vec![Action::Exec("select.toggle".into(), json!({ "id": id.0 }))],
                    (false, None) => vec![],
                }
            }
            (PointerKind::Up, State::Scaling { .. } | State::Rotating { .. }) => {
                self.state = State::Idle;
                self.measure = None;
                vec![Action::Commit]
            }
            (PointerKind::Up, State::Marquee { start, add, .. }) => {
                self.state = State::Idle;
                let r = Rect::from_points(start, p);
                if r.width() < Self::drag_threshold(cx) && r.height() < Self::drag_threshold(cx) {
                    return if add { vec![] } else { vec![Action::Exec("select.none".into(), json!({}))] };
                }
                let ids: Vec<NodeId> = marquee(cx.doc, r, cx.isolation, false);
                if add {
                    vec![Action::Exec("select.add".into(), json!({ "ids": json_ids(&ids) }))]
                } else {
                    vec![Action::Exec("select.set".into(), json!({ "ids": json_ids(&ids) }))]
                }
            }
            _ => vec![],
        }
    }

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let mut o = vec![];
        match &self.state {
            State::Marquee { start, cur, .. } => o.push(Overlay::Marquee(Rect::from_points(*start, *cur))),
            State::Corner(c) => o.extend(c.overlays(cx)),
            _ => {}
        }
        o.extend(self.guides.iter().cloned());
        if let Some((p, t)) = &self.measure {
            o.push(Overlay::Measure { p: *p, text: t.clone() });
        }
        o
    }

    fn cursor(&self, cx: &ToolContext, p: Point, m: Mods) -> Cursor {
        match self.state {
            State::Rotating { .. } => return Cursor::Rotate,
            State::Scaling { handle, bx, .. } => return handle_cursor(handle, bx.angle),
            State::Moving { began: true, .. } => return Cursor::Arrow,
            State::Corner(_) => return Cursor::CornerRadius,
            _ => {}
        }
        if over_widget(cx, p) {
            return Cursor::CornerRadius;
        }
        if cx.show_bbox
            && let Some(bx) = selection_box(cx)
        {
            match box_hit(cx, &bx, p) {
                Some(BoxHit::Handle(h)) => return handle_cursor(h, bx.angle),
                Some(BoxHit::Rotate) => return Cursor::Rotate,
                None => {}
            }
        }
        if let Some(h) = hit_test(cx.doc, p, cx.hit_options()) {
            if cx.selection.contains(h.top_object(cx.isolation)) || m.alt {
                return Cursor::Move;
            }
            return Cursor::Arrow;
        }
        Cursor::Arrow
    }
}

/// Is `n` area type (text in a frame) whose frame the tools reshape? Type in perspective isn't:
/// it transforms whole.
pub(crate) fn is_area_type(n: &vectorcraft_doc::Node) -> bool {
    n.perspective.is_none() && matches!(&n.kind, vectorcraft_doc::NodeKind::Text(t) if matches!(t.kind, vectorcraft_doc::TextKind::Area { .. }))
}

/// The resize cursor for handle `h` of a box turned by `angle` (counter-clockwise degrees): the
/// handle that sits where `h` appears on screen, in 45° steps.
fn handle_cursor(h: Handle, angle: f64) -> Cursor {
    let steps = if angle.is_finite() { (angle / 45.0).round() as i64 } else { 0 };
    let h = Handle::ALL.get((h as i64 - steps).rem_euclid(8) as usize).copied().unwrap_or(h);
    match h {
        Handle::Top | Handle::Bottom => Cursor::ResizeV,
        Handle::Left | Handle::Right => Cursor::ResizeH,
        Handle::TopLeft | Handle::BottomRight => Cursor::ResizeNwSe,
        Handle::TopRight | Handle::BottomLeft => Cursor::ResizeNeSw,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
        PointerEvent::new(kind, x, y)
    }

    #[test]
    fn click_selects_object() {
        let (d, id) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        let a = t.pointer(&cx, &ev(PointerKind::Down, 150.0, 150.0));
        assert_eq!(a, vec![Action::Exec("select.set".into(), json!({"ids": [id.0]}))]);
        assert!(t.pointer(&cx, &ev(PointerKind::Up, 150.0, 150.0)).is_empty());
    }

    #[test]
    fn drag_moves_with_single_undo() {
        let (d, id) = doc_with_rect();
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        assert!(t.pointer(&cx, &ev(PointerKind::Down, 150.0, 150.0)).is_empty());
        let a = t.pointer(&cx, &ev(PointerKind::Drag, 160.0, 150.0));
        assert_eq!(a[0], Action::Begin("Move".into()));
        assert!(matches!(&a[1], Action::Preview(c, v) if c == "object.transform" && v["matrix"][4] == 10.0));
        let a = t.pointer(&cx, &ev(PointerKind::Up, 160.0, 150.0));
        assert_eq!(a, vec![Action::Commit]);
    }

    #[test]
    fn shift_drag_on_a_selected_object_moves_it_constrained() {
        let (d, id) = doc_with_rect();
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        let shift = |kind, x, y| ev(kind, x, y).with_mods(Mods { shift: true, ..Mods::default() });
        // Pressing keeps the selection: nothing is toggled yet.
        assert!(t.pointer(&cx, &shift(PointerKind::Down, 150.0, 150.0)).is_empty());
        let a = t.pointer(&cx, &shift(PointerKind::Drag, 190.0, 160.0));
        assert_eq!(a[0], Action::Begin("Move".into()));
        assert!(
            matches!(&a[1], Action::Preview(c, v) if c == "object.transform"
                && v["matrix"][4].as_f64().is_some_and(|x| x > 39.0) && v["matrix"][5] == 0.0),
            "moved right, constrained to horizontal: {a:?}"
        );
        assert_eq!(t.pointer(&cx, &shift(PointerKind::Up, 190.0, 160.0)), vec![Action::Commit]);
    }

    #[test]
    fn shift_click_toggles_an_object_on_release() {
        let (d, id) = doc_with_rect();
        let p = paint();
        let shift = |kind, x, y| ev(kind, x, y).with_mods(Mods { shift: true, ..Mods::default() });
        let toggle = vec![Action::Exec("select.toggle".into(), json!({"id": id.0}))];
        // Selected: it leaves the selection when the click is released.
        let mut s = Selection::default();
        s.add(id);
        let cx1 = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        assert!(t.pointer(&cx1, &shift(PointerKind::Down, 150.0, 150.0)).is_empty());
        assert_eq!(t.pointer(&cx1, &shift(PointerKind::Up, 150.0, 150.0)), toggle);
        // Not selected: it joins the selection at once, ready to be dragged.
        let s = Selection::default();
        let cx2 = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        assert_eq!(t.pointer(&cx2, &shift(PointerKind::Down, 150.0, 150.0)), toggle);
        assert!(t.pointer(&cx2, &shift(PointerKind::Up, 150.0, 150.0)).is_empty());
    }

    #[test]
    fn marquee_selects() {
        let (d, id) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        t.pointer(&cx, &ev(PointerKind::Down, 50.0, 50.0));
        t.pointer(&cx, &ev(PointerKind::Drag, 120.0, 120.0));
        assert_eq!(t.overlays(&cx).len(), 1);
        let a = t.pointer(&cx, &ev(PointerKind::Up, 120.0, 120.0));
        assert_eq!(a, vec![Action::Exec("select.set".into(), json!({"ids": [id.0]}))]);
    }

    #[test]
    fn handle_drag_scales() {
        let (d, id) = doc_with_rect();
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        assert_eq!(t.cursor(&cx, Point::new(200.0, 200.0), Mods::default()), Cursor::ResizeNwSe);
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Down, 200.0, 200.0)), vec![Action::Begin("Scale".into())]);
        let a = t.pointer(&cx, &ev(PointerKind::Drag, 300.0, 300.0));
        assert!(matches!(&a[0], Action::Preview(_, v) if v["matrix"][0] == 2.0));
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Up, 300.0, 300.0)), vec![Action::Commit]);
    }

    #[test]
    fn handle_drag_on_area_type_resizes_its_frame() {
        let (d, text) = doc_with_area_type();
        let p = paint();
        let mut s = Selection::default();
        s.add(text);
        let cx1 = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        // Bottom-right handle of the 120 × 40 frame at (300, 300).
        assert_eq!(t.pointer(&cx1, &ev(PointerKind::Down, 420.0, 340.0)), vec![Action::Begin("Resize Type Area".into())]);
        let a = t.pointer(&cx1, &ev(PointerKind::Drag, 460.0, 400.0));
        assert!(matches!(&a[0], Action::Preview(c, v) if c == "object.transform" && v["typeAreas"] == true), "{a:?}");
        assert_eq!(t.pointer(&cx1, &ev(PointerKind::Up, 460.0, 400.0)), vec![Action::Commit]);
        // With another object the step is a scale, still resizing the type area.
        let rect = d.layers[0].children().unwrap()[0].id;
        s.add(rect);
        let cx2 = cx(&d, &s, &p);
        assert_eq!(t.pointer(&cx2, &ev(PointerKind::Down, 420.0, 340.0)), vec![Action::Begin("Scale".into())]);
        let a = t.pointer(&cx2, &ev(PointerKind::Drag, 460.0, 400.0));
        assert!(matches!(&a[0], Action::Preview(_, v) if v["typeAreas"] == true), "{a:?}");
        t.pointer(&cx2, &ev(PointerKind::Up, 460.0, 400.0));
        // Without area type the drag scales as before.
        let mut s = Selection::default();
        s.add(rect);
        let cx3 = cx(&d, &s, &p);
        t.pointer(&cx3, &ev(PointerKind::Down, 200.0, 200.0));
        let a = t.pointer(&cx3, &ev(PointerKind::Drag, 300.0, 300.0));
        assert!(matches!(&a[0], Action::Preview(_, v) if v.get("typeAreas").is_none()), "{a:?}");
    }

    #[test]
    fn turned_box_handles_turn_and_scale_along_the_object() {
        let (mut d, id) = doc_with_rect();
        let c = Point::new(150.0, 150.0);
        let turn = Affine::translate(c.to_vec2()) * Affine::rotate(-std::f64::consts::FRAC_PI_4) * Affine::translate(-c.to_vec2());
        d.node_mut(id).unwrap().transform(turn, false);
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let cx = cx(&d, &s, &p);
        // The Right handle sits up and to the right, where a diagonal cursor fits it.
        let k = 50.0 * std::f64::consts::FRAC_1_SQRT_2;
        let right = Point::new(150.0 + k, 150.0 - k);
        let mut t = SelectionTool::default();
        assert_eq!(t.cursor(&cx, right, Mods::default()), Cursor::ResizeNeSw);
        // The page box's corner is no handle any more.
        assert_ne!(t.cursor(&cx, Point::new(150.0 + 2.0 * k, 150.0 - 2.0 * k), Mods::default()), Cursor::ResizeNeSw);
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Down, right.x, right.y)), vec![Action::Begin("Scale".into())]);
        let a = t.pointer(&cx, &ev(PointerKind::Drag, right.x + k, right.y - k));
        let Action::Preview(_, v) = &a[0] else { panic!("{a:?}") };
        // Doubling the width along 45° about the left side: the left handle stays put.
        let m: Vec<f64> = v["matrix"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let m = Affine::new([m[0], m[1], m[2], m[3], m[4], m[5]]);
        let left = Point::new(150.0 - k, 150.0 + k);
        assert!((m * left).distance(left) < 1e-9);
        assert!((m * right).distance(Point::new(right.x + k, right.y - k)) < 1e-9);
    }

    #[test]
    fn click_empty_deselects() {
        let (d, _) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = SelectionTool::default();
        t.pointer(&cx, &ev(PointerKind::Down, 400.0, 400.0));
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Up, 400.0, 400.0)), vec![Action::Exec("select.none".into(), json!({}))]);
    }
}
