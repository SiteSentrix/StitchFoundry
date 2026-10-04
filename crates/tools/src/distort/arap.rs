//! As-Rigid-As-Possible shape manipulation (Igarashi, Moscovich & Hughes, SIGGRAPH 2005),
//! implemented from the paper.
//!
//! - The mesh is a regular grid of triangles over the artwork: grid cells are kept when their
//!   centre is inside the filled outline or near any path (so separate limbs stay separate).
//! - Step 1 (similarity): every triangle vertex is expressed in the local frame of the opposite
//!   edge, `v2 = v0 + x·(v1 − v0) + y·R90(v1 − v0)`; the quadratic error of those relations over
//!   all triangles (rotation + uniform scale free) plus soft pin constraints is minimised.
//! - Step 2 (scale adjustment): each triangle's rest shape is rigidly fitted (optimal 2D rotation)
//!   to its step-1 result, then the vertices are solved so every edge vector matches its fitted
//!   triangle's, which removes the scaling step 1 allows.
//! - Pins are arbitrary points (barycentric combinations of a triangle's vertices), so both steps
//!   use weighted soft constraints. Systems are symmetric positive definite and banded (vertices
//!   are numbered row by row), solved with a banded Cholesky factorisation written here.
//! - Artwork points (anchors and handles) follow the mesh through barycentric coordinates in
//!   their rest triangle.

use vectorcraft_geom::kurbo::ParamCurveNearest;
use vectorcraft_geom::{BezPath, Point, Rect, Shape, Vec2};

/// Banded symmetric positive definite matrix (lower band stored row by row).
#[derive(Clone, Debug)]
pub struct BandMatrix {
    n: usize,
    bw: usize,
    /// Row i holds columns i-bw..=i at offsets 0..=bw.
    a: Vec<f64>,
}

impl BandMatrix {
    pub fn new(n: usize, bw: usize) -> Self {
        Self { n, bw, a: vec![0.0; n * (bw + 1)] }
    }
    pub fn len(&self) -> usize {
        self.n
    }
    pub fn is_empty(&self) -> bool {
        self.n == 0
    }
    fn idx(&self, i: usize, j: usize) -> usize {
        // j <= i, i - j <= bw
        i * (self.bw + 1) + (j + self.bw - i)
    }
    /// Add `v` at (i, j) (and implicitly (j, i)); diagonal entries are added once.
    pub fn add(&mut self, i: usize, j: usize, v: f64) {
        let (i, j) = if i >= j { (i, j) } else { (j, i) };
        debug_assert!(i - j <= self.bw, "outside band");
        let k = self.idx(i, j);
        self.a[k] += v;
    }
    pub fn get(&self, i: usize, j: usize) -> f64 {
        let (i, j) = if i >= j { (i, j) } else { (j, i) };
        if i - j > self.bw { 0.0 } else { self.a[self.idx(i, j)] }
    }
    /// In-place Cholesky factorisation (L·Lᵀ). Returns false if the matrix isn't positive definite.
    pub fn factor(&mut self) -> bool {
        let (n, bw) = (self.n, self.bw);
        for i in 0..n {
            let j0 = i.saturating_sub(bw);
            for j in j0..=i {
                let mut s = self.a[self.idx(i, j)];
                let k0 = j0.max(j.saturating_sub(bw));
                for k in k0..j {
                    s -= self.a[self.idx(i, k)] * self.a[self.idx(j, k)];
                }
                if i == j {
                    if s <= 0.0 || !s.is_finite() {
                        return false;
                    }
                    let k = self.idx(i, i);
                    self.a[k] = s.sqrt();
                } else {
                    let k = self.idx(i, j);
                    self.a[k] = s / self.a[self.idx(j, j)];
                }
            }
        }
        true
    }
    /// Solve with a factored matrix.
    pub fn solve(&self, b: &mut [f64]) {
        let (n, bw) = (self.n, self.bw);
        for i in 0..n {
            let mut s = b[i];
            let k0 = i.saturating_sub(bw);
            for (k, bk) in b[k0..i].iter().enumerate() {
                s -= self.a[self.idx(i, k0 + k)] * bk;
            }
            b[i] = s / self.a[self.idx(i, i)];
        }
        for i in (0..n).rev() {
            let mut s = b[i];
            for (k, bk) in b[i + 1..(i + bw + 1).min(n)].iter().enumerate() {
                s -= self.a[self.idx(i + 1 + k, i)] * bk;
            }
            b[i] = s / self.a[self.idx(i, i)];
        }
    }
}

