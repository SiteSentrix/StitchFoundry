//! Stylize › Glowing Edges.

use super::blur_plane;

/// Find Sobel edges using alpha-weighted straight colour, smooth and expand them, then draw
/// coloured outlines on black. Samples outside the raster are transparent.
pub(super) fn glow(px: &mut [[u8; 4]], w: usize, h: usize, width: f64, brightness: f64, smoothness: f64) {
    if w == 0 || h == 0 || w.checked_mul(h) != Some(px.len()) {
        return;
    }
    let src = px.to_vec();
    let signal = |x: isize, y: isize| -> f32 {
        if x < 0 || y < 0 { return 0.0; }
        let Some(p) = src.get(y as usize * w + x as usize) else { return 0.0 };
        let alpha = f32::from(p[3]) / 255.0;
        let straight = if alpha > 0.0 { [f32::from(p[0]) / 255.0 / alpha, f32::from(p[1]) / 255.0 / alpha, f32::from(p[2]) / 255.0 / alpha] } else { [0.0; 3] };
        (0.2126 * straight[0] + 0.7152 * straight[1] + 0.0722 * straight[2]) * alpha
    };
    let mut edges = vec![0.0f32; px.len()];
    for y in 0..h {
        for x in 0..w {
            let x = x as isize;
            let y = y as isize;
            let gx = -signal(x - 1, y - 1) + signal(x + 1, y - 1) - 2.0 * signal(x - 1, y) + 2.0 * signal(x + 1, y) - signal(x - 1, y + 1) + signal(x + 1, y + 1);
            let gy = -signal(x - 1, y - 1) - 2.0 * signal(x, y - 1) - signal(x + 1, y - 1) + signal(x - 1, y + 1) + 2.0 * signal(x, y + 1) + signal(x + 1, y + 1);
            if let Some(edge) = edges.get_mut(y as usize * w + x as usize) { *edge = gx.hypot(gy).clamp(0.0, 1.0); }
        }
    }
    blur_plane(&mut edges, w, h, smoothness.max(0.0));
    if width > 0.5 { blur_plane(&mut edges, w, h, width * 0.5); }
    for (out, (original, edge)) in px.iter_mut().zip(src.iter().zip(edges)) {
        let alpha = f32::from(original[3]) / 255.0;
        let glow = (edge * brightness as f32 / 6.0).clamp(0.0, 1.0);
        let straight = if original[3] > 0 { [f32::from(original[0]) / 255.0 / alpha.max(1e-6), f32::from(original[1]) / 255.0 / alpha.max(1e-6), f32::from(original[2]) / 255.0 / alpha.max(1e-6)] } else { [1.0, 1.0, 1.0] };
        let a = alpha.max(glow);
        *out = [(straight[0] * glow * a * 255.0).round().clamp(0.0, 255.0) as u8, (straight[1] * glow * a * 255.0).round().clamp(0.0, 255.0) as u8, (straight[2] * glow * a * 255.0).round().clamp(0.0, 255.0) as u8, (a * 255.0).round() as u8];
    }
}
