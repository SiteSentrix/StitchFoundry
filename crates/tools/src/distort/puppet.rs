//! Puppet Warp tool.
//!
//! With artwork selected the tool places three pins automatically (spread over the mesh). Click
//! on the artwork adds a pin, Alt-click deletes one, Delete/Backspace removes the selected pin,
//! and dragging a pin warps the art: every drag previews
//! `object.puppetWarp {ids, pins, moved}` (pins = current pin positions, moved = the same list
//! with the dragged pin at the pointer), committed on release as one undo step. The mesh follows
//! the current shape, so successive drags compose.

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

#[derive(Default)]
pub struct PuppetWarpTool {
    ids: Vec<NodeId>,
    pins: Vec<Point>,
    selected: Option<usize>,
    drag: Option<(usize, Point)>,
    /// Control bar: Show Mesh.
    show_mesh: bool,
    /// Control bar: Expand Mesh (points).
    expand: f64,
}

impl PuppetWarpTool {
    /// Re-seed pins when the selection changed.
    fn sync(&mut self, cx: &ToolContext) {
        let ids = cx.selection.objects.clone();
        if ids == self.ids {
            return;
        }
        self.ids = ids;
        self.selected = None;
        self.pins = if self.ids.is_empty() {
            vec![]
        } else {
            mesh_for(cx.doc, &self.ids, self.expand_or_default()).map(|m| auto_pins(&m, 3)).unwrap_or_default()
        };
    }
    fn expand_or_default(&self) -> f64 {
        if self.expand > 0.0 { self.expand } else { 3.0 }
    }
    fn pin_at(&self, cx: &ToolContext, p: Point) -> Option<usize> {
        let tol = cx.tol(7.0);
        self.pins.iter().enumerate().filter(|(_, q)| q.distance(p) <= tol).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p))).map(|(i, _)| i)
    }
    fn params(&self, i: usize, to: Point) -> Value {
        let mut moved = self.pins.clone();
        moved[i] = to;
        let pj = |v: &[Point]| Value::Array(v.iter().map(|p| json!([p.x, p.y])).collect());
        json!({"ids": crate::json_ids(&self.ids), "pins": pj(&self.pins), "moved": pj(&moved), "expand": self.expand_or_default()})
    }
}

impl Tool for PuppetWarpTool {
    fn id(&self) -> &'static str {
        "puppetWarp"
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        match ev.kind {
            PointerKind::Down => {
                self.sync(cx);
                if self.ids.is_empty() {
                    // Click an object to select it; pins appear on the next event.
                    if let Some(h) = vectorcraft_doc::hit::hit_test(cx.doc, p, cx.hit_options()) {
                        return vec![Action::Exec("select.set".into(), json!({"ids": [h.top_object(cx.isolation).0]}))];
                    }
                    return vec![];
                }
                if let Some(i) = self.pin_at(cx, p) {
                    if ev.mods.alt {
                        self.pins.remove(i);
                        self.selected = None;
                        return vec![];
                    }
                    self.selected = Some(i);
                    self.drag = Some((i, p));
                    return vec![Action::Begin("Puppet Warp".into())];
                }
                let inside = cx.doc.bounds_of(&self.ids, false).is_some_and(|b| b.inflate(cx.tol(4.0), cx.tol(4.0)).contains(p));
                if inside && !ev.mods.alt {
                    self.pins.push(p);
                    self.selected = Some(self.pins.len() - 1);
                }
                vec![]
            }
            PointerKind::Drag => match self.drag {
                Some((i, _)) => {
                    self.drag = Some((i, p));
                    vec![Action::Preview("object.puppetWarp".into(), self.params(i, p))]
                }
                None => vec![],
            },
            PointerKind::Up => match self.drag.take() {
                Some((i, _)) => {
                    self.pins[i] = p;
                    vec![Action::Commit]
                }
                None => vec![],
            },
            _ => vec![],
        }
    }
    /// Delete/Backspace remove the selected pin; with none selected they are the shortcut's
    /// (Clear deletes the selected objects).
    fn claims_key(&self, cx: &ToolContext, key: ToolKey) -> bool {
        matches!(key, ToolKey::Delete | ToolKey::Backspace)
            && self.drag.is_none()
            && cx.selection.objects == self.ids
            && self.selected.is_some_and(|i| i < self.pins.len())
    }
    fn key(&mut self, _cx: &ToolContext, key: ToolKey, _mods: Mods) -> Vec<Action> {
        if matches!(key, ToolKey::Delete | ToolKey::Backspace)
            && let Some(i) = self.selected.take()
            && i < self.pins.len()
        {
            self.pins.remove(i);
        }
        vec![]
    }
    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let mut out = vec![];
        if self.show_mesh
            && cx.selection.objects == self.ids
            && let Some(m) = mesh_for(cx.doc, &self.ids, self.expand_or_default())
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
        for (i, q) in self.pins.iter().enumerate() {
            let q = match self.drag {
                Some((d, to)) if d == i => to,
                _ => *q,
            };
            let circle = super::ellipse_path(q, r, r, 0.0);
            out.push(Overlay::Path { path: circle, color: BLUE, width: if self.selected == Some(i) { 3.0 } else { 1.5 }, dashed: false });
            out.push(Overlay::Anchor { p: q, color: BLUE, filled: self.selected == Some(i), size: 3.0 });
        }
        out
    }
    fn cursor(&self, cx: &ToolContext, p: Point, mods: Mods) -> Cursor {
        match self.pin_at(cx, p) {
            Some(_) if mods.alt => Cursor::PenDelete,
            Some(_) => Cursor::Move,
            None => Cursor::PenAdd,
        }
    }
    fn options(&self) -> Value {
        json!({"showMesh": self.show_mesh, "expand": self.expand_or_default(), "pins": self.pins.iter().map(|p| json!([p.x, p.y])).collect::<Vec<_>>()})
    }
    fn set_option(&mut self, key: &str, value: &Value) {
        match key {
            "showMesh" => self.show_mesh = value.as_bool().unwrap_or(self.show_mesh),
            "expand" => self.expand = value.as_f64().unwrap_or(self.expand).clamp(0.0, 1000.0),
            "selectAllPins" => self.selected = None,
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
        if self.drag.take().is_some() { vec![Action::Commit] } else { vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

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
        // Alt-click deletes it.
        t.pointer(&c, &PointerEvent::new(PointerKind::Down, 120.0, 190.0).with_mods(Mods { alt: true, ..Default::default() }));
        assert_eq!(t.pins.len(), 3);
    }
}