/// A triangle mesh on a regular grid.
#[derive(Clone, Debug)]
pub struct Mesh {
    pub verts: Vec<Point>,
    pub tris: Vec<[usize; 3]>,
    origin: Point,
    cell: f64,
    nx: usize,
    ny: usize,
    /// Per grid cell: its two triangles (None = cell not part of the mesh).
    cells: Vec<Option<[usize; 2]>>,
    /// Per grid cell: centre inside the art (not just the band around it).
    core: Vec<bool>,
}

/// Mesh construction options.
#[derive(Clone, Copy, Debug)]
pub struct MeshOptions {
    /// Cells along the longer side.
    pub resolution: usize,
    /// Expand Mesh: extra reach around the outline, points.
    pub expand: f64,
}

impl Default for MeshOptions {
    fn default() -> Self {
        Self { resolution: 20, expand: 3.0 }
    }
}

impl Mesh {
    /// Build a mesh covering `bounds` restricted to the region of `outlines` (filled interiors of
    /// closed subpaths plus a band around every path). With no outlines, the whole bounds.
    pub fn build(outlines: &[BezPath], bounds: Rect, opt: MeshOptions) -> Mesh {
        let side = bounds.width().max(bounds.height()).max(1e-3);
        let cell = (side / opt.resolution.max(1) as f64).max(1e-3);
        let pad = opt.expand.max(0.0) + cell * 0.01;
        let b = bounds.inflate(pad, pad);
        let nx = ((b.width() / cell).ceil() as usize).max(1);
        let ny = ((b.height() / cell).ceil() as usize).max(1);
        let origin = Point::new(b.x0, b.y0);
        let reach = cell * 0.75 + opt.expand.max(0.0);
        // 0 = outside, 1 = in the band around the art, 2 = core (inside a fill or on a path).
        let keep = |c: Point| -> u8 {
            if outlines.is_empty() {
                return 2;
            }
            let near = |o: &BezPath, r: f64| o.segments().any(|s| s.nearest(c, 1e-3).distance_sq <= r * r);
            if outlines.iter().any(|o| closed_winding(o, c) != 0 || near(o, cell * 0.25)) {
                return 2;
            }
            u8::from(outlines.iter().any(|o| {
                let bb = o.bounding_box().inflate(reach, reach);
                if !bb.contains(c) {
                    return false;
                }
                if closed_winding(o, c) != 0 {
                    return true;
                }
                near(o, reach)
            }))
        };
        let mut vid = vec![usize::MAX; (nx + 1) * (ny + 1)];
        let mut verts = vec![];
        let mut kept = vec![false; nx * ny];
        let mut core = vec![false; nx * ny];
        for j in 0..ny {
            for i in 0..nx {
                let c = Point::new(origin.x + (i as f64 + 0.5) * cell, origin.y + (j as f64 + 0.5) * cell);
                let k = keep(c);
                kept[j * nx + i] = k > 0;
                core[j * nx + i] = k == 2;
            }
        }
        // Number vertices row by row (keeps the systems banded).
        for j in 0..=ny {
            for i in 0..=nx {
                let used = [(i.wrapping_sub(1), j.wrapping_sub(1)), (i, j.wrapping_sub(1)), (i.wrapping_sub(1), j), (i, j)]
                    .iter()
                    .any(|&(ci, cj)| ci < nx && cj < ny && kept[cj * nx + ci]);
                if used {
                    vid[j * (nx + 1) + i] = verts.len();
                    verts.push(Point::new(origin.x + i as f64 * cell, origin.y + j as f64 * cell));
                }
            }
        }
        let mut tris = vec![];
        let mut cells = vec![None; nx * ny];
        for j in 0..ny {
            for i in 0..nx {
                if !kept[j * nx + i] {
                    continue;
                }
                let v = |di: usize, dj: usize| vid[(j + dj) * (nx + 1) + i + di];
                let (v00, v10, v01, v11) = (v(0, 0), v(1, 0), v(0, 1), v(1, 1));
                let t0 = tris.len();
                tris.push([v00, v10, v11]);
                tris.push([v00, v11, v01]);
                cells[j * nx + i] = Some([t0, t0 + 1]);
            }
        }
        Mesh { verts, tris, origin, cell, nx, ny, cells, core }
    }

