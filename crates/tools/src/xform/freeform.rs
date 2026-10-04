//! The Gradient tool on a freeform gradient.
//!
//! The annotator shows each point as a colour chip, the lines through them and, around the
//! selected point, its spread. Click a point to select it (`paint.freeform.selectPoint`; the
//! Gradient and Color panels edit the selected point) and drag it to move it. Click inside the
//! selected art to add a point there, coloured as the gradient is there; in Lines mode (the
//! Gradient panel's Draw toggle) the new point joins the selected one by a line. Click a line to add
//! a point on it. Delete or Backspace removes the selected point.

use serde_json::{Value, json};
use vectorcraft_color::freeform::LineHit;
use vectorcraft_color::{Freeform, FreeformMode};
use vectorcraft_doc::hit::hit_test;
use vectorcraft_geom::{Point, Shape};

use super::paint_owner;
use crate::{Action, Cursor, Overlay, ToolContext, ToolKey};

const LINE: [u8; 3] = [0x20, 0x20, 0x20];
/// Hit radius of a point's chip and of a line, screen pixels.
const POINT_HIT: f64 = 7.0;
const LINE_HIT: f64 = 4.0;
/// A press becomes a drag after this much movement, screen pixels.
const DRAG_START: f64 = 3.0;

/// The freeform gradient behind the active proxy of the first selected object that has one: its
/// points in document coordinates and the length spreads are fractions of there.
pub(super) struct Annotator {
    pub freeform: Freeform,
    pub scale: f64,
}

impl Annotator {
    pub fn of(cx: &ToolContext) -> Option<Self> {
        cx.selection.objects.iter().find_map(|id| {
            let (freeform, scale) = cx.doc.node(*id)?.proxy_freeform(!cx.fill_active, cx.appearance_item)?;
            Some(Self { freeform, scale })
        })
    }

    /// The point under `p` (the topmost chip where chips overlap).
    fn point_at(&self, cx: &ToolContext, p: Point) -> Option<usize> {
        (0..self.freeform.points.len()).rev().find(|i| self.freeform.points[*i].at.distance(p) <= cx.tol(POINT_HIT))
    }

    /// The spot on a line under `p`.
    fn line_at(&self, cx: &ToolContext, p: Point) -> Option<LineHit> {
        self.freeform.nearest_on_lines(p).filter(|h| h.distance <= cx.tol(LINE_HIT))
    }

    /// The selected point, if it exists.
    fn selected(&self, cx: &ToolContext) -> Option<usize> {
        cx.freeform_point.filter(|i| *i < self.freeform.points.len())
    }

    pub fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let f = &self.freeform;
        let mut o: Vec<Overlay> =
            (0..f.lines.len()).map(|l| Overlay::Path { path: f.line_path(l), color: LINE, width: 1.0, dashed: false }).collect();
        if let Some(i) = self.selected(cx) {
            let pt = &f.points[i];
            let r = pt.spread as f64 * self.scale;
            if r > 0.0 {
                let path = vectorcraft_geom::kurbo::Circle::new(pt.at, r).to_path(cx.tol(0.25));
                o.push(Overlay::Path { path, color: LINE, width: 1.0, dashed: true });
            }
        }
        o.extend(f.points.iter().enumerate().map(|(i, pt)| Overlay::Swatch {
            p: pt.at,
            color: pt.color.to_rgba8(pt.opacity),
            selected: cx.freeform_point == Some(i),
        }));
        o
    }
}

/// What a press on a freeform annotator does.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Grab {
    /// A point: a drag moves it.
    Point(usize),
    /// A line: a click adds a point there.
    Line(LineHit),
    /// Inside selected art: a click adds a point there.
    Add,
    /// Elsewhere: a click leaves no point selected.
    Clear,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Gesture {
    grab: Grab,
    at: Point,
    began: bool,
}

impl Gesture {
    /// Has the press become a drag (an interaction is open)?
    pub fn began(&self) -> bool {
        self.began
    }
}

/// `params` aimed at the paint behind the active proxy.
fn on_proxy(cx: &ToolContext, mut params: Value) -> Value {
    params["stroke"] = json!(!cx.fill_active);
    params
}

