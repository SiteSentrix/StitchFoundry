//! Stylize › Find Edges.

use super::Px16;

/// Convert premultiplied RGBA pixels to a Sobel edge map in place.
pub(super) fn find(px: &mut [[u8; 4]], w: usize, h: usize) {
    if w < 3 || h < 3 || w.checked_mul(h) != Some(px.len()) {
        return;
    }
    let src: Vec<Px16> = px.iter().map(|p| p.map(u16::from)).collect();
    let luminance = |x: usize, y: usize| -> f32 {
        src.get(y * w + x).map_or(0.0, |p| {
            (0.2126 * f32::from(p[0]) + 0.7152 * f32::from(p[1]) + 0.0722 * f32::from(p[2])) / 257.0
        })
    };
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let gx = -luminance(x - 1, y - 1) + luminance(x + 1, y - 1)
                - 2.0 * luminance(x - 1, y) + 2.0 * luminance(x + 1, y)
                - luminance(x - 1, y + 1) + luminance(x + 1, y + 1);
            let gy = -luminance(x - 1, y - 1) - 2.0 * luminance(x, y - 1) - luminance(x + 1, y - 1)
                + luminance(x - 1, y + 1) + 2.0 * luminance(x, y + 1) + luminance(x + 1, y + 1);
            let edge = gx.hypot(gy).clamp(0.0, 255.0) as u8;
            if let Some(out) = px.get_mut(y * w + x) {
                let alpha = out[3];
                *out = [edge.min(alpha), edge.min(alpha), edge.min(alpha), alpha];
            }
        }
    }
}
