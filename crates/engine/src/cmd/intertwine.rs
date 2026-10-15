//! Object › Intertwine (#1763): objects that pass over and under each other.
//!
//! `object.intertwine.make` puts the selected objects in an Intertwine group
//! (`NodeKind::Intertwine`), `object.intertwine.release` gives them back as they were. In between,
//! `object.intertwine.arrange` picks overlap areas, by a click (`at`) or by an enclosed path drawn
//! around them (`area`), and changes which member is drawn on top there: each member keeps the
//! area where it is drawn over the others (`NodeExtras::intertwine`, see
//! `vectorcraft_doc::intertwine`).
//!
//! An overlap area is a face of the planar arrangement of what the members paint (their fills and
//! strokes, type outlined; other objects by their bounds): the part where two or more of them
//! overlap, split wherever a member's edge crosses it.
//!
//! Behaviour: <https://helpx.adobe.com/illustrator/using/intertwine-objects.html> (Make,
//! Release, clicking or drawing around an overlap, Bring to Front and Send to Back where three or
//! more objects overlap).

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_doc::intertwine::over_area;
use vectorcraft_doc::{AppearanceItem, Document, Node, NodeId, NodeKind};
use vectorcraft_geom::{FillRule, PathData, Point, SubPath, shapes};
use vectorcraft_pathops as po;
use vectorcraft_render::effects;

use super::edit::selected_roots;
use super::pathops::Beside;
use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "object.intertwine.make",
            "Make",
            ["Object", "Intertwine"],
            None,
            "{ids?: two or more objects (default: the selection)} put the objects in an Intertwine group, where overlap areas can be arranged with object.intertwine.arrange → {id}",
            has_selection,
            make
        ),
        cmd!(
            "object.intertwine.release",
            "Release",
            ["Object", "Intertwine"],
            None,
            "{} give the objects of the selected Intertwine groups back as they were made, dropping the arranged areas → {ids}",
            has_intertwine,
            release
        ),
        cmd!(
            "object.intertwine.arrange",
            "Arrange Intertwine Area",
            [],
            None,
            "{id?: Intertwine group (default: the selected one), at?: [x,y] (the overlap area under this point, as a click picks it) | area?: [[x,y]…] (a closed polygon drawn around overlap areas: every area whose inside it encloses), order?: toggle|front|back (toggle: the object drawn right below the top comes to the front, so two objects swap), member?: id (front and back: the object to move; default: back moves the top one)} → {count: areas changed, top: [id of the object now on top in each area]}",
            has_intertwine,
            arrange
        ),
    ]
}

// ---------- predicates ----------

/// The Intertwine groups of the selection: selected ones, and those around a selected object.
fn selected_groups(s: &Session) -> Vec<NodeId> {
    let Some(st) = s.active() else { return vec![] };
    let mut out = vec![];
    for id in &st.selection.objects {
        let group = st.doc.ancestry(*id).unwrap_or_default().into_iter().rev().find(|a| st.doc.node(*a).is_some_and(is_intertwine));
        if let Some(g) = group
            && !out.contains(&g)
        {
            out.push(g);
        }
    }
    out
}

fn is_intertwine(n: &Node) -> bool {
    matches!(n.kind, NodeKind::Intertwine { .. })
}

fn has_intertwine(s: &Session) -> std::result::Result<(), String> {
    has_selection(s)?;
    if selected_groups(s).is_empty() { Err("select an Intertwine group".into()) } else { Ok(()) }
}

// ---------- Make / Release ----------

/// Why `n` can't go into an Intertwine group, if it can't (the help page's limits: no nested
/// Intertwine groups, Live Paint, Repeat or graphs).
fn refused(d: &Document, n: &Node) -> Option<&'static str> {
    if d.ancestry(n.id).unwrap_or_default().iter().filter_map(|a| d.node(*a)).any(is_intertwine) {
        return Some("an object already in an Intertwine group");
    }
    let mut why = None;
    n.walk(&mut |c| {
        why = why.or(match &c.kind {
            NodeKind::Intertwine { .. } => Some("an Intertwine group"),
            NodeKind::Repeat(_) => Some("a Repeat"),
            NodeKind::Group { .. } if c.extras().graph.is_some() => Some("a graph"),
            NodeKind::Group { .. } if vectorcraft_tools::builder::is_live_paint(c) => Some("a Live Paint group"),
            _ => None,
        });
    });
    why
}

