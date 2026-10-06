//! Perspective Grid: the grid model, plane homographies, overlays, and the Perspective Grid
//! (Shift+P) and Perspective Selection (Shift+V) tools.
//!
//! The grid is stored in the document (`Document.unknown["perspectiveGrid"]`) as a
//! [`PerspectiveGrid`]. Every plane is a homography `H` from plane coordinates `(u, v)` in points
//! to the page, built from homogeneous columns: `H = [c1 c2 O]` where `O` is the ground-level
//! origin (the corner where the planes meet) and `c1`/`c2` are the images of the plane axes'
//! points at infinity — a vanishing point scaled by `1/distance` for receding axes, a direction
//! for axes parallel to the picture plane. Along a receding axis `u ↦ O + (VP − O)·u/(u + d)`,
//! the projective foreshortening of a camera `d` points from the origin.
//!
//! - 1-point: Left = the flat front plane, Right = the side wall receding to `vpLeft`, Ground
//!   recedes to `vpLeft` too.
//! - 2-point: Left recedes to `vpLeft`, Right to `vpRight`, verticals stay vertical; Ground spans
//!   both vanishing points.
//! - 3-point: as 2-point, but verticals converge to `vpVertical`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{Point, Rect};

use crate::{Action, Cursor, Mods, Overlay, PointerEvent, PointerKind, Tool, ToolContext};

pub mod define;
pub use define::{GridDefinition, Rgb, Station};

/// Key under `Document.unknown`.
pub const DOC_KEY: &str = "perspectiveGrid";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Plane {
    #[default]
    Left,
    Right,
    Ground,
    /// No active plane (widget key 4).
    None,
}

impl Plane {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "left" => Self::Left,
            "right" => Self::Right,
            "ground" | "horizontal" => Self::Ground,
            "none" => Self::None,
            _ => return None,
        })
    }
    pub fn id(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Ground => "ground",
            Self::None => "none",
        }
    }
    /// Illustrator's plane colours: left blue, right orange, ground green.
    pub fn color(self) -> [u8; 3] {
        match self {
            Self::Left => [0x33, 0x66, 0xff],
            Self::Right => [0xff, 0x8c, 0x1a],
            Self::Ground => [0x2e, 0xb8, 0x4a],
            Self::None => [0x99, 0x99, 0x99],
        }
    }
    pub const ALL: [Plane; 3] = [Plane::Left, Plane::Right, Plane::Ground];
}

/// Plane Switching widget: (plane, face quad) list and the "no plane" circle (centre, radius).
pub type WidgetGeom = (Vec<(Plane, [Point; 4])>, (Point, f64));

/// A 3×3 projective transform (row-major).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Homography(pub [[f64; 3]; 3]);

impl Homography {
    /// From the images of (1,0,0), (0,1,0) and (0,0,1).
    pub fn from_cols(c1: [f64; 3], c2: [f64; 3], c3: [f64; 3]) -> Self {
        Self([[c1[0], c2[0], c3[0]], [c1[1], c2[1], c3[1]], [c1[2], c2[2], c3[2]]])
    }
    /// Map a point; None when it lands on or beyond the horizon (w ≤ 0).
    pub fn apply(&self, p: Point) -> Option<Point> {
        let m = &self.0;
        let x = m[0][0] * p.x + m[0][1] * p.y + m[0][2];
        let y = m[1][0] * p.x + m[1][1] * p.y + m[1][2];
        let w = m[2][0] * p.x + m[2][1] * p.y + m[2][2];
        if w <= 1e-9 || !x.is_finite() || !y.is_finite() {
            return None;
        }
        Some(Point::new(x / w, y / w))
    }
    pub fn det(&self) -> f64 {
        let m = &self.0;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }
    /// Inverse.
    pub fn inverse(&self) -> Option<Self> {
        let m = &self.0;
        let d = self.det();
        if d.abs() < 1e-15 {
            return None;
        }
        let c = |r0: usize, c0: usize, r1: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
        let adj = [
            [c(1, 1, 2, 2), -c(0, 1, 2, 2), c(0, 1, 1, 2)],
            [-c(1, 0, 2, 2), c(0, 0, 2, 2), -c(0, 0, 1, 2)],
            [c(1, 0, 2, 1), -c(0, 0, 2, 1), c(0, 0, 1, 1)],
        ];
        let mut out = [[0.0; 3]; 3];
        for (r, row) in adj.iter().enumerate() {
            for (k, v) in row.iter().enumerate() {
                out[r][k] = v / d;
            }
        }
        // H⁻¹·(x, y, 1) = (u, v, 1)/w_H, so points in front of the camera keep w > 0.
        Some(Self(out))
    }
}

