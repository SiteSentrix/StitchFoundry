//! Puppet Warp tool.
//!
//! With artwork selected the tool places three pins automatically (spread over the mesh). A click
//! on the artwork adds a pin and selects it; Shift-click adds pins to the selection (or takes them
//! out); dragging a pin moves every selected pin and warps the art; Alt-dragging near (not on) a
//! selected pin turns the art around it, and that pin keeps the turn through later warps;
//! Delete/Backspace remove the selected pins. Every drag previews
//! `object.puppetWarp {ids, pins, moved, angles, expand}` (pins = current pin positions, moved =
//! where they go, angles = the turn held at each pin), committed on release as one undo step. The
//! mesh follows the current shape, so successive drags compose.
//!
//! Control bar (`tool.setOption`): `showMesh` (on by default), `expand` (Expand Mesh, points, 0
//! allowed) and `selectAllPins`.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{BezPath, Point, Rect};

use super::arap::{Mesh, MeshOptions, auto_pins};
use super::{BLUE, collect_points};
use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, ToolKey};

/// Outlines and bounds of the given objects (what the mesh is built over).
pub fn mesh_input(doc: &Document, ids: &[NodeId]) -> Option<(Vec<BezPath>, Rect)> {
    let mut outlines = vec![];
    let mut pts = vec![];
    for id in ids {
        let n = doc.node(*id)?;
        collect_points(n, &mut pts);
        n.walk(&mut |c| {
            if let Some(p) = c.path_data() {
                outlines.push(p.to_bezpath());
            }
        });
    }
    let first = *pts.first()?;
    let b = pts.iter().fold(Rect::from_points(first, first), |r, p| r.union_pt(*p));
    Some((outlines, b))
}

/// The mesh the engine uses for `ids` (shared so tool overlays match the command).
pub fn mesh_for(doc: &Document, ids: &[NodeId], expand: f64) -> Option<Mesh> {
    let (o, b) = mesh_input(doc, ids)?;
    Some(Mesh::build(&o, b, MeshOptions { expand, ..Default::default() }))
}

/// Within this many pixels of a selected pin (and not on it), Alt-dragging turns the art around it.
const TURN_REACH_PX: f64 = 24.0;
/// Radius (pixels) of the dotted circle shown around the pin the art would turn around.
const TURN_RING_PX: f64 = 16.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct PinState {
    p: Point,
    /// The art was turned around the pin: later warps keep that turn.
    held: bool,
}

/// A drag in progress.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Gesture {
    /// The selected pins move by `to - from`.
    Move { from: Point, to: Point },
    /// The art turns by `angle` (radians) around pin `pin`; `from`: the pointer's angle around it
    /// when the drag began.
    Turn { pin: usize, from: f64, angle: f64 },
}

pub struct PuppetWarpTool {
    ids: Vec<NodeId>,
    pins: Vec<PinState>,
    /// Selected pins (indices into `pins`).
    selected: Vec<usize>,
    drag: Option<Gesture>,
    /// The pointer and the modifiers held while hovering (for the turn ring).
    hover: Option<(Point, Mods)>,
    /// Control bar: Show Mesh.
    show_mesh: bool,
    /// Control bar: Expand Mesh (points, 0 allowed).
    expand: f64,
}

impl Default for PuppetWarpTool {
    fn default() -> Self {
        Self { ids: vec![], pins: vec![], selected: vec![], drag: None, hover: None, show_mesh: true, expand: 3.0 }
    }
}

