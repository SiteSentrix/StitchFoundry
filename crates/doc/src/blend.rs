//! Live blends: interpolating key objects and laying the steps out along the spine.
//!
//! Key objects are matched anchor by anchor ([`PathPair`]): subpaths paired in order (a missing
//! one grows out of a point) and resampled to equal anchor counts, closed ones turned to the same
//! winding; the Blend tool's clicks on anchor points choose where each path starts. Paints,
//! strokes and opacity interpolate. A blend prepares each pair of keys once ([`Lerp`]) and only
//! interpolates per step.

use std::sync::Arc;

use vectorcraft_color::{Color, Gradient, GradientPaint, GradientStop, Paint};
use vectorcraft_geom::{Affine, Anchor, FillRule, PathData, Point, SubPath};

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

/// A spine flattened to a polyline with cumulative lengths.
pub struct Spine {
    pts: Vec<Point>,
    cum: Vec<f64>,
}

impl Spine {
    pub fn new(path: &PathData) -> Option<Self> {
        let sp = path.subpaths.first()?;
        let pts = crate::live::flatten_subpath(sp, 0.1);
        if pts.len() < 2 {
            return None;
        }
        Some(Self { cum: crate::live::cumulative_lengths(&pts), pts })
    }
    pub fn length(&self) -> f64 {
        *self.cum.last().unwrap_or(&0.0)
    }
    /// Point and tangent angle (radians) at arc-length fraction `f` (0..1).
    pub fn at(&self, f: f64) -> (Point, f64) {
        let total = self.length();
        let s = f.clamp(0.0, 1.0) * total;
        let i = match self.cum.binary_search_by(|c| c.total_cmp(&s)) {
            Ok(i) => i.min(self.pts.len() - 2),
            Err(i) => i.saturating_sub(1).min(self.pts.len() - 2),
        };
        let seg = (self.cum[i + 1] - self.cum[i]).max(1e-12);
        let u = ((s - self.cum[i]) / seg).clamp(0.0, 1.0);
        let d = self.pts[i + 1] - self.pts[i];
        (self.pts[i].lerp(self.pts[i + 1], u), d.y.atan2(d.x))
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
    let spine = spec.spine.as_ref().and_then(Spine::new);
    let place = |mut n: Node, f: f64| -> Node {
        if let Some(sp) = &spine {
            let (p, ang) = sp.at(f);
            let c = center_of(&n);
            let rot = if spec.orientation == BlendOrientation::AlignToPath { Affine::rotate(ang) } else { Affine::IDENTITY };
            n.transform(Affine::translate(p.to_vec2()) * rot * Affine::translate(-c.to_vec2()), false);
        }
        n
    };
    let mut out = Vec::new();
    for (i, a) in keys.iter().enumerate() {
        out.push(place((**a).clone(), i as f64 / (k - 1) as f64));
        let Some(b) = keys.get(i + 1) else { break };
        let len = match &spine {
            Some(sp) => sp.length() / (k - 1) as f64,
            None => center_of(a).distance(center_of(b)),
        };
        let n = blend_step_count(a, b, spec.spacing, len);
        let lerp = Lerp::new(a, b, (spec.start(i), spec.start(i + 1)));
        for j in 1..=n {
            let t = j as f64 / (n + 1) as f64;
            out.push(place(lerp.at(t), (i as f64 + t) / (k - 1) as f64));
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
}