fn two() -> u8 {
    2
}
fn yes() -> bool {
    true
}

/// The perspective grid definition (Define Grid) plus view state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerspectiveGrid {
    /// 1, 2 or 3-point perspective.
    #[serde(default = "two")]
    pub kind: u8,
    /// Ground-level origin (where the planes meet), page coordinates.
    pub origin: [f64; 2],
    /// Horizon height (page y).
    pub horizon: f64,
    /// Left vanishing point x (the single vanishing point in 1-point perspective).
    pub vp_left: f64,
    /// Right vanishing point x.
    pub vp_right: f64,
    /// Third (vertical) vanishing point, 3-point only.
    pub vp_vertical: [f64; 2],
    /// Viewing distance (points): how fast receding axes foreshorten.
    pub distance: f64,
    /// Gridline every (points, in plane units).
    pub cell: f64,
    /// Horizontal extent of the planes (points, plane units).
    pub extent: f64,
    /// Vertical extent of the wall planes (points, plane units).
    pub height: f64,
    #[serde(default = "yes")]
    pub visible: bool,
    /// Active plane (Plane Switching widget).
    #[serde(default)]
    pub plane: Plane,
    /// Objects attached to planes (node id → plane).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub attached: BTreeMap<String, Plane>,
    /// The preset the grid came from ("" = custom).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub name: String,
    /// Viewing angle in degrees (two/three-point): where the station point stands between the
    /// vanishing points ([`Station`]). None for grids made before it: they foreshorten every axis
    /// by `distance`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub angle: Option<f64>,
    /// The unit Define Grid measures in (a unit name, e.g. `points`, `inches`).
    #[serde(default = "define::points", skip_serializing_if = "define::is_points")]
    pub units: String,
    /// Scale: `[artboard, real world]` lengths (Define Grid's real-world lengths over this).
    #[serde(default = "define::one_to_one", skip_serializing_if = "define::is_one_to_one")]
    pub scale: [f64; 2],
    /// Gridline colours of the left, right and horizontal (ground) planes.
    #[serde(default = "define::left_rgb")]
    pub left_color: Rgb,
    #[serde(default = "define::right_rgb")]
    pub right_color: Rgb,
    #[serde(default = "define::ground_rgb")]
    pub ground_color: Rgb,
    /// Gridline opacity, 0–100 %.
    #[serde(default = "define::half")]
    pub opacity: f64,
}

impl PerspectiveGrid {
    /// Illustrator-like preset for `kind` fitted to an artboard.
    pub fn preset(kind: u8, ab: Rect) -> Self {
        let (w, h) = (ab.width().max(1.0), ab.height().max(1.0));
        let kind = kind.clamp(1, 3);
        let horizon = ab.y0 + 0.42 * h;
        let ground = ab.y0 + 0.78 * h;
        let ox = if kind == 1 { ab.x0 + 0.3 * w } else { ab.x0 + 0.5 * w };
        let (vl, vr) = if kind == 1 { (ab.x0 + 0.55 * w, ab.x1) } else { (ab.x0 + 0.02 * w, ab.x1 - 0.02 * w) };
        Self {
            kind,
            origin: [ox, ground],
            horizon,
            vp_left: vl,
            vp_right: vr,
            vp_vertical: [ox, horizon - 3.0 * (ground - horizon)],
            distance: 0.5 * w,
            cell: (w / 30.0).max(1.0).round(),
            extent: 0.6 * w,
            height: (ground - ab.y0) * 0.8,
            visible: true,
            plane: Plane::Left,
            attached: BTreeMap::new(),
            name: String::new(),
            angle: None,
            units: define::points(),
            scale: define::one_to_one(),
            left_color: define::left_rgb(),
            right_color: define::right_rgb(),
            ground_color: define::ground_rgb(),
            opacity: define::half(),
        }
    }

