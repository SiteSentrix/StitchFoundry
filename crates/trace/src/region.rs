//! Magic wand for click-to-stitch: the region of a raster a click picks (the pixels joined to the
//! one clicked whose colour is close to its), traced as an outline or as centre lines.

use vectorcraft_geom::PathData;

use crate::contour::trace_mask;
use crate::fit;
use crate::quantize::TRANSPARENT;
use crate::subpixel::Crack;
use crate::{Raster, TraceParams, centerline};

/// A picked region: which pixels (row-major, the raster's size), how many, and their mean colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    pub mask: Vec<bool>,
    pub pixels: usize,
    pub color: [u8; 4],
}

/// Pixel `i`'s colour.
fn pixel(img: &Raster, i: usize) -> Option<[u8; 4]> {
    let s = img.rgba.get(i.checked_mul(4)?..i.checked_mul(4)?.checked_add(4)?)?;
    <[u8; 4]>::try_from(s).ok()
}

/// The pixels 4-connected to `(x, y)` whose colour is within `tolerance` of its (on every
/// channel, alpha too); none when the point is outside the raster or on a (half) transparent
/// pixel.
pub fn pick(img: &Raster, x: u32, y: u32, tolerance: u8) -> Option<Region> {
    let (w, h) = (img.width as usize, img.height as usize);
    if x as usize >= w || y as usize >= h || img.rgba.len() != w.checked_mul(h)?.checked_mul(4)? {
        return None;
    }
    let seed = y as usize * w + x as usize;
    let c0 = pixel(img, seed).filter(|c| c[3] >= 128)?;
    let near = |c: [u8; 4]| c[3] >= 128 && c.iter().zip(c0).all(|(a, b)| a.abs_diff(b) <= tolerance);
    let mut mask = vec![false; w * h];
    let mut sum = [0u64; 4];
    let mut pixels = 0usize;
    let mut stack = vec![seed];
    if let Some(m) = mask.get_mut(seed) {
        *m = true;
    }
    while let Some(i) = stack.pop() {
        let Some(c) = pixel(img, i) else { continue };
        pixels += 1;
        for (s, v) in sum.iter_mut().zip(c) {
            *s += u64::from(v);
        }
        let (cx, cy) = (i % w, i / w);
        let around = [(cx > 0, i.wrapping_sub(1)), (cx + 1 < w, i + 1), (cy > 0, i.wrapping_sub(w)), (cy + 1 < h, i + w)];
        for (inside, j) in around {
            if inside && mask.get(j) == Some(&false) && pixel(img, j).is_some_and(near) {
                if let Some(m) = mask.get_mut(j) {
                    *m = true;
                }
                stack.push(j);
            }
        }
    }
    let n = pixels.max(1) as u64;
    let color = sum.map(|s| u8::try_from(s / n).unwrap_or(u8::MAX));
    Some(Region { mask, pixels, color })
}

/// How picked regions are fitted: smooth, keeping corners, as a detailed trace would.
fn style() -> crate::logo::Style {
    TraceParams { paths: 80.0, corners: 60.0, ..TraceParams::default() }.outline_style()
}

/// Fits one loop of `mask`: its edges on the pixel cracks (a mask has no tones to place them
/// between pixels), and no junctions (one region against the rest).
fn fit_mask_loop(l: &crate::contour::Loop, mask: &[bool], w: usize, h: usize, style: &crate::logo::Style) -> Option<vectorcraft_geom::SubPath> {
    let across = |c: Crack| (0.0, c.at(1).and_then(|i| mask.get(i)).map_or(1, |&m| u32::from(!m)));
    fit::fit_loop(l, w, h, &across, &|_, _| None, style).map(|(sp, _)| sp)
}

/// The outline of `mask` (`w` × `h`) in pixel space: its largest piece's outer edge and, with
/// `holes`, its holes of at least `min_hole` pixels.
pub fn outline(mask: &[bool], w: usize, h: usize, holes: bool, min_hole: usize) -> Option<PathData> {
    if mask.len() != w.checked_mul(h)? {
        return None;
    }
    let o = style();
    let comp = trace_mask(mask, w, h).into_iter().max_by_key(|c| c.pixels)?;
    let mut subs = vec![fit_mask_loop(&comp.outer, mask, w, h, &o)?];
    if holes {
        for hole in &comp.holes {
            if usize::try_from(hole.area2.unsigned_abs() / 2).unwrap_or(usize::MAX) >= min_hole {
                subs.extend(fit_mask_loop(hole, mask, w, h, &o));
            }
        }
    }
    Some(PathData::new(subs))
}

/// The centre lines of `mask` (`w` × `h`) in pixel space, each with its mean width in pixels.
pub fn centre_lines(mask: &[bool], w: usize, h: usize) -> Vec<(PathData, f64)> {
    if w.checked_mul(h) != Some(mask.len()) {
        return Vec::new();
    }
    let labels: Vec<u16> = mask.iter().map(|m| if *m { 0 } else { TRANSPARENT }).collect();
    let widest = (w.max(h) as f64) * 2.0;
    // the lines are read against the mask drawn black on white
    let (Ok(wu), Ok(hu)) = (u32::try_from(w), u32::try_from(h)) else { return Vec::new() };
    let img =
        Raster::from_fn(wu, hu, |x, y| if mask.get(y as usize * w + x as usize) == Some(&true) { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
    let (_, lines) = centerline::lines(&labels, 0, w, h, widest, &img, [0.0; 3], &style());
    lines.into_iter().map(|l| (l.path, l.width)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A white raster with a red ring (a square with a square hole) and a blue bar.
    fn picture() -> Raster {
        Raster::from_fn(100, 60, |x, y| {
            if (10..50).contains(&x) && (10..50).contains(&y) && !((25..35).contains(&x) && (25..35).contains(&y)) {
                [210, 20, 20, 255]
            } else if (60..95).contains(&x) && (28..32).contains(&y) {
                [20, 20, 200, 255]
            } else {
                [255, 255, 255, 255]
            }
        })
    }

    #[test]
    fn a_click_picks_the_region_of_its_colour() {
        let img = picture();
        let r = pick(&img, 12, 12, 32).unwrap();
        assert_eq!(r.pixels, 40 * 40 - 100);
        assert_eq!(r.color, [210, 20, 20, 255]);
        // its outline with the hole, or without it
        let with = outline(&r.mask, 100, 60, true, 16).unwrap();
        assert_eq!(with.subpaths.len(), 2);
        let b = with.bounds().unwrap();
        assert!((b.x0 - 10.0).abs() < 1.0 && (b.x1 - 50.0).abs() < 1.0, "{b:?}");
        assert_eq!(outline(&r.mask, 100, 60, false, 16).unwrap().subpaths.len(), 1);
        // the bar's centre line, about as wide as the bar
        let bar = pick(&img, 70, 30, 32).unwrap();
        let lines = centre_lines(&bar.mask, 100, 60);
        assert_eq!(lines.len(), 1);
        assert!((lines[0].1 - 4.0).abs() < 1.5, "{}", lines[0].1);
        // outside the raster, or a transparent pixel: nothing
        assert!(pick(&img, 100, 0, 32).is_none());
        let clear = Raster::from_fn(4, 4, |_, _| [0, 0, 0, 0]);
        assert!(pick(&clear, 1, 1, 32).is_none());
    }
}
