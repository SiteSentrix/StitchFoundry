//! The stitch overlay: every stitch of the plan drawn on the canvas as a GPU mesh,
//! either as flat lines or "TrueView" shaded thread, over a fabric-tinted artboard.

use std::sync::Arc;

use egui::epaint::{Mesh, Vertex, WHITE_UV};
use egui::{Color32, Painter, Pos2, Shape, Stroke};
use vectorcraft_embroidery::sf_core::pattern::{Cmd, Pattern};
use vectorcraft_embroidery::sf_core::threads::Rgb;
use vectorcraft_embroidery::{PT_PER_MM, StitchPlan, pt};

use super::{ArtView, StitchView};
use crate::canvas::Xf;

/// Thread thickness of 40 wt polyester lying on fabric, in mm.
const THREAD_WIDTH_MM: f64 = 0.36;

/// Most vertices the repeats of a design may add up to, so a big design repeated many times
/// keeps the canvas responsive (copies beyond it are left out).
const MAX_REPEAT_VERTICES: usize = 6_000_000;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Key {
    plan: usize,
    view: [i64; 6],
    mode: StitchView,
    upto: usize,
    points: bool,
    repeats: [u32; 2],
    gap: i64,
}

/// The overlay mesh for the last view; rebuilt only when the plan or view changes.
#[derive(Default)]
pub struct MeshCache {
    key: Option<Key>,
    mesh: Arc<Mesh>,
    jumps: Vec<[Pos2; 2]>,
}

fn c32(c: Rgb, f: f32) -> Color32 {
    let s = |v: u8| ((v as f32) * f).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(s(c.r), s(c.g), s(c.b))
}

