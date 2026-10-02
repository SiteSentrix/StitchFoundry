//! Direct Selection (A) and Group Selection tools.
//!
//! Direct Selection: click an anchor to select it (Shift toggles), click a segment to select the
//! path's anchors on that segment, drag to move selected anchors, drag a direction handle to
//! reshape, marquee to select anchors, drag a live rectangle's corner widget to round its corners.
//! Group Selection: click selects the leaf; each further click on it adds the next enclosing group.

use serde_json::{Value, json};
use vectorcraft_doc::hit::hit_test;
use vectorcraft_doc::{AnchorRef, NodeId, NodeKind};
use vectorcraft_geom::{Point, Rect};

use crate::bbox::move_delta;
use crate::corners::{CornerDrag, over_widget};
use crate::select::matrix_json;
use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Clone, Debug)]
enum State {
    Idle,
    MoveAnchors { start: Point, began: bool },
    MoveObject { start: Point, began: bool },
    Handle { id: NodeId, si: usize, ai: usize, out: bool },
    Marquee { start: Point, cur: Point, add: bool },
    Corner(CornerDrag),
}

pub struct DirectSelectionTool {
    group: bool,
    state: State,
}

impl DirectSelectionTool {
    pub fn new(group: bool) -> Self {
        Self { group, state: State::Idle }
    }
}

/// Anchor (or handle) of any selected/visible path under `p`.
fn hit_anchor(cx: &ToolContext, p: Point, tol: f64, selected_only: bool) -> Option<(NodeId, usize, usize)> {
    let ids: Vec<NodeId> = if selected_only {
        cx.selection.objects.clone()
    } else {
        let mut v = vec![];
        cx.doc.walk(|n| {
            if matches!(n.kind, NodeKind::Path { .. }) {
                v.push(n.id)
            }
        });
        v.reverse();
        v
    };
    for id in ids {
        if !cx.doc.is_editable(id) {
            continue;
        }
        if let Some(pd) = cx.doc.node(id).and_then(|n| n.path_data()) {
            for (si, ai, a) in pd.anchors() {
                if a.p.distance(p) <= tol {
                    return Some((id, si, ai));
                }
            }
        }
    }
    None
}

/// Direction handle of a partially selected anchor under `p`: (id, si, ai, is_out).
fn hit_handle(cx: &ToolContext, p: Point, tol: f64) -> Option<(NodeId, usize, usize, bool)> {
    for (id, set) in &cx.selection.anchors {
        let Some(pd) = cx.doc.node(*id).and_then(|n| n.path_data()) else { continue };
        for &(si, ai) in set {
            let Some(a) = pd.subpaths.get(si).and_then(|s| s.anchors.get(ai)) else { continue };
            if a.has_out() && a.h_out.distance(p) <= tol {
                return Some((*id, si, ai, true));
            }
            if a.has_in() && a.h_in.distance(p) <= tol {
                return Some((*id, si, ai, false));
            }
        }
    }
    None
}

fn anchors_json(v: &[AnchorRef]) -> Value {
    Value::Array(v.iter().map(|(s, a)| json!([s, a])).collect())
}

