//! Live blends: interpolating key objects and laying the steps out along the spine.
//!
//! Key objects are matched anchor by anchor ([`PathPair`]): subpaths paired in order (a missing
//! one grows out of a point) and resampled to equal anchor counts, closed ones turned to the same
//! winding; the Blend tool's clicks on anchor points choose where each path starts. Paints,
//! strokes and opacity interpolate. A blend prepares each pair of keys once ([`Lerp`]) and only
//! interpolates per step.
//!
//! The spine ([`blend_spine`]) is the straight lines between the key centres until it is edited
//! or replaced; then a path whose anchors the keys sit on (`key_anchors`). Steps follow it by arc
//! length; Align to Path turns them to its direction.

use std::sync::Arc;

use vectorcraft_color::{Color, Gradient, GradientPaint, GradientStop, Paint};
use vectorcraft_geom::kurbo::ParamCurveArclen;
use vectorcraft_geom::{Affine, Anchor, BezPath, FillRule, PathData, Point, SubPath};

use crate::appearance::{Appearance, AppearanceItem};
use crate::live::{BlendOrientation, BlendSpacing, BlendSpec};
use crate::node::{Node, NodeId, NodeKind};

// =====================================================================================
// Colour / paint interpolation
// =====================================================================================

/// Interpolate two colours in their shared model (display RGB when the models differ).
pub fn lerp_color(a: &Color, b: &Color, t: f32) -> Color {
    let l = |x: f32, y: f32| x + (y - x) * t;
    match (*a, *b) {
        (Color::Cmyk { c, m, y, k }, Color::Cmyk { c: c2, m: m2, y: y2, k: k2 }) => Color::cmyk(l(c, c2), l(m, m2), l(y, y2), l(k, k2)),
        (Color::Gray { k }, Color::Gray { k: k2 }) => Color::gray(l(k, k2)),
        (Color::Lab { l: l1, a, b }, Color::Lab { l: l2, a: a2, b: b2 }) => Color::lab(l(l1, l2), l(a, a2), l(b, b2)),
        _ => a.lerp(b, t),
    }
}

fn lerp_f32(x: f32, y: f32, t: f32) -> f32 {
    x + (y - x) * t
}

fn lerp_f64(x: f64, y: f64, t: f64) -> f64 {
    x + (y - x) * t
}

fn solid_gradient(c: Color, like: &Gradient) -> Gradient {
    let mut g = like.clone();
    for s in &mut g.stops {
        s.color = c;
        s.opacity = 1.0;
    }
    g
}

fn lerp_gradient(a: &GradientPaint, b: &GradientPaint, t: f32) -> Option<GradientPaint> {
    if a.gradient.kind != b.gradient.kind || a.gradient.stops.len() != b.gradient.stops.len() {
        return None;
    }
    let (ga, gb) = (&a.gradient, &b.gradient);
    let mut out = if t < 0.5 { a.clone() } else { b.clone() };
    out.gradient.stops = ga
        .stops
        .iter()
        .zip(&gb.stops)
        .map(|(sa, sb)| GradientStop {
            opacity: lerp_f32(sa.opacity, sb.opacity, t),
            midpoint: lerp_f32(sa.midpoint, sb.midpoint, t),
            ..GradientStop::new(lerp_f32(sa.offset, sb.offset, t), lerp_color(&sa.color, &sb.color, t))
        })
        .collect();
    out.angle = a.angle + (b.angle - a.angle) * t as f64;
    out.swatch = None;
    if let (Some(ga), Some(gb)) = (a.geom, b.geom) {
        let mut g = ga;
        g.start = ga.start.lerp(gb.start, t as f64);
        g.end = ga.end.lerp(gb.end, t as f64);
        g.aspect = ga.aspect + (gb.aspect - ga.aspect) * t as f64;
        g.focal = (ga.focal.is_some() || gb.focal.is_some()).then(|| ga.focal_point().lerp(gb.focal_point(), t as f64));
        out.geom = Some(g);
    } else {
        out.geom = None;
    }
    Some(out)
}