    /// The document's grid, if one was defined.
    pub fn from_doc(doc: &Document) -> Option<Self> {
        doc.unknown.get(DOC_KEY).and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// The document's grid, or the default two-point preset for the first artboard (hidden).
    pub fn effective(doc: &Document) -> Self {
        Self::from_doc(doc).unwrap_or_else(|| Self { visible: false, ..Self::normal(2, define::first_artboard(doc)) })
    }

    /// Store into the document.
    pub fn store(&self, doc: &mut Document) {
        let mut g = self.clone();
        g.attached.retain(|k, _| k.parse::<u64>().ok().is_some_and(|id| doc.node(NodeId(id)).is_some()));
        if let Ok(v) = serde_json::to_value(&g) {
            doc.unknown.insert(DOC_KEY.into(), v);
        }
    }

    /// Definition only (for tool previews and the Define Grid dialog).
    pub fn definition_json(&self) -> Value {
        let mut v = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(o) = v.as_object_mut() {
            o.remove("attached");
        }
        v
    }

    /// Merge `patch` (any subset of the fields) into this grid.
    pub fn merged(&self, patch: &Value) -> Result<Self, String> {
        let mut v = serde_json::to_value(self).map_err(|e| e.to_string())?;
        if let (Some(o), Some(p)) = (v.as_object_mut(), patch.as_object()) {
            for (k, val) in p {
                if k == "ids" || k == "attached" {
                    continue;
                }
                o.insert(k.clone(), val.clone());
            }
        }
        let mut g: Self = serde_json::from_value(v).map_err(|e| e.to_string())?;
        g.reconcile(self, patch);
        g.validate()?;
        Ok(g)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !(1..=3).contains(&self.kind) {
            return Err("kind must be 1, 2 or 3".into());
        }
        let nums = [
            self.origin[0],
            self.origin[1],
            self.horizon,
            self.vp_left,
            self.vp_right,
            self.vp_vertical[0],
            self.vp_vertical[1],
            self.distance,
            self.cell,
            self.extent,
            self.height,
        ];
        if nums.iter().any(|v| !v.is_finite() || v.abs() > 4.0e6) {
            return Err("grid values must be finite".into());
        }
        if self.distance <= 0.0 || self.cell <= 0.0 || self.extent <= 0.0 || self.height <= 0.0 {
            return Err("distance, cell, extent and height must be positive".into());
        }
        if (self.origin[1] - self.horizon).abs() < 1e-6 {
            return Err("ground level must differ from the horizon".into());
        }
        self.validate_definition()
    }

    pub fn planes(&self) -> [Plane; 3] {
        Plane::ALL
    }

    /// The homography of `plane` (plane coordinates in points → page).
    pub fn homography(&self, plane: Plane) -> Option<Homography> {
        let (ll, lr, lu) = self.foreshortening();
        let o = [self.origin[0], self.origin[1], 1.0];
        let vl = [self.vp_left * ll, self.horizon * ll, ll];
        let vr = [self.vp_right * lr, self.horizon * lr, lr];
        let up = if self.kind == 3 { [self.vp_vertical[0] * lu, self.vp_vertical[1] * lu, lu] } else { [0.0, -1.0, 0.0] };
        let flat = [1.0, 0.0, 0.0];
        let (c1, c2) = match (self.kind, plane) {
            (_, Plane::None) => return None,
            (1, Plane::Left) => (flat, [0.0, -1.0, 0.0]),
            (1, Plane::Right) => (vl, [0.0, -1.0, 0.0]),
            (1, Plane::Ground) => (flat, vl),
            (_, Plane::Left) => (vl, up),
            (_, Plane::Right) => (vr, up),
            (_, Plane::Ground) => (vr, vl),
        };
        let h = Homography::from_cols(c1, c2, o);
        (h.det().abs() > 1e-15).then_some(h)
    }

    /// Plane-coordinate extent of a plane.
    pub fn domain(&self, plane: Plane) -> Rect {
        match plane {
            Plane::Ground => Rect::new(0.0, 0.0, self.extent, self.extent),
            _ => Rect::new(0.0, 0.0, self.extent, self.height),
        }
    }

    /// Page → plane coordinates.
    pub fn to_plane(&self, plane: Plane, p: Point) -> Option<Point> {
        self.homography(plane)?.inverse()?.apply(p)
    }
    /// Plane → page coordinates.
    pub fn to_page(&self, plane: Plane, q: Point) -> Option<Point> {
        self.homography(plane)?.apply(q)
    }

    /// The map that puts flat art with bounds `b` onto `plane`: an axis-aligned affine map sends
    /// `b` to the plane rectangle whose opposite corners project to `b`'s top-left and
    /// bottom-right, then the plane homography projects it (so a drawn rectangle keeps the two
    /// corners the user dragged between).
    pub fn attach_map(&self, plane: Plane, b: Rect) -> Option<impl Fn(Point) -> Option<Point> + use<>> {
        let h = self.homography(plane)?;
        let hi = h.inverse()?;
        let a = hi.apply(Point::new(b.x0, b.y0))?;
        let c = hi.apply(Point::new(b.x1, b.y1))?;
        let sx = if b.width().abs() > 1e-9 { (c.x - a.x) / b.width() } else { 1.0 };
        let sy = if b.height().abs() > 1e-9 { (c.y - a.y) / b.height() } else { sx.abs() * if c.y < a.y { -1.0 } else { 1.0 } };
        let sx = if b.width().abs() > 1e-9 { sx } else { sy.abs() };
        let (x0, y0) = (b.x0, b.y0);
        Some(move |p: Point| h.apply(Point::new(a.x + (p.x - x0) * sx, a.y + (p.y - y0) * sy)))
    }

    /// The map that slides art lying on `plane` by the plane-space offset between `from` and `to`.
    pub fn move_map(&self, plane: Plane, from: Point, to: Point) -> Option<impl Fn(Point) -> Option<Point> + use<>> {
        let h = self.homography(plane)?;
        let hi = h.inverse()?;
        let d = hi.apply(to)? - hi.apply(from)?;
        Some(move |p: Point| h.apply(hi.apply(p)? + d))
    }

    pub fn attached_plane(&self, id: NodeId) -> Option<Plane> {
        self.attached.get(&id.0.to_string()).copied()
    }

    /// Grid lines of `plane` (page space).
    pub fn lines(&self, plane: Plane) -> Vec<(Point, Point)> {
        let Some(h) = self.homography(plane) else { return vec![] };
        let d = self.domain(plane);
        let mut out = vec![];
        let step = |len: f64| {
            let mut s = self.cell;
            while len / s > 120.0 {
                s *= 2.0;
            }
            s
        };
        let (su, sv) = (step(d.width()), step(d.height()));
        let mut u = 0.0;
        while u <= d.width() + 1e-9 {
            if let (Some(a), Some(b)) = (h.apply(Point::new(u, 0.0)), h.apply(Point::new(u, d.height()))) {
                out.push((a, b));
            }
            u += su;
        }
        let mut v = 0.0;
        while v <= d.height() + 1e-9 {
            if let (Some(a), Some(b)) = (h.apply(Point::new(0.0, v)), h.apply(Point::new(d.width(), v))) {
                out.push((a, b));
            }
            v += sv;
        }
        out
    }

    /// Plane Switching widget geometry: (plane, quad) faces and the "no plane" circle (centre, r).
    pub fn widget(&self, doc: &Document, tol: f64) -> WidgetGeom {
        let ab = doc.artboards.first().map(|a| a.rect).unwrap_or(Rect::new(0.0, 0.0, 612.0, 792.0));
        let s = 40.0 * tol;
        let (cx, cy) = (ab.x0 + 10.0 * tol + s / 2.0, ab.y0 + 10.0 * tol + s / 2.0);
        let p = |dx: f64, dy: f64| Point::new(cx + dx * s, cy + dy * s);
        let faces = vec![
            (Plane::Left, [p(-0.5, -0.25), p(0.0, 0.0), p(0.0, 0.5), p(-0.5, 0.25)]),
            (Plane::Right, [p(0.0, 0.0), p(0.5, -0.25), p(0.5, 0.25), p(0.0, 0.5)]),
            (Plane::Ground, [p(0.0, -0.5), p(0.5, -0.25), p(0.0, 0.0), p(-0.5, -0.25)]),
        ];
        (faces, (p(0.62, 0.62), 0.12 * s))
    }

    /// Which widget control is at `p`.
    pub fn widget_hit(&self, doc: &Document, tol: f64, p: Point) -> Option<Plane> {
        let (faces, (c, r)) = self.widget(doc, tol);
        if p.distance(c) <= r * 1.5 {
            return Some(Plane::None);
        }
        faces.into_iter().find(|(_, q)| in_quad(q, p)).map(|(pl, _)| pl)
    }
}

fn in_quad(q: &[Point; 4], p: Point) -> bool {
    let mut sign = 0.0;
    for i in 0..4 {
        let (a, b) = (q[i], q[(i + 1) % 4]);
        let c = (b - a).cross(p - a);
        if c.abs() < 1e-12 {
            continue;
        }
        if sign == 0.0 {
            sign = c.signum();
        } else if c.signum() != sign {
            return false;
        }
    }
    true
}

/// Grid overlays (lines, horizon, vanishing points, widget) drawn whenever the grid is visible or
/// a perspective tool is active. `tol` = document units per screen pixel.
pub fn grid_overlays(doc: &Document, tol: f64, tool_id: &str) -> Vec<Overlay> {
    let g = PerspectiveGrid::effective(doc);
    let tool = matches!(tool_id, "perspectiveGrid" | "perspectiveSelection");
    if !g.visible && !tool {
        return vec![];
    }
    let mut out = vec![];
    for pl in g.planes() {
        let color = pl.color();
        for (a, b) in g.lines(pl) {
            out.push(Overlay::Line { a, b, color, dashed: false });
        }
        if pl == g.plane
            && let Some(h) = g.homography(pl)
        {
            let d = g.domain(pl);
            let corners: Vec<Point> =
                [(d.x0, d.y0), (d.x1, d.y0), (d.x1, d.y1), (d.x0, d.y1)].iter().filter_map(|&(x, y)| h.apply(Point::new(x, y))).collect();
            if corners.len() == 4 {
                out.push(Overlay::Path { path: crate::xform::polygon(&corners, true), color, width: 2.0, dashed: false });
            }
        }
    }
    let xs = [g.vp_left, g.vp_right, g.origin[0]];
    let (x0, x1) = (xs.iter().cloned().fold(f64::MAX, f64::min) - 50.0 * tol, xs.iter().cloned().fold(f64::MIN, f64::max) + 50.0 * tol);
    out.push(Overlay::Line { a: Point::new(x0, g.horizon), b: Point::new(x1, g.horizon), color: [0x60, 0x60, 0x60], dashed: true });
    // Widget.
    let (faces, (c, r)) = g.widget(doc, tol);
    for (pl, q) in faces {
        let [cr, cg, cb] = pl.color();
        out.push(Overlay::Highlight { quad: q, color: [cr, cg, cb, if pl == g.plane { 230 } else { 70 }] });
        out.push(Overlay::Path { path: crate::xform::polygon(&q, true), color: [0x40, 0x40, 0x40], width: 1.0, dashed: false });
    }
    out.push(Overlay::Path {
        path: super::ellipse_path(c, r, r, 0.0),
        color: if g.plane == Plane::None { [0x20, 0x20, 0x20] } else { [0x99, 0x99, 0x99] },
        width: 1.5,
        dashed: false,
    });
    out
}

// ---------- Perspective Grid tool ----------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Handle {
    VpLeft,
    VpRight,
    VpVertical,
    Horizon,
    Origin,
    ExtentLeft,
    ExtentRight,
    Height,
}

