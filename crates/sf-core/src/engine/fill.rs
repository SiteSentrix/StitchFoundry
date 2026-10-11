//! Scanline fills (tatami, satin fill, gradient density).
//!
//! The shape is rotated so stitch rows are horizontal, cut into rows, and the row
//! spans are grouped into *sections* (boustrophedon cells: runs of rows with one
//! span each, joined end to end). Sections are sewn back and forth without jumps.
//! Moving between sections uses running stitches routed over a travel graph made
//! of the outline and the rows themselves; routes that run *under* areas still to
//! be sewn are cheap, routes on top of finished stitching are expensive, so
//! connecting stitches end up hidden — the same trick human digitizers use.

use super::out::Out;
use crate::geom::{Affine, Point, Polygon, poly, polyline};
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Cost multiplier for travelling along an outline stretch whose neighbouring
/// rows are already sewn (the travel would sit visibly on top).
const COVERED_RING_COST: f64 = 6.0;
/// Cost multiplier for travelling along a row that is already sewn (nearly
/// invisible because it follows the existing stitch line).
const COVERED_SPAN_COST: f64 = 1.6;
/// Penalty for a straight hop when the graph offers no route.
const NO_ROUTE_COST: f64 = 25.0;

#[derive(Clone, Debug)]
pub(crate) struct ScanConfig {
    /// Stitch direction, degrees counter-clockwise on screen.
    pub angle: f64,
    /// Distance between rows (mm).
    pub row_spacing: f64,
    /// Row spacing at the far side for gradients; 0 = constant.
    pub row_spacing_end: f64,
    /// Needle step inside rows; `f64::INFINITY` = satin (one stitch per row).
    pub stitch_length: f64,
    /// Stagger of the needle grid per row, as a fraction of `stitch_length`.
    pub offset: f64,
    /// Satin rows longer than this are split; 0 = never.
    pub split_length: f64,
    /// Row end extension (mm).
    pub pull_comp: f64,
    /// Stitch length for travel runs.
    pub travel_length: f64,
    /// Needle-point jitter (0–1, fraction of a quarter stitch).
    pub random: f64,
}

impl Default for ScanConfig {
    fn default() -> Self {
        ScanConfig {
            angle: 0.0,
            row_spacing: 0.4,
            row_spacing_end: 0.0,
            stitch_length: 4.0,
            offset: 0.25,
            split_length: 0.0,
            pull_comp: 0.0,
            travel_length: 2.5,
            random: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Crossing {
    x: f64,
    ring: usize,
    /// Arc-length position along the ring.
    pos: f64,
}

#[derive(Clone, Debug)]
struct Span {
    row: usize,
    y: f64,
    l: Crossing,
    r: Crossing,
    /// Graph node ids of the left / right crossings.
    ln: usize,
    rn: usize,
}

#[derive(Clone, Copy, Debug)]
enum EdgeKind {
    /// Outline stretch; covered when both owning spans are sewn.
    Ring(usize, usize),
    /// Along a row.
    Span(usize),
}

#[derive(Clone, Copy, Debug)]
struct Edge {
    to: usize,
    len: f64,
    kind: EdgeKind,
}

struct Graph {
    pts: Vec<Point>,
    adj: Vec<Vec<Edge>>,
}

impl Graph {
    fn add_node(&mut self, p: Point) -> usize {
        self.pts.push(p);
        self.adj.push(Vec::new());
        self.pts.len() - 1
    }

    fn add_edge(&mut self, a: usize, b: usize, kind: EdgeKind) {
        let len = self.pts[a].distance(self.pts[b]);
        self.adj[a].push(Edge { to: b, len, kind });
        self.adj[b].push(Edge { to: a, len, kind });
    }
}

#[derive(Clone, Copy, PartialEq)]
struct Item(f64, usize);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.total_cmp(&self.0).then_with(|| self.1.cmp(&o.1))
    }
}

fn dijkstra(g: &Graph, src: usize, sewn: &[bool]) -> (Vec<f64>, Vec<usize>) {
    let n = g.pts.len();
    let mut dist = vec![f64::INFINITY; n];
    let mut prev = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    dist[src] = 0.0;
    heap.push(Item(0.0, src));
    let covered = |s: usize| s != usize::MAX && sewn[s];
    while let Some(Item(d, u)) = heap.pop() {
        if d > dist[u] {
            continue;
        }
        for e in &g.adj[u] {
            let w = match e.kind {
                EdgeKind::Ring(a, b) => {
                    if covered(a) && covered(b) {
                        COVERED_RING_COST
                    } else {
                        1.0
                    }
                }
                EdgeKind::Span(s) => {
                    if sewn[s] {
                        COVERED_SPAN_COST
                    } else {
                        1.0
                    }
                }
            };
            let nd = d + e.len * w;
            if nd < dist[e.to] {
                dist[e.to] = nd;
                prev[e.to] = u;
                heap.push(Item(nd, e.to));
            }
        }
    }
    (dist, prev)
}

fn path_to(prev: &[usize], src: usize, dst: usize) -> Option<Vec<usize>> {
    let mut path = vec![dst];
    let mut cur = dst;
    while cur != src {
        let p = prev[cur];
        if p == usize::MAX {
            return None;
        }
        path.push(p);
        cur = p;
    }
    path.reverse();
    Some(path)
}

/// How a section is entered: from its first or last row, at its left or right end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    section: usize,
    from_first: bool,
    left: bool,
}