/// Interpolate paints: solid↔solid, compatible gradients, solid↔gradient; otherwise switch halfway.
pub fn lerp_paint(a: &Paint, b: &Paint, t: f32) -> Paint {
    match (a, b) {
        (Paint::Solid { color: x, .. }, Paint::Solid { color: y, .. }) => Paint::solid(lerp_color(x, y, t)),
        (Paint::Gradient(ga), Paint::Gradient(gb)) => match lerp_gradient(ga, gb, t) {
            Some(g) => Paint::Gradient(Box::new(g)),
            None => (if t < 0.5 { a } else { b }).clone(),
        },
        (Paint::Solid { color, .. }, Paint::Gradient(g)) => {
            let ga = GradientPaint { gradient: solid_gradient(*color, &g.gradient), ..(**g).clone() };
            Paint::Gradient(Box::new(lerp_gradient(&ga, g, t).unwrap_or_else(|| (**g).clone())))
        }
        (Paint::Gradient(g), Paint::Solid { color, .. }) => {
            let gb = GradientPaint { gradient: solid_gradient(*color, &g.gradient), ..(**g).clone() };
            Paint::Gradient(Box::new(lerp_gradient(g, &gb, t).unwrap_or_else(|| (**g).clone())))
        }
        _ => (if t < 0.5 { a } else { b }).clone(),
    }
}

/// Interpolate appearance stacks: item by item when their structure matches, else the top fill
/// and stroke only (on the structure of the nearer key).
pub fn lerp_appearance(a: &Appearance, b: &Appearance, t: f64) -> Appearance {
    let tf = t as f32;
    let mut out = if t < 0.5 { a.clone() } else { b.clone() };
    let same = a.items.len() == b.items.len()
        && a.items.iter().zip(&b.items).all(|(x, y)| {
            matches!((x, y), (AppearanceItem::Fill(_), AppearanceItem::Fill(_)) | (AppearanceItem::Stroke(_), AppearanceItem::Stroke(_)))
        });
    if same {
        for ((it, x), y) in out.items.iter_mut().zip(&a.items).zip(&b.items) {
            match (it, x, y) {
                (AppearanceItem::Fill(o), AppearanceItem::Fill(x), AppearanceItem::Fill(y)) => {
                    o.paint = lerp_paint(&x.paint, &y.paint, tf);
                    o.opacity = lerp_f32(x.opacity, y.opacity, tf);
                }
                (AppearanceItem::Stroke(o), AppearanceItem::Stroke(x), AppearanceItem::Stroke(y)) => {
                    o.paint = lerp_paint(&x.paint, &y.paint, tf);
                    o.width = lerp_f64(x.width, y.width, t);
                    o.opacity = lerp_f32(x.opacity, y.opacity, tf);
                }
                _ => {}
            }
        }
        return out;
    }
    let (fa, fb) = (a.fill_paint(), b.fill_paint());
    if !(fa.is_none() && fb.is_none()) {
        out.set_fill(lerp_paint(&fa, &fb, tf));
    }
    let (sa, sb) = (a.stroke_paint(), b.stroke_paint());
    if !(sa.is_none() && sb.is_none()) {
        out.set_stroke(lerp_paint(&sa, &sb, tf));
        let w = a.stroke_width() + (b.stroke_width() - a.stroke_width()) * t;
        if let Some(s) = out.stroke_mut() {
            s.width = w;
        }
    }
    out
}

// =====================================================================================
// Path interpolation
// =====================================================================================

/// Grow a subpath to `n` anchors by splitting its longest segments.
fn grow(sp: &mut SubPath, n: usize) {
    let mut guard = 0;
    while sp.anchors.len() < n && guard < 20_000 {
        guard += 1;
        if sp.segment_count() == 0 {
            match sp.anchors.last().copied() {
                Some(a) => sp.anchors.push(Anchor::corner(a.p)),
                None => return,
            }
            continue;
        }
        let seg = (0..sp.segment_count())
            .max_by(|i, j| {
                let (ci, cj) = (sp.segment(*i), sp.segment(*j));
                (ci.p3 - ci.p0).hypot().total_cmp(&(cj.p3 - cj.p0).hypot())
            })
            .unwrap_or(0);
        sp.insert_anchor(seg, 0.5);
    }
}

fn lerp_anchor(x: &Anchor, y: &Anchor, t: f64) -> Anchor {
    Anchor { p: x.p.lerp(y.p, t), h_in: x.h_in.lerp(y.h_in, t), h_out: x.h_out.lerp(y.h_out, t), kind: if t < 0.5 { x.kind } else { y.kind } }
}

