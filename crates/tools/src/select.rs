//! The Selection tool (V): click/shift-click, marquee, move (Alt copies, Shift constrains),
//! bounding-box scale (Shift proportional, Alt from centre) and rotate (outside corners, Shift 45°),
//! drag a live rectangle's corner widget to round its corners, double-click to enter isolation mode.

use serde_json::{Value, json};
use vectorcraft_doc::NodeId;
use vectorcraft_doc::hit::{hit_test, marquee};
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
    },
    Scaling {
        handle: Handle,
        rect: Rect,
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
                    && let Some(r) = selection_bounds(cx)
                {
                    let tol = cx.tol(5.0);
                    if let Some(h) = hit_handle(r, p, tol) {
                        self.state = State::Scaling { handle: h, rect: r };
                        return vec![Action::Begin("Scale".into())];
                    }
                    if in_rotate_zone(r, p, tol, cx.tol(18.0)).is_some() {
                        self.state = State::Rotating { center: r.center(), start: p };
                        return vec![Action::Begin("Rotate".into())];
                    }
                }
                // 2. Objects.
                match hit_test(cx.doc, p, cx.hit_options()) {
                    Some(h) => {
                        let top = h.top_object(cx.isolation);
                        let mut out = vec![];
                        if m.shift {
                            out.push(Action::Exec("select.toggle".into(), json!({ "id": top.0 })));
                            if cx.selection.contains(top) {
                                self.state = State::Idle;
                                return out;
                            }
                        } else if !cx.selection.contains(top) {
                            out.push(Action::Exec("select.set".into(), json!({ "ids": [top.0] })));
                        }
                        self.state = State::Moving { start: p, began: false };
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
                self.state = State::Moving { start, began: true };
                self.measure = Some((p, cx.offset_label(d.x, d.y)));
                out.push(Action::Preview("object.transform".into(), json!({ "matrix": matrix_json(Affine::translate(d)), "copy": m.alt })));
                out
            }
            (PointerKind::Drag, State::Scaling { handle, rect, .. }) => {
                let a = scale_for_drag(rect, handle, p, m.shift, m.alt);
                let nr = a.transform_rect_bbox(rect);
                self.measure = Some((p, cx.size_label(nr.width(), nr.height())));
                vec![Action::Preview("object.transform".into(), json!({ "matrix": matrix_json(a), "copy": false }))]
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
            (PointerKind::Up, State::Moving { began, .. }) => {
                self.state = State::Idle;
                self.measure = None;
                self.guides.clear();
                self.targets = None;
                if began { vec![Action::Commit] } else { vec![] }
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
            State::Scaling { handle, .. } => return handle_cursor(handle),
            State::Moving { began: true, .. } => return Cursor::Arrow,
            State::Corner(_) => return Cursor::CornerRadius,
            _ => {}
        }
        if over_widget(cx, p) {
            return Cursor::CornerRadius;
        }
        if cx.show_bbox
            && let Some(r) = selection_bounds(cx)
        {
            let tol = cx.tol(5.0);
            if let Some(h) = hit_handle(r, p, tol) {
                return handle_cursor(h);
            }
            if in_rotate_zone(r, p, tol, cx.tol(18.0)).is_some() {
                return Cursor::Rotate;
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

fn handle_cursor(h: Handle) -> Cursor {
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