impl PuppetWarpTool {
    /// Re-seed pins when the selection changed.
    fn sync(&mut self, cx: &ToolContext) {
        let ids = cx.selection.objects.clone();
        if ids == self.ids {
            return;
        }
        self.ids = ids;
        self.selected.clear();
        self.pins = if self.ids.is_empty() {
            vec![]
        } else {
            let auto = mesh_for(cx.doc, &self.ids, self.expand).map(|m| auto_pins(&m, 3)).unwrap_or_default();
            auto.into_iter().map(|p| PinState { p, held: false }).collect()
        };
    }
    fn pin_at(&self, cx: &ToolContext, p: Point) -> Option<usize> {
        let tol = cx.tol(7.0);
        self.pins
            .iter()
            .enumerate()
            .filter(|(_, q)| q.p.distance(p) <= tol)
            .min_by(|a, b| a.1.p.distance(p).total_cmp(&b.1.p.distance(p)))
            .map(|(i, _)| i)
    }
    /// The selected pin the art would turn around with Alt held at `p`: near it, not on a pin.
    fn turn_pin_at(&self, cx: &ToolContext, p: Point) -> Option<usize> {
        if self.pin_at(cx, p).is_some() {
            return None;
        }
        let reach = cx.tol(TURN_REACH_PX);
        self.selected
            .iter()
            .filter_map(|&i| Some((i, self.pins.get(i)?.p.distance(p))))
            .filter(|(_, d)| *d <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }
    /// Where each pin is during gesture `g`, and the turn it holds (radians).
    fn targets(&self, g: Gesture) -> Vec<(Point, Option<f64>)> {
        self.pins
            .iter()
            .enumerate()
            .map(|(i, q)| match g {
                Gesture::Move { from, to } if self.selected.contains(&i) => (q.p + (to - from), q.held.then_some(0.0)),
                Gesture::Turn { pin, angle, .. } if pin == i => (q.p, Some(angle)),
                _ => (q.p, q.held.then_some(0.0)),
            })
            .collect()
    }
    fn params(&self, g: Gesture) -> Value {
        let moved = self.targets(g);
        let pj = |v: &mut dyn Iterator<Item = Point>| Value::Array(v.map(|p| json!([p.x, p.y])).collect());
        json!({
            "ids": crate::json_ids(&self.ids),
            "pins": pj(&mut self.pins.iter().map(|q| q.p)),
            "moved": pj(&mut moved.iter().map(|m| m.0)),
            "angles": moved.iter().map(|m| m.1.map(f64::to_degrees)).collect::<Vec<_>>(),
            "expand": self.expand,
        })
    }
    /// Select pin `i` (Shift: add it to the selection or take it out). → whether it is selected.
    fn pick(&mut self, i: usize, shift: bool) -> bool {
        match (shift, self.selected.iter().position(|s| *s == i)) {
            (true, Some(k)) => {
                self.selected.remove(k);
                false
            }
            (true, None) => {
                self.selected.push(i);
                true
            }
            // A press on one of several selected pins drags them all.
            (false, Some(_)) => true,
            (false, None) => {
                self.selected = vec![i];
                true
            }
        }
    }
}

impl Tool for PuppetWarpTool {
    fn id(&self) -> &'static str {
        "puppetWarp"
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        match ev.kind {
            PointerKind::Move => {
                self.hover = Some((p, ev.mods));
                vec![]
            }
            PointerKind::Down => {
                self.sync(cx);
                if self.ids.is_empty() {
                    // Click an object to select it; pins appear on the next event.
                    if let Some(h) = vectorcraft_doc::hit::hit_test(cx.doc, p, cx.hit_options()) {
                        return vec![Action::Exec("select.set".into(), json!({"ids": [h.top_object(cx.isolation).0]}))];
                    }
                    return vec![];
                }
                if ev.mods.alt
                    && let Some(i) = self.turn_pin_at(cx, p)
                    && let Some(c) = self.pins.get(i).map(|q| q.p)
                {
                    self.drag = Some(Gesture::Turn { pin: i, from: (p - c).atan2(), angle: 0.0 });
                    return vec![Action::Begin("Puppet Warp".into())];
                }
                if let Some(i) = self.pin_at(cx, p) {
                    if !self.pick(i, ev.mods.shift) {
                        return vec![];
                    }
                    self.drag = Some(Gesture::Move { from: p, to: p });
                    return vec![Action::Begin("Puppet Warp".into())];
                }
                let inside = cx.doc.bounds_of(&self.ids, false).is_some_and(|b| b.inflate(cx.tol(4.0), cx.tol(4.0)).contains(p));
                if inside {
                    self.pins.push(PinState { p, held: false });
                    self.pick(self.pins.len() - 1, ev.mods.shift);
                } else if !ev.mods.shift {
                    self.selected.clear();
                }
                vec![]
            }
            PointerKind::Drag => {
                let Some(g) = self.drag.as_mut() else { return vec![] };
                match g {
                    Gesture::Move { to, .. } => *to = p,
                    Gesture::Turn { pin, from, angle } => {
                        let Some(c) = self.pins.get(*pin).map(|q| q.p) else { return vec![] };
                        *angle = (p - c).atan2() - *from;
                    }
                }
                let g = *g;
                vec![Action::Preview("object.puppetWarp".into(), self.params(g))]
            }
            PointerKind::Up => match self.drag.take() {
                Some(g) => {
                    let moved = self.targets(g);
                    for (q, (to, angle)) in self.pins.iter_mut().zip(moved) {
                        (q.p, q.held) = (to, angle.is_some());
                    }
                    vec![Action::Commit]
                }
                None => vec![],
            },
            PointerKind::DoubleClick => vec![],
        }
    }
    /// Delete/Backspace remove the selected pins; with none selected they are the shortcut's
    /// (Clear deletes the selected objects).
    fn claims_key(&self, cx: &ToolContext, key: ToolKey) -> bool {
        matches!(key, ToolKey::Delete | ToolKey::Backspace)
            && self.drag.is_none()
            && cx.selection.objects == self.ids
            && self.selected.iter().any(|i| *i < self.pins.len())
    }
    fn key(&mut self, _cx: &ToolContext, key: ToolKey, _mods: Mods) -> Vec<Action> {
        if matches!(key, ToolKey::Delete | ToolKey::Backspace) && self.drag.is_none() {
            let gone = std::mem::take(&mut self.selected);
            self.pins = std::mem::take(&mut self.pins).into_iter().enumerate().filter(|(i, _)| !gone.contains(i)).map(|(_, q)| q).collect();
        }
        vec![]
    }
    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let mut out = vec![];
        if self.show_mesh
            && cx.selection.objects == self.ids
            && let Some(m) = mesh_for(cx.doc, &self.ids, self.expand)
        {
            let mut bp = BezPath::new();
            for t in &m.tris {
                bp.move_to(m.verts[t[0]]);
                bp.line_to(m.verts[t[1]]);
                bp.line_to(m.verts[t[2]]);
                bp.close_path();
            }
            out.push(Overlay::Path { path: bp, color: [0x9a, 0x9a, 0x9a], width: 0.5, dashed: false });
        }
        let r = cx.tol(5.0);
        let shown: Vec<Point> = match self.drag {
            Some(g) => self.targets(g).into_iter().map(|t| t.0).collect(),
            None => self.pins.iter().map(|q| q.p).collect(),
        };
        for (i, q) in shown.iter().enumerate() {
            let sel = self.selected.contains(&i);
            out.push(Overlay::Path { path: super::ellipse_path(*q, r, r, 0.0), color: BLUE, width: if sel { 3.0 } else { 1.5 }, dashed: false });
            out.push(Overlay::Anchor { p: *q, color: BLUE, filled: sel, size: 3.0 });
        }
        // The dotted circle of a turn: around the pin being turned, or the one Alt would turn.
        let ring = match (self.drag, self.hover) {
            (Some(Gesture::Turn { pin, .. }), _) => Some(pin),
            (None, Some((h, mods))) if mods.alt => self.turn_pin_at(cx, h),
            _ => None,
        };
        if let Some(c) = ring.and_then(|i| self.pins.get(i)) {
            let rr = cx.tol(TURN_RING_PX);
            out.push(Overlay::Path { path: super::ellipse_path(c.p, rr, rr, 0.0), color: BLUE, width: 1.0, dashed: true });
        }
        out
    }
    fn cursor(&self, cx: &ToolContext, p: Point, mods: Mods) -> Cursor {
        if mods.alt && self.turn_pin_at(cx, p).is_some() {
            return Cursor::Rotate;
        }
        match self.pin_at(cx, p) {
            Some(_) => Cursor::Move,
            None => Cursor::PenAdd,
        }
    }
    fn options(&self) -> Value {
        json!({
            "showMesh": self.show_mesh,
            "expand": self.expand,
            "pins": self.pins.iter().map(|q| json!([q.p.x, q.p.y])).collect::<Vec<_>>(),
            "selected": self.selected,
        })
    }
    fn set_option(&mut self, key: &str, value: &Value) {
        match key {
            "showMesh" => self.show_mesh = value.as_bool().unwrap_or(self.show_mesh),
            "expand" => {
                if let Some(v) = value.as_f64().filter(|v| v.is_finite()) {
                    self.expand = v.clamp(0.0, 1000.0);
                }
            }
            // Select All Pins (false: deselect them all).
            "selectAllPins" => self.selected = if value.as_bool() == Some(false) { vec![] } else { (0..self.pins.len()).collect() },
            _ => {}
        }
    }
    fn busy(&self) -> bool {
        self.drag.is_some()
    }
    fn notify(&mut self, cx: &ToolContext, _what: &str) {
        self.sync(cx);
    }
    fn deactivate(&mut self, _cx: &ToolContext) -> Vec<Action> {
        self.ids.clear();
        self.pins.clear();
        self.selected.clear();
        self.hover = None;
        if self.drag.take().is_some() { vec![Action::Commit] } else { vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    fn alt() -> Mods {
        Mods { alt: true, ..Default::default() }
    }

    fn shift() -> Mods {
        Mods { shift: true, ..Default::default() }
    }

    #[test]
    fn puppet_tool_autopins_adds_and_drags() {
        let (d, id) = doc_with_rect();
        let s = Selection { objects: vec![id], ..Default::default() };
        let p = paint();
        let c = cx(&d, &s, &p);
        let mut t = PuppetWarpTool::default();
        // Click inside the rect away from auto pins: adds a pin.
        let acts = t.pointer(&c, &PointerEvent::new(PointerKind::Down, 130.0, 170.0));
        assert!(acts.is_empty());
        assert_eq!(t.pins.len(), 4, "3 auto pins + 1");
        t.pointer(&c, &PointerEvent::new(PointerKind::Up, 130.0, 170.0));
        // Drag the new pin.
        assert_eq!(t.pointer(&c, &PointerEvent::new(PointerKind::Down, 130.0, 170.0)), vec![Action::Begin("Puppet Warp".into())]);
        let acts = t.pointer(&c, &PointerEvent::new(PointerKind::Drag, 120.0, 190.0));
        let Action::Preview(cmd, v) = &acts[0] else { panic!() };
        assert_eq!(cmd, "object.puppetWarp");
        assert_eq!(v["moved"][3], json!([120.0, 190.0]));
        assert_eq!(v["pins"][3], json!([130.0, 170.0]));
        assert_eq!(t.pointer(&c, &PointerEvent::new(PointerKind::Up, 120.0, 190.0)), vec![Action::Commit]);
        // Alt-click on a pin no longer deletes it (Alt turns the art around a pin): Delete does.
        t.pointer(&c, &PointerEvent::new(PointerKind::Down, 120.0, 190.0).with_mods(alt()));
        t.pointer(&c, &PointerEvent::new(PointerKind::Up, 120.0, 190.0).with_mods(alt()));
        assert_eq!(t.pins.len(), 4);
        assert!(t.claims_key(&c, ToolKey::Delete));
        t.key(&c, ToolKey::Delete, Mods::default());
        assert_eq!(t.pins.len(), 3);
        assert!(!t.claims_key(&c, ToolKey::Delete), "nothing selected: Delete is Clear's");
    }

    #[test]
    fn shift_click_selects_several_pins_that_move_together() {
        let (d, id) = doc_with_rect();
        let s = Selection { objects: vec![id], ..Default::default() };
        let p = paint();
        let c = cx(&d, &s, &p);
        let mut t = PuppetWarpTool::default();
        let ev = |k, x, y, m| PointerEvent::new(k, x, y).with_mods(m);
        // Two new pins, the second Shift-added to the selection.
        t.pointer(&c, &ev(PointerKind::Down, 110.0, 110.0, Mods::default()));
        t.pointer(&c, &ev(PointerKind::Down, 190.0, 110.0, shift()));
        let (a, b) = (t.pins.len() - 2, t.pins.len() - 1);
        assert_eq!(t.selected, vec![a, b]);
        // Dragging one moves both.
        t.pointer(&c, &ev(PointerKind::Down, 110.0, 110.0, Mods::default()));
        let acts = t.pointer(&c, &ev(PointerKind::Drag, 110.0, 100.0, Mods::default()));
        let Action::Preview(_, v) = &acts[0] else { panic!("{acts:?}") };
        assert_eq!((v["moved"][a].clone(), v["moved"][b].clone()), (json!([110.0, 100.0]), json!([190.0, 100.0])));
        assert_eq!(v["moved"][0], v["pins"][0], "unselected pins stay");
        t.pointer(&c, &ev(PointerKind::Up, 110.0, 100.0, Mods::default()));
        // Shift-click takes a pin out of the selection without dragging.
        assert!(t.pointer(&c, &ev(PointerKind::Down, 190.0, 100.0, shift())).is_empty());
        assert_eq!(t.selected, vec![a]);
        // Select All Pins.
        t.set_option("selectAllPins", &json!(true));
        assert_eq!(t.selected, (0..t.pins.len()).collect::<Vec<_>>());
        t.key(&c, ToolKey::Backspace, Mods::default());
        assert!(t.pins.is_empty());
    }

    #[test]
    fn alt_drag_near_a_selected_pin_turns_the_art_around_it() {
        let (d, id) = doc_with_rect();
        let s = Selection { objects: vec![id], ..Default::default() };
        let p = paint();
        let c = cx(&d, &s, &p);
        let mut t = PuppetWarpTool::default();
        t.pointer(&c, &PointerEvent::new(PointerKind::Down, 110.0, 150.0));
        let i = t.pins.len() - 1;
        // Alt near (not on) the selected pin: the turn ring and cursor.
        t.pointer(&c, &PointerEvent::new(PointerKind::Move, 125.0, 150.0).with_mods(alt()));
        assert_eq!(t.cursor(&c, Point::new(125.0, 150.0), alt()), Cursor::Rotate);
        assert!(t.overlays(&c).iter().any(|o| matches!(o, Overlay::Path { dashed: true, .. })), "dotted ring");
        assert_ne!(t.cursor(&c, Point::new(125.0, 150.0), Mods::default()), Cursor::Rotate, "only with Alt");
        // Drag a quarter turn around it.
        assert_eq!(t.pointer(&c, &PointerEvent::new(PointerKind::Down, 125.0, 150.0).with_mods(alt())), vec![Action::Begin("Puppet Warp".into())]);
        let acts = t.pointer(&c, &PointerEvent::new(PointerKind::Drag, 110.0, 165.0).with_mods(alt()));
        let Action::Preview(cmd, v) = &acts[0] else { panic!("{acts:?}") };
        assert_eq!(cmd, "object.puppetWarp");
        assert!((v["angles"][i].as_f64().unwrap() - 90.0).abs() < 1e-6, "{v}");
        assert_eq!(v["moved"][i], v["pins"][i], "the pin stays put");
        assert_eq!(v["angles"][0], Value::Null, "other pins turn freely");
        t.pointer(&c, &PointerEvent::new(PointerKind::Up, 110.0, 165.0).with_mods(alt()));
        // The turned pin keeps its turn through the next warp.
        t.pointer(&c, &PointerEvent::new(PointerKind::Down, t.pins[0].p.x, t.pins[0].p.y));
        let acts = t.pointer(&c, &PointerEvent::new(PointerKind::Drag, t.pins[0].p.x + 5.0, t.pins[0].p.y));
        let Action::Preview(_, v) = &acts[0] else { panic!("{acts:?}") };
        assert_eq!(v["angles"][i], json!(0.0));
    }

    #[test]
    fn show_mesh_is_on_by_default_and_expand_may_be_zero() {
        let mut t = PuppetWarpTool::default();
        assert_eq!((t.options()["showMesh"].clone(), t.options()["expand"].clone()), (json!(true), json!(3.0)));
        t.set_option("expand", &json!(0));
        assert_eq!(t.options()["expand"], json!(0.0));
        t.set_option("expand", &json!(f64::NAN));
        t.set_option("expand", &json!("x"));
        assert_eq!(t.options()["expand"], json!(0.0));
    }
}