fn select(i: Option<usize>) -> Action {
    Action::Exec("paint.freeform.selectPoint".into(), json!({ "index": i }))
}

pub(super) fn press(cx: &ToolContext, a: &Annotator, p: Point) -> (Vec<Action>, Gesture) {
    let mut out = vec![];
    let grab = if let Some(i) = a.point_at(cx, p) {
        if cx.freeform_point != Some(i) {
            out.push(select(Some(i)));
        }
        Grab::Point(i)
    } else if let Some(h) = a.line_at(cx, p) {
        Grab::Line(h)
    } else if hit_test(cx.doc, p, cx.hit_options()).is_some_and(|h| cx.selection.objects.contains(&paint_owner(cx.doc, h.leaf))) {
        Grab::Add
    } else {
        Grab::Clear
    };
    (out, Gesture { grab, at: p, began: false })
}

pub(super) fn drag(cx: &ToolContext, g: &mut Gesture, p: Point) -> Vec<Action> {
    let Grab::Point(index) = g.grab else { return vec![] };
    let mut out = vec![];
    if !g.began {
        if p.distance(g.at) < cx.tol(DRAG_START) {
            return out;
        }
        g.began = true;
        out.push(Action::Begin("Gradient".into()));
    }
    out.push(Action::Preview("paint.freeform.setPoint".into(), on_proxy(cx, json!({ "index": index, "at": [p.x, p.y] }))));
    out
}

pub(super) fn release(cx: &ToolContext, a: Option<&Annotator>, g: Gesture) -> Vec<Action> {
    if g.began {
        return vec![Action::Commit];
    }
    match g.grab {
        Grab::Point(_) => vec![],
        Grab::Line(h) => {
            vec![Action::Exec("paint.freeform.splitLine".into(), on_proxy(cx, json!({ "line": h.line, "segment": h.segment, "t": h.t })))]
        }
        Grab::Add => {
            let lines = a.is_some_and(|a| a.freeform.mode == FreeformMode::Lines);
            vec![Action::Exec("paint.freeform.addPoint".into(), on_proxy(cx, json!({ "at": [g.at.x, g.at.y], "line": lines })))]
        }
        Grab::Clear if cx.freeform_point.is_some() => vec![select(None)],
        Grab::Clear => vec![],
    }
}

/// Delete / Backspace remove the selected point (never the last one).
pub(super) fn claims_key(cx: &ToolContext, a: &Annotator, key: ToolKey) -> bool {
    matches!(key, ToolKey::Delete | ToolKey::Backspace) && a.selected(cx).is_some() && a.freeform.points.len() > 1
}

pub(super) fn key(cx: &ToolContext, a: &Annotator, key: ToolKey) -> Vec<Action> {
    if !claims_key(cx, a, key) {
        return vec![];
    }
    vec![Action::Exec("paint.freeform.deletePoint".into(), on_proxy(cx, json!({ "index": a.selected(cx) })))]
}

pub(super) fn cursor(cx: &ToolContext, a: &Annotator, g: Option<&Gesture>, p: Point) -> Cursor {
    if g.is_some_and(|g| g.began) || a.point_at(cx, p).is_some() {
        return Cursor::Move;
    }
    if a.line_at(cx, p).is_some()
        || hit_test(cx.doc, p, cx.hit_options()).is_some_and(|h| cx.selection.objects.contains(&paint_owner(cx.doc, h.leaf)))
    {
        return Cursor::AddStop;
    }
    Cursor::Crosshair
}

#[cfg(test)]
mod tests {
    use super::super::GradientTool;
    use super::*;
    use crate::testutil::*;
    use crate::{PointerEvent, PointerKind, Tool};
    use vectorcraft_color::{FreeformPoint, Gradient, GradientKind, GradientPaint, Paint};
    use vectorcraft_doc::{Document, Selection};

