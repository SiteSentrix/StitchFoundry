//! Gradient tool (G) and the gradient annotator.
//!
//! Drag across selected objects to set the gradient vector of the paint behind the active proxy
//! (fill or stroke; type objects' runs), a solid paint becoming the default gradient. Shift
//! constrains the angle to 45°. With nothing selected, the press targets the object under the
//! pointer; a click inside selected art applies the gradient from that point.
//!
//! The annotator is a bar from a round start handle to a square end handle. Drag the start handle
//! (or the bar) to move the gradient, the end handle to change its length and angle, and just past
//! the end to rotate it (Shift snaps to 45°). Colour stops sit under the bar: click the bar to add
//! one, drag one to move it (Alt drags a copy), drag it off the bar to delete it, and drag a
//! diamond above the bar to move a midpoint. Double-clicking a stop opens its popover. The
//! selected stop (`gradient.selectStop`) is shared with the Gradient and Color panels: Delete or
//! Backspace removes it (never below two stops) and the arrow keys nudge it.

use serde_json::{Value, json};
use vectorcraft_color::gradient::{duplicate_stop, insert_stop, midpoint_from_pos, midpoint_pos, move_stop, remove_stop, set_midpoint};
use vectorcraft_color::{Gradient, GradientGeom, GradientKind, GradientStop};
use vectorcraft_doc::NodeId;
use vectorcraft_doc::hit::hit_test;
use vectorcraft_geom::{Affine, Point, Vec2};

use super::paint_owner;
use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, ToolKey};

const BAR: [u8; 3] = [0x20, 0x20, 0x20];
const LIGHT: [u8; 3] = [0xf0, 0xf0, 0xf0];

// Annotator layout and hit radii, in screen pixels.
/// Start and end handles.
const HANDLE: f64 = 5.0;
/// The rotate zone reaches this far past the end handle.
const ROTATE: f64 = 14.0;
/// Stop chips sit this far below the bar.
const STOP_GAP: f64 = 10.0;
const STOP_HIT: f64 = 6.0;
/// Midpoint diamonds sit this far above the bar.
const MID_GAP: f64 = 7.0;
const MID_HIT: f64 = 5.0;
/// The bar (where a click adds a stop) reaches this far above it, and down through the stop row.
const BAR_HIT: f64 = 4.0;
/// A stop dragged farther than this from the bar is deleted on release.
const OFF_BAR: f64 = 24.0;
/// A press becomes a drag after this much movement.
const DRAG_START: f64 = 3.0;
/// Arrow-key nudges of the selected stop (Shift: the big step).
const NUDGE: f32 = 0.01;
const NUDGE_BIG: f32 = 0.1;

/// The gradient annotator: the gradient behind the active proxy of the first selected object that
/// has one, placed in document coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct Annotator {
    pub geom: GradientGeom,
    pub gradient: Gradient,
}

/// A part of the annotator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Part {
    Stop(usize),
    /// The midpoint diamond after stop `i`.
    Mid(usize),
    Start,
    End,
    /// Just past the end: rotates the vector about the start.
    Rotate,
    /// The bar at an offset (0..1): a click adds a stop there.
    Bar(f32),
}

