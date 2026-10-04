//! Paint conversion (solid, gradients, patterns) and image decoding.

use vectorcraft_color::{GradientKind, Paint};
use vectorcraft_doc::Document;
use vectorcraft_geom::{Affine, Rect, Vec2};
use vello_cpu::RenderContext;
use vello_cpu::peniko::{self, ColorStop};

fn color(c: &vectorcraft_color::Color, alpha: f32) -> peniko::Color {
    let [r, g, b, a] = c.to_rgba8(alpha);
    peniko::Color::from_rgba8(r, g, b, a)
}

/// Set the context paint. Returns false if nothing should be drawn.
pub fn set_paint(ctx: &mut RenderContext, p: &Paint, bounds: Rect, doc: &Document) -> bool {
    match p {
        Paint::None => false,
        Paint::Solid { color: c, .. } => {
            ctx.set_paint(color(c, 1.0));
            true
        }
        Paint::Gradient(g) => {
            let geom = g.resolve(bounds);
            let stops: Vec<ColorStop> = g.gradient.expanded_stops().iter().map(|(o, c, a)| ColorStop::from((*o, color(c, *a)))).collect();
            if stops.is_empty() {
                return false;
            }
            let grad = match g.gradient.kind {
                GradientKind::Radial => {
                    let r = geom.length().max(1e-6) as f32;
                    let angle = (geom.end - geom.start).atan2();
                    // Aspect ratio squashes the circle along the perpendicular of the gradient vector.
                    ctx.set_paint_transform(
                        Affine::translate(geom.start.to_vec2())
                            * Affine::rotate(angle)
                            * Affine::scale_non_uniform(1.0, geom.aspect.max(1e-3))
                            * Affine::rotate(-angle)
                            * Affine::translate(-geom.start.to_vec2()),
                    );
                    peniko::Gradient::new_radial(geom.start, r).with_stops(stops.as_slice())
                }
                _ => {
                    ctx.reset_paint_transform();
                    let (s, e) =
                        if geom.start.distance(geom.end) < 1e-9 { (geom.start, geom.start + Vec2::new(1.0, 0.0)) } else { (geom.start, geom.end) };
                    peniko::Gradient::new_linear(s, e).with_stops(stops.as_slice())
                }
            };
            ctx.set_paint(grad);
            true
        }
        Paint::Pattern { pattern, xf } => crate::pattern::set_pattern_paint(ctx, pattern, *xf, doc),
    }
}

/// Decode encoded image bytes into a premultiplied pixmap.
pub fn decode_pixmap(bytes: &[u8]) -> Option<vello_cpu::Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || w > u16::MAX as u32 || h > u16::MAX as u32 {
        return None;
    }
    let data: Vec<vello_cpu::color::PremulRgba8> = img
        .pixels()
        .map(|p| {
            let a = p[3] as u16;
            let m = |c: u8| ((c as u16 * a + 127) / 255) as u8;
            vello_cpu::color::PremulRgba8 { r: m(p[0]), g: m(p[1]), b: m(p[2]), a: p[3] }
        })
        .collect();
    Some(vello_cpu::Pixmap::from_parts(data, w as u16, h as u16))
}