/// Start `sp` at anchor `i`: a closed subpath turns round to it, an open one clicked at its last
/// anchor runs the other way (a click on any other anchor of an open path is ignored).
fn start_at(sp: &mut SubPath, i: usize) {
    let n = sp.anchors.len();
    if i >= n {
        return;
    }
    if sp.closed {
        sp.anchors.rotate_left(i);
    } else if i + 1 == n && n > 1 {
        sp.reverse();
    }
}

/// Reverse a closed subpath keeping its start anchor first.
fn reverse_closed(sp: &mut SubPath) {
    sp.reverse();
    sp.anchors.rotate_right(1);
}

/// Two paths prepared for interpolation: subpaths paired in order, with equal anchor counts, the
/// same winding and the clicked start points.
#[derive(Clone, Debug)]
pub struct PathPair {
    a: Vec<SubPath>,
    b: Vec<SubPath>,
}

impl PathPair {
    /// `starts`: the anchor of each path's first subpath its blend starts from (the Blend tool's
    /// clicks).
    pub fn new(a: &PathData, b: &PathData, starts: (Option<usize>, Option<usize>)) -> Self {
        let n = a.subpaths.len().max(b.subpaths.len());
        let ca = a.bounds().map(|r| r.center()).unwrap_or_default();
        let cb = b.bounds().map(|r| r.center()).unwrap_or_default();
        let degenerate = |like: &SubPath, c: Point| SubPath::new(vec![Anchor::corner(c); like.anchors.len().max(1)], like.closed);
        let (mut va, mut vb) = (Vec::with_capacity(n), Vec::with_capacity(n));
        for i in 0..n {
            let (mut sa, mut sb) = match (a.subpaths.get(i), b.subpaths.get(i)) {
                (Some(x), Some(y)) => (x.clone(), y.clone()),
                (Some(x), None) => (x.clone(), degenerate(x, cb)),
                (None, Some(y)) => (degenerate(y, ca), y.clone()),
                (None, None) => continue,
            };
            if i == 0 {
                if let Some(s) = starts.0 {
                    start_at(&mut sa, s);
                }
                if let Some(s) = starts.1 {
                    start_at(&mut sb, s);
                }
            }
            let closed = sa.closed && sb.closed && sa.anchors.len() > 2 && sb.anchors.len() > 2;
            if closed && (sa.area() * sb.area()) < 0.0 {
                reverse_closed(&mut sb);
            }
            let m = sa.anchors.len().max(sb.anchors.len());
            grow(&mut sa, m);
            grow(&mut sb, m);
            va.push(sa);
            vb.push(sb);
        }
        Self { a: va, b: vb }
    }

    /// The path at `t` (0 = `a`, 1 = `b`).
    pub fn at(&self, t: f64) -> PathData {
        let subs = self
            .a
            .iter()
            .zip(&self.b)
            .map(|(sa, sb)| {
                let anchors = sa.anchors.iter().zip(&sb.anchors).map(|(x, y)| lerp_anchor(x, y, t)).collect();
                SubPath::new(anchors, if t < 0.5 { sa.closed } else { sb.closed })
            })
            .collect();
        PathData::new(subs)
    }
}

/// Interpolate two paths anchor-by-anchor (see [`PathPair`]).
pub fn lerp_path(a: &PathData, b: &PathData, t: f64) -> PathData {
    PathPair::new(a, b, (None, None)).at(t)
}

/// Path data of a path or compound path node (compound children concatenated).
fn node_path(n: &Node) -> Option<(PathData, FillRule)> {
    match &n.kind {
        NodeKind::Path { path, rule, .. } => Some((path.clone(), *rule)),
        NodeKind::Compound { children, rule } => {
            let subs = children.iter().filter_map(|c| c.path_data()).flat_map(|p| p.subpaths.iter().cloned()).collect();
            Some((PathData::new(subs), *rule))
        }
        _ => None,
    }
}

// =====================================================================================
// Object interpolation
// =====================================================================================

/// How two key objects interpolate.
#[derive(Clone, Debug)]
enum Shape {
    /// Groups with as many children: children paired in stacking order.
    Group { children: Vec<Lerp>, clip: bool },
    /// Gradient meshes with the same grid.
    Mesh,
    /// Paths and compound paths.
    Path { pair: PathPair, rules: (FillRule, FillRule) },
    /// Anything else: a copy of the nearer key moved and scaled into the interpolated box.
    Other,
}

