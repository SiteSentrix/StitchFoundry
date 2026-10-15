//! Gouraud lighting represented by an ordinary vector linear gradient on each triangle.

use vectorcraft_geom::{Point, Vec2};

/// Document-space gradient placement and the brightness at its two ends. Clipping a
/// face must keep this placement unchanged, so newly cut edges retain their lighting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadeGradient {
    pub start: Point,
    pub end: Point,
    pub brightness: [f64; 2],
}

impl ShadeGradient {
    pub(crate) fn triangle(points: &[Point], values: &[f64]) -> Option<Self> {
        let [p0, p1, p2] = points else { return None };
        let [b0, b1, b2] = values else { return None };
        let u = *p1 - *p0;
        let v = *p2 - *p0;
        let det = u.cross(v);
        if det.abs() < 1e-10 {
            return None;
        }
        // Solve the brightness plane in screen space. Its perpendicular is the gradient
        // vector, so all three corner values and shared triangle edges agree exactly.
        let g = Vec2::new(((b1 - b0) * v.y - (b2 - b0) * u.y) / det, (u.x * (b2 - b0) - v.x * (b1 - b0)) / det);
        let lo = b0.min(*b1).min(*b2);
        let hi = b0.max(*b1).max(*b2);
        if hi - lo < 1e-10 || g.hypot2() < 1e-30 {
            return None;
        }
        let start = *p0 + g * ((lo - b0) / g.hypot2());
        let end = *p0 + g * ((hi - b0) / g.hypot2());
        [start.x, start.y, end.x, end.y].iter().all(|n| n.is_finite()).then_some(Self { start, end, brightness: [lo, hi] })
    }

    /// Brightness at a projected point, including a point introduced by visibility clipping.
    pub fn at(self, p: Point) -> f64 {
        let v = self.end - self.start;
        let t = if v.hypot2() > 0.0 { ((p - self.start).dot(v) / v.hypot2()).clamp(0.0, 1.0) } else { 0.0 };
        self.brightness[0] + (self.brightness[1] - self.brightness[0]) * t
    }
}