impl Annotator {
    pub fn of(cx: &ToolContext) -> Option<Self> {
        cx.selection.objects.iter().find_map(|id| {
            let (g, geom) = cx.doc.node(*id)?.proxy_gradient(!cx.fill_active)?;
            Some(Self { geom, gradient: g.gradient.clone() })
        })
    }
    /// An annotator showing `stops` on `geom` (the state a drag started from).
    fn with(geom: GradientGeom, stops: &[GradientStop]) -> Self {
        Self { geom, gradient: Gradient { kind: GradientKind::Linear, stops: stops.to_vec() } }
    }
    fn vector(&self) -> Vec2 {
        self.geom.end - self.geom.start
    }
    /// The point at offset `t` along the bar.
    pub fn at(&self, t: f64) -> Point {
        self.geom.start + self.vector() * t
    }
    /// Unit normal to the bar, towards the stop row.
    fn normal(&self) -> Vec2 {
        let v = self.vector();
        let len = v.hypot();
        if len < 1e-12 { Vec2::new(0.0, 1.0) } else { Vec2::new(-v.y, v.x) / len }
    }
    /// Where stop `i`'s chip sits.
    pub fn stop_point(&self, cx: &ToolContext, i: usize) -> Option<Point> {
        let s = self.gradient.stops.get(i)?;
        Some(self.at(s.offset as f64) + self.normal() * cx.tol(STOP_GAP))
    }
    /// Where the midpoint diamond after stop `i` sits.
    pub fn mid_point(&self, cx: &ToolContext, i: usize) -> Option<Point> {
        Some(self.at(midpoint_pos(&self.gradient.stops, i)? as f64) - self.normal() * cx.tol(MID_GAP))
    }
    /// `p` as (offset along the bar, signed distance from it towards the stop row).
    fn project(&self, p: Point) -> (f64, f64) {
        let v = self.vector();
        let l2 = v.hypot2();
        let d = p - self.geom.start;
        if l2 < 1e-18 {
            return (0.0, d.hypot());
        }
        (d.dot(v) / l2, d.dot(self.normal()))
    }
    /// The part under `p`: stops, then midpoints, the handles, the rotate zone and the bar.
    pub fn hit(&self, cx: &ToolContext, p: Point) -> Option<Part> {
        let near = |q: Option<Point>, r: f64| q.is_some_and(|q| q.distance(p) <= cx.tol(r));
        let n = self.gradient.stops.len();
        // The topmost (last drawn) chip wins where chips overlap.
        if let Some(i) = (0..n).rev().find(|i| near(self.stop_point(cx, *i), STOP_HIT)) {
            return Some(Part::Stop(i));
        }
        if let Some(i) = (0..n.saturating_sub(1)).find(|i| near(self.mid_point(cx, *i), MID_HIT)) {
            return Some(Part::Mid(i));
        }
        if near(Some(self.geom.end), HANDLE) {
            return Some(Part::End);
        }
        if near(Some(self.geom.start), HANDLE) {
            return Some(Part::Start);
        }
        let (t, d) = self.project(p);
        if t > 1.0 && near(Some(self.geom.end), ROTATE) {
            return Some(Part::Rotate);
        }
        let across = -cx.tol(BAR_HIT)..=cx.tol(STOP_GAP + STOP_HIT);
        if self.vector().hypot() > 1e-12 && (0.0..=1.0).contains(&t) && across.contains(&d) {
            return Some(Part::Bar(t as f32));
        }
        None
    }
}

