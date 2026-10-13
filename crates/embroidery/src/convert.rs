//! Vector objects → StitchFoundry embroidery objects (design space: millimetres, y down).

use crate::{MM_PER_PT, mm};
use sf_core::attr::{Embroidery, StitchType};
use sf_core::direction::{Guide, auto_fill_angle, auto_guides};
use sf_core::geom::{Column, Contour, Point, Polygon, Rung, Shape, poly, polyline, shape::flatten_cubic};
use sf_core::model::{AppliqueObject, FillKind, FillObject, FillParams, ObjectKind, RedworkObject, RunObject, SatinObject, StitchBlock};
use sf_core::pattern::{Cmd, Stitch};
use sf_core::satin_line::satin_line;
use sf_core::threads::Rgb;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Node, NodeKind};
use vectorcraft_geom::FillRule;
use vectorcraft_geom::path::SubPath;

/// Flattening tolerance in mm.
const TOL_MM: f64 = 0.04;

/// A flattened subpath in mm.
#[derive(Clone, Debug)]
pub(crate) struct Line {
    pub pts: Vec<Point>,
    pub closed: bool,
}

/// For each subpath: `Some(area in mm²)` when it is a hole (inside an odd number of
/// the other closed outlines, as when an "O" is painted), else `None`.
pub fn holes(subpaths: &[&SubPath]) -> Vec<Option<f64>> {
    let lines: Vec<Option<Line>> = subpaths.iter().map(|sp| flatten_subpath(sp)).collect();
    let polys: Vec<Option<Polygon>> =
        lines.iter().map(|l| l.as_ref().filter(|l| l.pts.len() >= 3).map(|l| Polygon::from_ring(l.pts.clone()))).collect();
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let l = l.as_ref().filter(|l| l.pts.len() >= 3)?;
            let probe = *l.pts.first()?;
            let depth = polys.iter().enumerate().filter(|(j, p)| *j != i && p.as_ref().is_some_and(|p| p.contains(probe))).count();
            (depth % 2 == 1).then(|| polyline::signed_area(&l.pts).abs())
        })
        .collect()
}

/// For each subpath, the outline it belongs to (an index into `subpaths`): itself for
/// an outer outline or an open path, else the smallest outer outline round it. A hole
/// stays with the shape it is cut from when an object is broken into its parts; an
/// island inside a hole is a part of its own.
pub fn parts(subpaths: &[&SubPath]) -> Vec<usize> {
    let rings: Vec<Option<Vec<Point>>> =
        subpaths.iter().map(|sp| flatten_subpath(sp).filter(|l| l.closed && l.pts.len() >= 3).map(|l| l.pts)).collect();
    let polys: Vec<Option<Polygon>> = rings.iter().map(|r| r.as_ref().map(|r| Polygon::from_ring(r.clone()))).collect();
    let inside =
        |p: Point, except: usize| polys.iter().enumerate().filter(|(j, q)| *j != except && q.as_ref().is_some_and(|q| q.contains(p))).count();
    let depth: Vec<Option<usize>> = rings.iter().enumerate().map(|(i, r)| r.as_ref().and_then(|r| r.first()).map(|p| inside(*p, i))).collect();
    let area = |j: usize| rings[j].as_ref().map_or(f64::INFINITY, |r| polyline::signed_area(r).abs());
    (0..subpaths.len())
        .map(|i| match (depth[i], rings[i].as_ref().and_then(|r| r.first())) {
            (Some(d), Some(&probe)) if d % 2 == 1 => (0..subpaths.len())
                .filter(|&j| j != i && depth[j].is_some_and(|dj| dj % 2 == 0) && polys[j].as_ref().is_some_and(|q| q.contains(probe)))
                .min_by(|&a, &b| area(a).total_cmp(&area(b)))
                .unwrap_or(i),
            _ => i,
        })
        .collect()
}

pub(crate) fn flatten_subpath(sp: &SubPath) -> Option<Line> {
    let first = sp.anchors.first()?;
    let mut pts = vec![mm(first.p)];
    for i in 0..sp.segment_count() {
        let c = sp.segment(i);
        if sp.segment_is_line(i) {
            pts.push(mm(c.p3));
        } else {
            let start = *pts.last()?;
            flatten_cubic(start, mm(c.p1), mm(c.p2), mm(c.p3), TOL_MM, 0, &mut pts);
        }
    }
    let pts = if sp.closed { polyline::dedup_ring(&pts, 1e-6) } else { polyline::dedup(&pts, 1e-6) };
    (pts.len() >= 2).then_some(Line { pts, closed: sp.closed })
}

