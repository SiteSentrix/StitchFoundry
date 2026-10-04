//! Freeform gradients in the renderer.
//!
//! vello has no freeform shading, so the colour field ([`vectorcraft_color::freeform`]) is
//! sampled on a grid of cells over the painted box and drawn as an image paint (bilinear, edges
//! padded): the fill or stroke being painted clips it. The grid is sized from the box's size on
//! screen (a cell per few device pixels, in power-of-two steps so zooming reuses it) and cached by
//! the gradient's content, so panning and redraws don't resample it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use vectorcraft_color::freeform::spread_scale;
use vectorcraft_color::{Color, Freeform, GradientPaint};
use vectorcraft_geom::{Affine, Point, Rect};
use vello_cpu::peniko::{Extend, ImageQuality, ImageSampler};
use vello_cpu::{Pixmap, RenderContext};

use crate::ink::Ink;

/// Device pixels per grid cell.
const CELL_PX: f64 = 4.0;
/// Fewest and most cells along the box's longer side.
const MIN_CELLS: f64 = 8.0;
const MAX_CELLS: f64 = 256.0;
/// Grids kept before the cache starts over.
const MAX_GRIDS: usize = 128;

thread_local! {
    static GRIDS: RefCell<HashMap<u64, Arc<Pixmap>>> = RefCell::new(HashMap::new());
}

fn hash_color(c: &Color, h: &mut impl Hasher) {
    match *c {
        Color::Rgb { r, g, b } => (0u8, r.to_bits(), g.to_bits(), b.to_bits()).hash(h),
        Color::Cmyk { c, m, y, k } => (1u8, c.to_bits(), m.to_bits(), y.to_bits(), k.to_bits()).hash(h),
        Color::Gray { k } => (2u8, k.to_bits()).hash(h),
        Color::Lab { l, a, b } => (3u8, l.to_bits(), a.to_bits(), b.to_bits()).hash(h),
    }
}

/// Cache key of the grid of `f` on box `b` at `cols` × `rows` painted with `ink` (and the active
/// colour settings, which turn colours into display RGB or inks).
fn key(f: &Freeform, b: Rect, cols: u16, rows: u16, ink: Ink) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for p in &f.points {
        (p.at.x.to_bits(), p.at.y.to_bits(), p.opacity.to_bits(), p.spread.to_bits()).hash(&mut h);
        hash_color(&p.color, &mut h);
    }
    f.lines.hash(&mut h);
    (b.x0.to_bits(), b.y0.to_bits(), b.x1.to_bits(), b.y1.to_bits(), cols, rows).hash(&mut h);
    (Arc::as_ptr(&vectorcraft_color::cms::active()) as usize, ink).hash(&mut h);
    h.finish()
}

/// Sample `f` at the centres of a `cols` × `rows` grid over `b` (premultiplied, colours as `ink`
/// paints them).
pub(crate) fn rasterize(f: &Freeform, b: Rect, cols: u16, rows: u16, ink: Ink) -> Pixmap {
    let field = f.field_with(spread_scale(b), &|c| ink.rgb(c));
    let (cw, ch) = (b.width() / cols as f64, b.height() / rows as f64);
    let data = (0..rows as usize * cols as usize)
        .map(|k| {
            let (i, j) = (k % cols as usize, k / cols as usize);
            let ([r, g, bl], a) = field.sample(Point::new(b.x0 + (i as f64 + 0.5) * cw, b.y0 + (j as f64 + 0.5) * ch));
            let a = a.clamp(0.0, 1.0);
            let q = |v: f32| (v.clamp(0.0, 1.0) * a * 255.0).round() as u8;
            vello_cpu::color::PremulRgba8 { r: q(r), g: q(g), b: q(bl), a: (a * 255.0).round() as u8 }
        })
        .collect();
    Pixmap::from_parts(data, cols, rows)
}

/// Cells along a side `len` (document units) long whose box's longer side `side` gets `cells`.
fn cells_along(len: f64, side: f64, cells: f64) -> u16 {
    (cells * len / side).ceil().clamp(2.0, MAX_CELLS) as u16
}

/// Set freeform gradient `g` on box `bounds` (in the paint's space, which the context's current
/// transform maps to pixels) as the context paint. Returns false for an empty box.
pub(crate) fn set_freeform_paint(ctx: &mut RenderContext, g: &GradientPaint, bounds: Rect, ink: Ink) -> bool {
    let b = bounds.abs();
    let side = b.width().max(b.height());
    if !(side > 1e-9 && side.is_finite()) {
        return false;
    }
    // A box without height (or width) gets a sliver of it, so the grid maps back invertibly.
    let b = Rect::from_center_size(b.center(), (b.width().max(side * 1e-3), b.height().max(side * 1e-3)));
    let device = side * ctx.transform().determinant().abs().sqrt();
    let cells = 2f64.powf((device / CELL_PX).max(1.0).log2().ceil()).clamp(MIN_CELLS, MAX_CELLS);
    let (cols, rows) = (cells_along(b.width(), side, cells), cells_along(b.height(), side, cells));
    let f = g.freeform_on(b);
    let k = key(&f, b, cols, rows, ink);
    let pm = GRIDS.with(|c| c.borrow().get(&k).cloned()).unwrap_or_else(|| {
        let pm = Arc::new(rasterize(&f, b, cols, rows, ink));
        GRIDS.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() >= MAX_GRIDS {
                c.clear();
            }
            c.insert(k, pm.clone());
        });
        pm
    });
    let sampler = ImageSampler { x_extend: Extend::Pad, y_extend: Extend::Pad, quality: ImageQuality::Medium, alpha: 1.0 };
    ctx.set_paint(vello_cpu::Image { image: vello_cpu::ImageSource::Pixmap(pm), sampler });
    ctx.set_paint_transform(Affine::translate(b.origin().to_vec2()) * Affine::scale_non_uniform(b.width() / cols as f64, b.height() / rows as f64));
    true
}
