//! Special fills: contour, spiral, radial and cross stitch.

use super::out::Out;
use super::run::CORNER_ANGLE;
use crate::geom::{Affine, Point, Polygon, poly, polyline};

/// Rings of the polygon offset inwards by `spacing/2 + k·spacing`, grouped by level
/// (outermost first).
fn inset_levels(polygon: &Polygon, spacing: f64, max_levels: usize) -> Vec<Vec<Vec<Point>>> {
    let mut levels = Vec::new();
    let mut d = spacing * 0.5;
    for _ in 0..max_levels {
        let ins = if d < 1e-9 { vec![polygon.clone()] } else { poly::offset(std::slice::from_ref(polygon), -d) };
        if ins.is_empty() {
            break;
        }
        let rings: Vec<Vec<Point>> = ins
            .iter()
            .flat_map(|p| p.rings().cloned().collect::<Vec<_>>())
            .filter(|r| r.len() >= 3 && polyline::ring_length(r) > spacing * 1.5)
            .collect();
        if rings.is_empty() {
            break;
        }
        levels.push(rings);
        d += spacing;
    }
    levels
}

/// Contour fill: concentric rows following the outline, outside in.
pub(crate) fn contour(polygon: &Polygon, spacing: f64, step: f64, out: &mut Out, hint: Option<Point>, trim: f64) {
    let levels = inset_levels(polygon, spacing.max(0.2), 2000);
    for rings in levels {
        let mut left = rings;
        while !left.is_empty() {
            let here = out.pos().or(hint).unwrap_or(left[0][0]);
            let i = (0..left.len())
                .min_by(|&a, &b| polyline::distance_to_ring(&left[a], here).total_cmp(&polyline::distance_to_ring(&left[b], here)))
                .unwrap_or(0);
            let ring = left.remove(i);
            let start = polyline::ring_starting_near(&ring, here);
            let pts = polyline::resample_ring(&start, step, CORNER_ANGLE);
            if let Some(&p0) = pts.first() {
                out.connect(p0, trim.max(spacing * 3.0), step);
                out.stitches(pts);
            }
        }
    }
}

/// Re-parameterises a closed ring (explicitly closed) by normalised arc length.
fn ring_at(ring: &[Point], cum: &[f64], t: f64) -> Point {
    let total = *cum.last().unwrap_or(&0.0);
    let d = t.rem_euclid(1.0) * total;
    let i = cum.partition_point(|&c| c <= d).clamp(1, ring.len() - 1);
    let seg = cum[i] - cum[i - 1];
    if seg < 1e-12 { ring[i] } else { ring[i - 1].lerp(ring[i], (d - cum[i - 1]) / seg) }
}

/// Spiral fill: one continuous spiral from the outline inwards. Falls back to
/// contour fill for shapes whose insets split or have holes.
pub(crate) fn spiral(polygon: &Polygon, spacing: f64, step: f64, out: &mut Out, hint: Option<Point>, trim: f64) {
    let spacing = spacing.max(0.2);
    let levels = inset_levels(polygon, spacing, 2000);
    if polygon.holes.iter().any(|h| h.len() >= 3) || levels.iter().any(|l| l.len() != 1) || levels.len() < 2 {
        contour(polygon, spacing, step, out, hint, trim);
        return;
    }
    let here = out.pos().or(hint).unwrap_or(levels[0][0][0]);
    let mut rings: Vec<Vec<Point>> = Vec::new();
    let mut anchor = here;
    for l in &levels {
        let r = polyline::closed(&polyline::ring_starting_near(&l[0], anchor));
        anchor = r[0];
        rings.push(r);
    }
    let cums: Vec<Vec<f64>> = rings.iter().map(|r| polyline::cumulative_lengths(r)).collect();
    let mut pts = Vec::new();
    for k in 0..rings.len() {
        let len = *cums[k].last().unwrap_or(&0.0);
        let n = ((len / step).ceil() as usize).max(8);
        let last = k + 1 == rings.len();
        for i in 0..n {
            let t = i as f64 / n as f64;
            let a = ring_at(&rings[k], &cums[k], t);
            let p = if last { a } else { a.lerp(ring_at(&rings[k + 1], &cums[k + 1], t), t) };
            pts.push(p);
        }
        if last {
            pts.push(ring_at(&rings[k], &cums[k], 0.0));
        }
    }
    if let Some(&p0) = pts.first() {
        out.connect(p0, trim, step);
        out.stitches(pts);
    }
}