struct Plan {
    spans: Vec<Span>,
    sections: Vec<Vec<usize>>,
    graph: Graph,
}

impl Plan {
    fn entry_node(&self, e: Entry) -> usize {
        let sec = &self.sections[e.section];
        let s = if e.from_first { sec[0] } else { sec[sec.len() - 1] };
        if e.left { self.spans[s].ln } else { self.spans[s].rn }
    }

    /// Node where the section ends when entered as `e`.
    fn exit_node(&self, e: Entry) -> usize {
        let sec = &self.sections[e.section];
        let last = if e.from_first { sec[sec.len() - 1] } else { sec[0] };
        // Rows alternate direction; with an odd number of rows we end on the opposite side.
        let ends_left = if sec.len() % 2 == 1 { !e.left } else { e.left };
        if ends_left { self.spans[last].ln } else { self.spans[last].rn }
    }

    fn entries(&self, section: usize) -> impl Iterator<Item = Entry> + '_ {
        let single = self.sections[section].len() == 1;
        [(true, true), (true, false), (false, true), (false, false)]
            .into_iter()
            .filter(move |(first, _)| *first || !single)
            .map(move |(from_first, left)| Entry { section, from_first, left })
    }
}

/// Builds rows, spans, sections and the travel graph in row space.
fn plan(rings: &[Vec<Point>], cfg: &ScanConfig) -> Option<Plan> {
    let bb = rings.first().map(|r| polyline::bbox(r))?;
    if bb.is_empty() {
        return None;
    }
    let (y0, y1) = (bb.min.y, bb.max.y);
    let s0 = cfg.row_spacing.max(0.05);
    let s1 = if cfg.row_spacing_end > 0.0 { cfg.row_spacing_end.max(0.05) } else { s0 };
    let height = y1 - y0;
    let mut ys = Vec::new();
    if (s1 - s0).abs() < 1e-12 {
        // Centre the rows so both edges get the same margin.
        let n = (height / s0).floor() as usize + 1;
        let used = (n - 1) as f64 * s0;
        let start = y0 + (height - used) * 0.5;
        for k in 0..n {
            ys.push(start + k as f64 * s0);
        }
    } else {
        let spacing_at = |y: f64| s0 + (s1 - s0) * ((y - y0) / height.max(1e-9)).clamp(0.0, 1.0);
        let mut y = y0 + spacing_at(y0) * 0.5;
        while y < y1 {
            ys.push(y);
            y += spacing_at(y);
        }
    }
    if ys.is_empty() {
        ys.push((y0 + y1) * 0.5);
    }
    // Nudge rows off exact vertex heights to avoid degenerate crossings.
    for y in &mut ys {
        if rings.iter().any(|r| r.iter().any(|p| (p.y - *y).abs() < 1e-9)) {
            *y += 1e-7;
        }
    }

    let cum: Vec<Vec<f64>> = rings
        .iter()
        .map(|r| {
            let mut c = Vec::with_capacity(r.len());
            let mut acc = 0.0;
            for i in 0..r.len() {
                c.push(acc);
                acc += r[i].distance(r[(i + 1) % r.len()]);
            }
            c
        })
        .collect();

    let mut graph = Graph { pts: Vec::new(), adj: Vec::new() };
    let mut spans: Vec<Span> = Vec::new();
    let mut row_spans: Vec<Vec<usize>> = Vec::new();
    for (row, &y) in ys.iter().enumerate() {
        let mut xs: Vec<Crossing> = Vec::new();
        for (ri, ring) in rings.iter().enumerate() {
            let n = ring.len();
            for i in 0..n {
                let a = ring[i];
                let b = ring[(i + 1) % n];
                if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                    let t = (y - a.y) / (b.y - a.y);
                    let x = a.x + t * (b.x - a.x);
                    xs.push(Crossing { x, ring: ri, pos: cum[ri][i] + t * a.distance(b) });
                }
            }
        }
        xs.sort_by(|a, b| a.x.total_cmp(&b.x));
        let mut ids = Vec::new();
        for pair in xs.chunks_exact(2) {
            let (l, r) = (pair[0], pair[1]);
            if r.x - l.x < 1e-6 {
                continue;
            }
            let ln = graph.add_node(Point::new(l.x, y));
            let rn = graph.add_node(Point::new(r.x, y));
            ids.push(spans.len());
            spans.push(Span { row, y, l, r, ln, rn });
        }
        row_spans.push(ids);
    }
    if spans.is_empty() {
        return None;
    }

    // Sections: a span continues the section of the single overlapping span in
    // the previous row, provided that one overlaps nothing else in this row.
    let overlap = |a: &Span, b: &Span| a.l.x < b.r.x && b.l.x < a.r.x;
    let mut section_of = vec![usize::MAX; spans.len()];
    let mut sections: Vec<Vec<usize>> = Vec::new();
    for k in 0..row_spans.len() {
        for &s in &row_spans[k] {
            let mut joined = false;
            if k > 0 {
                let prev: Vec<usize> = row_spans[k - 1].iter().copied().filter(|&p| overlap(&spans[p], &spans[s])).collect();
                if prev.len() == 1 {
                    let p = prev[0];
                    let fanout = row_spans[k].iter().filter(|&&c| overlap(&spans[p], &spans[c])).count();
                    let sec = section_of[p];
                    if fanout == 1 && sections[sec].last() == Some(&p) {
                        sections[sec].push(s);
                        section_of[s] = sec;
                        joined = true;
                    }
                }
            }
            if !joined {
                section_of[s] = sections.len();
                sections.push(vec![s]);
            }
        }
    }

    // Travel graph along the rings.
    for (ri, ring) in rings.iter().enumerate() {
        // (position, node, owning span)
        let mut items: Vec<(f64, usize, usize)> = Vec::new();
        for (i, &p) in ring.iter().enumerate() {
            let id = graph.add_node(p);
            items.push((cum[ri][i], id, usize::MAX));
        }
        for (si, s) in spans.iter().enumerate() {
            if s.l.ring == ri {
                items.push((s.l.pos, s.ln, si));
            }
            if s.r.ring == ri {
                items.push((s.r.pos, s.rn, si));
            }
        }
        items.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
        let n = items.len();
        if n < 2 {
            continue;
        }
        let has_owner = items.iter().any(|it| it.2 != usize::MAX);
        let mut prev_owner = vec![usize::MAX; n];
        let mut next_owner = vec![usize::MAX; n];
        if has_owner {
            let mut last = items.iter().rev().find(|it| it.2 != usize::MAX).map(|it| it.2).unwrap_or(usize::MAX);
            for i in 0..n {
                if items[i].2 != usize::MAX {
                    last = items[i].2;
                }
                prev_owner[i] = last;
            }
            let mut next = items.iter().find(|it| it.2 != usize::MAX).map(|it| it.2).unwrap_or(usize::MAX);
            for i in (0..n).rev() {
                if items[i].2 != usize::MAX {
                    next = items[i].2;
                }
                next_owner[i] = next;
            }
        }
        for i in 0..n {
            let j = (i + 1) % n;
            graph.add_edge(items[i].1, items[j].1, EdgeKind::Ring(prev_owner[i], next_owner[j]));
        }
    }
    for (si, s) in spans.iter().enumerate() {
        graph.add_edge(s.ln, s.rn, EdgeKind::Span(si));
    }
    Some(Plan { spans, sections, graph })
}