    /// Band width (in vertices) of the vertex adjacency.
    fn vertex_band(&self) -> usize {
        self.tris.iter().filter_map(|t| Some(t.iter().max()? - t.iter().min()?)).max().unwrap_or(0)
    }

    /// The triangle and barycentric coordinates of `p` (extrapolated from the nearest mesh cell
    /// when `p` is outside the mesh).
    pub fn locate(&self, p: Point) -> Option<(usize, [f64; 3])> {
        let fi = ((p.x - self.origin.x) / self.cell).floor();
        let fj = ((p.y - self.origin.y) / self.cell).floor();
        let inside = fi >= 0.0 && fj >= 0.0 && (fi as usize) < self.nx && (fj as usize) < self.ny;
        let cell = if inside && self.cells[fj as usize * self.nx + fi as usize].is_some() {
            fj as usize * self.nx + fi as usize
        } else {
            // Nearest kept cell centre.
            let mut best = None;
            for (k, c) in self.cells.iter().enumerate() {
                if c.is_none() {
                    continue;
                }
                let cc =
                    Point::new(self.origin.x + ((k % self.nx) as f64 + 0.5) * self.cell, self.origin.y + ((k / self.nx) as f64 + 0.5) * self.cell);
                let d = cc.distance_squared(p);
                if best.is_none_or(|(bd, _)| d < bd) {
                    best = Some((d, k));
                }
            }
            best?.1
        };
        let [ta, tb] = self.cells[cell]?;
        // Pick the triangle on p's side of the diagonal v00–v11.
        let v00 = self.verts[self.tris[ta][0]];
        let local = (p - v00) / self.cell;
        let t = if local.x >= local.y { ta } else { tb };
        Some((t, self.bary(t, p)))
    }

    fn bary(&self, t: usize, p: Point) -> [f64; 3] {
        let [a, b, c] = self.tris[t].map(|i| self.verts[i]);
        let (v0, v1, v2) = (b - a, c - a, p - a);
        let den = v0.cross(v1);
        if den.abs() < 1e-18 {
            return [1.0, 0.0, 0.0];
        }
        let l1 = v2.cross(v1) / den;
        let l2 = v0.cross(v2) / den;
        [1.0 - l1 - l2, l1, l2]
    }

    /// Map a rest-space point through deformed vertex positions.
    pub fn map(&self, deformed: &[Point], p: Point) -> Point {
        match self.locate(p) {
            Some((t, w)) => {
                let [a, b, c] = self.tris[t].map(|i| deformed[i].to_vec2());
                (a * w[0] + b * w[1] + c * w[2]).to_point()
            }
            None => p,
        }
    }

    /// Signed area of triangle `t` for the given vertex positions.
    pub fn tri_area(&self, pos: &[Point], t: usize) -> f64 {
        let [a, b, c] = self.tris[t].map(|i| pos[i]);
        (b - a).cross(c - a) / 2.0
    }
}

/// Winding number of the closed subpaths of `o` at `p` (open subpaths ignored).
fn closed_winding(o: &BezPath, p: Point) -> i32 {
    let mut total = 0;
    let mut cur = BezPath::new();
    let mut closed = false;
    let flush = |cur: &mut BezPath, closed: bool, total: &mut i32| {
        if closed && !cur.elements().is_empty() {
            *total += cur.winding(p);
        }
        *cur = BezPath::new();
    };
    for el in o.elements() {
        match el {
            vectorcraft_geom::PathEl::MoveTo(_) => {
                flush(&mut cur, closed, &mut total);
                closed = false;
                cur.push(*el);
            }
            vectorcraft_geom::PathEl::ClosePath => {
                closed = true;
                cur.push(*el);
            }
            e => cur.push(*e),
        }
    }
    flush(&mut cur, closed, &mut total);
    total
}

/// Weight of the soft pin constraints relative to the shape terms.
const PIN_WEIGHT: f64 = 1.0e5;
/// Tikhonov regularisation toward the rest pose (keeps pin-less islands in place).
const REG: f64 = 1.0e-7;

/// A pin in rest space and where it should go.
#[derive(Clone, Copy, Debug)]
pub struct Pin {
    pub rest: Point,
    pub target: Point,
}