/// The anchor points of a subpath (for manual stitches), in mm.
fn anchor_points(sp: &SubPath) -> Vec<Point> {
    let mut v: Vec<Point> = sp.anchors.iter().map(|a| mm(a.p)).collect();
    if sp.closed
        && let Some(&f) = v.first()
    {
        v.push(f);
    }
    v
}

/// The glyph outlines of type (with its underline and strikethrough bars), in
/// document space.
fn text_paths(t: &vectorcraft_doc::TextObject) -> Vec<vectorcraft_geom::path::PathData> {
    let db = vectorcraft_text::FontDb::global();
    let lay = vectorcraft_text::layout(db, t);
    let glyphs = lay.glyphs.iter().map(|g| &g.outline).cloned();
    let bars = vectorcraft_text::decorations(&lay, db, t).into_iter().map(|(_, bar)| bar);
    glyphs.chain(bars).map(|b| vectorcraft_geom::path::PathData::from_bezpath(&b).transformed(t.xf)).filter(|p| !p.is_empty()).collect()
}

/// Flattened geometry of a path, compound path or type, with its fill rule.
pub(crate) fn node_lines(node: &Node) -> Option<(Vec<Line>, FillRule)> {
    match &node.kind {
        // Glyphs wind non-zero (counters run the other way round).
        NodeKind::Text(t) => Some((text_paths(t).iter().flat_map(|p| p.subpaths.iter().filter_map(flatten_subpath)).collect(), FillRule::NonZero)),
        NodeKind::Path { path, rule, .. } => Some((path.subpaths.iter().filter_map(flatten_subpath).collect(), *rule)),
        NodeKind::Compound { children, rule } => {
            let mut lines = Vec::new();
            for c in children {
                if let NodeKind::Path { path, .. } = &c.kind {
                    lines.extend(path.subpaths.iter().filter_map(flatten_subpath));
                }
            }
            Some((lines, *rule))
        }
        _ => None,
    }
}

fn subpaths(node: &Node) -> Vec<&SubPath> {
    match &node.kind {
        NodeKind::Path { path, .. } => path.subpaths.iter().collect(),
        NodeKind::Compound { children, .. } => children
            .iter()
            .filter_map(|c| if let NodeKind::Path { path, .. } = &c.kind { Some(path.subpaths.iter()) } else { None })
            .flatten()
            .collect(),
        _ => Vec::new(),
    }
}

/// Filled regions (with holes) of the lines, honouring the fill rule. Open
/// subpaths are closed implicitly, as when they are painted.
pub(crate) fn regions(lines: &[Line], rule: FillRule) -> Vec<Polygon> {
    let rings: Vec<Polygon> = lines.iter().filter(|l| l.pts.len() >= 3).map(|l| Polygon { outer: l.pts.clone(), holes: Vec::new() }).collect();
    if rings.is_empty() {
        return Vec::new();
    }
    match rule {
        FillRule::EvenOdd => poly::clean(&rings),
        // Non-zero needs the rings' own orientation, so they are passed raw
        // (`Polygon::from_ring` would normalise it away).
        FillRule::NonZero => poly::union(&rings),
    }
}

/// The paint colour of a node: fill for areas, stroke for lines (falling back to
/// the other one).
pub(crate) fn paint_color(node: &Node, prefer_stroke: bool) -> Option<Rgb> {
    let solid = |p: Paint| match p {
        Paint::Solid { color, .. } => {
            let c = color.to_rgba8(1.0);
            Some(Rgb::new(c[0], c[1], c[2]))
        }
        _ => None,
    };
    // Type is painted by its characters' styles.
    let appearance = match &node.kind {
        NodeKind::Text(t) => std::borrow::Cow::Owned(t.first_style().appearance()),
        _ => std::borrow::Cow::Borrowed(&node.appearance),
    };
    let fill = || solid(appearance.fill_paint());
    let stroke = || solid(appearance.stroke_paint());
    if prefer_stroke { stroke().or_else(fill) } else { fill().or_else(stroke) }
}

fn guides_mm(e: &Embroidery) -> Vec<Guide> {
    e.guides.iter().map(|g| Guide::new(g.a * MM_PER_PT, g.b * MM_PER_PT)).collect()
}

