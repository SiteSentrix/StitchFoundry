//! Ink/Stitch designs: Ink/Stitch, an embroidery extension for Inkscape, keeps its stitch
//! settings on the art as `inkstitch:*` attributes, which the SVG import keeps as `inkstitch-*`
//! data on each object. Here they become StitchFoundry stitching: dashed strokes run, other
//! strokes satin lines as wide as the stroke, satin columns (`satin_column`) satin columns
//! between their two rails with their rungs as inclination lines, and filled shapes fills
//! (`fill_method`), with their spacing, lengths, underlay and repeats. A design with any of these
//! settings is an Ink/Stitch design, and Ink/Stitch sews all its art: art without settings of its
//! own gets the defaults the same way. What it does comes from Ink/Stitch's public documentation
//! (https://inkstitch.org/docs/stitches/ and https://inkstitch.org/docs/params/), not its code.

use std::collections::HashSet;

use sf_core::attr::{Embroidery, StitchType};
use sf_core::direction::Guide;
use sf_core::geom::{Point, polyline};
use sf_core::model::{FillKind, FillParams, MAX_PASSES, RunKind, RunParams, SatinParams};
use vectorcraft_doc::{Document, Node, NodeId, NodeKind};
use vectorcraft_geom::path::{PathData, SubPath};

use crate::convert::flatten_subpath;
use crate::{MM_PER_PT, PT_PER_MM};

/// The data key prefix the SVG import gives Ink/Stitch's attributes.
pub const PREFIX: &str = "inkstitch-";
/// Most subpaths a satin column is sorted into rails and rungs from (the file is untrusted).
const MAX_SUBPATHS: usize = 200;

/// Ink/Stitch's settings on `n`, by name.
fn settings(n: &Node) -> Vec<(&str, &str)> {
    n.extras().attrs.as_deref().map_or_else(Vec::new, |a| a.data.iter().filter_map(|(k, v)| Some((k.strip_prefix(PREFIX)?, v.as_str()))).collect())
}

fn get<'a>(s: &[(&str, &'a str)], key: &str) -> Option<&'a str> {
    s.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// A number setting: the first of its values (two give the sides of a satin column).
fn num(s: &[(&str, &str)], key: &str) -> Option<f64> {
    get(s, key)?.split_whitespace().next()?.parse::<f64>().ok().filter(|v| v.is_finite())
}

fn flag(s: &[(&str, &str)], key: &str) -> Option<bool> {
    get(s, key).map(|v| v.eq_ignore_ascii_case("true") || v == "1")
}

/// Whether `doc` is an Ink/Stitch design: some object carries Ink/Stitch settings.
pub fn is_inkstitch(doc: &Document) -> bool {
    let mut found = false;
    doc.walk(|n| found |= n.extras().attrs.as_deref().is_some_and(|a| a.data.iter().any(|(k, _)| k.starts_with(PREFIX))));
    found
}

/// What sewing `n` the way Ink/Stitch would takes: its stitching, and for a satin column the
/// path cut down to its two rails (the rungs become inclination lines).
pub fn stitching(n: &Node) -> Option<(Embroidery, Option<PathData>)> {
    let path = &one_path(n)?;
    let s = settings(n);
    if get(&s, "satin_column").is_some_and(|v| v.eq_ignore_ascii_case("true")) || get(&s, "satin_method").is_some_and(|v| v == "satin_column") {
        return satin_column(path, &s);
    }
    let fill = !matches!(n.appearance.fill_paint(), vectorcraft_color::Paint::None);
    let stroke = n.appearance.stroke().filter(|st| !st.paint.is_none());
    if fill && path.subpaths.iter().any(|sp| sp.closed || sp.anchors.len() >= 3) {
        // Ink/Stitch sews fills at their own angle (0° unless set): no automatic direction
        let mut e = Embroidery::new(StitchType::Fill(fill_params(&s)));
        e.auto_direction = false;
        return Some((e, None));
    }
    let st = stroke?;
    let dashed = st.dash.as_ref().is_some_and(|d| d.pattern.iter().any(|v| *v > 0.0));
    let running = dashed || get(&s, "stroke_method").is_some_and(|m| m != "zigzag_stitch" && m != "satin");
    if running {
        return Some((Embroidery::new(StitchType::Run(run_params(&s))), None));
    }
    // a solid stroke: a satin line as wide as the stroke
    let mut e = Embroidery::new(StitchType::Satin(satin_params(&s)));
    e.width = Some((st.width * MM_PER_PT).clamp(0.3, 20.0));
    Some((e, None))
}