fn make(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "object.intertwine.make";
    let roots = match p.get("ids") {
        Some(_) => super::edit::roots_of(&s.doc()?.doc, checked_ids_param(s, p, "ids", C)?),
        None => selected_roots(s)?,
    };
    if roots.len() < 2 {
        return Err(bad(C, "select two or more objects to intertwine"));
    }
    {
        let d = &s.doc()?.doc;
        for r in &roots {
            let n = d.node(*r).ok_or(EngineError::NoNode(*r))?;
            if n.is_layer() {
                return Err(bad(C, "layers can't be intertwined: select the objects in them"));
            }
            if let Some(what) = refused(d, n) {
                return Err(bad(C, format!("{what} can't be intertwined")));
            }
        }
    }
    let id = s.edit("Make Intertwine", |d, sel| {
        let id = d.alloc_id();
        d.wrap(&roots, |children| Node::new(id, NodeKind::Intertwine { children }))?;
        sel.set([id]);
        Ok(id)
    })?;
    Ok(json!({ "id": id.0 }))
}

fn release(s: &mut Session, _: &Value) -> Result<Value> {
    let groups = selected_groups(s);
    let ids = s.edit("Release Intertwine", |d, sel| {
        let mut out = vec![];
        let mut released = Beside::replacing(d, &groups);
        for g in &groups {
            let Some(n) = d.node(*g).cloned() else { continue };
            let members: Vec<Node> = n
                .children()
                .into_iter()
                .flatten()
                .map(|c| {
                    let mut c = (**c).clone();
                    c.extras_mut().intertwine = None;
                    c
                })
                .collect();
            out.extend(members.iter().map(|c| c.id));
            released.put(d, *g, members)?;
        }
        released.finish(d)?;
        sel.set(out.iter().copied());
        Ok(out)
    })?;
    Ok(json!({ "ids": ids.iter().map(|i| i.0).collect::<Vec<_>>() }))
}

// ---------- Arrange ----------

#[derive(Clone, Copy, PartialEq)]
enum Order {
    Toggle,
    Front,
    Back,
}

/// The area `n` paints, in document coordinates, filled non-zero: its visible fills and stroke
/// outlines; type by its outlines; groups by their visible members; other objects by their
/// bounds. Effects are left out. `None` when it paints nothing.
pub(crate) fn painted_area(n: &Node) -> Option<PathData> {
    if !n.visible {
        return None;
    }
    let mut parts: Vec<PathData> = vec![];
    let shape = match &n.kind {
        NodeKind::Path { path, rule, guide: false, clipping: false, .. } => Some((path.clone(), *rule)),
        NodeKind::Compound { children, rule } => {
            let subs: Vec<SubPath> = children.iter().filter_map(|c| c.path_data()).flat_map(|p| p.subpaths.iter().cloned()).collect();
            Some((PathData::new(subs), *rule))
        }
        NodeKind::Path { .. } => return None,
        NodeKind::Group { children, clip: true } => {
            // What the members paint inside the clipping path.
            let clip = children.first().and_then(|c| match &c.kind {
                NodeKind::Path { path, rule, .. } => Some(po::normalize(path, *rule)),
                _ => c.geometric_bounds().map(shapes::rectangle),
            })?;
            let inside = unite(children.iter().skip(1).filter_map(|c| painted_area(c)).collect())?;
            return nonempty(po::boolean(&inside, FillRule::NonZero, &clip, FillRule::NonZero, po::BoolOp::Intersect));
        }
        NodeKind::Group { children, .. } | NodeKind::Layer { children, .. } | NodeKind::Intertwine { children } => {
            return unite(children.iter().filter_map(|c| painted_area(c)).collect());
        }
        NodeKind::Text(_) => return effects::outline_text(n).as_ref().and_then(painted_area),
        _ => None,
    };
    match shape {
        Some((path, rule)) => {
            let filled = n.appearance.items.iter().any(|i| matches!(i, AppearanceItem::Fill(f) if f.visible && !f.paint.is_none()));
            if filled {
                parts.push(po::normalize(&path, rule));
            }
            for st in n.appearance.painted_strokes().filter(|st| st.width > 0.0 && st.width.is_finite()) {
                parts.push(effects::stroke::outline_region(&path, rule, st));
            }
        }
        None => parts.push(shapes::rectangle(n.visual_bounds()?)),
    }
    unite(parts)
}

