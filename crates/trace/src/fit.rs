//! Loop → Bézier curves, and centre line → Bézier curves.

use vectorcraft_geom::{Anchor, AnchorKind, CubicBez, ParamCurve, PathData, Point, SubPath};
use vectorcraft_pathops::{SimplifyOptions, simplify_with};

use crate::contour::Loop;
use crate::logo::{Cubic, Stats, Style, all_within, fit_outline, resample_closed, sample_cubics};
use crate::subpixel::{self, Crack};

#[derive(Clone, Copy, Debug)]
pub(crate) struct FitOptions {
    pub polygon_tol: f64,
    pub fit_tol: f64,
    pub corner_angle: f64,
    pub snap_lines: bool,
}

fn seg_dist(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let l2 = ab.hypot2();
    if l2 < 1e-18 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / l2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Douglas–Peucker on `pts[a..=b]`, marking kept indices.
fn rdp(pts: &[Point], a: usize, b: usize, tol: f64, keep: &mut [bool]) {
    let mut stack = vec![(a, b)];
    while let Some((a, b)) = stack.pop() {
        if b <= a + 1 {
            continue;
        }
        let (pa, pb) = (pts[a], pts[b % pts.len()]);
        let mut best = (0.0, a);
        for (i, &q) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = seg_dist(q, pa, pb);
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            stack.push((a, best.1));
            stack.push((best.1, b));
        }
    }
}

/// Fit one loop of a `w × h` image into a closed Bézier subpath (pixel coordinates): its points
/// are placed to a fraction of a pixel ([`subpixel::outline`], which explains `shift`) and fitted as
/// Flat Logo mode fits its outlines (corners restored, straight runs as lines, the rest as few
/// curves as `style.tol` allows).
pub(crate) fn fit_loop(l: &Loop, w: usize, h: usize, shift: &dyn Fn(Crack) -> f64, style: &Style) -> Option<SubPath> {
    let (pts, hard) = subpixel::outline(l, w, h, shift);
    let mut style = *style;
    if hard {
        // No anti-aliasing: the edge is only known to about half a pixel (as in Flat Logo mode).
        style.tol *= 2.5;
        style.line_tol *= 1.8;
        style.circle_tol *= 1.8;
        style.sigma = 0.9;
    }
    let mut stats = Stats::default();
    let mut segs = fit_outline(&pts, &style, &mut stats)?;
    // A fit that strayed was replaced by a plain one that nothing has checked: one that strays too
    // (a wild handle on a tiny or ragged loop) gives way to the outline's own points.
    if stats.reverted > 0 && !all_within(&sample_cubics(&segs, 0.25), &resample_closed(&pts, 0.5).map_or_else(|| pts.clone(), |(p, _)| p), 1.5) {
        segs = polygon(&pts, style.tol);
    }
    let n = segs.len();
    // A straight piece has no handles.
    let straight = |c: &Cubic| seg_dist(c[1], c[0], c[3]) < 1e-6 && seg_dist(c[2], c[0], c[3]) < 1e-6;
    let mut anchors = Vec::with_capacity(n);
    for (i, cur) in segs.iter().enumerate() {
        let before = segs.get((i + n - 1) % n)?;
        let p = cur[0];
        let (h_in, h_out) = (if straight(before) { p } else { before[2] }, if straight(cur) { p } else { cur[1] });
        let (vi, vo) = (p - h_in, h_out - p);
        let smooth = vi.hypot() > 1e-9 && vo.hypot() > 1e-9 && vi.cross(vo).abs() < 0.07 * vi.hypot() * vo.hypot() && vi.dot(vo) > 0.0;
        anchors.push(Anchor { p, h_in, h_out, kind: if smooth { AnchorKind::Smooth } else { AnchorKind::Corner } });
    }
    (n >= 2).then(|| SubPath::new(anchors, true))
}

/// The closed outline `pts` as straight pieces within `tol` of it (Douglas–Peucker).
fn polygon(pts: &[Point], tol: f64) -> Vec<Cubic> {
    let n = pts.len();
    let Some(&first) = pts.first() else { return vec![] };
    let far = (1..n).max_by(|&i, &j| pts[i].distance_squared(first).total_cmp(&pts[j].distance_squared(first))).unwrap_or(0);
    let mut ext = pts.to_vec();
    ext.push(first);
    let mut keep = vec![false; n + 1];
    keep[0] = true;
    keep[far] = true;
    rdp(&ext, 0, far, tol, &mut keep);
    rdp(&ext, far, n, tol, &mut keep);
    let kept: Vec<Point> = (0..n).filter(|&i| keep[i]).map(|i| pts[i]).collect();
    (0..kept.len()).map(|i| [kept[i], kept[i], kept[(i + 1) % kept.len()], kept[(i + 1) % kept.len()]]).collect()
}

/// Fit a centre line through `pts` (pixel-centre points in order) into a Bézier subpath: the
/// points within `polygon_tol` of it are dropped (Douglas–Peucker), then curves are fitted. A
/// `closed` line runs back to its first point.
pub(crate) fn fit_polyline(pts: &[Point], closed: bool, o: &FitOptions) -> Option<SubPath> {
    let n = pts.len();
    let first = *pts.first()?;
    if n < 2 {
        return None;
    }
    let mut ext = pts.to_vec();
    let mut keep = vec![false; n + 1];
    keep[0] = true;
    if closed {
        // Split the ring at its first point and the point farthest from it.
        ext.push(first);
        let far = (1..n).max_by(|&i, &j| pts[i].distance_squared(first).total_cmp(&pts[j].distance_squared(first))).unwrap_or(n / 2);
        keep[far] = true;
        rdp(&ext, 0, far, o.polygon_tol, &mut keep);
        rdp(&ext, far, n, o.polygon_tol, &mut keep);
    } else {
        keep[n - 1] = true;
        rdp(&ext, 0, n - 1, o.polygon_tol, &mut keep);
    }
    let mut poly: Vec<Point> = (0..n).filter(|&i| keep[i]).map(|i| pts[i]).collect();
    if closed && poly.len() < 3 {
        poly = pts.to_vec();
    }
    if poly.len() < if closed { 3 } else { 2 } {
        return None;
    }
    let sp = SubPath::polyline(&poly, closed);
    let fitted =
        simplify_with(&PathData::single(sp), &SimplifyOptions { tolerance: o.fit_tol, corner_angle_deg: o.corner_angle, straight_lines: false });
    let mut sp = fitted.subpaths.into_iter().next()?;
    if sp.anchors.len() < 2 {
        return None;
    }
    if o.snap_lines {
        snap_to_lines(&mut sp, (o.fit_tol * 2.0).max(1.5));
    }
    Some(sp)
}

/// Retract the handles of curves that stay within `tol` of their chord.
fn snap_to_lines(sp: &mut SubPath, tol: f64) {
    let n = sp.anchors.len();
    for i in 0..sp.segment_count() {
        let j = (i + 1) % n;
        let (a, b) = (sp.anchors[i], sp.anchors[j]);
        if !a.has_out() && !b.has_in() {
            continue;
        }
        let c = CubicBez::new(a.p, a.h_out, b.h_in, b.p);
        if (1..8).all(|k| seg_dist(c.eval(k as f64 / 8.0), a.p, b.p) <= tol) {
            sp.anchors[i].h_out = a.p;
            sp.anchors[j].h_in = b.p;
        }
    }
    for a in &mut sp.anchors {
        if !a.has_in() || !a.has_out() {
            a.kind = AnchorKind::Corner;
        }
    }
}