/// The art of `n` sewn as one: a path, or all the pieces of a compound path (an SVG path of
/// several subpaths opens as one, Ink/Stitch's satin columns among them).
fn one_path(n: &Node) -> Option<PathData> {
    match &n.kind {
        NodeKind::Path { path, .. } => Some(path.clone()),
        NodeKind::Compound { children, .. } => {
            let subpaths: Vec<SubPath> = children
                .iter()
                .filter_map(|c| if let NodeKind::Path { path, .. } = &c.kind { Some(path.subpaths.iter().cloned()) } else { None })
                .flatten()
                .collect();
            (!subpaths.is_empty()).then(|| PathData::new(subpaths))
        }
        _ => None,
    }
}

fn run_params(s: &[(&str, &str)]) -> RunParams {
    let mut r = RunParams { stitch_length: num(s, "running_stitch_length_mm").unwrap_or(2.5).clamp(0.3, 12.0), ..RunParams::default() };
    // a bean stitch: each stitch sewn there, back and there again
    if num(s, "bean_stitch_repeats").is_some_and(|n| n >= 1.0) {
        r.kind = RunKind::Triple;
    }
    if let Some(n) = num(s, "repeats") {
        r.passes = (n.round().clamp(1.0, f64::from(MAX_PASSES))) as u32;
    }
    r
}

fn satin_params(s: &[(&str, &str)]) -> SatinParams {
    let mut p = SatinParams::default();
    if let Some(v) = num(s, "zigzag_spacing_mm") {
        p.spacing = v.clamp(0.1, 5.0);
    }
    if let Some(v) = get(s, "pull_compensation_mm") {
        let mut sides = v.split_whitespace().filter_map(|x| x.parse::<f64>().ok().filter(|x| x.is_finite()));
        if let Some(a) = sides.next() {
            p.pull_comp = a.clamp(-2.0, 5.0);
            p.pull_comp_2 = sides.next().map(|b| b.clamp(-2.0, 5.0));
        }
    }
    let (edge, center, zigzag) = (flag(s, "contour_underlay"), flag(s, "center_walk_underlay"), flag(s, "zigzag_underlay"));
    if edge.is_some() || center.is_some() || zigzag.is_some() {
        p.underlay.auto = false;
        p.underlay.edge_run = edge.unwrap_or(false);
        p.underlay.center_run = center.unwrap_or(false);
        p.underlay.zigzag = zigzag.unwrap_or(false);
    }
    if let Some(v) = num(s, "contour_underlay_inset_mm") {
        p.underlay.inset = v.clamp(0.0, 5.0);
    }
    p
}

fn fill_params(s: &[(&str, &str)]) -> FillParams {
    let kind = match get(s, "fill_method") {
        Some("contour_fill") => FillKind::Contour,
        Some("meander_fill") => FillKind::Meander,
        Some("circular_fill") => FillKind::Spiral,
        _ => FillKind::Tatami,
    };
    let mut f = FillParams { kind, angle: num(s, "angle").map_or(0.0, |a| a.rem_euclid(360.0)), ..FillParams::default() };
    if let Some(v) = num(s, "row_spacing_mm") {
        f.spacing = v.clamp(0.1, 10.0);
    }
    if let Some(v) = num(s, "max_stitch_length_mm") {
        f.stitch_length = v.clamp(0.5, 12.0);
    }
    // rows repeat their needle points every `staggers` rows
    if let Some(n) = num(s, "staggers").filter(|n| *n >= 1.0) {
        f.offset = 1.0 / n.round().min(20.0);
    }
    if flag(s, "fill_underlay") == Some(false) {
        f.underlay.tatami = false;
        f.underlay.edge_run = false;
    }
    f
}

