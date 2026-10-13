//! Click a highlighted segment, then click to place its linked dimension. Empty-space clicks
//! measure two points (three for an angle). Escape cancels without leaving any artwork.
use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext, ToolKey};
use serde_json::{Value, json};
use vectorcraft_doc::{
    Node,
    dimension::{Dimension, DimensionOptions, DimensionSource, SegmentRef},
};
use vectorcraft_geom::{BezPath, Point, Vec2};

#[derive(Default)]
pub struct DimensionTool {
    mode: String,
    style: DimensionOptions,
    points: Vec<Point>,
    point_refs: Vec<Option<vectorcraft_doc::dimension::PointRef>>,
    first_edge: Option<SegmentRef>,
    pending: Option<Dimension>,
    hover: Option<SegmentRef>,
    at: Point,
    began: bool,
}

/// Nearest editable segment, with bounded work and no snapping to dimension artwork itself.
pub fn pick(cx: &ToolContext, p: Point) -> Option<SegmentRef> {
    let mut best = None;
    let mut dist = cx.tol(8.0);
    let mut budget = 20_000usize;
    fn visit(n: &Node, p: Point, budget: &mut usize, dist: &mut f64, best: &mut Option<SegmentRef>) {
        if *budget == 0 || !n.visible || n.locked || n.extras().dimension.is_some() {
            return;
        }
        if let Some(path) = n.path_data() {
            for (si, sp) in path.subpaths.iter().enumerate() {
                for i in 0..sp.segment_count() {
                    if *budget == 0 {
                        return;
                    }
                    *budget -= 1;
                    use vectorcraft_geom::kurbo::ParamCurveNearest;
                    let near = sp.segment(i).nearest(p, 1e-4);
                    if near.distance_sq < *dist * *dist {
                        *dist = near.distance_sq.sqrt();
                        *best = Some(SegmentRef { node: n.id, subpath: si, segment: i });
                    }
                }
            }
        }
        if let Some(ch) = n.children() {
            for c in ch.iter().rev() {
                visit(c, p, budget, dist, best);
            }
        }
    }
    for n in cx.doc.layers.iter().rev() {
        visit(n, p, &mut budget, &mut dist, &mut best);
    }
    best
}

impl DimensionTool {
    fn point_source(&self, angular: bool) -> Option<DimensionSource> {
        if self.point_refs.iter().all(Option::is_none) {
            return None;
        }
        Some(DimensionSource::Points {
            a: self.point_refs.first().cloned().flatten(),
            b: self.point_refs.get(if angular { 2 } else { 1 }).cloned().flatten(),
            vertex: if angular { self.point_refs.get(1).cloned().flatten() } else { None },
        })
    }
    fn mode(&self) -> &str {
        if self.mode.is_empty() { "linear" } else { &self.mode }
    }
    fn recipe(&self, a: Point, b: Point, vertex: Point, source: Option<DimensionSource>) -> Dimension {
        Dimension { mode: self.mode().into(), a, b, vertex, offset: 30.0, length: a.distance(b), options: self.style.clone(), source }
    }
    fn params(d: &Dimension) -> Value {
        let mut p = json!({"mode":d.mode,"a":[d.a.x,d.a.y],"b":[d.b.x,d.b.y],"vertex":[d.vertex.x,d.vertex.y],"offset":d.offset,"options":d.options});
        if let Some(source) = &d.source {
            p["source"] = json!(source);
        }
        p
    }
    fn preview(&mut self, cx: &ToolContext, p: Point) -> Vec<Action> {
        let Some(d) = &mut self.pending else { return vec![] };
        if d.mode == "linear" {
            let u = d.b - d.a;
            d.offset = (p - d.a).dot(Vec2::new(-u.y, u.x) / u.hypot());
        } else if d.mode == "angular" {
            d.offset = p.distance(d.vertex).max(12.0);
        } else if p.distance(d.a) > 1e-6 {
            let r = d.a.distance(d.b);
            let u = p - d.a;
            d.b = d.a + u / u.hypot() * r;
        }
        if d.resolve(cx.doc).is_err() {
            return vec![];
        }
        let mut actions = vec![];
        if !self.began {
            actions.push(Action::Begin("Create Dimension".into()));
            self.began = true;
        }
        actions.push(Action::Preview("dimension.create".into(), Self::params(d)));
        actions
    }
    fn reset(&mut self) {
        self.pending = None;
        self.points.clear();
        self.point_refs.clear();
        self.first_edge = None;
        self.began = false;
    }
}