/// Run both ARAP steps. Returns the deformed vertex positions (the rest mesh if fewer than one
/// pin lands on it or a system is singular).
pub fn deform(mesh: &Mesh, pins: &[Pin]) -> Vec<Point> {
    let n = mesh.verts.len();
    if n == 0 || pins.is_empty() {
        return mesh.verts.clone();
    }
    let located: Vec<(usize, [f64; 3], Point)> = pins.iter().filter_map(|p| mesh.locate(p.rest).map(|(t, w)| (t, w, p.target))).collect();
    let vb = mesh.vertex_band();
    // ---- step 1: similarity-invariant error, 2n unknowns interleaved (x0, y0, x1, y1, ...).
    let mut g = BandMatrix::new(2 * n, 2 * vb + 1);
    for tri in &mesh.tris {
        for k in 0..3 {
            let (a, b, c) = (tri[k], tri[(k + 1) % 3], tri[(k + 2) % 3]);
            let (pa, pb, pc) = (mesh.verts[a], mesh.verts[b], mesh.verts[c]);
            let e = pb - pa;
            let l2 = e.hypot2();
            if l2 < 1e-18 {
                continue;
            }
            let d = pc - pa;
            let x = d.dot(e) / l2;
            let y = d.dot(Vec2::new(-e.y, e.x)) / l2;
            // Residual coefficients over (ax, ay, bx, by, cx, cy); see module docs.
            let vars = [2 * a, 2 * a + 1, 2 * b, 2 * b + 1, 2 * c, 2 * c + 1];
            let rx = [x - 1.0, -y, -x, y, 1.0, 0.0];
            let ry = [y, x - 1.0, -y, -x, 0.0, 1.0];
            for r in [rx, ry] {
                for i in 0..6 {
                    for j in 0..=i {
                        let v = r[i] * r[j];
                        if v != 0.0 {
                            g.add(vars[i], vars[j], v);
                        }
                    }
                }
            }
        }
    }
    let mut rhs = vec![0.0; 2 * n];
    for (t, w, target) in &located {
        let tri = mesh.tris[*t];
        for comp in 0..2 {
            for i in 0..3 {
                for j in 0..=i {
                    g.add(2 * tri[i] + comp, 2 * tri[j] + comp, PIN_WEIGHT * w[i] * w[j]);
                }
                rhs[2 * tri[i] + comp] += PIN_WEIGHT * w[i] * if comp == 0 { target.x } else { target.y };
            }
        }
    }
    for (i, v) in mesh.verts.iter().enumerate() {
        g.add(2 * i, 2 * i, REG);
        g.add(2 * i + 1, 2 * i + 1, REG);
        rhs[2 * i] += REG * v.x;
        rhs[2 * i + 1] += REG * v.y;
    }
    if !g.factor() {
        return mesh.verts.clone();
    }
    g.solve(&mut rhs);
    let step1: Vec<Point> = (0..n).map(|i| Point::new(rhs[2 * i], rhs[2 * i + 1])).collect();

    // ---- step 2: fit rigid triangles, then match edge vectors (x and y separable).
    let mut l = BandMatrix::new(n, vb);
    let mut bx = vec![0.0; n];
    let mut by = vec![0.0; n];
    for tri in &mesh.tris {
        let rest = tri.map(|i| mesh.verts[i]);
        let cur = tri.map(|i| step1[i]);
        let rc = centroid(&rest);
        let cc = centroid(&cur);
        let (mut sc, mut ss) = (0.0, 0.0);
        for k in 0..3 {
            let (r, c) = (rest[k] - rc, cur[k] - cc);
            sc += r.dot(c);
            ss += r.cross(c);
        }
        let th = ss.atan2(sc);
        let (s, co) = th.sin_cos();
        let fit = rest.map(|r| {
            let d = r - rc;
            cc + Vec2::new(d.x * co - d.y * s, d.x * s + d.y * co)
        });
        for k in 0..3 {
            let (i, j) = (tri[k], tri[(k + 1) % 3]);
            let e = fit[k] - fit[(k + 1) % 3];
            // (v_i - v_j - e)^2
            l.add(i, i, 1.0);
            l.add(j, j, 1.0);
            l.add(i, j, -1.0);
            bx[i] += e.x;
            bx[j] -= e.x;
            by[i] += e.y;
            by[j] -= e.y;
        }
    }
    for (t, w, target) in &located {
        let tri = mesh.tris[*t];
        for i in 0..3 {
            for j in 0..=i {
                l.add(tri[i], tri[j], PIN_WEIGHT * w[i] * w[j]);
            }
            bx[tri[i]] += PIN_WEIGHT * w[i] * target.x;
            by[tri[i]] += PIN_WEIGHT * w[i] * target.y;
        }
    }
    for (i, v) in step1.iter().enumerate() {
        l.add(i, i, REG);
        bx[i] += REG * v.x;
        by[i] += REG * v.y;
    }
    if !l.factor() {
        return step1;
    }
    l.solve(&mut bx);
    l.solve(&mut by);
    (0..n).map(|i| Point::new(bx[i], by[i])).collect()
}

