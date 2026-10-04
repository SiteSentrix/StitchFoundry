//! Eyedropper tool (I).
//!
//! Click an object: copy its appearance (fill, stroke, weight, opacity) to the selection and the
//! paint defaults (`appearance.copyFrom`). Shift-click: sample only the colour under the cursor into
//! the active fill/stroke (`paint.sampleColor`).
//!
//! The Gradient panel's eyedropper sets the `stop` option (the tool to return to): the next click
//! samples the colour under the cursor into the selected gradient stop (`paint.sampleColor
//! {stop}`) and switches back to that tool.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::hit::{HitKind, hit_test};
use vectorcraft_geom::Point;

use super::paint_owner;
use crate::{Action, Cursor, Mods, PointerEvent, PointerKind, Tool, ToolContext};

#[derive(Default)]
pub struct EyedropperTool {
    /// Sampling for the selected gradient stop: the tool to switch back to afterwards.
    stop: Option<String>,
}

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
        let stop = self.stop.as_ref().zip(cx.gradient_stop);
        if ev.mods.shift || stop.is_some() {
            let fill_none = n.proxy_paint(false, None).is_none_or(|(p, ..)| p.is_none());
            let Some((paint, to_doc, bounds)) = n.proxy_paint(h.kind == HitKind::Stroke || fill_none, None) else { return vec![] };
            // Sample in the paint's own space (text space for type runs) and box.
            let p = if to_doc.determinant().abs() > 1e-12 { to_doc.inverse() * ev.pos } else { ev.pos };
            let Some(c) = paint_color_at(paint, Some(bounds), p) else { return vec![] };
            let Some((back, i)) = stop else { return vec![Action::Exec("paint.sampleColor".into(), json!({ "color": c.to_hex() }))] };
            let out = vec![Action::Exec("paint.sampleColor".into(), json!({ "color": c.to_hex(), "stop": i })), Action::SwitchTool(back.clone())];
            self.stop = None;
            return out;
        }
        vec![Action::Exec("appearance.copyFrom".into(), json!({ "source": src.0 }))]
    }

    fn options(&self) -> Value {
        json!({ "stop": self.stop })
    }

    fn set_option(&mut self, key: &str, value: &Value) {
        if key == "stop" {
            self.stop = value.as_str().map(str::to_string);
        }
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
        let mut t = EyedropperTool::default();
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
    fn the_stop_option_samples_into_the_selected_stop_then_switches_back() {
        let (d, _) = doc_with_rect();
        let s = Selection::default();
        let p = paint();
        let mut cx = cx(&d, &s, &p);
        let mut t = EyedropperTool::default();
        t.set_option("stop", &json!("gradient"));
        assert_eq!(t.options(), json!({"stop": "gradient"}));
        // Without a selected stop the click copies the appearance as usual.
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 150.0, 150.0));
        assert!(matches!(&a[0], Action::Exec(c, _) if c == "appearance.copyFrom"));
        cx.gradient_stop = Some(1);
        // Empty canvas: still waiting.
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 400.0, 400.0)).is_empty());
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 100.0, 150.0));
        assert_eq!(a, vec![Action::Exec("paint.sampleColor".into(), json!({"color": "#000000", "stop": 1})), Action::SwitchTool("gradient".into())]);
        assert_eq!(t.options(), json!({"stop": null}), "one sample only");
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
    fn type_gradients_are_sampled_in_text_space() {
        use vectorcraft_color::{Gradient, GradientGeom, GradientPaint};
        use vectorcraft_doc::{CharStyle, Document, Node, NodeKind, TextObject};
        let mut d = Document::new(500.0, 500.0);
        let l = d.layers[0].id;
        let id = d.alloc_id();
        // Type turned 90°: its baseline runs down from (100, 100).
        let mut t = TextObject::point(Point::ZERO, "Gradient", CharStyle { size: 20.0, ..Default::default() });
        t.xf = vectorcraft_geom::Affine::translate((100.0, 100.0)) * vectorcraft_geom::Affine::rotate(std::f64::consts::FRAC_PI_2);
        let mut g = GradientPaint::new(Gradient::default());
        g.geom = Some(GradientGeom { start: Point::ZERO, end: Point::new(100.0, 0.0), aspect: 1.0 });
        t.runs[0].style.fill = Paint::Gradient(Box::new(g));
        d.insert(Some(l), 0, Node::new(id, NodeKind::Text(Box::new(t)))).unwrap();
        let (s, p) = (Selection::default(), paint());
        let shift = Mods { shift: true, ..Default::default() };
        // 80 % along the baseline (text space (80, -5)).
        let a = EyedropperTool::default().pointer(&cx(&d, &s, &p), &PointerEvent::new(PointerKind::Down, 105.0, 180.0).with_mods(shift));
        assert_eq!(a, vec![Action::Exec("paint.sampleColor".into(), json!({"color": "#333333"}))]);
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