/// What a press grabbed.
#[derive(Clone, Debug, PartialEq)]
enum Grab {
    /// The art (or empty canvas): a click applies the gradient from there, a drag draws a vector.
    Art,
    /// The start handle or the bar: a drag moves the gradient (a click on the bar adds a stop).
    Move {
        geom: GradientGeom,
        bar: Option<f32>,
    },
    End(GradientGeom),
    Rotate(GradientGeom),
    Stop {
        index: usize,
        from: Annotator,
        copy: bool,
    },
    Mid {
        index: usize,
        from: Annotator,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct Gesture {
    grab: Grab,
    /// Where the press happened.
    at: Point,
    /// Moved past the drag threshold (an interaction is open).
    began: bool,
    /// The vector drawn so far (Art drags).
    vector: Option<(Point, Point)>,
    /// Where a dragged stop ends up (selected on release): its index, or None while it is dragged
    /// off the bar.
    stop: Option<Option<usize>>,
}

#[derive(Default)]
pub struct GradientTool {
    gesture: Option<Gesture>,
}

/// Overlays of an annotator: the bar, the handles, the midpoint diamonds and a chip per stop.
fn annotator_overlays(cx: &ToolContext, a: &Annotator) -> Vec<Overlay> {
    let (s, e) = (a.geom.start, a.geom.end);
    let shade = Vec2::new(0.0, cx.tol(1.0));
    let mut o = vec![
        Overlay::Line { a: s, b: e, color: BAR, dashed: false },
        Overlay::Line { a: s + shade, b: e + shade, color: LIGHT, dashed: false },
        Overlay::Handle { p: s, color: BAR },
        Overlay::Anchor { p: e, color: BAR, filled: true, size: 7.0 },
    ];
    let len = a.vector().hypot();
    if len < 1e-12 {
        return o;
    }
    let r = cx.tol(4.0);
    let (along, across) = (a.vector() / len * r, a.normal() * r);
    for i in 0..a.gradient.stops.len().saturating_sub(1) {
        let Some(c) = a.mid_point(cx, i) else { continue };
        let path = super::polygon(&[c - along, c - across, c + along, c + across], true);
        o.push(Overlay::Path { path, color: BAR, width: 1.0, dashed: false });
    }
    for (i, st) in a.gradient.stops.iter().enumerate() {
        let Some(p) = a.stop_point(cx, i) else { continue };
        o.push(Overlay::Line { a: a.at(st.offset as f64), b: p, color: BAR, dashed: false });
        o.push(Overlay::Swatch { p, color: st.color.to_rgba8(st.opacity), selected: cx.gradient_stop == Some(i) });
    }
    o
}

/// The vector a click applies to `id`: its current gradient's, else the default fit on its box.
fn click_vector(cx: &ToolContext, id: NodeId) -> Option<Vec2> {
    let stroke = !cx.fill_active;
    let n = cx.doc.node(id)?;
    if let Some((_, g)) = n.proxy_gradient(stroke) {
        return Some(g.end - g.start);
    }
    let (to_doc, b) = match n.proxy_paint(stroke) {
        Some((_, a, b)) => (a, b),
        None => (Affine::IDENTITY, n.geometric_bounds()?),
    };
    let mut g = GradientGeom::fit(GradientKind::Linear, b, 0.0);
    g.transform(to_doc, GradientKind::Linear);
    Some(g.end - g.start)
}

impl GradientTool {
    fn geom_params(cx: &ToolContext, start: Point, end: Point) -> Value {
        json!({ "start": [start.x, start.y], "end": [end.x, end.y], "stroke": !cx.fill_active })
    }
    fn stops_params(cx: &ToolContext, stops: &[GradientStop]) -> Value {
        json!({ "stops": crate::params::stops_json(stops), "stroke": !cx.fill_active })
    }
    fn select_stop(i: usize) -> Action {
        Action::Exec("gradient.selectStop".into(), json!({ "index": i }))
    }

    fn press(&mut self, cx: &ToolContext, p: Point, mods: Mods) -> Vec<Action> {
        let annotator = Annotator::of(cx);
        let mut out = vec![];
        let grab = match annotator.as_ref().and_then(|a| a.hit(cx, p).map(|part| (a, part))) {
            Some((a, Part::Stop(index))) => {
                out.push(Self::select_stop(index));
                Grab::Stop { index, from: a.clone(), copy: mods.alt }
            }
            Some((a, Part::Mid(index))) => Grab::Mid { index, from: a.clone() },
            Some((a, Part::Start)) => Grab::Move { geom: a.geom, bar: None },
            Some((a, Part::Bar(t))) => Grab::Move { geom: a.geom, bar: Some(t) },
            Some((a, Part::End)) => Grab::End(a.geom),
            Some((a, Part::Rotate)) => Grab::Rotate(a.geom),
            None => {
                if cx.selection.is_empty() {
                    let Some(h) = hit_test(cx.doc, p, cx.hit_options()) else { return out };
                    out.push(Action::Exec("select.set".into(), json!({ "ids": [paint_owner(cx.doc, h.leaf).0] })));
                }
                Grab::Art
            }
        };
        self.gesture = Some(Gesture { grab, at: p, began: false, vector: None, stop: None });
        out
    }

    fn drag(&mut self, cx: &ToolContext, p: Point, mods: Mods) -> Vec<Action> {
        let Some(g) = &mut self.gesture else { return vec![] };
        let mut out = vec![];
        if !g.began {
            if p.distance(g.at) < cx.tol(DRAG_START) {
                return out;
            }
            g.began = true;
            out.push(Action::Begin("Gradient".into()));
        }
        let snap = |from: Point, to: Point| if mods.shift { from + vectorcraft_geom::constrain_angle(to - from, 45.0) } else { to };
        let (cmd, params) = match &g.grab {
            Grab::Art => {
                let end = snap(g.at, p);
                g.vector = Some((g.at, end));
                ("paint.setGradientGeom", Self::geom_params(cx, g.at, end))
            }
            Grab::Move { geom, .. } => {
                let d = p - g.at;
                ("paint.setGradientGeom", Self::geom_params(cx, geom.start + d, geom.end + d))
            }
            Grab::End(geom) => ("paint.setGradientGeom", Self::geom_params(cx, geom.start, snap(geom.start, p))),
            Grab::Rotate(geom) => {
                let dir = snap(geom.start, p) - geom.start;
                let len = dir.hypot();
                let end = if len < 1e-12 { geom.end } else { geom.start + dir * (geom.length() / len) };
                ("paint.setGradientGeom", Self::geom_params(cx, geom.start, end))
            }
            Grab::Stop { index, from, copy } => {
                let (t, d) = from.project(p);
                let t = t.clamp(0.0, 1.0) as f32;
                let stops = &from.gradient.stops;
                let removed = (!copy && d.abs() > cx.tol(OFF_BAR)).then(|| remove_stop(stops, *index)).flatten();
                let (stops, at) = match removed {
                    Some(v) => (v, None),
                    None => {
                        let (v, i) = if *copy { duplicate_stop(stops, *index, t) } else { move_stop(stops, *index, t) };
                        (v, Some(i))
                    }
                };
                g.stop = Some(at);
                ("paint.editGradient", Self::stops_params(cx, &stops))
            }
            Grab::Mid { index, from } => {
                let stops = &from.gradient.stops;
                let m = midpoint_from_pos(stops, *index, from.project(p).0.clamp(0.0, 1.0) as f32).unwrap_or(0.5);
                ("paint.editGradient", Self::stops_params(cx, &set_midpoint(stops, *index, m)))
            }
        };
        out.push(Action::Preview(cmd.into(), params));
        out
    }

    fn release(&mut self, cx: &ToolContext, p: Point) -> Vec<Action> {
        let Some(g) = self.gesture.take() else { return vec![] };
        if g.began {
            let mut out = vec![Action::Commit];
            match (g.grab, g.stop) {
                (Grab::Stop { .. }, Some(Some(i))) => out.push(Self::select_stop(i)),
                // Dragged off the bar: the stop that slid into its place is selected.
                (Grab::Stop { index, from, .. }, Some(None)) => out.push(Self::select_stop(index.min(from.gradient.stops.len() - 2))),
                _ => {}
            }
            return out;
        }
        match g.grab {
            // A click on the bar adds a stop there.
            Grab::Move { bar: Some(t), .. } => {
                let Some(a) = Annotator::of(cx) else { return vec![] };
                let (stops, i) = insert_stop(&a.gradient, t);
                vec![Action::Exec("paint.editGradient".into(), Self::stops_params(cx, &stops)), Self::select_stop(i)]
            }
            // A click inside selected art applies the gradient from that point.
            Grab::Art => {
                let Some(h) = hit_test(cx.doc, p, cx.hit_options()) else { return vec![] };
                let id = paint_owner(cx.doc, h.leaf);
                let Some(v) = cx.selection.objects.contains(&id).then(|| click_vector(cx, id)).flatten() else { return vec![] };
                let mut params = Self::geom_params(cx, p, p + v);
                params["ids"] = json!([id.0]);
                vec![Action::Exec("paint.setGradientGeom".into(), params)]
            }
            _ => vec![],
        }
    }
}

impl Tool for GradientTool {
    fn id(&self) -> &'static str {
        "gradient"
    }

    fn busy(&self) -> bool {
        self.gesture.as_ref().is_some_and(|g| g.began)
    }

    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        match ev.kind {
            PointerKind::Down => self.press(cx, ev.pos, ev.mods),
            PointerKind::Drag => self.drag(cx, ev.pos, ev.mods),
            PointerKind::Up => self.release(cx, ev.pos),
            PointerKind::DoubleClick => {
                let Some(a) = Annotator::of(cx) else { return vec![] };
                let Some(Part::Stop(i)) = a.hit(cx, ev.pos) else { return vec![] };
                let at = a.stop_point(cx, i).unwrap_or(ev.pos);
                vec![Self::select_stop(i), Action::Dialog("gradientStop".into(), json!({ "index": i, "x": at.x, "y": at.y }))]
            }
            PointerKind::Move => vec![],
        }
    }