/// A point inside the polygon near its centroid.
fn interior_point(polygon: &Polygon) -> Point {
    let c = polygon.centroid();
    if polygon.contains(c) {
        return c;
    }
    // Middle of the widest horizontal span through the centroid's height.
    let y = c.y;
    let mut xs: Vec<f64> = Vec::new();
    for ring in polygon.rings() {
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                xs.push(a.x + (y - a.y) / (b.y - a.y) * (b.x - a.x));
            }
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.chunks_exact(2).max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0]))).map(|s| Point::new((s[0] + s[1]) * 0.5, y)).unwrap_or(c)
}

/// Radial fill: satin-like rays between the outline and a centre. Ray lengths
/// alternate so the centre does not get perforated.
pub(crate) fn radial(polygon: &Polygon, center: Option<Point>, spacing: f64, split: f64, out: &mut Out, trim: f64) {
    let c = center.filter(|c| polygon.contains(*c)).unwrap_or_else(|| interior_point(polygon));
    let outer = polyline::closed(&polygon.outer);
    let perim = polyline::length(&outer);
    // `spacing` is measured between penetrations on the outline (same side).
    let n = ((perim / (spacing.max(0.1))).ceil() as usize).max(8);
    const REACH: [f64; 4] = [0.04, 0.55, 0.3, 0.7];
    let mut pts = Vec::with_capacity(n * 2 + 2);
    // Start at the outline point nearest the current position.
    let start = out.pos().and_then(|p| polyline::project(&outer, p)).map(|pr| pr.along).unwrap_or(0.0);
    for i in 0..=n {
        let s = (start + perim * i as f64 / n as f64) % perim;
        let b = polyline::point_at(&outer, s);
        let inner = c.lerp(b, REACH[i % REACH.len()]);
        if i % 2 == 0 {
            pts.push(inner);
            pts.push(b);
        } else {
            pts.push(b);
            pts.push(inner);
        }
    }
    if let Some(&p0) = pts.first() {
        out.connect(p0, trim, 2.5);
    }
    let mut prev = pts[0];
    for (i, &p) in pts.iter().enumerate().skip(1) {
        let d = prev.distance(p);
        if split > 0.0 && d > split {
            let k = (d / (split * 0.75)).ceil() as usize;
            let shift = [0.0, 0.3, -0.25][i % 3] / k as f64;
            for m in 1..k {
                out.stitch(prev.lerp(p, (m as f64 / k as f64 + shift).clamp(0.05, 0.95)));
            }
        }
        out.stitch(p);
        prev = p;
    }
}

/// Cross stitch on a square grid (rotated by `angle`).
pub(crate) fn cross(polygon: &Polygon, cell: f64, angle: f64, out: &mut Out, trim: f64) {
    let cell = cell.clamp(1.0, 10.0);
    let fwd = Affine::rotate(angle.to_radians());
    let inv = Affine::rotate(-angle.to_radians());
    let mut p = polygon.clone();
    p.transform(&fwd);
    let bb = p.bbox();
    let cols = ((bb.width() / cell).ceil() as i64).max(1);
    let rows = ((bb.height() / cell).ceil() as i64).max(1);
    let x0 = bb.min.x - (cols as f64 * cell - bb.width()) * 0.5;
    let y0 = bb.min.y - (rows as f64 * cell - bb.height()) * 0.5;
    let corner = |c: i64, r: i64| inv.apply(Point::new(x0 + c as f64 * cell, y0 + r as f64 * cell));
    for r in 0..rows {
        // contiguous runs of cells whose centres are inside
        let mut runs: Vec<(i64, i64)> = Vec::new();
        let mut c = 0;
        while c < cols {
            let inside = |c: i64| p.contains(Point::new(x0 + (c as f64 + 0.5) * cell, y0 + (r as f64 + 0.5) * cell));
            if inside(c) {
                let s = c;
                while c < cols && inside(c) {
                    c += 1;
                }
                runs.push((s, c - 1));
            } else {
                c += 1;
            }
        }
        if r % 2 == 1 {
            runs.reverse();
        }
        for (a, b) in runs {
            // forward: "/" diagonals, return: "\" diagonals
            let bl = |c: i64| corner(c, r + 1);
            let tl = |c: i64| corner(c, r);
            out.connect(bl(a), trim, 2.5);
            for c in a..=b {
                out.stitch(tl(c + 1));
                out.stitch(bl(c + 1));
            }
            for c in (a..=b).rev() {
                out.stitch(tl(c));
                out.stitch(bl(c));
            }
        }
    }
}