/// Two key objects prepared for interpolation, so each step of a blend only interpolates.
#[derive(Clone, Debug)]
pub struct Lerp {
    a: Node,
    b: Node,
    shape: Shape,
}

fn center_of(n: &Node) -> Point {
    n.geometric_bounds().map(|b| b.center()).unwrap_or_default()
}

impl Lerp {
    /// Prepare `a` → `b`; `starts` as in [`PathPair::new`].
    pub fn new(a: &Node, b: &Node, starts: (Option<usize>, Option<usize>)) -> Self {
        let shape = match (&a.kind, &b.kind) {
            (NodeKind::Group { children: ca, clip: k1 }, NodeKind::Group { children: cb, clip: k2 }) if ca.len() == cb.len() && k1 == k2 => {
                Shape::Group { children: ca.iter().zip(cb).map(|(x, y)| Lerp::new(x, y, (None, None))).collect(), clip: *k1 }
            }
            (NodeKind::Mesh(ma), NodeKind::Mesh(mb)) if ma.rows == mb.rows && ma.cols == mb.cols && ma.points.len() == mb.points.len() => Shape::Mesh,
            _ => match (node_path(a), node_path(b)) {
                (Some((pa, ra)), Some((pb, rb))) => Shape::Path { pair: PathPair::new(&pa, &pb, starts), rules: (ra, rb) },
                _ => Shape::Other,
            },
        };
        Self { a: a.clone(), b: b.clone(), shape }
    }

    /// The object at `t` (0 = `a`, 1 = `b`), with id 0.
    pub fn at(&self, t: f64) -> Node {
        let (a, b) = (&self.a, &self.b);
        let base = if t < 0.5 { a } else { b };
        let mut n = match &self.shape {
            Shape::Group { children, clip } => {
                Node::new(NodeId(0), NodeKind::Group { children: children.iter().map(|c| Arc::new(c.at(t))).collect(), clip: *clip })
            }
            Shape::Mesh => {
                let mut n = base.clone();
                if let (NodeKind::Mesh(m), NodeKind::Mesh(ma), NodeKind::Mesh(mb)) = (&mut n.kind, &a.kind, &b.kind) {
                    for ((p, x), y) in m.points.iter_mut().zip(&ma.points).zip(&mb.points) {
                        p.p = x.p.lerp(y.p, t);
                        p.color = lerp_color(&x.color, &y.color, t as f32);
                        p.opacity = lerp_f32(x.opacity, y.opacity, t as f32);
                        for h in 0..4 {
                            p.handles[h] = x.handles[h].lerp(y.handles[h], t);
                        }
                    }
                }
                n
            }
            Shape::Path { pair, rules } => {
                let mut n = Node::path(NodeId(0), pair.at(t), Appearance::default());
                if let NodeKind::Path { rule, .. } = &mut n.kind {
                    *rule = if t < 0.5 { rules.0 } else { rules.1 };
                }
                n
            }
            Shape::Other => {
                // Different structure: move/scale a copy of the nearer key into the interpolated box.
                let mut n = base.clone();
                if let (Some(ba), Some(bb), Some(bn)) = (a.geometric_bounds(), b.geometric_bounds(), base.geometric_bounds()) {
                    let w = ba.width() + (bb.width() - ba.width()) * t;
                    let h = ba.height() + (bb.height() - ba.height()) * t;
                    let c = ba.center().lerp(bb.center(), t);
                    let sx = if bn.width() > 1e-9 { w / bn.width() } else { 1.0 };
                    let sy = if bn.height() > 1e-9 { h / bn.height() } else { 1.0 };
                    n.transform(
                        Affine::translate(c.to_vec2()) * Affine::scale_non_uniform(sx, sy) * Affine::translate(-bn.center().to_vec2()),
                        false,
                    );
                }
                if let NodeKind::Path { live, .. } = &mut n.kind {
                    *live = None;
                }
                n
            }
        };
        n.id = NodeId(0);
        n.appearance = lerp_appearance(&a.appearance, &b.appearance, t);
        n.opacity = lerp_f32(a.opacity, b.opacity, t as f32);
        n.blend = base.blend;
        n.visible = true;
        n
    }
}