/// A satin column: its two rails, the rest rungs, kept as inclination lines where they cross
/// both rails. The rails are the pair of long subpaths the most others cross both of (every rung
/// crosses both rails), the longer pair on a tie; just two subpaths are the rails.
fn satin_column(path: &PathData, s: &[(&str, &str)]) -> Option<(Embroidery, Option<PathData>)> {
    let subs = path.subpaths.get(..path.subpaths.len().min(MAX_SUBPATHS))?;
    let lines: Vec<Vec<Point>> = subs.iter().map(|sp| flatten_subpath(sp).map(|l| l.pts).unwrap_or_default()).collect();
    let boxes: Vec<sf_core::geom::BBox> = lines.iter().map(|l| sf_core::geom::BBox::from_points(l.iter().copied())).collect();
    let crosses = |i: usize, j: usize| {
        let (Some(a), Some(b), Some(ba), Some(bb)) = (lines.get(i), lines.get(j), boxes.get(i), boxes.get(j)) else { return false };
        ba.intersects(bb) && a.windows(2).any(|p| b.windows(2).any(|q| polyline::segment_intersection(p[0], p[1], q[0], q[1]).is_some()))
    };
    let mut by_length: Vec<usize> = (0..lines.len()).filter(|&i| lines.get(i).is_some_and(|l| l.len() >= 2)).collect();
    let length = |i: usize| lines.get(i).map_or(0.0, |l| polyline::length(l));
    by_length.sort_by(|&a, &b| length(b).total_cmp(&length(a)));
    let candidates = by_length.get(..by_length.len().min(6)).unwrap_or_default();
    let mut best: Option<((usize, f64), [usize; 2])> = None;
    for (k, &a) in candidates.iter().enumerate() {
        for &b in candidates.iter().skip(k + 1) {
            let both = by_length.iter().filter(|&&r| r != a && r != b && crosses(r, a) && crosses(r, b)).count();
            let score = (both, length(a) + length(b));
            if best.is_none_or(|(s, _)| score.0 > s.0 || (score.0 == s.0 && score.1 > s.1)) {
                best = Some((score, [a, b]));
            }
        }
    }
    let (_, [a, b]) = best?;
    let mut kept = PathData::new(vec![subs.get(a)?.clone(), subs.get(b)?.clone()]);
    for sp in &mut kept.subpaths {
        sp.closed = false;
    }
    // Ink/Stitch's reverse_rails turns rails round the way they were drawn
    match get(s, "reverse_rails") {
        Some("first") => reverse(&mut kept, 0),
        Some("second") => reverse(&mut kept, 1),
        Some("both") => {
            reverse(&mut kept, 0);
            reverse(&mut kept, 1);
        }
        _ => {}
    }
    let mut e = Embroidery::new(StitchType::Satin(satin_params(s)));
    e.guides = by_length
        .iter()
        .filter(|&&r| r != a && r != b && crosses(r, a) && crosses(r, b))
        .filter_map(|&r| {
            let l = lines.get(r)?;
            Some(Guide::new(*l.first()? * PT_PER_MM, *l.last()? * PT_PER_MM))
        })
        .collect();
    Some((e, Some(kept)))
}

fn reverse(p: &mut PathData, i: usize) {
    if let Some(sp) = p.subpaths.get_mut(i) {
        sp.reverse();
    }
}

/// Stitches the art of an Ink/Stitch design the way Ink/Stitch would (see the module notes): what
/// carries StitchFoundry stitching already keeps it. Nothing happens to a design without Ink/Stitch
/// settings. Returns how many objects it stitched.
pub fn apply(doc: &mut Document) -> usize {
    if !is_inkstitch(doc) {
        return 0;
    }
    stitch_all(doc)
}

/// Stitches all of `doc`'s art the way Ink/Stitch would, whether or not it carries settings (an
/// embroidery font's glyphs); what carries StitchFoundry stitching keeps it. Returns how many
/// objects it stitched.
pub fn stitch_all(doc: &mut Document) -> usize {
    // paths, and compound paths as one (not their pieces)
    let mut ids: Vec<NodeId> = Vec::new();
    let mut pieces: HashSet<NodeId> = HashSet::new();
    doc.walk(|n| match &n.kind {
        NodeKind::Compound { children, .. } => {
            pieces.extend(children.iter().map(|c| c.id));
            if n.embroidery.is_none() {
                ids.push(n.id);
            }
        }
        NodeKind::Path { .. } if n.embroidery.is_none() => ids.push(n.id),
        _ => {}
    });
    ids.retain(|id| !pieces.contains(id));
    let mut count = 0;
    for id in ids {
        let Some(n) = doc.node_mut(id) else { continue };
        let Some((e, rails)) = stitching(n) else { continue };
        if let Some(rails) = rails {
            match &mut n.kind {
                NodeKind::Path { path, .. } => *path = rails,
                // a compound path's pieces become one path of the two rails
                NodeKind::Compound { children, .. } => {
                    let Some(mut kind) = children.iter().find(|c| matches!(c.kind, NodeKind::Path { .. })).map(|c| c.kind.clone()) else {
                        continue;
                    };
                    if let NodeKind::Path { path, .. } = &mut kind {
                        *path = rails;
                    }
                    n.kind = kind;
                }
                _ => continue,
            }
        }
        n.embroidery = Some(Box::new(e));
        count += 1;
    }
    count
}
