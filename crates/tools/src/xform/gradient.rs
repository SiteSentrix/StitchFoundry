//! Gradient tool (G).
//!
//! Drag across selected objects to set the gradient vector of their fill (a solid fill becomes the
//! default gradient). Shift constrains the angle to 45°. With nothing selected, the drag targets
//! the object under the pointer. The overlay is the gradient annotator: a bar from a round start
//! handle to a square end handle, with a tick for each colour stop.

use serde_json::{Value, json};
use vectorcraft_color::Paint;
use vectorcraft_doc::AppearanceItem;
use vectorcraft_doc::hit::hit_test;
use vectorcraft_geom::{Point, Vec2};

use super::paint_owner;
use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, ToolKey};

const BAR: [u8; 3] = [0x20, 0x20, 0x20];
const LIGHT: [u8; 3] = [0xf0, 0xf0, 0xf0];

#[derive(Default)]
pub struct GradientTool {
    drag: Option<(Point, Point, bool)>,
}

/// Start/end/stop offsets of the gradient annotator for the first selected object with a gradient
/// in the Appearance panel's active item (when it is one), else in its fill.
pub fn annotator(cx: &ToolContext) -> Option<(Point, Point, Vec<f32>)> {
    cx.selection.objects.iter().find_map(|id| {
        let n = cx.doc.node(*id)?;
        let item = cx.appearance_item.and_then(|i| n.appearance.items.get(i));
        let paint = match item {
            Some(AppearanceItem::Stroke(s)) => &s.paint,
            Some(AppearanceItem::Fill(f)) => &f.paint,
            None => &n.appearance.fill()?.paint,
        };
        let Paint::Gradient(g) = paint else { return None };
        let geom = g.resolve(n.geometric_bounds()?);
        Some((geom.start, geom.end, g.gradient.stops.iter().map(|s| s.offset).collect()))
    })
}

fn annotator_overlays(cx: &ToolContext, a: Point, b: Point, stops: &[f32]) -> Vec<Overlay> {
    let mut o = vec![
        Overlay::Line { a, b, color: BAR, dashed: false },
        Overlay::Line { a: a + Vec2::new(0.0, cx.tol(1.0)), b: b + Vec2::new(0.0, cx.tol(1.0)), color: LIGHT, dashed: false },
        Overlay::Handle { p: a, color: BAR },
        Overlay::Anchor { p: b, color: BAR, filled: true, size: 7.0 },
    ];
    let v = b - a;
    let len = v.hypot();
    if len > 1e-9 {
        let n = Vec2::new(-v.y, v.x) / len * cx.tol(6.0);
        for s in stops {
            let p = a + v * *s as f64;
            o.push(Overlay::Line { a: p, b: p + n, color: BAR, dashed: false });
        }
    }
    o
}

impl Tool for GradientTool {
    fn id(&self) -> &'static str {
        "gradient"
    }

    fn busy(&self) -> bool {
        self.drag.is_some_and(|d| d.2)
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        match ev.kind {
            PointerKind::Down => {
                let mut out = vec![];
                if cx.selection.is_empty() {
                    let Some(h) = hit_test(cx.doc, p, cx.hit_options()) else { return out };
                    let id = paint_owner(cx.doc, h.leaf);
                    out.push(Action::Exec("select.set".into(), json!({ "ids": [id.0] })));
                }
                self.drag = Some((p, p, false));
                out
            }
            PointerKind::Drag => {
                let Some((start, _, began)) = self.drag else { return vec![] };
                let mut out = vec![];
                if !began {
                    if p.distance(start) < cx.tol(3.0) {
                        return out;
                    }
                    out.push(Action::Begin("Gradient".into()));
                }
                let end = if ev.mods.shift { start + vectorcraft_geom::constrain_angle(p - start, 45.0) } else { p };
                self.drag = Some((start, end, true));
                out.push(Action::Preview("paint.setGradientGeom".into(), json!({ "start": [start.x, start.y], "end": [end.x, end.y] })));
                out
            }
            PointerKind::Up => match self.drag.take() {
                Some((_, _, true)) => vec![Action::Commit],
                _ => vec![],
            },
            _ => vec![],
        }
    }

    fn key(&mut self, _cx: &ToolContext, key: ToolKey, _mods: Mods) -> Vec<Action> {
        if key == ToolKey::Escape && self.busy() {
            self.drag = None;
            return vec![Action::Cancel];
        }
        vec![]
    }

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        match (self.drag, annotator(cx)) {
            (_, Some((a, b, stops))) => annotator_overlays(cx, a, b, &stops),
            (Some((a, b, true)), None) => annotator_overlays(cx, a, b, &[0.0, 1.0]),
            _ => vec![],
        }
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::Crosshair
    }

    fn options(&self) -> Value {
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    #[test]
    fn drag_sets_gradient_vector_and_constrains() {
        let (d, id) = doc_with_rect();
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 110.0, 150.0)).is_empty());
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 190.0, 152.0).with_mods(Mods { shift: true, ..Default::default() }));
        assert_eq!(a[0], Action::Begin("Gradient".into()));
        let Action::Preview(c, v) = &a[1] else { panic!() };
        assert_eq!(c, "paint.setGradientGeom");
        assert_eq!(v["start"], json!([110.0, 150.0]));
        assert!((v["end"][1].as_f64().unwrap() - 150.0).abs() < 1e-9);
        // While dragging a solid-filled object the annotator follows the drag.
        assert!(t.overlays(&cx).iter().any(|o| matches!(o, Overlay::Handle { p, .. } if p.x == 110.0)));
        assert_eq!(t.pointer(&cx, &PointerEvent::new(PointerKind::Up, 190.0, 150.0)), vec![Action::Commit]);
    }

    #[test]
    fn empty_selection_targets_hit_object() {
        let (d, id) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 150.0));
        assert_eq!(a, vec![Action::Exec("select.set".into(), json!({"ids": [id.0]}))]);
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 450.0, 450.0)).is_empty());
    }

    #[test]
    fn annotator_follows_the_active_appearance_item() {
        let (mut d, id) = doc_with_rect();
        let geom = vectorcraft_color::GradientGeom { start: Point::new(100.0, 120.0), end: Point::new(200.0, 120.0), aspect: 1.0 };
        let mut gp = vectorcraft_color::GradientPaint::new(Default::default());
        gp.geom = Some(geom);
        d.node_mut(id).unwrap().appearance.stroke_mut().unwrap().paint = Paint::Gradient(Box::new(gp));
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let mut cx = cx(&d, &s, &p);
        // The fill is solid: no annotator until the stroke row (item 1) is the active item.
        assert!(annotator(&cx).is_none());
        cx.appearance_item = Some(1);
        let (a, b, _) = annotator(&cx).unwrap();
        assert_eq!((a, b), (geom.start, geom.end));
    }
}