impl Tool for DimensionTool {
    fn id(&self) -> &'static str {
        "dimension"
    }
    fn busy(&self) -> bool {
        self.pending.is_some() || !self.points.is_empty() || self.first_edge.is_some()
    }
    fn cursor(&self, _: &ToolContext, _: Point, _: Mods) -> Cursor {
        Cursor::Crosshair
    }
    fn options(&self) -> Value {
        json!({"mode":self.mode(),"style":self.style})
    }
    fn set_option(&mut self, key: &str, value: &Value) {
        if key == "mode"
            && let Some(v) = value.as_str().filter(|v| ["linear", "angular", "radius", "diameter"].contains(v))
        {
            self.mode = v.into();
        }
        if key == "style"
            && let Ok(o) = serde_json::from_value::<DimensionOptions>(value.clone())
            && o.validate().is_ok()
        {
            self.style = o;
        }
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        self.at = ev.pos;
        if matches!(ev.kind, PointerKind::Move | PointerKind::Drag) {
            if self.pending.is_some() {
                return self.preview(cx, ev.pos);
            }
            self.hover = pick(cx, ev.pos);
            return vec![];
        }
        if ev.kind != PointerKind::Down {
            return vec![];
        }
        if self.pending.is_some() {
            let mut out = self.preview(cx, ev.pos);
            if self.began {
                out.push(Action::Commit);
            }
            self.reset();
            return out;
        }
        let hit = pick(cx, ev.pos);
        let mut candidate = None;
        match self.mode() {
            "linear" if self.points.is_empty() && hit.is_some() && !ev.mods.alt => {
                candidate = hit.map(|edge| self.recipe(Point::ZERO, Point::new(1.0, 0.0), Point::ZERO, Some(DimensionSource::Segment { edge })));
            }
            "angular" if hit.is_some() && self.points.is_empty() && !ev.mods.alt => {
                if let Some(second) = hit {
                    let straight = |edge: &SegmentRef| {
                        cx.doc
                            .node(edge.node)
                            .and_then(Node::path_data)
                            .and_then(|p| p.subpaths.get(edge.subpath))
                            .is_some_and(|sp| edge.segment < sp.segment_count() && sp.segment_is_line(edge.segment))
                    };
                    if !straight(&second) {
                        return vec![];
                    }
                    if let Some(first) = self.first_edge.clone() {
                        candidate = Some(self.recipe(Point::ZERO, Point::new(1.0, 0.0), Point::ZERO, Some(DimensionSource::Angle { first, second })));
                    } else {
                        self.first_edge = Some(second);
                    }
                }
            }
            "radius" | "diameter" => {
                if let Some(edge) = hit
                    && let Some((center, _)) = vectorcraft_doc::dimension::circle(cx.doc, edge.node)
                {
                    candidate = Some(self.recipe(center, ev.pos, Point::ZERO, Some(DimensionSource::Circle { node: edge.node })));
                }
            }
            _ => {
                let (mut p, _) = crate::guides::snap_pick(cx, ev.pos);
                let reference = hit.as_ref().and_then(|edge| {
                    let sp = cx.doc.node(edge.node)?.path_data()?.subpaths.get(edge.subpath)?;
                    let next = if edge.segment + 1 < sp.anchors.len() { edge.segment + 1 } else { 0 };
                    [edge.segment, next]
                        .into_iter()
                        .filter_map(|i| Some((i, sp.anchors.get(i)?.p)))
                        .filter(|(_, point)| point.distance(ev.pos) <= cx.tol(8.0))
                        .min_by(|(_, a), (_, b)| a.distance(ev.pos).total_cmp(&b.distance(ev.pos)))
                        .map(|(anchor, point)| {
                            p = point;
                            vectorcraft_doc::dimension::PointRef { node: edge.node, subpath: edge.subpath, anchor }
                        })
                });
                if ev.mods.shift
                    && let Some(a) = self.points.last()
                {
                    p = *a + vectorcraft_geom::constrain_angle_from(p - *a, 45.0, cx.constrain_angle);
                }
                self.points.push(p);
                self.point_refs.push(reference.filter(|r| r.position(cx.doc) == Some(p)));
                match self.points.as_slice() {
                    [a, b] if self.mode() == "linear" => candidate = Some(self.recipe(*a, *b, Point::ZERO, self.point_source(false))),
                    [a, v, b] if self.mode() == "angular" => candidate = Some(self.recipe(*a, *b, *v, self.point_source(true))),
                    _ => {}
                }
            }
        }
        if let Some(mut d) = candidate {
            if d.resolve(cx.doc).is_ok() {
                self.pending = Some(d);
                self.points.clear();
                self.point_refs.clear();
                self.first_edge = None;
            } else if self.points.len() >= 2 {
                self.points.clear();
                self.point_refs.clear();
            }
        }
        vec![]
    }
    fn key(&mut self, _: &ToolContext, key: ToolKey, _: Mods) -> Vec<Action> {
        if key == ToolKey::Escape {
            let cancel = self.began;
            self.reset();
            return if cancel { vec![Action::Cancel] } else { vec![] };
        }
        vec![]
    }
    fn deactivate(&mut self, cx: &ToolContext) -> Vec<Action> {
        self.key(cx, ToolKey::Escape, Mods::default())
    }
    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let color = [60, 150, 255];
        let mut out = vec![];
        if self.pending.is_none()
            && let Some(edge) = &self.hover
            && let Some(sp) = cx.doc.node(edge.node).and_then(Node::path_data).and_then(|p| p.subpaths.get(edge.subpath))
            && edge.segment < sp.segment_count()
        {
            let c = sp.segment(edge.segment);
            let mut path = BezPath::new();
            path.move_to(c.p0);
            path.curve_to(c.p1, c.p2, c.p3);
            out.push(Overlay::Path { path, color, width: 3.0, dashed: false });
            for p in [c.p0, c.p3] {
                out.push(Overlay::Anchor { p, color, filled: true, size: 6.0 });
            }
            if let Some((_, _, length)) = edge.endpoints(cx.doc)
                && let Ok(text) = self.style.label(length, false)
            {
                out.push(Overlay::Measure { p: self.at + Vec2::new(12.0, 12.0), text });
            }
        }
        for p in &self.points {
            out.push(Overlay::Anchor { p: *p, color, filled: true, size: 6.0 });
        }
        if let Some(p) = self.points.last() {
            out.push(Overlay::Line { a: *p, b: self.at, color, dashed: true });
        }
        out
    }
}