/// Interpolate two objects at `t` (0 = `a`, 1 = `b`). A blend prepares each pair once with
/// [`Lerp`] instead.
pub fn lerp_node(a: &Node, b: &Node, t: f64) -> Node {
    Lerp::new(a, b, (None, None)).at(t)
}

// =====================================================================================
// Step counts
// =====================================================================================

/// Largest channel difference (0..1) between the solid colours of two objects' top fill and stroke.
fn color_distance(a: &Node, b: &Node) -> f32 {
    let mut d = 0.0f32;
    let mut cmp = |x: &Paint, y: &Paint| {
        let cols = |p: &Paint| -> Vec<Color> {
            match p {
                Paint::Solid { color, .. } => vec![*color],
                Paint::Gradient(g) => g.gradient.stops.iter().map(|s| s.color).collect(),
                _ => vec![],
            }
        };
        let (cx, cy) = (cols(x), cols(y));
        for (i, c) in cx.iter().enumerate() {
            if let Some(o) = cy.get(i).or(cy.first()) {
                let (p, q) = (c.to_rgb(), o.to_rgb());
                for k in 0..3 {
                    d = d.max((p[k] - q[k]).abs());
                }
            }
        }
    };
    cmp(&a.appearance.fill_paint(), &b.appearance.fill_paint());
    cmp(&a.appearance.stroke_paint(), &b.appearance.stroke_paint());
    d
}

/// Number of intermediate steps between two keys `len` apart (along the spine).
pub fn blend_step_count(a: &Node, b: &Node, spacing: BlendSpacing, len: f64) -> usize {
    match spacing {
        BlendSpacing::Steps(n) => n.clamp(1, 1000) as usize,
        BlendSpacing::Distance(d) => {
            let d = d.max(0.01);
            ((len / d).round() as i64 - 1).clamp(0, 1000) as usize
        }
        BlendSpacing::SmoothColor => {
            let cd = color_distance(a, b);
            if cd > 1.0 / 255.0 {
                ((cd * 255.0 / 2.0).ceil() as usize).clamp(1, 256)
            } else {
                // Same colours: base the count on the distance between the objects.
                let (ba, bb) = (a.geometric_bounds(), b.geometric_bounds());
                let dist = match (ba, bb) {
                    (Some(x), Some(y)) => (x.x0 - y.x0).abs().max((x.x1 - y.x1).abs()).max((x.y0 - y.y0).abs()).max((x.y1 - y.y1).abs()),
                    _ => len,
                };
                ((dist / 2.0).ceil() as usize).clamp(1, 256)
            }
        }
    }
}

// =====================================================================================
// Spine
// =====================================================================================

/// A spine (the first subpath of a path) flattened to a polyline, with cumulative lengths and the
/// arc length at each of its anchors.
pub struct Spine {
    pts: Vec<Point>,
    cum: Vec<f64>,
    anchors: Vec<f64>,
    closed: bool,
}