    fn claims_key(&self, cx: &ToolContext, key: ToolKey) -> bool {
        matches!(key, ToolKey::Delete | ToolKey::Backspace | ToolKey::Left | ToolKey::Right)
            && cx.gradient_stop.is_some_and(|i| Annotator::of(cx).is_some_and(|a| i < a.gradient.stops.len()))
    }

    fn key(&mut self, cx: &ToolContext, key: ToolKey, mods: Mods) -> Vec<Action> {
        if key == ToolKey::Escape && self.busy() {
            self.gesture = None;
            return vec![Action::Cancel];
        }
        if self.busy() || !self.claims_key(cx, key) {
            return vec![];
        }
        let (Some(a), Some(i)) = (Annotator::of(cx), cx.gradient_stop) else { return vec![] };
        let stops = &a.gradient.stops;
        let (new, sel) = match key {
            // Never below two stops: Delete then does nothing.
            ToolKey::Delete | ToolKey::Backspace => match remove_stop(stops, i) {
                Some(v) => {
                    let sel = i.min(v.len() - 1);
                    (v, sel)
                }
                None => return vec![],
            },
            _ => {
                let step = if mods.shift { NUDGE_BIG } else { NUDGE };
                move_stop(stops, i, stops[i].offset + if key == ToolKey::Left { -step } else { step })
            }
        };
        vec![Action::Exec("paint.editGradient".into(), Self::stops_params(cx, &new)), Self::select_stop(sel)]
    }

    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        match (Annotator::of(cx), self.gesture.as_ref().and_then(|g| g.vector)) {
            (Some(a), _) => annotator_overlays(cx, &a),
            // A vector drawn on a paint that isn't a gradient yet.
            (None, Some((s, e))) => {
                annotator_overlays(cx, &Annotator::with(GradientGeom { start: s, end: e, aspect: 1.0 }, &Gradient::default().stops))
            }
            _ => vec![],
        }
    }

    fn cursor(&self, cx: &ToolContext, p: Point, _m: Mods) -> Cursor {
        if let Some(g) = &self.gesture {
            return match (&g.grab, g.stop) {
                (Grab::Stop { .. }, Some(None)) => Cursor::RemoveStop,
                (Grab::Rotate(_), _) => Cursor::Rotate,
                (Grab::Art, _) => Cursor::Crosshair,
                _ => Cursor::Move,
            };
        }
        match Annotator::of(cx).and_then(|a| a.hit(cx, p)) {
            Some(Part::Bar(_)) => Cursor::AddStop,
            Some(Part::Rotate) => Cursor::Rotate,
            Some(_) => Cursor::Move,
            None => Cursor::Crosshair,
        }
    }

    fn options(&self) -> Value {
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_color::{GradientPaint, Paint};
    use vectorcraft_doc::{Document, Selection};

    fn ev(kind: PointerKind, p: Point) -> PointerEvent {
        PointerEvent::new(kind, p.x, p.y)
    }

    fn with(m: Mods, e: PointerEvent) -> PointerEvent {
        e.with_mods(m)
    }

    const SHIFT: Mods = Mods { shift: true, alt: false, cmd: false, ctrl: false, space: false };
    const ALT: Mods = Mods { shift: false, alt: true, cmd: false, ctrl: false, space: false };

    /// The test rectangle (100..200) with a horizontal gradient of `stops` from (100,150) to (200,150).
    fn graded(stops: Vec<GradientStop>) -> (Document, Selection) {
        let (mut d, id) = doc_with_rect();
        let mut g = GradientPaint::new(Gradient { kind: GradientKind::Linear, stops });
        g.geom = Some(GradientGeom { start: Point::new(100.0, 150.0), end: Point::new(200.0, 150.0), aspect: 1.0 });
        d.node_mut(id).unwrap().appearance.set_fill(Paint::Gradient(Box::new(g)));
        let mut s = Selection::default();
        s.add(id);
        (d, s)
    }

    fn two() -> Vec<GradientStop> {
        Gradient::default().stops
    }

    fn three() -> Vec<GradientStop> {
        insert_stop(&Gradient::default(), 0.5).0
    }

    fn preview(a: &[Action]) -> &Value {
        a.iter().find_map(|a| if let Action::Preview(_, v) = a { Some(v) } else { None }).unwrap_or_else(|| panic!("no preview in {a:?}"))
    }

    fn exec(a: &Action) -> (&str, &Value) {
        match a {
            Action::Exec(c, v) => (c.as_str(), v),
            a => panic!("not an Exec: {a:?}"),
        }
    }

    fn point(v: &Value) -> Point {
        Point::new(v[0].as_f64().unwrap(), v[1].as_f64().unwrap())
    }

    /// Stop offsets in percent (rounded: offsets travel as f32).
    fn offsets(v: &Value) -> Vec<f64> {
        v["stops"].as_array().unwrap().iter().map(|s| (s["offset"].as_f64().unwrap() * 100.0).round()).collect()
    }

    #[test]
    fn drag_sets_gradient_vector_and_constrains() {
        let (d, id) = doc_with_rect();
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        assert!(t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 110.0, 150.0)).is_empty());
        let a = t.pointer(&cx, &with(SHIFT, PointerEvent::new(PointerKind::Drag, 190.0, 152.0)));
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
    fn stroke_proxy_drags_and_annotates_the_stroke() {
        let (mut d, id) = doc_with_rect();
        d.node_mut(id).unwrap().appearance.set_stroke(Paint::Gradient(Box::new(GradientPaint::new(Gradient::default()))));
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let mut cx = cx(&d, &s, &p);
        // The fill is solid: with the Fill proxy in front there is no annotator.
        assert!(GradientTool::default().overlays(&cx).is_empty());
        cx.fill_active = false;
        // The stroke's annotator spans the stroke-inflated box (1 pt stroke: 99.5..200.5).
        let a = Annotator::of(&cx).unwrap();
        assert_eq!((a.geom.start, a.geom.end, a.gradient.stops.len()), (Point::new(99.5, 150.0), Point::new(200.5, 150.0), 2));
        let mut t = GradientTool::default();
        t.pointer(&cx, &PointerEvent::new(PointerKind::Down, 110.0, 120.0));
        let a = t.pointer(&cx, &PointerEvent::new(PointerKind::Drag, 190.0, 120.0));
        assert_eq!(preview(&a)["stroke"], json!(true));
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
    fn the_end_handle_changes_only_the_end_and_the_start_moves_both() {
        let (d, s) = graded(two());
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        t.pointer(&cx, &ev(PointerKind::Down, Point::new(200.0, 150.0)));
        let v = preview(&t.pointer(&cx, &ev(PointerKind::Drag, Point::new(220.0, 170.0)))).clone();
        assert_eq!((point(&v["start"]), point(&v["end"])), (Point::new(100.0, 150.0), Point::new(220.0, 170.0)));
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Up, Point::new(220.0, 170.0))), vec![Action::Commit]);
        // The start handle moves the whole vector.
        t.pointer(&cx, &ev(PointerKind::Down, Point::new(100.0, 150.0)));
        let v = preview(&t.pointer(&cx, &ev(PointerKind::Drag, Point::new(110.0, 140.0)))).clone();
        assert_eq!((point(&v["start"]), point(&v["end"])), (Point::new(110.0, 140.0), Point::new(210.0, 140.0)));
    }

    #[test]
    fn past_the_end_rotates_and_shift_snaps() {
        let (d, s) = graded(two());
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        assert_eq!(t.cursor(&cx, Point::new(210.0, 151.0), Mods::default()), Cursor::Rotate);
        t.pointer(&cx, &ev(PointerKind::Down, Point::new(210.0, 151.0)));
        let end = point(&preview(&t.pointer(&cx, &with(SHIFT, ev(PointerKind::Drag, Point::new(103.0, 60.0)))))["end"]);
        // Snapped to straight up, keeping the 100 pt length.
        assert!(end.distance(Point::new(100.0, 50.0)) < 1e-9, "{end:?}");
    }

    #[test]
    fn a_bar_click_adds_a_stop_and_selects_it() {
        let (d, s) = graded(two());
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        assert_eq!(t.cursor(&cx, Point::new(130.0, 150.0), Mods::default()), Cursor::AddStop);
        assert!(t.pointer(&cx, &ev(PointerKind::Down, Point::new(130.0, 150.0))).is_empty());
        let a = t.pointer(&cx, &ev(PointerKind::Up, Point::new(130.0, 150.0)));
        let (c, v) = exec(&a[0]);
        assert_eq!((c, offsets(v)), ("paint.editGradient", vec![0.0, 30.0, 100.0]));
        assert_eq!(a[1], GradientTool::select_stop(1));
    }

    #[test]
    fn stops_drag_along_the_bar_off_it_to_delete_and_alt_copies() {
        let (d, s) = graded(two());
        let p = paint();
        let cx = cx(&d, &s, &p);
        let chip = Annotator::of(&cx).unwrap().stop_point(&cx, 0).unwrap();
        let mut t = GradientTool::default();
        // Pressing a stop selects it; dragging moves it.
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Down, chip)), vec![GradientTool::select_stop(0)]);
        assert_eq!(offsets(preview(&t.pointer(&cx, &ev(PointerKind::Drag, Point::new(140.0, chip.y))))), vec![40.0, 100.0]);
        assert_eq!(t.cursor(&cx, chip, Mods::default()), Cursor::Move);
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Up, Point::new(140.0, chip.y))), vec![Action::Commit, GradientTool::select_stop(0)]);
        // Two stops never lose one: dragged off the bar, the stop stays.
        t.pointer(&cx, &ev(PointerKind::Down, chip));
        assert_eq!(offsets(preview(&t.pointer(&cx, &ev(PointerKind::Drag, Point::new(100.0, 260.0))))), vec![0.0, 100.0]);
        t.pointer(&cx, &ev(PointerKind::Up, Point::new(100.0, 260.0)));
        // Alt-drag leaves the original and drags a copy.
        t.pointer(&cx, &with(ALT, ev(PointerKind::Down, chip)));
        assert_eq!(offsets(preview(&t.pointer(&cx, &with(ALT, ev(PointerKind::Drag, Point::new(150.0, chip.y)))))), vec![0.0, 50.0, 100.0]);
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Up, Point::new(150.0, chip.y))), vec![Action::Commit, GradientTool::select_stop(1)]);

        // With three stops, dragging the middle one off the bar deletes it.
        let (d, s) = graded(three());
        let cx = crate::testutil::cx(&d, &s, &p);
        let chip = Annotator::of(&cx).unwrap().stop_point(&cx, 1).unwrap();
        t.pointer(&cx, &ev(PointerKind::Down, chip));
        assert_eq!(offsets(preview(&t.pointer(&cx, &ev(PointerKind::Drag, Point::new(150.0, 200.0))))), vec![0.0, 100.0]);
        assert_eq!(t.cursor(&cx, Point::new(150.0, 200.0), Mods::default()), Cursor::RemoveStop);
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Up, Point::new(150.0, 200.0))), vec![Action::Commit, GradientTool::select_stop(1)]);
    }

    #[test]
    fn diamonds_move_midpoints() {
        let (d, s) = graded(two());
        let p = paint();
        let cx = cx(&d, &s, &p);
        let diamond = Annotator::of(&cx).unwrap().mid_point(&cx, 0).unwrap();
        let mut t = GradientTool::default();
        t.pointer(&cx, &ev(PointerKind::Down, diamond));
        let v = preview(&t.pointer(&cx, &ev(PointerKind::Drag, Point::new(130.0, diamond.y)))).clone();
        assert!((v["stops"][0]["midpoint"].as_f64().unwrap() - 0.3).abs() < 1e-6, "{v}");
    }

    #[test]
    fn delete_and_arrows_act_on_the_selected_stop() {
        let (d, s) = graded(two());
        let p = paint();
        let mut cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        assert!(!t.claims_key(&cx, ToolKey::Delete), "no stop selected");
        cx.gradient_stop = Some(1);
        assert!(t.claims_key(&cx, ToolKey::Delete) && t.claims_key(&cx, ToolKey::Backspace) && !t.claims_key(&cx, ToolKey::Enter));
        // Two stops: Delete is claimed but does nothing.
        assert!(t.key(&cx, ToolKey::Delete, Mods::default()).is_empty());
        let a = t.key(&cx, ToolKey::Left, SHIFT);
        let (c, v) = exec(&a[0]);
        assert_eq!((c, offsets(v)), ("paint.editGradient", vec![0.0, 90.0]));
        assert_eq!(a[1], GradientTool::select_stop(1));
        // With three stops Delete removes the selected one and selects its successor.
        let (d, s) = graded(three());
        let mut cx = crate::testutil::cx(&d, &s, &p);
        cx.gradient_stop = Some(1);
        let a = t.key(&cx, ToolKey::Backspace, Mods::default());
        assert_eq!(offsets(exec(&a[0]).1), vec![0.0, 100.0]);
        assert_eq!(a[1], GradientTool::select_stop(1));
    }

    #[test]
    fn a_click_inside_the_art_applies_the_gradient_from_there() {
        let (d, id) = doc_with_rect();
        let mut s = Selection::default();
        s.add(id);
        let p = paint();
        let cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        t.pointer(&cx, &ev(PointerKind::Down, Point::new(150.0, 120.0)));
        let a = t.pointer(&cx, &ev(PointerKind::Up, Point::new(150.0, 120.0)));
        let (c, v) = exec(&a[0]);
        assert_eq!(c, "paint.setGradientGeom");
        // The default fit's 100 pt horizontal vector, starting at the click.
        assert_eq!((point(&v["start"]), point(&v["end"]), &v["ids"]), (Point::new(150.0, 120.0), Point::new(250.0, 120.0), &json!([id.0])));
        // Outside the art a click does nothing.
        t.pointer(&cx, &ev(PointerKind::Down, Point::new(400.0, 400.0)));
        assert!(t.pointer(&cx, &ev(PointerKind::Up, Point::new(400.0, 400.0))).is_empty());
    }

    #[test]
    fn double_click_on_a_stop_opens_its_popover() {
        let (d, s) = graded(two());
        let p = paint();
        let cx = cx(&d, &s, &p);
        let chip = Annotator::of(&cx).unwrap().stop_point(&cx, 1).unwrap();
        let a = GradientTool::default().pointer(&cx, &ev(PointerKind::DoubleClick, chip));
        assert_eq!(a[0], GradientTool::select_stop(1));
        assert_eq!(a[1], Action::Dialog("gradientStop".into(), json!({"index": 1, "x": chip.x, "y": chip.y})));
    }

    #[test]
    fn overlays_draw_a_chip_per_stop_marking_the_selected_one() {
        let (d, s) = graded(two());
        let p = paint();
        let mut cx = cx(&d, &s, &p);
        cx.gradient_stop = Some(1);
        let o = GradientTool::default().overlays(&cx);
        let chips: Vec<bool> = o.iter().filter_map(|o| if let Overlay::Swatch { selected, .. } = o { Some(*selected) } else { None }).collect();
        assert_eq!(chips, vec![false, true]);
        assert_eq!(o.iter().filter(|o| matches!(o, Overlay::Path { .. })).count(), 1, "one midpoint diamond");
    }
}