fn splits_mm(e: &Embroidery) -> Vec<Vec<Point>> {
    e.splits.iter().map(|line| line.iter().map(|&p| p * MM_PER_PT).collect()).collect()
}

fn flow_mm(e: &Embroidery) -> Vec<Vec<Point>> {
    e.flow.iter().map(|line| line.iter().map(|&p| p * MM_PER_PT).collect()).collect()
}

/// Whether a closed outline reads as a column (long and thin), so satin should
/// turn along it rather than run straight across.
fn column_like(ring: &[Point], guides: &[Guide]) -> bool {
    let poly = Polygon::from_ring(ring.to_vec());
    let area = poly.area();
    let half = poly.perimeter() * 0.5;
    // elongation (half perimeter)² / area: 4 for a square, about 7 for a 5:1 bar
    !guides.is_empty() && area > 1e-9 && half * half / area > 6.5
}

/// The lines of a node: its own (a path, compound path or type), or those of all the visible art
/// in a group.
fn art_lines(node: &Node) -> Vec<Line> {
    match &node.kind {
        NodeKind::Group { .. } | NodeKind::Layer { .. } => {
            node.children().into_iter().flatten().filter(|c| c.visible).flat_map(|c| art_lines(c)).collect()
        }
        _ => node_lines(node).map(|(lines, _)| lines).unwrap_or_default(),
    }
}