impl Spine {
    pub fn new(path: &PathData) -> Option<Self> {
        let sp = path.subpaths.first()?;
        let mut pts: Vec<Point> = Vec::new();
        let mut cum: Vec<f64> = Vec::new();
        let mut anchors = Vec::with_capacity(sp.anchors.len());
        for i in 0..sp.segment_count() {
            anchors.push(cum.last().copied().unwrap_or(0.0));
            let c = sp.segment(i);
            let mut bp = BezPath::new();
            bp.move_to(c.p0);
            if sp.segment_is_line(i) {
                bp.line_to(c.p3);
            } else {
                bp.curve_to(c.p1, c.p2, c.p3);
            }
            vectorcraft_geom::kurbo::flatten(bp.iter(), 0.1, |el| {
                if let vectorcraft_geom::PathEl::MoveTo(p) | vectorcraft_geom::PathEl::LineTo(p) = el
                    && p.x.is_finite()
                    && p.y.is_finite()
                    && pts.last().is_none_or(|q| q.distance(p) > 1e-9)
                {
                    let s = match (pts.last(), cum.last()) {
                        (Some(q), Some(l)) => l + q.distance(p),
                        _ => 0.0,
                    };
                    pts.push(p);
                    cum.push(s);
                }
            });
        }
        if !sp.closed {
            anchors.push(cum.last().copied().unwrap_or(0.0));
        }
        if pts.len() < 2 {
            return None;
        }
        Some(Self { pts, cum, anchors, closed: sp.closed })
    }
    pub fn length(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }
    /// Arc length at anchor `i`.
    pub fn anchor_length(&self, i: usize) -> Option<f64> {
        self.anchors.get(i).copied()
    }
    /// Point and tangent angle (radians) at arc-length fraction `f` (0..1).
    pub fn at(&self, f: f64) -> (Point, f64) {
        self.at_length(f.clamp(0.0, 1.0) * self.length())
    }
    /// Point and tangent angle (radians) at arc length `s` (clamped to the spine).
    pub fn at_length(&self, s: f64) -> (Point, f64) {
        let last = self.pts.len().saturating_sub(2);
        let s = if s.is_finite() { s.clamp(0.0, self.length()) } else { 0.0 };
        let i = match self.cum.binary_search_by(|c| c.total_cmp(&s)) {
            Ok(i) => i.min(last),
            Err(i) => i.saturating_sub(1).min(last),
        };
        let (Some(p0), Some(p1), Some(c0), Some(c1)) = (self.pts.get(i), self.pts.get(i + 1), self.cum.get(i), self.cum.get(i + 1)) else {
            return (self.pts.first().copied().unwrap_or_default(), 0.0);
        };
        let u = ((s - c0) / (c1 - c0).max(1e-12)).clamp(0.0, 1.0);
        let d = *p1 - *p0;
        (p0.lerp(*p1, u), d.y.atan2(d.x))
    }
}

/// The anchor of the first subpath each key sits on, when they are all valid for `n` anchors.
fn pinned(spec: &BlendSpec, keys: usize, n: usize) -> Option<Vec<usize>> {
    (spec.key_anchors.len() == keys && keys >= 2 && spec.key_anchors.iter().all(|a| (*a as usize) < n))
        .then(|| spec.key_anchors.iter().map(|a| *a as usize).collect())
}

/// The spine of a blend of `keys` and the anchor of it each key sits on:
/// - without a stored spine: the straight lines between the key centres;
/// - a pinned spine (`key_anchors`): the stored spine's first subpath with each key's anchor (and
///   its handles) moved onto the key's centre, so moving a key moves its end of the spine;
/// - a spine from Replace Spine: its first subpath, `None` for the anchors (the keys spread
///   evenly along it by arc length).
pub fn blend_spine(keys: &[Arc<Node>], spec: &BlendSpec) -> Option<(PathData, Option<Vec<usize>>)> {
    if keys.len() < 2 {
        return None;
    }
    let Some(stored) = &spec.spine else {
        let centers: Vec<Point> = keys.iter().map(|k| center_of(k)).collect();
        return Some((PathData::single(SubPath::polyline(&centers, false)), Some((0..keys.len()).collect())));
    };
    let mut first = stored.subpaths.first()?.clone();
    let Some(ka) = pinned(spec, keys.len(), first.anchors.len()) else { return Some((PathData::single(first), None)) };
    for (k, &ai) in keys.iter().zip(&ka) {
        let c = center_of(k);
        if let Some(a) = first.anchors.get_mut(ai) {
            let d = c - a.p;
            a.translate(d);
        }
    }
    Some((PathData::single(first), Some(ka)))
}

/// [`blend_spine`] with every key on an anchor: a spine from Replace Spine gets anchors where its
/// keys sit (by arc length), so editing it keeps them in place.
pub fn pin_spine(keys: &[Arc<Node>], spec: &BlendSpec) -> Option<(PathData, Vec<usize>)> {
    let (mut path, anchors) = blend_spine(keys, spec)?;
    if let Some(a) = anchors {
        return Some((path, a));
    }
    let k = keys.len();
    let total = Spine::new(&path)?.length();
    let closed = path.subpaths.first().is_some_and(|s| s.closed);
    let mut out = vec![0usize];
    for i in 1..k.saturating_sub(1) {
        let s = total * i as f64 / (k - 1) as f64;
        out.push(anchor_at_length(&mut path, s)?);
    }
    let last = path.subpaths.first().map_or(0, |sp| if closed { 0 } else { sp.anchors.len().saturating_sub(1) });
    out.push(last);
    Some((path, out))
}

