//! Eyedropper tool (I).
//!
//! Click an object: copy its appearance (fill, stroke, weight, opacity) to the selection and the
//! paint defaults (`appearance.copyFrom`). Shift-click: sample only the colour under the cursor into
//! the active fill/stroke (`paint.sampleColor`).

use serde_json::json;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::hit::{HitKind, hit_test};
use vectorcraft_geom::Point;

use super::paint_owner;
use crate::{Action, Cursor, Mods, PointerEvent, PointerKind, Tool, ToolContext};

pub struct EyedropperTool;

/// The colour of `paint` at `p` (gradients are sampled along their vector, radial ones honouring
/// their aspect ratio).
pub fn paint_color_at(paint: &Paint, bounds: Option<vectorcraft_geom::Rect>, p: Point) -> Option<Color> {
    match paint {
        Paint::Solid { color, .. } => Some(*color),
        Paint::Gradient(g) => {
            let geom = g.geom.or_else(|| bounds.map(|b| g.resolve(b)))?;
            let t = geom.param_at(g.gradient.kind, p);
            Some(g.gradient.sample(t.clamp(0.0, 1.0) as f32).0)
        }
        _ => None,
    }
}

impl Tool for EyedropperTool {
    fn id(&self) -> &'static str {
        "eyedropper"
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        if ev.kind != PointerKind::Down {
            return vec![];
        }
        let opts = vectorcraft_doc::hit::HitOptions { path_only: false, ..cx.hit_options() };
        let Some(h) = hit_test(cx.doc, ev.pos, opts) else { return vec![] };
        let src = paint_owner(cx.doc, h.leaf);
        let Some(n) = cx.doc.node(src) else { return vec![] };
        if ev.mods.shift {
            let (fill, stroke) = match &n.kind {
                vectorcraft_doc::NodeKind::Text(t) => {
                    let st = t.runs.first().map(|r| r.style.clone());
                    (st.as_ref().map(|s| s.fill.clone()).unwrap_or_default(), st.map(|s| s.stroke).unwrap_or_default())
                }
                _ => (n.appearance.fill_paint(), n.appearance.stroke_paint()),
            };
            let paint = if h.kind == HitKind::Stroke || fill.is_none() { stroke } else { fill };
            return match paint_color_at(&paint, n.geometric_bounds(), ev.pos) {
                Some(c) => vec![Action::Exec("paint.sampleColor".into(), json!({ "color": c.to_hex() }))],
                None => vec![],
            };
        }
        vec![Action::Exec("appearance.copyFrom".into(), json!({ "source": src.0 }))]
    }

    fn cursor(&self, _cx: &ToolContext, _p: Point, _m: Mods) -> Cursor {
        Cursor::Eyedropper
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    #[test]
    fn click_copies_appearance_shift_samples_color() {
        let (d, id) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = EyedropperTool;
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 150.0));
        assert_eq!(a, vec![Action::Exec("appearance.copyFrom".into(), json!({"source": id.0}))]);
        let shift = Mods { shift: true, ..Default::default() };
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 150.0).with_mods(shift));
        assert_eq!(a, vec![Action::Exec("paint.sampleColor".into(), json!({"color": "#ffffff"}))]);
        // On the stroke edge → the stroke colour.
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 100.0, 150.0).with_mods(shift));
        assert_eq!(a, vec![Action::Exec("paint.sampleColor".into(), json!({"color": "#000000"}))]);
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 400.0, 400.0)).is_empty());
    }

    #[test]
    fn gradient_sampled_along_vector() {
        let g = vectorcraft_color::GradientPaint::new(vectorcraft_color::Gradient::default());
        let paint = Paint::Gradient(Box::new(g));
        let r = vectorcraft_geom::Rect::new(0.0, 0.0, 100.0, 10.0);
        assert_eq!(paint_color_at(&paint, Some(r), Point::new(0.0, 5.0)).unwrap().to_hex(), "#ffffff");
        assert_eq!(paint_color_at(&paint, Some(r), Point::new(100.0, 5.0)).unwrap().to_hex(), "#000000");
    }

    #[test]
    fn radial_sample_honours_aspect() {
        let mut g = vectorcraft_color::GradientPaint::new(vectorcraft_color::Gradient {
            kind: vectorcraft_color::GradientKind::Radial,
            ..Default::default()
        });
        // Radius 100 along x, 50 across: 50 pt above the centre is already the outer (black) edge.
        g.geom = Some(vectorcraft_color::GradientGeom { start: Point::ZERO, end: Point::new(100.0, 0.0), aspect: 0.5 });
        let paint = Paint::Gradient(Box::new(g));
        assert_eq!(paint_color_at(&paint, None, Point::new(0.0, -50.0)).unwrap().to_hex(), "#000000");
        assert_eq!(paint_color_at(&paint, None, Point::new(50.0, 0.0)).unwrap().to_hex(), "#808080");
    }
}