fn handles(g: &PerspectiveGrid) -> Vec<(Handle, Point)> {
    let mut v = vec![(Handle::VpLeft, Point::new(g.vp_left, g.horizon))];
    if g.kind != 1 {
        v.push((Handle::VpRight, Point::new(g.vp_right, g.horizon)));
    }
    if g.kind == 3 {
        v.push((Handle::VpVertical, Point::new(g.vp_vertical[0], g.vp_vertical[1])));
    }
    v.push((Handle::Origin, Point::new(g.origin[0], g.origin[1])));
    let (lp, rp) = (Plane::Left, Plane::Right);
    if let Some(p) = g.to_page(lp, Point::new(g.extent, 0.0)) {
        v.push((Handle::ExtentLeft, p));
    }
    if let Some(p) = g.to_page(rp, Point::new(g.extent, 0.0)) {
        v.push((Handle::ExtentRight, p));
    }
    if let Some(p) = g.to_page(Plane::Left, Point::new(0.0, g.height)) {
        v.push((Handle::Height, p));
    }
    // The horizon handle sits on the horizon above the origin.
    v.push((Handle::Horizon, Point::new(g.origin[0], g.horizon)));
    v
}

/// Apply a handle drag to a copy of the grid.
fn drag_handle(g: &PerspectiveGrid, h: Handle, p: Point) -> PerspectiveGrid {
    let mut n = g.clone();
    match h {
        Handle::VpLeft => {
            n.vp_left = p.x;
            n.horizon = p.y;
        }
        Handle::VpRight => {
            n.vp_right = p.x;
            n.horizon = p.y;
        }
        Handle::VpVertical => n.vp_vertical = [p.x, p.y],
        Handle::Horizon => n.horizon = p.y,
        Handle::Origin => n.origin = [p.x, p.y],
        Handle::ExtentLeft | Handle::ExtentRight | Handle::Height => {
            let pl = if h == Handle::ExtentRight { Plane::Right } else { Plane::Left };
            if let Some(q) = g.to_plane(pl, p) {
                match h {
                    Handle::Height => n.height = q.y.max(g.cell),
                    _ => n.extent = q.x.max(g.cell),
                }
            }
        }
    }
    if n.validate().is_ok() { n } else { g.clone() }
}