fn unite(parts: Vec<PathData>) -> Option<PathData> {
    let parts: Vec<PathData> = parts.into_iter().filter(|p| !p.is_empty()).collect();
    match parts.len() {
        0 => None,
        1 => parts.into_iter().next(),
        _ => nonempty(po::unite_all(&parts.iter().map(|p| (p, FillRule::NonZero)).collect::<Vec<_>>())),
    }
}

/// Areas smaller than this (pt²) are what a boolean leaves of an area taken from itself.
const SLIVER: f64 = 1e-3;

fn nonempty(p: PathData) -> Option<PathData> {
    (!p.is_empty() && po::area(&p, FillRule::NonZero).abs() > SLIVER).then_some(p)
}

/// A closed polygon from `[[x,y]…]` (three points or more, finite).
fn polygon(v: &Value) -> Option<PathData> {
    let pts: Vec<Point> = v.as_array()?.iter().map(|q| Some(Point::new(q.get(0)?.as_f64()?, q.get(1)?.as_f64()?))).collect::<Option<_>>()?;
    if pts.len() < 3 || pts.iter().any(|q| !(q.x.is_finite() && q.y.is_finite())) {
        return None;
    }
    Some(PathData::single(SubPath::polyline(&pts, true)))
}

fn contains(area: &PathData, p: Point) -> bool {
    vectorcraft_geom::hit::fill_contains(&area.to_bezpath(), FillRule::NonZero, p)
}

/// One overlap area to arrange: its outline, a point inside it, and the members (child indices,
/// bottom first) that paint there.
struct Face {
    path: PathData,
    inside: Point,
    members: Vec<usize>,
}

/// The overlap areas of `children` that `at` lies in, or that `lasso` encloses.
fn faces(children: &[Arc<Node>], at: Option<Point>, lasso: Option<&PathData>) -> Vec<Face> {
    let shapes: Vec<po::Shape> =
        children.iter().enumerate().filter_map(|(i, c)| Some(po::Shape::new(painted_area(c)?, FillRule::NonZero, i as u64))).collect();
    let key = |k: usize| shapes.get(k).map(|s| s.key as usize);
    po::regions(&shapes)
        .into_iter()
        .filter(|r| r.sources.len() >= 2)
        .filter_map(|r| {
            let inside = match at {
                Some(p) => r.contains(p).then_some(p)?,
                None => po::interior_point(&r.path)?,
            };
            if lasso.is_some_and(|l| !contains(l, inside)) {
                return None;
            }
            Some(Face { members: r.sources.iter().filter_map(|k| key(*k)).collect(), path: r.path, inside })
        })
        .collect()
}

/// The members of `face` in the order they are drawn there, bottom first: those drawn in their
/// own order, then those drawn over the others there, each run in the group's order.
fn drawn_order(children: &[Arc<Node>], face: &Face) -> Vec<usize> {
    let over = |i: &usize| children.get(*i).and_then(|c| over_area(c)).is_some_and(|a| contains(a, face.inside));
    let mut order = face.members.clone();
    order.sort_by_key(|i| (over(i), *i));
    order
}

/// `n`'s area with `face` added (`add`) or taken out.
fn with_area(n: &mut Node, face: &PathData, add: bool) {
    let old = n.extras().intertwine.as_deref().cloned();
    let new = match (old, add) {
        (None, true) => nonempty(face.clone()),
        (None, false) => None,
        (Some(a), true) => nonempty(po::boolean(&a, FillRule::NonZero, face, FillRule::NonZero, po::BoolOp::Union)),
        (Some(a), false) => nonempty(po::boolean(&a, FillRule::NonZero, face, FillRule::NonZero, po::BoolOp::Difference)),
    };
    if new.as_ref() != n.extras().intertwine.as_deref() {
        n.extras_mut().intertwine = new.map(Box::new);
    }
}