fn toward_white(c: Color32, t: f32) -> Color32 {
    let m = |v: u8| (v as f32 + (255.0 - v as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

fn quad(mesh: &mut Mesh, a: Pos2, b: Pos2, half_w: f32, col: Color32) {
    let d = b - a;
    let len = d.length();
    if len < 0.01 {
        return;
    }
    let n = egui::vec2(-d.y, d.x) / len * half_w;
    let i = mesh.vertices.len() as u32;
    for p in [a + n, a - n, b - n, b + n] {
        mesh.vertices.push(Vertex { pos: p, uv: WHITE_UV, color: col });
    }
    mesh.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
}

/// One stitch as lit thread: four lanes across its width shaded as a cylinder with
/// a soft specular highlight, ends tucked into the fabric.
fn thread(mesh: &mut Mesh, a: Pos2, b: Pos2, half_w: f32, color: Rgb) {
    let d = b - a;
    let len = d.length();
    if len < 0.01 {
        return;
    }
    let t = d / len;
    let nrm = egui::vec2(-t.y, t.x);
    let inset = (half_w * 0.35).min(len * 0.2);
    let (a2, b2) = (a + t * inset, b - t * inset);
    // light from the top left, a little towards the viewer
    const L: [f32; 3] = [-0.45, -0.55, 0.70];
    const LANES: [f32; 4] = [-1.0, -0.4, 0.4, 1.0];
    let base = mesh.vertices.len() as u32;
    for &u in &LANES {
        let z = (1.0 - u * u).max(0.0).sqrt();
        let ndl = nrm.x * u * L[0] + nrm.y * u * L[1] + z * L[2];
        let diffuse = ndl.max(0.0);
        let spec = (2.0 * ndl * z - L[2]).max(0.0).powf(12.0);
        let mut col = c32(color, 0.38 + 0.75 * diffuse);
        if spec > 0.01 {
            col = toward_white(col, (spec * 0.55).min(0.6));
        }
        let off = nrm * (u * half_w);
        mesh.vertices.push(Vertex { pos: a2 + off, uv: WHITE_UV, color: col });
        mesh.vertices.push(Vertex { pos: b2 + off, uv: WHITE_UV, color: col });
    }
    for k in 0..3u32 {
        let i = base + k * 2;
        mesh.indices.extend_from_slice(&[i, i + 1, i + 3, i, i + 3, i + 2]);
    }
}

fn build(pattern: &Pattern, xf: &Xf, mode: StitchView, upto: usize, points: bool, visible: egui::Rect) -> (Mesh, Vec<[Pos2; 2]>) {
    let mut mesh = Mesh::default();
    let mut jumps = Vec::new();
    let true_view = mode == StitchView::TrueView;
    let half_w = if true_view { ((THREAD_WIDTH_MM * PT_PER_MM * xf.zoom) as f32 * 0.5).max(0.45) } else { 0.6 };
    let n = upto.min(pattern.stitches.len());
    let mut thread_ix = 0usize;
    let mut color = pattern.thread(0).color;
    let mut prev: Option<Pos2> = None;
    let mut sewing = false;
    for s in pattern.stitches.iter().take(n) {
        let p = xf.to_screen(pt(s.pos()));
        match s.cmd {
            Cmd::Stitch => {
                if let Some(a) = prev
                    && sewing
                    && egui::Rect::from_two_pos(a, p).intersects(visible)
                {
                    if true_view {
                        thread(&mut mesh, a, p, half_w, color);
                    } else {
                        quad(&mut mesh, a, p, half_w, c32(color, 1.0));
                    }
                }
                if points && visible.contains(p) {
                    quad(&mut mesh, p - egui::vec2(1.2, 0.0), p + egui::vec2(1.2, 0.0), 1.2, Color32::from_black_alpha(190));
                }
                prev = Some(p);
                sewing = true;
            }
            Cmd::Jump => {
                if let Some(a) = prev {
                    jumps.push([a, p]);
                }
                prev = Some(p);
                sewing = false;
            }
            Cmd::Trim => sewing = false,
            Cmd::ColorChange => {
                thread_ix += 1;
                color = pattern.thread(thread_ix).color;
                sewing = false;
            }
            Cmd::Stop | Cmd::End => {}
        }
    }
    (mesh, jumps)
}

impl MeshCache {
    fn update(&mut self, plan: &Arc<StitchPlan>, xf: &Xf, (mode, upto, points): (StitchView, usize, bool), (repeats, gap): ([u32; 2], f64)) {
        let q = |v: f64| (v * 1000.0).round() as i64;
        let key = Key {
            plan: Arc::as_ptr(plan) as usize,
            view: [q(xf.center.x), q(xf.center.y), q(xf.zoom), q(xf.rot), q(xf.rect.min.x as f64), q(xf.rect.min.y as f64)],
            mode,
            upto,
            points,
            repeats,
            gap: q(gap),
        };
        if self.key == Some(key) {
            return;
        }
        // Stitches off screen are left out, unless a copy of them shows.
        let step = repeat_step(plan.pattern(), xf, gap);
        let visible = xf.rect.expand(40.0);
        let [cols, rows] = repeats.map(|n| n.clamp(1, 10) as f32 - 1.0);
        let cull = [step[0] * cols, step[1] * rows, step[0] * cols + step[1] * rows].into_iter().fold(visible, |r, d| r.union(visible.translate(-d)));
        let (mesh, jumps) = build(plan.pattern(), xf, mode, upto, points, cull);
        self.mesh = Arc::new(tiled(mesh, repeats, step));
        self.jumps = jumps;
        self.key = Some(key);
    }
}

/// How far apart repeats of the design are drawn on screen, across and down: its stitches'
/// width and height plus `gap` (mm).
fn repeat_step(pattern: &Pattern, xf: &Xf, gap: f64) -> [egui::Vec2; 2] {
    let b = pattern.bounds();
    if b.is_empty() {
        return [egui::Vec2::ZERO; 2];
    }
    let gap = if gap.is_finite() { gap.clamp(0.0, 500.0) } else { 0.0 };
    let at = |x: f64, y: f64| xf.to_screen(pt(vectorcraft_embroidery::sf_core::Point::new(x, y)));
    let o = at(b.min.x, b.min.y);
    [at(b.max.x + gap, b.min.y) - o, at(b.min.x, b.max.y + gap) - o]
}

/// The design's mesh repeated `cols × rows` times, `step` apart across and down: the first
/// where the design is, the others beside it and below.
fn tiled(mesh: Mesh, [cols, rows]: [u32; 2], step: [egui::Vec2; 2]) -> Mesh {
    let (cols, rows) = (cols.clamp(1, 10), rows.clamp(1, 10));
    if cols * rows == 1 || mesh.vertices.is_empty() {
        return mesh;
    }
    let room = (MAX_REPEAT_VERTICES / mesh.vertices.len()).max(1);
    let mut out = mesh.clone();
    let offsets = (0..rows).flat_map(|j| (0..cols).map(move |i| (i, j))).skip(1).take(room.saturating_sub(1));
    for (i, j) in offsets {
        let mut copy = mesh.clone();
        copy.translate(step[0] * i as f32 + step[1] * j as f32);
        out.append(copy);
    }
    out
}

/// Paints the stitch plan over the art.
pub fn paint(app: &mut crate::VectorcraftApp, painter: &Painter, xf: &Xf) {
    let ui = &app.embroidery;
    if ui.stitches == StitchView::Off {
        return;
    }
    let Some(plan) = app.session.embroidery_plan() else { return };
    if plan.is_empty() {
        return;
    }
    let Some(st) = app.session.active() else { return };
    // Fabric under the stitches: the artboards, tinted.
    let fabric = ui.fabric;
    let cover = match ui.art {
        ArtView::Normal => None,
        ArtView::Dimmed => Some(Color32::from_rgba_unmultiplied(fabric.r(), fabric.g(), fabric.b(), 190)),
        ArtView::Hidden => Some(fabric),
    };
    if let Some(c) = cover {
        for ab in &st.doc.artboards {
            painter.add(Shape::convex_polygon(xf.quad(ab.rect), c, Stroke::NONE));
        }
    }
    let total = plan.pattern().stitches.len();
    let upto = if ui.player.active { (ui.player.pos as usize).min(total) } else { total };
    let mode = ui.stitches;
    let points = ui.needle_points;
    let connectors = ui.connectors;
    let (beads, guides) = (ui.beads, ui.guides);
    let selected: Vec<_> = st.selection.objects.clone();
    // The hoop's sewing field, centred on the design as the machine centres it (or where the
    // design is placed in it).
    if let Some(e) = st.doc.embroidery.as_deref()
        && let Some(hoop) = e.hoop.as_ref()
    {
        hoop_outline(painter, xf, hoop, plan.pattern(), e.settings.hoop_offset.unwrap_or([0.0, 0.0]));
    }
    let repeats = (app.embroidery.repeats, app.embroidery.repeat_gap);
    let cache = &mut app.embroidery.mesh;
    cache.update(&plan, xf, (mode, upto, points), repeats);
    painter.add(Shape::mesh(cache.mesh.clone()));
    if connectors {
        for [a, b] in &cache.jumps {
            painter.add(Shape::dashed_line(&[*a, *b], Stroke::new(1.0, Color32::from_rgba_unmultiplied(120, 60, 200, 200)), 4.0, 3.0));
        }
    }
    // Inclination lines of the selected stitched objects.
    if guides {
        let orange = Color32::from_rgb(255, 120, 0);
        for id in &selected {
            let Some(e) = st.doc.node(*id).and_then(|n| n.embroidery.as_deref()) else { continue };
            for g in &e.guides {
                let a = xf.to_screen(vectorcraft_geom::Point::new(g.a.x, g.a.y));
                let b = xf.to_screen(vectorcraft_geom::Point::new(g.b.x, g.b.y));
                painter.line_segment([a, b], Stroke::new(1.5, orange));
                for p in [a, b] {
                    painter.circle_filled(p, 3.0, orange);
                }
            }
        }
    }
    // Split lines of the selected fills, dashed: each stitch row has a needle point where it
    // crosses one.
    let teal = Color32::from_rgb(0, 150, 170);
    for id in &selected {
        let Some(e) = st.doc.node(*id).and_then(|n| n.embroidery.as_deref()) else { continue };
        for line in &e.splits {
            let pts: Vec<Pos2> = line.iter().map(|q| xf.to_screen(vectorcraft_geom::Point::new(q.x, q.y))).collect();
            painter.add(Shape::dashed_line(&pts, Stroke::new(1.2, teal), 5.0, 3.0));
        }
    }
    // Flow lines of the selected fills, dashed: the rows follow them.
    let violet = Color32::from_rgb(150, 80, 200);
    for id in &selected {
        let Some(e) = st.doc.node(*id).and_then(|n| n.embroidery.as_deref()) else { continue };
        for line in &e.flow {
            let pts: Vec<Pos2> = line.iter().map(|q| xf.to_screen(vectorcraft_geom::Point::new(q.x, q.y))).collect();
            painter.add(Shape::dashed_line(&pts, Stroke::new(1.5, violet), 8.0, 4.0));
        }
    }
    // Start (green) and stop (red) beads of the selected stitched objects.
    for id in selected.iter().copied().filter(|_| beads) {
        let ranges = plan.records_of(id);
        let pat = plan.pattern();
        let first = ranges.first().and_then(|r| pat.stitches.get(r.start..r.end)).and_then(|s| s.iter().find(|s| s.cmd == Cmd::Stitch));
        let last = ranges.last().and_then(|r| pat.stitches.get(r.start..r.end)).and_then(|s| s.iter().rev().find(|s| s.cmd == Cmd::Stitch));
        for (s, col) in [(first, Color32::from_rgb(40, 190, 90)), (last, Color32::from_rgb(220, 50, 50))] {
            if let Some(s) = s {
                let p = xf.to_screen(pt(s.pos()));
                painter.circle_filled(p, 5.0, col);
                painter.circle_stroke(p, 5.0, Stroke::new(1.5, Color32::WHITE));
            }
        }
    }
    // The stitch player's needle.
    if app.embroidery.player.active
        && let Some(s) = plan.pattern().stitches.get(upto.saturating_sub(1))
    {
        let p = xf.to_screen(pt(s.pos()));
        painter.circle_stroke(p, 7.0, Stroke::new(2.0, Color32::from_rgb(255, 170, 40)));
        painter.line_segment([p - egui::vec2(0.0, 14.0), p + egui::vec2(0.0, 14.0)], Stroke::new(1.0, Color32::from_rgb(255, 170, 40)));
    }
}

/// The hoop's sewing field (dashed, red where the design doesn't fit) round the
/// design's centre, with centre marks and the hoop's name.
fn hoop_outline(painter: &Painter, xf: &Xf, hoop: &vectorcraft_embroidery::sf_core::hoops::Hoop, pattern: &Pattern, [ox, oy]: [f64; 2]) {
    let b = pattern.bounds();
    if b.is_empty() {
        return;
    }
    // the hoop's centre: the design's middle, less where the design sits in the hoop
    let c = b.center() - vectorcraft_embroidery::sf_core::Point::new(ox, oy);
    let fits = hoop.fits(&vectorcraft_embroidery::sf_core::geom::BBox::new(b.min - c, b.max - c));
    let color = if fits { Color32::from_rgb(70, 110, 170) } else { Color32::from_rgb(210, 50, 50) };
    let mut ring: Vec<Pos2> = hoop.outline().into_iter().map(|q| xf.to_screen(pt(q + c))).collect();
    if let Some(&first) = ring.first() {
        ring.push(first);
    }
    painter.add(Shape::dashed_line(&ring, Stroke::new(1.5, color), 8.0, 5.0));
    let centre = xf.to_screen(pt(c));
    for d in [egui::vec2(10.0, 0.0), egui::vec2(0.0, 10.0)] {
        painter.line_segment([centre - d, centre + d], Stroke::new(1.0, color));
    }
    let top_left = xf.to_screen(pt(c + vectorcraft_embroidery::sf_core::Point::new(-hoop.width / 2.0, -hoop.height / 2.0)));
    painter.text(top_left + egui::vec2(4.0, -4.0), egui::Align2::LEFT_BOTTOM, &hoop.name, egui::FontId::proportional(11.0), color);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeats_tile_the_mesh_across_and_down() {
        let mut mesh = Mesh::default();
        quad(&mut mesh, Pos2::new(0.0, 0.0), Pos2::new(10.0, 0.0), 1.0, Color32::RED);
        let n = mesh.vertices.len();
        let step = [egui::vec2(20.0, 0.0), egui::vec2(0.0, 15.0)];
        // once: the mesh as it is
        assert_eq!(tiled(mesh.clone(), [1, 1], step).vertices.len(), n);
        // three across, two down: six copies, the last one along and down
        let t = tiled(mesh.clone(), [3, 2], step);
        assert_eq!(t.vertices.len(), n * 6);
        let last = &t.vertices[n * 5..];
        assert!(last.iter().zip(&mesh.vertices).all(|(a, b)| a.pos == b.pos + egui::vec2(40.0, 15.0)));
        // more than ten each way is ten
        assert_eq!(tiled(mesh, [50, 1], step).vertices.len(), n * 10);
    }
}