/// Appends running stitches along a graph path. The node chain is very dense
/// (every row crossing is a node), so it is simplified and re-sampled first.
fn travel_path(pts: &mut Vec<Point>, g: &Graph, path: &[usize], step: f64) {
    let mut line: Vec<Point> = Vec::with_capacity(path.len() + 1);
    if let Some(&cur) = pts.last() {
        line.push(cur);
    }
    line.extend(path.iter().map(|&n| g.pts[n]));
    let line = polyline::simplify(&polyline::dedup(&line, 1e-9), 0.05);
    if line.len() < 2 {
        return;
    }
    let res = polyline::resample(&line, step, 0.35);
    if pts.is_empty() {
        pts.extend(res);
    } else {
        pts.extend(res.into_iter().skip(1));
    }
}

/// Needle positions along one row from `xa` to `xb` (inclusive).
fn row_points(xa: f64, xb: f64, row: usize, cfg: &ScanConfig, out: &mut Vec<f64>) {
    out.clear();
    let (lo, hi) = (xa.min(xb), xa.max(xb));
    let len = hi - lo;
    let (step, offset) = if cfg.stitch_length.is_finite() {
        (cfg.stitch_length.max(0.3), cfg.offset)
    } else if cfg.split_length > 0.0 && len > cfg.split_length {
        (cfg.split_length * 0.75, 0.37)
    } else {
        out.push(xa);
        out.push(xb);
        return;
    };
    let phase = (row as f64 * offset).rem_euclid(1.0) * step;
    let min_end = (step * 0.35).clamp(0.4, 1.5).min(len * 0.5);
    let m0 = ((lo + min_end - phase) / step).ceil() as i64;
    let m1 = ((hi - min_end - phase) / step).floor() as i64;
    // Optional jitter, deterministic per needle point (hash of row and index).
    let jitter = |m: i64| {
        if cfg.random <= 0.0 {
            return 0.0;
        }
        let mut h = (row as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ (m as u64).wrapping_mul(0xc2b2_ae3d_27d4_eb4f);
        h ^= h >> 29;
        h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        h ^= h >> 32;
        let u = (h >> 11) as f64 / (1u64 << 53) as f64;
        (u - 0.5) * 0.5 * step * cfg.random.clamp(0.0, 1.0)
    };
    let x_at = |m: i64| (phase + m as f64 * step + jitter(m)).clamp(lo + min_end * 0.5, hi - min_end * 0.5);
    out.push(xa);
    if m1 >= m0 {
        if xa <= xb {
            for m in m0..=m1 {
                out.push(x_at(m));
            }
        } else {
            for m in (m0..=m1).rev() {
                out.push(x_at(m));
            }
        }
    }
    out.push(xb);
}

/// Fills one polygon. Returns needle positions in design space.
pub(crate) fn scan_fill(polygon: &Polygon, cfg: &ScanConfig, hint: Option<Point>, exit: Option<Point>) -> Vec<Point> {
    let theta = cfg.angle.to_radians();
    let fwd = Affine::rotate(theta);
    let inv = Affine::rotate(-theta);
    let mut p = polygon.clone();
    p.transform(&fwd);
    let rings: Vec<Vec<Point>> = p.rings().filter(|r| r.len() >= 3).cloned().collect();
    if rings.is_empty() {
        return Vec::new();
    }
    let Some(mut plan) = plan(&rings, cfg) else {
        return Vec::new();
    };
    let hint_rs = hint.map(|h| fwd.apply(h));
    let pc = cfg.pull_comp;
    let travel = cfg.travel_length.max(0.5);

    let mut sewn = vec![false; plan.spans.len()];
    let mut done = vec![false; plan.sections.len()];
    let mut remaining = plan.sections.len();
    let mut pts: Vec<Point> = Vec::new();
    let mut cur: Option<usize> = None;
    let mut xs = Vec::new();

    let push_run = |pts: &mut Vec<Point>, to: Point, step: f64| {
        if let Some(&from) = pts.last() {
            let d = from.distance(to);
            if d < 1e-9 {
                return;
            }
            let n = ((d / step).ceil() as usize).max(1);
            for k in 1..=n {
                pts.push(from.lerp(to, k as f64 / n as f64));
            }
        } else {
            pts.push(to);
        }
    };

    while remaining > 0 {
        let entry = match cur {
            None => {
                // First section: the entry closest to the hint, or the top-left one.
                let target = hint_rs.unwrap_or_else(|| {
                    let b = polyline::bbox(&rings[0]);
                    Point::new(b.min.x, b.min.y)
                });
                let first = (0..plan.sections.len()).flat_map(|s| plan.entries(s).collect::<Vec<_>>()).min_by(|a, b| {
                    let da = plan.graph.pts[plan.entry_node(*a)].distance(target);
                    let db = plan.graph.pts[plan.entry_node(*b)].distance(target);
                    da.total_cmp(&db)
                });
                let Some(first) = first else { break };
                first
            }
            Some(node) => {
                let (dist, prev) = dijkstra(&plan.graph, node, &sewn);
                let here = plan.graph.pts[node];
                let cost = |e: Entry, dist: &[f64], from: Point| {
                    let n = plan.entry_node(e);
                    if dist[n].is_finite() { dist[n] } else { from.distance(plan.graph.pts[n]) * NO_ROUTE_COST }
                };
                let mut cands: Vec<(f64, Entry)> = (0..plan.sections.len())
                    .filter(|&s| !done[s])
                    .flat_map(|s| plan.entries(s).collect::<Vec<_>>())
                    .map(|e| (cost(e, &dist, here), e))
                    .collect();
                cands.sort_by(|a, b| a.0.total_cmp(&b.0));
                // Look one section ahead for the nearest few sections, trying every
                // entry of each: entering a branch from its far end is often what
                // keeps later travel hidden.
                let k_sections = if remaining <= 1 {
                    1
                } else if remaining <= 30 {
                    4
                } else if remaining <= 100 {
                    2
                } else {
                    1
                };
                let mut chosen_sections: Vec<usize> = Vec::new();
                for (_, e) in &cands {
                    if !chosen_sections.contains(&e.section) {
                        chosen_sections.push(e.section);
                        if chosen_sections.len() == k_sections {
                            break;
                        }
                    }
                }
                let mut best = cands[0];
                if remaining > 1 && k_sections > 1 {
                    let mut best_score = f64::INFINITY;
                    for &(c1, e) in cands.iter().filter(|(_, e)| chosen_sections.contains(&e.section)) {
                        for &s in &plan.sections[e.section] {
                            sewn[s] = true;
                        }
                        let exit_node = plan.exit_node(e);
                        let (d2, _) = dijkstra(&plan.graph, exit_node, &sewn);
                        let from = plan.graph.pts[exit_node];
                        let c2 = (0..plan.sections.len())
                            .filter(|&s| !done[s] && s != e.section)
                            .flat_map(|s| plan.entries(s).collect::<Vec<_>>())
                            .map(|e2| cost(e2, &d2, from))
                            .fold(f64::INFINITY, f64::min);
                        for &s in &plan.sections[e.section] {
                            sewn[s] = false;
                        }
                        let score = c1 + if c2.is_finite() { c2 } else { 0.0 };
                        if score < best_score {
                            best_score = score;
                            best = (c1, e);
                        }
                    }
                }
                let e = best.1;
                let target = plan.entry_node(e);
                match path_to(&prev, node, target) {
                    Some(path) => travel_path(&mut pts, &plan.graph, &path, travel),
                    None => push_run(&mut pts, plan.graph.pts[target], travel),
                }
                e
            }
        };

        // Sew the section back and forth.
        let sec = plan.sections[entry.section].clone();
        let order: Vec<usize> = if entry.from_first { sec.clone() } else { sec.iter().rev().copied().collect() };
        let mut left_to_right = entry.left;
        for (i, &si) in order.iter().enumerate() {
            let s = &plan.spans[si];
            let (xa, xb) = if left_to_right { (s.l.x - pc, s.r.x + pc) } else { (s.r.x + pc, s.l.x - pc) };
            let start = Point::new(xa, s.y);
            if i == 0 && pts.is_empty() {
                pts.push(start);
            } else {
                push_run(&mut pts, start, travel.min(cfg.stitch_length.max(1.0)));
            }
            row_points(xa, xb, s.row, cfg, &mut xs);
            for &x in xs.iter().skip(1) {
                pts.push(Point::new(x, s.y));
            }
            sewn[si] = true;
            left_to_right = !left_to_right;
        }
        done[entry.section] = true;
        remaining -= 1;
        cur = Some(plan.exit_node(entry));
    }

    // Finish near the requested exit point.
    if let (Some(node), Some(ex)) = (cur, exit) {
        let ex_rs = fwd.apply(ex);
        let (_, prev) = dijkstra(&plan.graph, node, &sewn);
        let target = (0..plan.graph.pts.len()).min_by(|&a, &b| plan.graph.pts[a].distance(ex_rs).total_cmp(&plan.graph.pts[b].distance(ex_rs)));
        if let Some(t) = target
            && let Some(path) = path_to(&prev, node, t)
        {
            travel_path(&mut pts, &plan.graph, &path, travel);
        }
    }
    plan.graph.adj.clear();
    pts.into_iter().map(|p| inv.apply(p)).collect()
}

/// Number of sections and rows a straight fill at `angle` would need.
pub(crate) fn statistics(polygon: &Polygon, angle: f64, spacing: f64) -> Option<(usize, usize)> {
    let mut p = polygon.clone();
    p.transform(&Affine::rotate(angle.to_radians()));
    let rings: Vec<Vec<Point>> = p.rings().filter(|r| r.len() >= 3).cloned().collect();
    let cfg = ScanConfig { row_spacing: spacing.max(0.05), ..ScanConfig::default() };
    let plan = plan(&rings, &cfg)?;
    let rows = plan.spans.iter().map(|s| s.row).max().map_or(0, |m| m + 1);
    Some((plan.sections.len(), rows))
}

/// Fills several polygons (e.g. the pieces of a split shape) in nearest-first
/// order, connecting them with runs or trimmed jumps.
pub(crate) fn scan_fill_all(polys: &[Polygon], cfg: &ScanConfig, out: &mut Out, hint: Option<Point>, exit: Option<Point>, trim_threshold: f64) {
    let mut left: Vec<&Polygon> = polys.iter().filter(|p| !p.is_empty()).collect();
    while !left.is_empty() {
        let here = out.pos().or(hint);
        let i = match here {
            Some(h) => (0..left.len()).min_by(|&a, &b| left[a].distance_to_boundary(h).total_cmp(&left[b].distance_to_boundary(h))).unwrap_or(0),
            None => 0,
        };
        let p = left.remove(i);
        let ex = if left.is_empty() { exit } else { None };
        let pts = scan_fill(p, cfg, here, ex);
        if let Some(&first) = pts.first() {
            out.connect(first, trim_threshold, cfg.travel_length);
            out.stitches(pts);
        }
    }
}

/// Clean polygons for filling: resolves self-intersections and drops slivers.
pub(crate) fn prepare(polygon: Polygon) -> Vec<Polygon> {
    poly::clean(&[polygon]).into_iter().filter(|p| p.area() > 0.01).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![Point::new(x0, y0), Point::new(x1, y0), Point::new(x1, y1), Point::new(x0, y1)]
    }

    fn max_stitch(pts: &[Point]) -> f64 {
        pts.windows(2).map(|w| w[0].distance(w[1])).fold(0.0, f64::max)
    }

    #[test]
    fn square_tatami() {
        let poly = Polygon::from_ring(rect(0.0, 0.0, 20.0, 20.0));
        let cfg = ScanConfig { angle: 0.0, ..Default::default() };
        let pts = scan_fill(&poly, &cfg, None, None);
        assert!(pts.len() > 50 * 5, "{}", pts.len());
        assert!(max_stitch(&pts) <= 4.0 + 1.5 + 1e-6, "{}", max_stitch(&pts));
        for p in &pts {
            assert!(p.x >= -1e-6 && p.x <= 20.0 + 1e-6 && p.y >= 0.0 && p.y <= 20.0, "{p:?}");
        }
        // ~50 rows at 0.4 mm
        let mut ys: Vec<i64> = pts.iter().map(|p| (p.y * 1000.0).round() as i64).collect();
        ys.sort();
        ys.dedup();
        assert!((48..=52).contains(&ys.len()), "{}", ys.len());
    }

    #[test]
    fn angled_fill_stays_inside() {
        let poly = Polygon::from_ring(rect(0.0, 0.0, 30.0, 12.0));
        for angle in [0.0, 30.0, 45.0, 90.0, 135.0] {
            let cfg = ScanConfig { angle, ..Default::default() };
            let pts = scan_fill(&poly, &cfg, None, None);
            assert!(!pts.is_empty());
            for p in &pts {
                assert!(p.x > -0.01 && p.x < 30.01 && p.y > -0.01 && p.y < 12.01, "{angle}: {p:?}");
            }
        }
    }

    #[test]
    fn donut_avoids_hole() {
        let outer: Vec<Point> = (0..64)
            .map(|i| {
                let a = i as f64 / 64.0 * std::f64::consts::TAU;
                Point::new(20.0 * a.cos(), 20.0 * a.sin())
            })
            .collect();
        let hole: Vec<Point> = outer.iter().map(|p| *p * 0.4).collect();
        let poly = Polygon::new(outer, vec![hole]);
        let cfg = ScanConfig { angle: 30.0, ..Default::default() };
        let pts = scan_fill(&poly, &cfg, Some(Point::new(-25.0, 0.0)), None);
        assert!(pts.len() > 600, "{}", pts.len());
        for p in &pts {
            assert!(p.length() > 7.9 - 1e-6, "inside hole: {p:?}");
            assert!(p.length() < 20.0 + 1e-6, "outside: {p:?}");
        }
        assert!(max_stitch(&pts) < 6.0);
        // starts near the hint
        assert!(pts[0].distance(Point::new(-20.0, 0.0)) < 3.0, "{:?}", pts[0]);
    }

    #[test]
    fn u_shape_covers_both_arms() {
        // U: two arms connected at the bottom.
        let ring = vec![
            Point::new(0.0, 0.0),
            Point::new(5.0, 0.0),
            Point::new(5.0, 20.0),
            Point::new(15.0, 20.0),
            Point::new(15.0, 0.0),
            Point::new(20.0, 0.0),
            Point::new(20.0, 25.0),
            Point::new(0.0, 25.0),
        ];
        let poly = Polygon::from_ring(ring);
        let cfg = ScanConfig { angle: 0.0, ..Default::default() };
        let pts = scan_fill(&poly, &cfg, None, None);
        let left = pts.iter().filter(|p| p.x < 5.0 && p.y < 19.0).count();
        let right = pts.iter().filter(|p| p.x > 15.0 && p.y < 19.0).count();
        assert!(left > 60 && right > 60, "{left} {right}");
        for p in &pts {
            let in_notch = p.x > 5.05 && p.x < 14.95 && p.y < 19.95;
            assert!(!in_notch, "stitch in the notch: {p:?}");
        }
        assert!(max_stitch(&pts) <= 5.5 + 1e-6, "{}", max_stitch(&pts));
    }

    #[test]
    fn satin_rows_and_split() {
        let poly = Polygon::from_ring(rect(0.0, 0.0, 5.0, 3.0));
        let cfg = ScanConfig { stitch_length: f64::INFINITY, row_spacing: 0.2, ..Default::default() };
        let pts = scan_fill(&poly, &cfg, None, None);
        // one stitch per row (plus connections): every point on an edge
        assert!(pts.iter().all(|p| p.x.abs() < 1e-6 || (p.x - 5.0).abs() < 1e-6));
        let poly = Polygon::from_ring(rect(0.0, 0.0, 20.0, 3.0));
        let cfg = ScanConfig { stitch_length: f64::INFINITY, split_length: 7.0, ..Default::default() };
        let pts = scan_fill(&poly, &cfg, None, None);
        assert!(max_stitch(&pts) <= 7.0 + 1e-6, "{}", max_stitch(&pts));
    }

    #[test]
    fn gradient_rows_spread_out() {
        let poly = Polygon::from_ring(rect(0.0, 0.0, 10.0, 20.0));
        let cfg = ScanConfig { row_spacing: 0.3, row_spacing_end: 2.0, ..Default::default() };
        let pts = scan_fill(&poly, &cfg, None, None);
        let top = pts.iter().filter(|p| p.y < 5.0).count();
        let bottom = pts.iter().filter(|p| p.y > 15.0).count();
        assert!(top > bottom * 2, "{top} {bottom}");
    }
}