/// Deterministic xorshift generator (no external RNG, same result everywhere).
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// Uniform in `[0, 1)`.
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Meander fill: a random maze is grown through the shape on a grid of
/// `2·spacing` cells, and the stitch path runs around it, giving one continuous,
/// wandering line with corridors `spacing` mm apart (quilting "stipple" look).
pub(crate) fn meander(polygon: &Polygon, spacing: f64, step: f64, random: f64, out: &mut Out, hint: Option<Point>, trim: f64) {
    let c = spacing.max(0.8);
    let cell = 2.0 * c;
    let inset = poly::offset(std::slice::from_ref(polygon), -c * 0.5);
    let bb = polygon.bbox();
    let cols = ((bb.width() / cell).ceil() as usize).clamp(1, 600);
    let rows = ((bb.height() / cell).ceil() as usize).clamp(1, 600);
    let origin = bb.min;
    let center = |i: usize, j: usize| Point::new(origin.x + (i as f64 + 0.5) * cell, origin.y + (j as f64 + 0.5) * cell);
    let inside: Vec<bool> = (0..cols * rows)
        .map(|k| {
            let p = center(k % cols, k / cols);
            inset.iter().any(|q| q.contains(p))
        })
        .collect();
    if !inside.iter().any(|&b| b) {
        contour(polygon, c, step, out, hint, trim);
        return;
    }
    // Random spanning forest by depth-first search.
    let seed = (bb.min.x * 1000.0) as i64 as u64 ^ ((bb.min.y * 1000.0) as i64 as u64).rotate_left(21) ^ (cols * 7919 + rows) as u64;
    let mut rng = Rng::new(seed);
    let mut visited = vec![false; cols * rows];
    // fine grid: node (i,j) -> (2i, 2j); edge cells in between
    let fw = 2 * cols + 1;
    let fh = 2 * rows + 1;
    let mut fine = vec![false; fw * fh];
    for start in 0..cols * rows {
        if !inside[start] || visited[start] {
            continue;
        }
        visited[start] = true;
        fine[(2 * (start / cols)) * fw + 2 * (start % cols)] = true;
        let mut stack = vec![start];
        while let Some(&cur) = stack.last() {
            let (i, j) = (cur % cols, cur / cols);
            let mut nb: Vec<usize> = Vec::with_capacity(4);
            if i > 0 {
                nb.push(cur - 1);
            }
            if i + 1 < cols {
                nb.push(cur + 1);
            }
            if j > 0 {
                nb.push(cur - cols);
            }
            if j + 1 < rows {
                nb.push(cur + cols);
            }
            nb.retain(|&n| inside[n] && !visited[n]);
            if nb.is_empty() {
                stack.pop();
                continue;
            }
            let next = nb[(rng.next_u64() % nb.len() as u64) as usize];
            visited[next] = true;
            let (ni, nj) = (next % cols, next / cols);
            fine[(2 * nj) * fw + 2 * ni] = true;
            fine[(j + nj) * fw + (i + ni)] = true;
            stack.push(next);
        }
    }
    // Boundary of the occupied fine cells: one closed loop per tree.
    let occ = |u: i64, v: i64| u >= 0 && v >= 0 && (u as usize) < fw && (v as usize) < fh && fine[v as usize * fw + u as usize];
    let mut next_of: std::collections::HashMap<(i64, i64), (i64, i64)> = std::collections::HashMap::new();
    for v in 0..fh as i64 {
        for u in 0..fw as i64 {
            if !occ(u, v) {
                continue;
            }
            // edges walk around the cell keeping it on the right (screen coordinates)
            if !occ(u, v - 1) {
                next_of.insert((u, v), (u + 1, v));
            }
            if !occ(u + 1, v) {
                next_of.insert((u + 1, v), (u + 1, v + 1));
            }
            if !occ(u, v + 1) {
                next_of.insert((u + 1, v + 1), (u, v + 1));
            }
            if !occ(u - 1, v) {
                next_of.insert((u, v + 1), (u, v));
            }
        }
    }
    let to_design = |(u, v): (i64, i64), rng: &mut Rng| {
        let j = random.clamp(0.0, 1.0) * c * 0.3;
        let (dx, dy) = ((rng.unit() - 0.5) * j, (rng.unit() - 0.5) * j);
        Point::new(origin.x + (u as f64) * c + c * 0.5 + dx, origin.y + (v as f64) * c + c * 0.5 + dy)
    };
    let mut loops: Vec<Vec<Point>> = Vec::new();
    let mut keys: Vec<(i64, i64)> = next_of.keys().copied().collect();
    keys.sort();
    let mut used = std::collections::HashSet::new();
    for k in keys {
        if used.contains(&k) {
            continue;
        }
        let mut ring = Vec::new();
        let mut cur = k;
        while used.insert(cur) {
            ring.push(to_design(cur, &mut rng));
            match next_of.get(&cur) {
                Some(&n) => cur = n,
                None => break,
            }
        }
        if ring.len() >= 4 {
            loops.push(ring);
        }
    }
    // Round the maze corners (Chaikin) for a flowing line.
    let smooth = |ring: &[Point]| {
        let mut r = ring.to_vec();
        for _ in 0..2 {
            let n = r.len();
            let mut s = Vec::with_capacity(n * 2);
            for i in 0..n {
                let (a, b) = (r[i], r[(i + 1) % n]);
                s.push(a.lerp(b, 0.25));
                s.push(a.lerp(b, 0.75));
            }
            r = s;
        }
        r
    };
    let mut loops: Vec<Vec<Point>> = loops.iter().map(|l| smooth(l)).collect();
    while !loops.is_empty() {
        let here = out.pos().or(hint).unwrap_or(loops[0][0]);
        let i = (0..loops.len())
            .min_by(|&a, &b| polyline::distance_to_ring(&loops[a], here).total_cmp(&polyline::distance_to_ring(&loops[b], here)))
            .unwrap_or(0);
        let ring = loops.remove(i);
        let start = polyline::ring_starting_near(&ring, here);
        let pts = polyline::resample_ring(&start, step.max(0.5), CORNER_ANGLE);
        if let Some(&p0) = pts.first() {
            out.connect(p0, trim, step);
            out.stitches(pts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::Cmd;

    fn square(s: f64) -> Polygon {
        Polygon::from_ring(vec![Point::new(0.0, 0.0), Point::new(s, 0.0), Point::new(s, s), Point::new(0.0, s)])
    }

    #[test]
    fn contour_rings_inside() {
        let mut out = Out::new();
        contour(&square(20.0), 1.0, 2.5, &mut out, None, 2.0);
        let st = out.finish();
        assert!(st.len() > 100);
        for s in st.iter().filter(|s| s.cmd == Cmd::Stitch) {
            assert!(s.x >= 0.0 && s.x <= 20.0 && s.y >= 0.0 && s.y <= 20.0);
        }
    }

    #[test]
    fn spiral_is_continuous() {
        let mut out = Out::new();
        spiral(&square(20.0), 1.0, 2.5, &mut out, None, 2.0);
        let st = out.finish();
        assert!(st.iter().all(|s| s.cmd == Cmd::Stitch));
        for w in st.windows(2) {
            assert!(w[0].pos().distance(w[1].pos()) < 3.5);
        }
    }

    #[test]
    fn meander_is_one_continuous_line_inside() {
        let poly = square(30.0);
        let mut out = Out::new();
        meander(&poly, 2.0, 2.5, 0.5, &mut out, None, 2.0);
        let st = out.finish();
        assert!(st.len() > 150, "{}", st.len());
        assert!(st.iter().all(|s| s.cmd == Cmd::Stitch), "a square is one tree: no trims");
        for s in &st {
            assert!(s.x > -0.5 && s.x < 30.5 && s.y > -0.5 && s.y < 30.5, "{s:?}");
        }
        for w in st.windows(2) {
            assert!(w[0].pos().distance(w[1].pos()) < 3.2);
        }
    }

    #[test]
    fn radial_and_cross() {
        let mut out = Out::new();
        radial(&square(20.0), None, 0.8, 7.0, &mut out, 2.0);
        let st = out.finish();
        assert!(st.len() > 100);
        for w in st.windows(2) {
            assert!(w[0].pos().distance(w[1].pos()) <= 7.0 + 1e-6);
        }
        let mut out = Out::new();
        cross(&square(12.0), 3.0, 0.0, &mut out, 2.0);
        let st = out.finish();
        // 4×4 cells, each 4 diagonal stitches → 64 + connections
        assert!(st.len() >= 64, "{}", st.len());
    }
}