/// Bring member `x` to the front of `face` (`front`) or send it to the back there; returns the
/// member now on top there.
fn arrange_face(children: &mut [Arc<Node>], face: &Face, x: usize, front: bool) -> Option<usize> {
    let lowest = face.members.iter().min().copied()?;
    let highest = face.members.iter().max().copied()?;
    for &i in &face.members {
        let Some(c) = children.get_mut(i) else { continue };
        let c = Arc::make_mut(c);
        if front {
            // Only `x` is drawn over the others here, unless it is on top in its own order.
            with_area(c, &face.path, i == x && x != highest);
        } else if i == x {
            with_area(c, &face.path, false);
        } else if x != lowest {
            // Every other member is drawn over `x`, keeping their own order among them.
            with_area(c, &face.path, true);
        }
    }
    drawn_order(children, face).last().copied()
}

fn arrange(s: &mut Session, p: &Value) -> Result<Value> {
    const C: &str = "object.intertwine.arrange";
    let group = match p.get("id") {
        Some(v) => {
            let id = checked_id(s, v, C)?;
            if !s.doc()?.doc.node(id).is_some_and(is_intertwine) {
                return Err(bad(C, format!("object {} is not an Intertwine group", id.0)));
            }
            id
        }
        None => match selected_groups(s).as_slice() {
            [g] => *g,
            [] => return Err(bad(C, "select an Intertwine group")),
            _ => return Err(bad(C, "select one Intertwine group")),
        },
    };
    let at = match p.get("at") {
        Some(_) => Some(point_param(p, "at").filter(|q| q.x.is_finite() && q.y.is_finite()).ok_or_else(|| bad(C, "at must be [x, y]"))?),
        None => None,
    };
    let lasso = match p.get("area") {
        Some(v) => Some(polygon(v).ok_or_else(|| bad(C, "area must be a closed polygon of three or more [x, y] points"))?),
        None => None,
    };
    if at.is_some() == lasso.is_some() {
        return Err(bad(C, "give either at (a click) or area (a path drawn around the overlaps)"));
    }
    let order = match str_param(p, "order").unwrap_or("toggle") {
        "toggle" => Order::Toggle,
        "front" => Order::Front,
        "back" => Order::Back,
        o => return Err(bad(C, format!("unknown order `{o}`: toggle, front or back"))),
    };
    let member = match p.get("member") {
        Some(v) => Some(checked_id(s, v, C)?),
        None => None,
    };
    if member.is_some() && order == Order::Toggle {
        return Err(bad(C, "member is for order front or back"));
    }
    let children: Vec<Arc<Node>> = s.doc()?.doc.node(group).and_then(Node::children).cloned().unwrap_or_default();
    let member = match member {
        Some(m) => Some(children.iter().position(|c| c.id == m).ok_or_else(|| bad(C, format!("object {} is not in this Intertwine group", m.0)))?),
        None => None,
    };
    let found = faces(&children, at, lasso.as_ref());
    if found.is_empty() {
        return Err(bad(
            C,
            if at.is_some() {
                "no overlap there: click where two or more objects of the group overlap"
            } else {
                "the path encloses no overlap of the group's objects"
            },
        ));
    }
    if let Some(m) = member
        && !found.iter().any(|f| f.members.contains(&m))
    {
        return Err(bad(C, "that object is not in the overlap"));
    }
    let tops = s.edit("Intertwine", |d, _| {
        let n = d.node_mut(group).ok_or(EngineError::NoNode(group))?;
        let NodeKind::Intertwine { children } = &mut n.kind else { return Err(EngineError::NoNode(group)) };
        let mut tops = vec![];
        for face in found.iter().filter(|f| member.is_none_or(|m| f.members.contains(&m))) {
            let drawn = drawn_order(children, face);
            let (x, front) = match (order, member) {
                (Order::Toggle, _) => (drawn.iter().rev().nth(1).copied(), true),
                (Order::Front, m) => (m.or(drawn.last().copied()), true),
                (Order::Back, m) => (m.or(drawn.last().copied()), false),
            };
            let Some(x) = x else { continue };
            if let Some(top) = arrange_face(children, face, x, front)
                && let Some(c) = children.get(top)
            {
                tops.push(c.id.0);
            }
        }
        Ok(tops)
    })?;
    Ok(json!({ "count": tops.len(), "top": tops }))
}