/// Converts one stitched node into embroidery objects (several for multi-part
/// runs or fills). Positions are in millimetres.
pub(crate) fn convert(node: &Node, e: &Embroidery) -> Vec<ObjectKind> {
    let mut out = Vec::new();
    // Stitches edited by hand are sewn as they are.
    if let Some(frozen) = &e.frozen {
        let stitches: Vec<Stitch> = frozen.iter().map(|s| Stitch::new(s.x * MM_PER_PT, s.y * MM_PER_PT, s.cmd)).collect();
        if stitches.iter().any(|s| s.cmd == Cmd::Stitch) {
            out.push(ObjectKind::Stitches(StitchBlock { stitches }));
        }
        return out;
    }
    match &e.stitch {
        // a branch group only says how its members are sewn, lettering what they say
        StitchType::Branch | StitchType::Lettering(_) => {}
        // redwork: all the lines of the node (a group: of the visible art in it) sewn as one line
        StitchType::Redwork(params) => {
            let lines: Vec<Contour> = art_lines(node).into_iter().map(|l| Contour::from_points(&l.pts, l.closed)).collect();
            if !lines.is_empty() {
                out.push(ObjectKind::Redwork(RedworkObject { lines, params: params.clone() }));
            }
        }
        StitchType::Decoration(params) => {
            use sf_core::decor::{DecorItem, DecorKind, DecorLayout, DecorObject, along, border, packed};
            let lines = || node_lines(node).map(|(l, _)| l).unwrap_or_default();
            let mut obj = DecorObject { params: params.clone(), items: Vec::new(), lines: Vec::new() };
            match params.kind {
                DecorKind::Cord => obj.lines = lines().into_iter().map(|l| (l.pts, l.closed)).collect(),
                DecorKind::Boring => obj.lines = crate::regions_mm(node).into_iter().map(|r| (r.outer, true)).collect(),
                _ => match params.layout {
                    DecorLayout::Run => {
                        for l in lines() {
                            let k = obj.items.len();
                            obj.items.extend(along(&l.pts, l.closed, params, k));
                        }
                    }
                    DecorLayout::Fill => {
                        for r in crate::regions_mm(node) {
                            obj.items.extend(packed(&r, params));
                        }
                    }
                    DecorLayout::Border => {
                        for r in crate::regions_mm(node) {
                            obj.items.extend(border(&r, params));
                        }
                    }
                    // one at each point drawn, turned the way the line goes there
                    DecorLayout::Manual => {
                        for sp in subpaths(node) {
                            let pts = anchor_points(sp);
                            for (i, p) in pts.iter().enumerate() {
                                let prev = i.checked_sub(1).and_then(|j| pts.get(j)).unwrap_or(p);
                                let next = pts.get(i + 1).unwrap_or(p);
                                let k = obj.items.len();
                                obj.items.push(DecorItem {
                                    at: *p,
                                    size: params.size_at(k),
                                    angle: (*next - *prev).angle(),
                                    flip: params.flip && k % 2 == 1,
                                });
                            }
                        }
                    }
                },
            }
            obj.items.truncate(sf_core::decor::MAX_ITEMS);
            if !obj.items.is_empty() || !obj.lines.is_empty() {
                out.push(ObjectKind::Decor(obj));
            }
        }
        StitchType::Manual => {
            let mut st: Vec<Stitch> = Vec::new();
            for sp in subpaths(node) {
                let pts = anchor_points(sp);
                for (i, p) in pts.into_iter().enumerate() {
                    let cmd = if i == 0 && !st.is_empty() { Cmd::Jump } else { Cmd::Stitch };
                    if cmd == Cmd::Jump {
                        st.push(Stitch::at(p, Cmd::Trim));
                    }
                    st.push(Stitch::at(p, cmd));
                    if cmd == Cmd::Jump {
                        st.push(Stitch::at(p, Cmd::Stitch));
                    }
                }
            }
            if st.len() >= 2 {
                out.push(ObjectKind::Stitches(StitchBlock { stitches: st }));
            }
        }
        StitchType::Run(params) => {
            let Some((lines, _)) = node_lines(node) else { return out };
            for l in lines {
                out.push(ObjectKind::Run(RunObject { path: Contour::from_points(&l.pts, l.closed), params: params.clone() }));
            }
        }
        StitchType::Satin(params) => {
            let Some((lines, rule)) = node_lines(node) else { return out };
            let open: Vec<&Line> = lines.iter().filter(|l| !l.closed).collect();
            if lines.len() == 2 && open.len() == 2 {
                // Classic column between two rails.
                if let Some(col) = rails_column(&open[0].pts, &open[1].pts, &guides_mm(e)) {
                    out.push(ObjectKind::Satin(SatinObject { column: col, params: params.clone() }));
                }
            } else if !lines.is_empty() && (open.len() == lines.len() || crate::is_line_art(node)) {
                // Satin line along each path ("steil" / brush stroke), round and
                // round a closed one painted as a stroke (a satin border).
                let width = e.width.unwrap_or_else(|| (node.appearance.stroke_width() * MM_PER_PT).max(0.0));
                let width = if width > 0.05 { width } else { 3.0 };
                for l in &lines {
                    if let Some(col) = satin_line(&l.pts, l.closed, width, &e.profile, 0.6, params.corners) {
                        out.push(ObjectKind::Satin(SatinObject { column: col, params: params.clone() }));
                    }
                }
            } else {
                // Closed shape: satin across it, turning with the inclination lines.
                let fill = FillParams {
                    kind: FillKind::Satin,
                    spacing: params.spacing,
                    pull_comp: params.pull_comp,
                    split_length: params.split_length,
                    angle: 0.0,
                    satin_column: Some(params.clone()),
                    ..FillParams::satin()
                };
                for region in regions(&lines, rule) {
                    fill_objects(region, fill.clone(), e, &mut out);
                }
            }
        }
        StitchType::Fill(params) => {
            let Some((lines, rule)) = node_lines(node) else { return out };
            for region in regions(&lines, rule) {
                fill_objects(region, params.clone(), e, &mut out);
            }
        }
        StitchType::Applique(params) => {
            let Some((lines, rule)) = node_lines(node) else { return out };
            let shapes: Vec<Shape> = regions(&lines, rule).iter().map(Shape::from_polygon).collect();
            if params.combine {
                // the pieces sewn together: one object, its placement lines, tack-downs and covers each in turn
                if let Some((first, rest)) = shapes.split_first() {
                    out.push(ObjectKind::Applique(AppliqueObject {
                        shape: first.clone(),
                        params: params.clone(),
                        covered: Vec::new(),
                        extra: rest.to_vec(),
                    }));
                }
            } else {
                for shape in shapes {
                    out.push(ObjectKind::Applique(AppliqueObject { shape, params: params.clone(), covered: Vec::new(), extra: Vec::new() }));
                }
            }
        }
    }
    out
}

/// Fill objects for one region, choosing the stitch direction automatically when
/// the user hasn't placed inclination lines. Satin turns along simple columns, so
/// the region is first cut open at its holes (an "O" becomes two columns) and at
/// the sharp inside corners where strokes join (a "T" becomes three).
fn fill_objects(region: Polygon, params: FillParams, e: &Embroidery, out: &mut Vec<ObjectKind>) {
    if params.kind == FillKind::Satin && e.guides.is_empty() && e.auto_direction {
        for piece in poly::split_holes(&region).iter().flat_map(poly::split_branches) {
            out.push(fill_object(piece, params.clone(), e));
        }
    } else {
        out.push(fill_object(region, params, e));
    }
}