#[derive(Default)]
pub struct PerspectiveGridTool {
    drag: Option<Handle>,
}

impl Tool for PerspectiveGridTool {
    fn id(&self) -> &'static str {
        "perspectiveGrid"
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        let g = PerspectiveGrid::effective(cx.doc);
        match ev.kind {
            PointerKind::Down => {
                let mut pre = vec![];
                if !g.visible {
                    pre.push(Action::Exec("perspective.grid.show".into(), json!({"visible": true})));
                }
                if let Some(pl) = g.widget_hit(cx.doc, cx.tol(1.0), p) {
                    pre.push(Action::Exec("perspective.plane.set".into(), json!({"plane": pl.id()})));
                    return pre;
                }
                let tol = cx.tol(6.0);
                if let Some((h, _)) =
                    handles(&g).into_iter().filter(|(_, q)| q.distance(p) <= tol).min_by(|a, b| a.1.distance(p).total_cmp(&b.1.distance(p)))
                {
                    self.drag = Some(h);
                    pre.push(Action::Begin("Edit Perspective Grid".into()));
                }
                pre
            }
            PointerKind::Drag => match self.drag {
                Some(h) => vec![Action::Preview("perspective.grid.set".into(), drag_handle(&g, h, p).definition_json())],
                None => vec![],
            },
            PointerKind::Up => match self.drag.take() {
                Some(_) => vec![Action::Commit],
                None => vec![],
            },
            _ => vec![],
        }
    }
    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        let g = PerspectiveGrid::effective(cx.doc);
        handles(&g)
            .into_iter()
            .map(|(h, p)| match h {
                Handle::VpLeft | Handle::VpRight | Handle::VpVertical => Overlay::Anchor { p, color: [0x20, 0x20, 0x20], filled: true, size: 7.0 },
                _ => Overlay::Anchor { p, color: [0x20, 0x20, 0x20], filled: false, size: 6.0 },
            })
            .collect()
    }
    fn cursor(&self, cx: &ToolContext, p: Point, _mods: Mods) -> Cursor {
        let g = PerspectiveGrid::effective(cx.doc);
        let tol = cx.tol(6.0);
        match handles(&g).into_iter().find(|(_, q)| q.distance(p) <= tol).map(|h| h.0) {
            Some(Handle::Horizon | Handle::Height) => Cursor::ResizeV,
            Some(Handle::ExtentLeft | Handle::ExtentRight) => Cursor::ResizeH,
            Some(_) => Cursor::Move,
            None => Cursor::Arrow,
        }
    }
    fn busy(&self) -> bool {
        self.drag.is_some()
    }
    fn deactivate(&mut self, _cx: &ToolContext) -> Vec<Action> {
        if self.drag.take().is_some() { vec![Action::Commit] } else { vec![] }
    }
}