    /// The test rectangle (100..200) with three points; `lines` mode joins new ones.
    fn doc(mode: FreeformMode) -> (Document, Selection) {
        let (mut d, id) = doc_with_rect();
        let mut g = GradientPaint::new(Gradient { kind: GradientKind::Freeform, ..Default::default() });
        let pt = |x, y| FreeformPoint { spread: 0.2, ..FreeformPoint::new(Point::new(x, y), vectorcraft_color::Color::rgb(1.0, 0.0, 0.0)) };
        g.freeform = Some(Freeform { points: vec![pt(120.0, 120.0), pt(180.0, 120.0), pt(150.0, 180.0)], lines: vec![vec![0, 1]], mode });
        d.node_mut(id).unwrap().appearance.set_fill(Paint::Gradient(Box::new(g)));
        let mut s = Selection::default();
        s.add(id);
        (d, s)
    }

    fn ev(kind: PointerKind, x: f64, y: f64) -> PointerEvent {
        PointerEvent::new(kind, x, y)
    }

    fn click(t: &mut GradientTool, cx: &ToolContext, x: f64, y: f64) -> Vec<Action> {
        let mut a = t.pointer(cx, &ev(PointerKind::Down, x, y));
        a.extend(t.pointer(cx, &ev(PointerKind::Up, x, y)));
        a
    }

    #[test]
    fn clicks_select_points_add_points_and_split_lines() {
        let (d, s) = doc(FreeformMode::Points);
        let p = paint();
        let mut cx = cx(&d, &s, &p);
        let mut t = GradientTool::default();
        assert_eq!(click(&mut t, &cx, 151.0, 181.0), vec![select(Some(2))]);
        cx.freeform_point = Some(2);
        // Inside the art: a point there (Points mode: not joined).
        let a = click(&mut t, &cx, 140.0, 150.0);
        assert_eq!(a, vec![Action::Exec("paint.freeform.addPoint".into(), json!({"at": [140.0, 150.0], "line": false, "stroke": false}))]);
        // On the line: a point on it.
        let a = click(&mut t, &cx, 150.0, 122.0);
        let Action::Exec(c, v) = &a[0] else { panic!("{a:?}") };
        assert_eq!((c.as_str(), &v["line"], &v["segment"]), ("paint.freeform.splitLine", &json!(0), &json!(0)));
        // Off the art: the point selection clears.
        assert_eq!(click(&mut t, &cx, 400.0, 400.0), vec![select(None)]);
        // The overlays: a chip per point (the selected one ringed), the line and the spread.
        let o = t.overlays(&cx);
        assert_eq!(o.iter().filter(|o| matches!(o, Overlay::Swatch { .. })).count(), 3);
        assert!(o.iter().any(|o| matches!(o, Overlay::Swatch { selected: true, p, .. } if p == &Point::new(150.0, 180.0))));
        assert_eq!(o.iter().filter(|o| matches!(o, Overlay::Path { dashed: true, .. })).count(), 1, "the spread circle");
    }

    #[test]
    fn lines_mode_joins_new_points_and_drags_move_points() {
        let (d, s) = doc(FreeformMode::Lines);
        let p = paint();
        let mut cx = cx(&d, &s, &p);
        cx.freeform_point = Some(1);
        let mut t = GradientTool::default();
        let a = click(&mut t, &cx, 190.0, 190.0);
        assert_eq!(a, vec![Action::Exec("paint.freeform.addPoint".into(), json!({"at": [190.0, 190.0], "line": true, "stroke": false}))]);
        t.pointer(&cx, &ev(PointerKind::Down, 120.0, 121.0));
        let a = t.pointer(&cx, &ev(PointerKind::Drag, 130.0, 140.0));
        assert_eq!(a[0], Action::Begin("Gradient".into()));
        assert_eq!(a[1], Action::Preview("paint.freeform.setPoint".into(), json!({"index": 0, "at": [130.0, 140.0], "stroke": false})));
        assert_eq!(t.cursor(&cx, Point::new(130.0, 140.0), Default::default()), Cursor::Move);
        assert_eq!(t.pointer(&cx, &ev(PointerKind::Up, 130.0, 140.0)), vec![Action::Commit]);
        // Delete removes the selected point.
        assert!(t.claims_key(&cx, ToolKey::Delete));
        assert_eq!(
            t.key(&cx, ToolKey::Delete, Default::default()),
            vec![Action::Exec("paint.freeform.deletePoint".into(), json!({"index": 1, "stroke": false}))]
        );
    }
}