impl Tool for DirectSelectionTool {
    fn id(&self) -> &'static str {
        if self.group { "groupSelection" } else { "directSelection" }
    }
    fn busy(&self) -> bool {
        !matches!(self.state, State::Idle)
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        let tol = cx.tol(4.0);
        match (ev.kind, self.state.clone()) {
            (PointerKind::Down, _) if self.group => {
                let Some(h) = hit_test(cx.doc, p, cx.hit_options()) else {
                    self.state = State::Marquee { start: p, cur: p, add: ev.mods.shift };
                    return vec![];
                };
                // Walk up from the leaf: select the first ancestor not yet selected (below the layer).
                let chain: Vec<NodeId> = h.ancestry.iter().skip(1).rev().copied().collect();
                let target = if cx.selection.contains(h.leaf) {
                    chain.iter().find(|id| !cx.selection.contains(**id)).copied().unwrap_or(h.leaf)
                } else {
                    h.leaf
                };
                self.state = State::MoveObject { start: p, began: false };
                if cx.selection.contains(target) {
                    vec![]
                } else if ev.mods.shift || cx.selection.contains(h.leaf) {
                    vec![Action::Exec("select.add".into(), json!({"ids": [target.0]}))]
                } else {
                    vec![Action::Exec("select.set".into(), json!({"ids": [target.0]}))]
                }
            }
            (PointerKind::Down, _) => {
                if let Some(c) = CornerDrag::hit(cx, p) {
                    self.state = State::Corner(c);
                    return vec![];
                }
                if let Some((id, si, ai, out)) = hit_handle(cx, p, tol) {
                    self.state = State::Handle { id, si, ai, out };
                    return vec![Action::Begin("Reshape".into())];
                }
                if let Some((id, si, ai)) = hit_anchor(cx, p, tol, false) {
                    let already = cx.selection.partial(id).is_some_and(|s| s.contains(&(si, ai)));
                    self.state = State::MoveAnchors { start: p, began: false };
                    if ev.mods.shift {
                        return vec![Action::Exec("select.anchors".into(), json!({"id": id.0, "anchors": [[si, ai]], "mode": "toggle"}))];
                    }
                    if !already {
                        return vec![Action::Exec("select.anchors".into(), json!({"id": id.0, "anchors": [[si, ai]], "mode": "set"}))];
                    }
                    return vec![];
                }
                if let Some(h) = hit_test(cx.doc, p, cx.hit_options()) {
                    // Clicking a segment/fill selects the whole leaf path (all anchors).
                    self.state = State::MoveObject { start: p, began: false };
                    if ev.mods.shift {
                        return vec![Action::Exec("select.toggle".into(), json!({"id": h.leaf.0}))];
                    }
                    if !cx.selection.contains(h.leaf) || cx.selection.partial(h.leaf).is_some() {
                        return vec![Action::Exec("select.set".into(), json!({"ids": [h.leaf.0]}))];
                    }
                    return vec![];
                }
                self.state = State::Marquee { start: p, cur: p, add: ev.mods.shift };
                vec![]
            }
            (PointerKind::Drag, State::MoveAnchors { start, began }) => {
                let mut out = vec![];
                if !began {
                    if p.distance(start) < cx.tol(3.0) {
                        return out;
                    }
                    out.push(Action::Begin("Move".into()));
                    self.state = State::MoveAnchors { start, began: true };
                }
                let d = move_delta(start, p, ev.mods.shift);
                out.push(Action::Preview("path.moveAnchors".into(), json!({"dx": d.x, "dy": d.y})));
                out
            }
            (PointerKind::Drag, State::MoveObject { start, began }) => {
                let mut out = vec![];
                if !began {
                    if p.distance(start) < cx.tol(3.0) {
                        return out;
                    }
                    out.push(Action::Begin("Move".into()));
                    self.state = State::MoveObject { start, began: true };
                }
                let d = move_delta(start, p, ev.mods.shift);
                out.push(Action::Preview(
                    "object.transform".into(),
                    json!({"matrix": matrix_json(vectorcraft_geom::Affine::translate(d)), "copy": ev.mods.alt}),
                ));
                out
            }
            (PointerKind::Drag, State::Handle { id, si, ai, out }) => {
                vec![Action::Preview(
                    "path.setHandle".into(),
                    json!({"id": id.0, "subpath": si, "anchor": ai, "which": if out {"out"} else {"in"}, "x": p.x, "y": p.y, "independent": ev.mods.alt}),
                )]
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
            (PointerKind::Up, State::MoveAnchors { began, .. } | State::MoveObject { began, .. }) => {
                self.state = State::Idle;
                if began { vec![Action::Commit] } else { vec![] }
            }
            (PointerKind::Up, State::Handle { .. }) => {
                self.state = State::Idle;
                vec![Action::Commit]
            }
            (PointerKind::Up, State::Marquee { start, add, .. }) => {
                self.state = State::Idle;
                let r = Rect::from_points(start, p);
                if r.width() < cx.tol(3.0) && r.height() < cx.tol(3.0) {
                    return if add { vec![] } else { vec![Action::Exec("select.none".into(), json!({}))] };
                }
                // Collect anchors inside the rect for every editable path.
                let mut sel: Vec<(NodeId, Vec<AnchorRef>)> = vec![];
                cx.doc.walk(|n| {
                    if let NodeKind::Path { path, .. } = &n.kind {
                        let v: Vec<AnchorRef> = path.anchors().filter(|(_, _, a)| r.contains(a.p)).map(|(s, i, _)| (s, i)).collect();
                        if !v.is_empty() {
                            sel.push((n.id, v));
                        }
                    }
                });
                sel.retain(|(id, _)| cx.doc.is_editable(*id));
                let items: Vec<Value> = sel.iter().map(|(id, v)| json!({"id": id.0, "anchors": anchors_json(v)})).collect();
                vec![Action::Exec("select.anchorsMany".into(), json!({"items": items, "add": add}))]
            }
            _ => vec![],
        }
    }
    fn overlays(&self, _cx: &ToolContext) -> Vec<Overlay> {
        match &self.state {
            State::Marquee { start, cur, .. } => vec![Overlay::Marquee(Rect::from_points(*start, *cur))],
            State::Corner(c) => c.overlays(),
            _ => vec![],
        }
    }
    fn cursor(&self, cx: &ToolContext, p: Point, _m: Mods) -> Cursor {
        if !self.group && (matches!(self.state, State::Corner(_)) || over_widget(cx, p)) {
            return Cursor::CornerRadius;
        }
        Cursor::ArrowHollow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    #[test]
    fn click_anchor_selects_it() {
        let (d, id) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = DirectSelectionTool::new(false);
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 100.0, 100.0));
        assert_eq!(a, vec![Action::Exec("select.anchors".into(), json!({"id": id.0, "anchors": [[0, 0]], "mode": "set"}))]);
    }

    #[test]
    fn marquee_selects_anchors() {
        let (d, id) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = DirectSelectionTool::new(false);
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 50.0));
        t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 250.0, 150.0));
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 250.0, 150.0));
        assert_eq!(a, vec![Action::Exec("select.anchorsMany".into(), json!({"items": [{"id": id.0, "anchors": [[0, 1]]}], "add": false}))]);
    }
}