// ---------- Perspective Selection tool ----------

#[derive(Default)]
pub struct PerspectiveSelectionTool {
    drag: Option<(Vec<NodeId>, Point)>,
}

impl Tool for PerspectiveSelectionTool {
    fn id(&self) -> &'static str {
        "perspectiveSelection"
    }
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action> {
        let p = ev.pos;
        let g = PerspectiveGrid::effective(cx.doc);
        match ev.kind {
            PointerKind::Down => {
                if g.visible
                    && let Some(pl) = g.widget_hit(cx.doc, cx.tol(1.0), p)
                {
                    return vec![Action::Exec("perspective.plane.set".into(), json!({"plane": pl.id()}))];
                }
                let Some(h) = vectorcraft_doc::hit::hit_test(cx.doc, p, cx.hit_options()) else {
                    return vec![Action::Exec("select.set".into(), json!({"ids": []}))];
                };
                let top = h.top_object(cx.isolation);
                let mut acts = vec![];
                let ids = if cx.selection.objects.contains(&top) {
                    cx.selection.objects.clone()
                } else {
                    acts.push(Action::Exec("select.set".into(), json!({"ids": [top.0]})));
                    vec![top]
                };
                acts.push(Action::Begin("Move in Perspective".into()));
                self.drag = Some((ids, p));
                acts
            }
            PointerKind::Drag => match &self.drag {
                Some((ids, from)) => {
                    let mut v = json!({"ids": crate::json_ids(ids), "from": [from.x, from.y], "to": [p.x, p.y]});
                    if g.plane != Plane::None {
                        v["plane"] = json!(g.plane.id());
                    }
                    vec![Action::Preview("perspective.move".into(), v)]
                }
                None => vec![],
            },
            PointerKind::Up => match self.drag.take() {
                Some(_) => vec![Action::Commit],
                None => vec![],
            },
            _ => vec![],
        }
    }
    fn overlays(&self, cx: &ToolContext) -> Vec<Overlay> {
        // Attached objects in the selection get their plane's colour outline.
        let g = PerspectiveGrid::effective(cx.doc);
        let mut out = vec![];
        for id in &cx.selection.objects {
            if let (Some(pl), Some(n)) = (g.attached_plane(*id), cx.doc.node(*id))
                && let Some(b) = n.geometric_bounds()
            {
                out.push(Overlay::Path {
                    path: crate::xform::polygon(&crate::xform::rect_corners(b), true),
                    color: pl.color(),
                    width: 1.0,
                    dashed: true,
                });
            }
        }
        out
    }
    fn cursor(&self, _cx: &ToolContext, _p: Point, _mods: Mods) -> Cursor {
        Cursor::Arrow
    }
    fn busy(&self) -> bool {
        self.drag.is_some()
    }
    fn deactivate(&mut self, _cx: &ToolContext) -> Vec<Action> {
        if self.drag.take().is_some() { vec![Action::Commit] } else { vec![] }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use vectorcraft_doc::Selection;

    fn grid(kind: u8) -> PerspectiveGrid {
        PerspectiveGrid::preset(kind, Rect::new(0.0, 0.0, 800.0, 600.0))
    }

    #[test]
    fn homography_maps_plane_corners_to_the_vanishing_points() {
        for kind in [1u8, 2, 3] {
            let g = grid(kind);
            for pl in Plane::ALL {
                let h = g.homography(pl).unwrap();
                // The plane origin is the ground-level origin.
                let o = h.apply(Point::new(0.0, 0.0)).unwrap();
                assert!((o - Point::new(g.origin[0], g.origin[1])).hypot() < 1e-9, "{kind} {pl:?}");
                // Far along a receding axis, points approach the vanishing point on the horizon.
                let far = h.apply(Point::new(1e9, 0.0)).unwrap();
                let expect = match (kind, pl) {
                    (1, Plane::Left | Plane::Ground) => None,
                    (1, Plane::Right) => Some(Point::new(g.vp_left, g.horizon)),
                    (_, Plane::Left) => Some(Point::new(g.vp_left, g.horizon)),
                    _ => Some(Point::new(g.vp_right, g.horizon)),
                };
                if let Some(e) = expect {
                    assert!((far - e).hypot() < 1e-3, "{kind} {pl:?} {far:?}");
                }
            }
        }
        // 2-point walls: verticals stay vertical and unforeshortened at the origin edge.
        let g = grid(2);
        let top = g.to_page(Plane::Left, Point::new(0.0, 100.0)).unwrap();
        assert!((top - Point::new(g.origin[0], g.origin[1] - 100.0)).hypot() < 1e-9);
        // Foreshortening: u = distance lands halfway to the vanishing point.
        let mid = g.to_page(Plane::Right, Point::new(g.distance, 0.0)).unwrap();
        let want = Point::new(g.origin[0], g.origin[1]).lerp(Point::new(g.vp_right, g.horizon), 0.5);
        assert!((mid - want).hypot() < 1e-9);
    }

    #[test]
    fn homography_round_trips() {
        for kind in [1u8, 2, 3] {
            let g = grid(kind);
            for pl in Plane::ALL {
                for q in [Point::new(10.0, 20.0), Point::new(300.0, 5.0), Point::new(0.0, 0.0), Point::new(123.0, 77.0)] {
                    let p = g.to_page(pl, q).unwrap();
                    let back = g.to_plane(pl, p).unwrap();
                    assert!((back - q).hypot() < 1e-6, "{kind} {pl:?} {q:?} → {back:?}");
                }
            }
        }
        assert!(grid(2).homography(Plane::None).is_none());
    }

    #[test]
    fn attach_map_keeps_the_dragged_corners() {
        let g = grid(2);
        let b = Rect::new(450.0, 300.0, 550.0, 420.0);
        let m = g.attach_map(Plane::Right, b).unwrap();
        assert!((m(Point::new(b.x0, b.y0)).unwrap() - Point::new(b.x0, b.y0)).hypot() < 1e-6);
        assert!((m(Point::new(b.x1, b.y1)).unwrap() - Point::new(b.x1, b.y1)).hypot() < 1e-6);
        // The other corners lie on grid lines through the plane: the top-right corner is on the
        // line from the top-left corner to the right vanishing point.
        let tr = m(Point::new(b.x1, b.y0)).unwrap();
        let vp = Point::new(g.vp_right, g.horizon);
        assert!((tr - Point::new(b.x0, b.y0)).cross(vp - Point::new(b.x0, b.y0)).abs() < 1e-6);
        assert!(tr.y != b.y0);
    }

    #[test]
    fn move_map_slides_within_the_plane() {
        let g = grid(2);
        let from = g.to_page(Plane::Left, Point::new(50.0, 50.0)).unwrap();
        let to = g.to_page(Plane::Left, Point::new(80.0, 60.0)).unwrap();
        let m = g.move_map(Plane::Left, from, to).unwrap();
        let p = g.to_page(Plane::Left, Point::new(10.0, 10.0)).unwrap();
        let q = g.to_plane(Plane::Left, m(p).unwrap()).unwrap();
        assert!((q - Point::new(40.0, 20.0)).hypot() < 1e-6);
    }

    #[test]
    fn grid_serde_and_merge() {
        let g = grid(3);
        let mut d = Document::new(800.0, 600.0);
        g.store(&mut d);
        assert_eq!(PerspectiveGrid::from_doc(&d), Some(g.clone()));
        let m = g.merged(&json!({"kind": 1, "cell": 25})).unwrap();
        assert_eq!((m.kind, m.cell), (1, 25.0));
        assert!(g.merged(&json!({"distance": -1})).is_err());
    }

    #[test]
    fn grid_tool_widget_and_handles() {
        let mut d = Document::new(800.0, 600.0);
        grid(2).store(&mut d);
        let (s, p) = (Selection::default(), paint());
        let c = cx(&d, &s, &p);
        let g = PerspectiveGrid::effective(&d);
        let mut t = PerspectiveGridTool::default();
        // Click the right face of the widget.
        let (faces, _) = g.widget(&d, 1.0);
        let q = faces[1].1;
        let centre = Point::new((q[0].x + q[1].x + q[2].x + q[3].x) / 4.0, (q[0].y + q[1].y + q[2].y + q[3].y) / 4.0);
        assert_eq!(
            t.pointer(&c, &PointerEvent::new(PointerKind::Down, centre.x, centre.y)),
            vec![Action::Exec("perspective.plane.set".into(), json!({"plane": "right"}))]
        );
        // Drag the left vanishing point.
        let acts = t.pointer(&c, &PointerEvent::new(PointerKind::Down, g.vp_left, g.horizon));
        assert_eq!(acts, vec![Action::Begin("Edit Perspective Grid".into())]);
        let acts = t.pointer(&c, &PointerEvent::new(PointerKind::Drag, g.vp_left - 40.0, g.horizon + 10.0));
        let Action::Preview(cmd, v) = &acts[0] else { panic!() };
        assert_eq!(cmd, "perspective.grid.set");
        assert_eq!(v["vpLeft"], json!(g.vp_left - 40.0));
        assert_eq!(v["horizon"], json!(g.horizon + 10.0));
        assert!(!grid_overlays(&d, 1.0, "selection").is_empty());
    }
}