/// The anchor of the first subpath at arc length `s`: an existing one within a hundredth of a
/// point, else a new one splitting the segment there.
fn anchor_at_length(path: &mut PathData, s: f64) -> Option<usize> {
    let spine = Spine::new(path)?;
    let sp = path.subpaths.first_mut()?;
    let n = sp.segment_count();
    let seg = (0..n).rev().find(|i| spine.anchor_length(*i).is_some_and(|l| l <= s)).unwrap_or(0);
    let from = spine.anchor_length(seg).unwrap_or(0.0);
    let to = spine.anchor_length(seg + 1).unwrap_or(spine.length());
    if s - from < 0.01 {
        return Some(seg);
    }
    if to - s < 0.01 {
        return Some((seg + 1) % sp.anchors.len().max(1));
    }
    let c = sp.segment(seg);
    // The flattened length and the curve's own differ slightly: scale into the curve's.
    let arc = c.arclen(1e-4).max(1e-12);
    let t = c.inv_arclen((s - from) / (to - from).max(1e-12) * arc, 1e-4).clamp(1e-4, 1.0 - 1e-4);
    Some(sp.insert_anchor(seg, t))
}

/// Where a blend's keys and steps go along its spine (see [`blend_spine`]).
struct Rail {
    spine: Spine,
    /// Arc length of each key.
    stops: Vec<f64>,
    centers: Vec<Point>,
    rotate: bool,
}

impl Rail {
    /// `None` when nothing moves: no stored spine and the steps keep the page's orientation.
    fn new(keys: &[Arc<Node>], spec: &BlendSpec) -> Option<Self> {
        let rotate = spec.orientation == BlendOrientation::AlignToPath;
        if spec.spine.is_none() && !rotate {
            return None;
        }
        let (path, anchors) = blend_spine(keys, spec)?;
        let spine = Spine::new(&path)?;
        let total = spine.length();
        let k = keys.len();
        let stops = match anchors {
            Some(ka) => {
                let mut v: Vec<f64> = Vec::with_capacity(k);
                for a in ka {
                    let mut s = spine.anchor_length(a).unwrap_or(0.0);
                    // On a closed spine the key back at the start sits at its far end.
                    if spine.closed
                        && let Some(prev) = v.last()
                        && s <= *prev + 1e-9
                    {
                        s += total;
                    }
                    v.push(s);
                }
                v
            }
            None => (0..k).map(|i| total * i as f64 / (k - 1).max(1) as f64).collect(),
        };
        Some(Self { spine, stops, centers: keys.iter().map(|k| center_of(k)).collect(), rotate })
    }

    /// Arc length between keys `i` and `i + 1`.
    fn len(&self, i: usize) -> f64 {
        match (self.stops.get(i), self.stops.get(i + 1)) {
            (Some(a), Some(b)) => (b - a).abs(),
            _ => 0.0,
        }
    }

    /// Move `n`, the object at `t` between keys `i` and `i + 1` (a key at `t` = 0), onto the
    /// spine: by how far the spine there is from the straight line between the key centres (so
    /// steps keep their own interpolated position on a straight spine), turned to the spine's
    /// direction with Align to Path.
    fn place(&self, n: &mut Node, i: usize, t: f64) {
        let (Some(&s0), Some(&c0)) = (self.stops.get(i), self.centers.get(i)) else { return };
        let (s, base) = match (self.stops.get(i + 1), self.centers.get(i + 1)) {
            (Some(&s1), Some(&c1)) if t > 0.0 => (lerp_f64(s0, s1, t), c0.lerp(c1, t)),
            _ => (s0, c0),
        };
        let (p, ang) = self.spine.at_length(if self.spine.closed { s.rem_euclid(self.spine.length().max(1e-12)) } else { s });
        let off = p - base;
        let mut m = if off.hypot() > 1e-9 { Affine::translate(off) } else { Affine::IDENTITY };
        if self.rotate && ang.abs() > 1e-12 {
            let c = (center_of(n) + off).to_vec2();
            m = Affine::translate(c) * Affine::rotate(ang) * Affine::translate(-c) * m;
        }
        if m != Affine::IDENTITY {
            n.transform(m, false);
        }
    }
}

// =====================================================================================
// Blend evaluation
// =====================================================================================