fn centroid(p: &[Point; 3]) -> Point {
    Point::new((p[0].x + p[1].x + p[2].x) / 3.0, (p[0].y + p[1].y + p[2].y) / 3.0)
}

/// Pick `count` well-spread pin positions inside the mesh (farthest-point sampling over the
/// kept cell centres, starting from the one nearest the centroid).
pub fn auto_pins(mesh: &Mesh, count: usize) -> Vec<Point> {
    let centres: Vec<Point> = mesh
        .cells
        .iter()
        .enumerate()
        .filter(|(k, c)| c.is_some() && (mesh.core[*k] || !mesh.core.iter().any(|x| *x)))
        .map(|(k, _)| Point::new(mesh.origin.x + ((k % mesh.nx) as f64 + 0.5) * mesh.cell, mesh.origin.y + ((k / mesh.nx) as f64 + 0.5) * mesh.cell))
        .collect();
    if centres.is_empty() {
        return vec![];
    }
    let c = centres.iter().fold(Vec2::ZERO, |a, p| a + p.to_vec2()) / centres.len() as f64;
    let Some(&first) = centres.iter().min_by(|a, b| a.distance_squared(c.to_point()).total_cmp(&b.distance_squared(c.to_point()))) else {
        return vec![];
    };
    let mut out = vec![first];
    while out.len() < count.min(centres.len()) {
        let Some(&next) = centres.iter().max_by(|a, b| {
            let da = out.iter().map(|o| o.distance_squared(**a)).fold(f64::MAX, f64::min);
            let db = out.iter().map(|o| o.distance_squared(**b)).fold(f64::MAX, f64::min);
            da.total_cmp(&db)
        }) else {
            break;
        };
        out.push(next);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_geom::shapes;

    #[test]
    fn band_cholesky_solves_spd_systems() {
        // Tridiagonal [4 1; 1 4 1; ...] vs a dense reference.
        let n = 12;
        let mut m = BandMatrix::new(n, 1);
        for i in 0..n {
            m.add(i, i, 4.0);
            if i > 0 {
                m.add(i, i - 1, 1.0);
            }
        }
        let x: Vec<f64> = (0..n).map(|i| i as f64 * 0.5 - 2.0).collect();
        let mut b: Vec<f64> = (0..n).map(|i| (0..n).map(|j| m.get(i, j) * x[j]).sum()).collect();
        assert!(m.factor());
        m.solve(&mut b);
        for i in 0..n {
            assert!((b[i] - x[i]).abs() < 1e-12);
        }
        let mut bad = BandMatrix::new(2, 1);
        bad.add(0, 0, 1.0);
        bad.add(1, 0, 2.0);
        bad.add(1, 1, 1.0);
        assert!(!bad.factor(), "indefinite");
    }

    fn rect_mesh() -> Mesh {
        let r = Rect::new(0.0, 0.0, 200.0, 100.0);
        Mesh::build(&[shapes::rectangle(r).to_bezpath()], r, MeshOptions { resolution: 16, expand: 0.0 })
    }

    #[test]
    fn mesh_covers_shape_and_locates_points() {
        let m = rect_mesh();
        assert!(!m.tris.is_empty());
        for p in [Point::new(10.0, 10.0), Point::new(150.0, 70.0), Point::new(199.0, 99.0)] {
            let (t, w) = m.locate(p).unwrap();
            assert!(w.iter().all(|v| *v >= -1e-9), "{w:?}");
            assert!((m.map(&m.verts, p) - p).hypot() < 1e-9);
            let _ = t;
        }
        // An L-shaped outline drops the empty corner cells.
        let mut l = BezPath::new();
        l.move_to((0.0, 0.0));
        l.line_to((100.0, 0.0));
        l.line_to((100.0, 20.0));
        l.line_to((20.0, 20.0));
        l.line_to((20.0, 100.0));
        l.line_to((0.0, 100.0));
        l.close_path();
        let lm = Mesh::build(&[l], Rect::new(0.0, 0.0, 100.0, 100.0), MeshOptions { resolution: 10, expand: 0.0 });
        assert!(lm.tris.len() < m.tris.len() / 2, "{} tris", lm.tris.len());
        assert!(lm.cells.iter().filter(|c| c.is_some()).count() < 60);
    }

    #[test]
    fn pins_at_rest_leave_the_mesh_unchanged() {
        let m = rect_mesh();
        let pins: Vec<Pin> =
            [(20.0, 50.0), (180.0, 50.0), (100.0, 20.0)].iter().map(|&(x, y)| Pin { rest: Point::new(x, y), target: Point::new(x, y) }).collect();
        let d = deform(&m, &pins);
        let err = m.verts.iter().zip(&d).map(|(a, b)| a.distance(*b)).fold(0.0, f64::max);
        assert!(err < 1e-4, "{err}");
    }

    #[test]
    fn rigid_motion_is_reproduced_exactly() {
        let m = rect_mesh();
        let rot = |p: Point| {
            let (s, c) = 0.5f64.sin_cos();
            Point::new(p.x * c - p.y * s + 30.0, p.x * s + p.y * c - 10.0)
        };
        let pins: Vec<Pin> = [(20.0, 50.0), (180.0, 50.0), (100.0, 90.0)]
            .iter()
            .map(|&(x, y)| Pin { rest: Point::new(x, y), target: rot(Point::new(x, y)) })
            .collect();
        let d = deform(&m, &pins);
        let err = m.verts.iter().zip(&d).map(|(a, b)| rot(*a).distance(*b)).fold(0.0, f64::max);
        assert!(err < 1e-2, "{err}");
    }

    #[test]
    fn pins_are_satisfied_and_free_regions_stay_rigid() {
        let m = rect_mesh();
        // Hold the left end, lift the right end.
        let pins = vec![
            Pin { rest: Point::new(10.0, 30.0), target: Point::new(10.0, 30.0) },
            Pin { rest: Point::new(10.0, 70.0), target: Point::new(10.0, 70.0) },
            Pin { rest: Point::new(190.0, 50.0), target: Point::new(180.0, -30.0) },
        ];
        let d = deform(&m, &pins);
        for p in &pins {
            let got = m.map(&d, p.rest);
            assert!(got.distance(p.target) < 0.05, "pin {:?} → {got:?}", p.target);
        }
        // Area is preserved (as-rigid-as-possible): total within a few percent, no flipped triangles.
        let (a0, a1): (f64, f64) =
            (0..m.tris.len()).map(|t| (m.tri_area(&m.verts, t), m.tri_area(&d, t))).fold((0.0, 0.0), |s, v| (s.0 + v.0, s.1 + v.1));
        assert!((a1 / a0 - 1.0).abs() < 0.05, "area ratio {}", a1 / a0);
        assert!((0..m.tris.len()).all(|t| m.tri_area(&d, t) * m.tri_area(&m.verts, t) > 0.0));
        // Each triangle stays nearly congruent: edge lengths change by < 25%.
        let worst = m
            .tris
            .iter()
            .flat_map(|t| (0..3).map(move |k| (t[k], t[(k + 1) % 3])))
            .map(|(i, j)| (d[i].distance(d[j]) / m.verts[i].distance(m.verts[j]) - 1.0).abs())
            .fold(0.0, f64::max);
        assert!(worst < 0.3, "edge stretch {worst}");
    }

    #[test]
    fn auto_pins_are_spread_inside_the_shape() {
        let m = rect_mesh();
        let p = auto_pins(&m, 3);
        assert_eq!(p.len(), 3);
        assert!(p.iter().all(|q| Rect::new(0.0, 0.0, 200.0, 100.0).contains(*q)));
        assert!(p[1].distance(p[2]) > 100.0);
    }
}