/// A fill object for one region (see [`fill_objects`]).
fn fill_object(region: Polygon, mut params: FillParams, e: &Embroidery) -> ObjectKind {
    let mut guides = guides_mm(e);
    let splits = splits_mm(e);
    let flow = if params.kind == FillKind::Tatami { flow_mm(e) } else { Vec::new() };
    // flow lines lead the rows: nothing to choose
    if guides.is_empty() && e.auto_direction && flow.is_empty() {
        let satin = params.kind == FillKind::Satin;
        // Textures and split lines show on straight rows: tatami with them doesn't turn.
        let textured = params.texture.is_some() || !splits.is_empty();
        let auto = if region.holes.is_empty() { auto_guides(&region.outer) } else { None };
        match auto {
            // satin (and tatami on long thin shapes) turns along the shape
            Some(g) if satin || (!textured && column_like(&region.outer, &g)) => guides = g,
            _ if matches!(params.kind, FillKind::Tatami | FillKind::Satin) => {
                params.angle = auto_fill_angle(&region, params.spacing.max(0.8));
            }
            _ => {}
        }
    }
    ObjectKind::Fill(FillObject { shape: Shape::from_polygon(&region), params, guides, splits, flow })
}

/// A satin column between two rails: rungs from the inclination lines when there
/// are two or more, otherwise paired proportionally along the rails.
fn rails_column(a: &[Point], b: &[Point], guides: &[Guide]) -> Option<Column> {
    let mut b = b.to_vec();
    // orient rail B like rail A
    if let (Some(a0), Some(b0), Some(bn)) = (a.first(), b.first(), b.last())
        && a0.distance(*bn) < a0.distance(*b0)
    {
        b.reverse();
    }
    let la = polyline::length(a);
    let lb = polyline::length(&b);
    if la < 1e-6 || lb < 1e-6 {
        return None;
    }
    let mut ts: Vec<(f64, f64)> = Vec::new();
    for g in guides {
        // where each guide crosses the rails (arc-length fractions)
        if let (Some(ta), Some(tb)) = (crossing(a, g), crossing(&b, g)) {
            ts.push((ta / la, tb / lb));
        }
    }
    ts.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut stations = vec![(0.0, 0.0)];
    stations.extend(ts.into_iter().filter(|t| t.0 > 1e-6 && t.0 < 1.0 - 1e-6));
    stations.push((1.0, 1.0));
    let mut rungs = Vec::new();
    for w in stations.windows(2) {
        let ((ta0, tb0), (ta1, tb1)) = (w[0], w[1]);
        let seg = ((ta1 - ta0) * la).max((tb1 - tb0) * lb);
        let n = ((seg / 1.0).ceil() as usize).max(1);
        for k in 0..n {
            let f = k as f64 / n as f64;
            let pa = polyline::point_at(a, (ta0 + (ta1 - ta0) * f) * la);
            let pb = polyline::point_at(&b, (tb0 + (tb1 - tb0) * f) * lb);
            rungs.push(Rung::new(pa, pb, Default::default()));
        }
    }
    if let (Some(&pa), Some(&pb)) = (a.last(), b.last()) {
        rungs.push(Rung::new(pa, pb, Default::default()));
    }
    (rungs.len() >= 2).then(|| Column::new(rungs))
}

/// Arc length along `rail` where the guide's line crosses it.
fn crossing(rail: &[Point], g: &Guide) -> Option<f64> {
    let d = g.b - g.a;
    let mut acc = 0.0;
    let mut best: Option<(f64, f64)> = None;
    for w in rail.windows(2) {
        let e = w[1] - w[0];
        let denom = d.cross(e);
        let seg = e.length();
        if denom.abs() > 1e-12 {
            let ap = w[0] - g.a;
            let t = ap.cross(e) / denom; // along the guide line
            let u = ap.cross(d) / denom; // along the rail segment
            if (0.0..=1.0).contains(&u) {
                let off = (t - 0.5).abs();
                if best.is_none_or(|b| off < b.0) {
                    best = Some((off, acc + u * seg));
                }
            }
        }
        acc += seg;
    }
    best.map(|b| b.1)
}