/// Evaluate a blend: the keys and generated steps in paint order.
pub fn blend_expand(keys: &[Arc<Node>], spec: &BlendSpec) -> Vec<Node> {
    let k = keys.len();
    if k < 2 {
        return keys.iter().map(|n| (**n).clone()).collect();
    }
    let rail = Rail::new(keys, spec);
    let mut out = Vec::new();
    for (i, a) in keys.iter().enumerate() {
        let mut key = (**a).clone();
        if let Some(r) = &rail {
            r.place(&mut key, i, 0.0);
        }
        out.push(key);
        let Some(b) = keys.get(i + 1) else { break };
        let len = rail.as_ref().map_or_else(|| center_of(a).distance(center_of(b)), |r| r.len(i));
        let n = blend_step_count(a, b, spec.spacing, len);
        let lerp = Lerp::new(a, b, (spec.start(i), spec.start(i + 1)));
        for j in 1..=n {
            let t = j as f64 / (n + 1) as f64;
            let mut s = lerp.at(t);
            if let Some(r) = &rail {
                r.place(&mut s, i, t);
            }
            out.push(s);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::{Rect, shapes};

    #[test]
    fn clicked_start_points_pair_up() {
        let a = shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0));
        let mut b = shapes::rectangle(Rect::new(100.0, 0.0, 110.0, 10.0));
        b.subpaths[0].anchors.rotate_left(2);
        // Each anchor of `a` meets the opposite corner of `b`: the middle step shrinks to a point.
        assert!(PathPair::new(&a, &b, (None, None)).at(0.5).bounds().unwrap().height() < 1e-6);
        // Starting `b` at its anchor 2 (its first corner again) pairs like corners.
        let r = PathPair::new(&a, &b, (Some(0), Some(2))).at(0.5).bounds().unwrap();
        assert!((r.width() - 10.0).abs() < 1e-6 && (r.height() - 10.0).abs() < 1e-6, "{r:?}");
        // An open path clicked at its end runs the other way.
        let line = |x: f64| PathData::single(SubPath::polyline(&[Point::new(x, 0.0), Point::new(x + 10.0, 0.0)], false));
        let m = PathPair::new(&line(0.0), &line(100.0), (None, Some(1))).at(0.5);
        assert_eq!(m.subpaths[0].anchors[0].p, Point::new(55.0, 0.0));
    }

    fn key(x: f64) -> Arc<Node> {
        Arc::new(Node::path(NodeId(1), vectorcraft_geom::shapes::rectangle(Rect::new(x, 0.0, x + 10.0, 10.0)), Appearance::default()))
    }

    #[test]
    fn a_replaced_spine_gets_anchors_where_its_keys_sit() {
        let keys = vec![key(0.0), key(100.0), key(200.0)];
        let line = PathData::single(SubPath::polyline(&[Point::new(0.0, 50.0), Point::new(300.0, 50.0)], false));
        let spec = BlendSpec { spine: Some(line), ..Default::default() };
        let (path, anchors) = pin_spine(&keys, &spec).unwrap();
        assert_eq!(anchors, vec![0, 1, 2]);
        assert!((path.subpaths[0].anchors[1].p.x - 150.0).abs() < 1e-6, "the middle key sits halfway");
        // Pinned, each key's anchor follows it.
        let pinned = BlendSpec { spine: Some(path), key_anchors: vec![0, 1, 2], ..Default::default() };
        let (moved, _) = blend_spine(&[key(0.0), key(100.0), key(300.0)], &pinned).unwrap();
        assert_eq!(moved.subpaths[0].anchors[2].p, Point::new(305.0, 5.0));
    }

    #[test]
    fn steps_follow_a_bent_spine() {
        let keys = vec![key(0.0), key(100.0)];
        // A spine through the key centres bent up through (55, -95).
        let spine = PathData::single(SubPath::polyline(&[Point::new(5.0, 5.0), Point::new(55.0, -95.0), Point::new(105.0, 5.0)], false));
        let spec = BlendSpec { spacing: BlendSpacing::Steps(1), spine: Some(spine), key_anchors: vec![0, 2], ..Default::default() };
        let out = blend_expand(&keys, &spec);
        let mid = out[1].geometric_bounds().unwrap().center();
        assert!(mid.distance(Point::new(55.0, -95.0)) < 1e-6, "{mid:?}");
        assert_eq!(out[0].geometric_bounds().unwrap().center(), Point::new(5.0, 5.0), "keys stay put");
    }
}
