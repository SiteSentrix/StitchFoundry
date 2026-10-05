//! The TIFF preview (baseline TIFF, little-endian, one PackBits strip): 1-bit black and white, or
//! 8-bit RGB with or without an alpha channel. Also the PNG thumbnail.

use crate::Raster;
use crate::ps::packbits;

/// TIFF field types.
const SHORT: u16 = 3;
const LONG: u16 = 4;
const RATIONAL: u16 = 5;

/// One directory entry: tag, type, count and its value (or, when longer than four bytes, the
/// bytes placed after the directory).
struct Entry {
    tag: u16,
    ty: u16,
    count: u32,
    data: Vec<u8>,
}

impl Entry {
    fn short(tag: u16, v: &[u16]) -> Self {
        Self { tag, ty: SHORT, count: v.len() as u32, data: v.iter().flat_map(|x| x.to_le_bytes()).collect() }
    }
    fn long(tag: u16, v: u32) -> Self {
        Self { tag, ty: LONG, count: 1, data: v.to_le_bytes().to_vec() }
    }
    fn rational(tag: u16, num: u32, den: u32) -> Self {
        Self { tag, ty: RATIONAL, count: 1, data: [num.to_le_bytes(), den.to_le_bytes()].concat() }
    }
}

/// Pixel `px` composited over white: RGB.
fn on_white(px: &[u8; 4]) -> [u8; 3] {
    let a = u32::from(px[3]);
    [0, 1, 2].map(|i| ((u32::from(px[i]) * a + 255 * (255 - a) + 127) / 255) as u8)
}

/// `img` as a TIFF: one bit a pixel (`bw`: black where darker than mid-grey on white), else RGB
/// over white or, with `alpha`, RGBA (unassociated alpha).
pub(crate) fn encode(img: &Raster, bw: bool, alpha: bool) -> Result<Vec<u8>, String> {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || w.checked_mul(h).and_then(|n| n.checked_mul(4)) != Some(img.rgba.len()) {
        return Err("the preview image is empty".into());
    }
    let px = img.rgba.as_chunks::<4>().0;
    let spp: usize = if bw {
        1
    } else if alpha {
        4
    } else {
        3
    };
    // Each row compressed on its own, as TIFF readers expect.
    let mut strip = Vec::with_capacity(w * h * spp / 4);
    let mut row = Vec::with_capacity(w * spp);
    for y in 0..h {
        row.clear();
        let line = px.get(y * w..(y + 1) * w).unwrap_or_default();
        if bw {
            row.resize(w.div_ceil(8), 0);
            for (x, p) in line.iter().enumerate() {
                let [r, g, b] = on_white(p);
                let luma = 299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b);
                // WhiteIsZero: a set bit is black.
                if luma < 128_000
                    && let Some(byte) = row.get_mut(x / 8)
                {
                    *byte |= 0x80 >> (x % 8);
                }
            }
        } else if alpha {
            row.extend(line.iter().flatten());
        } else {
            row.extend(line.iter().flat_map(on_white));
        }
        packbits(&row, &mut strip);
    }
    let strip_len = u32::try_from(strip.len()).map_err(|_| "the preview image is too large".to_string())?;
    let bits: Vec<u16> = vec![if bw { 1 } else { 8 }; spp];
    let mut entries = vec![
        Entry::long(256, img.width),
        Entry::long(257, img.height),
        Entry::short(258, &bits),
        // PackBits.
        Entry::short(259, &[32773]),
        // WhiteIsZero, or RGB.
        Entry::short(262, &[if bw { 0 } else { 2 }]),
        // StripOffsets, filled in below.
        Entry::long(273, 0),
        Entry::short(277, &[spp as u16]),
        Entry::long(278, img.height),
        Entry::long(279, strip_len),
        Entry::rational(282, 72, 1),
        Entry::rational(283, 72, 1),
        // Inches.
        Entry::short(296, &[2]),
    ];
    if alpha && !bw {
        // Unassociated alpha.
        entries.push(Entry::short(338, &[2]));
    }
    // Header, directory (count, entries, next = 0), the longer values, then the strip.
    let dir_len = 2 + entries.len() * 12 + 4;
    let mut extra_at = 8 + dir_len;
    let extra_len: usize = entries.iter().filter(|e| e.data.len() > 4).map(|e| e.data.len().next_multiple_of(2)).sum();
    let strip_at = u32::try_from(8 + dir_len + extra_len).map_err(|_| "the preview image is too large".to_string())?;
    if let Some(e) = entries.iter_mut().find(|e| e.tag == 273) {
        e.data = strip_at.to_le_bytes().to_vec();
    }
    let mut out = Vec::with_capacity(strip_at as usize + strip.len());
    out.extend(b"II");
    out.extend(42u16.to_le_bytes());
    out.extend(8u32.to_le_bytes());
    out.extend((entries.len() as u16).to_le_bytes());
    let mut extra = vec![];
    for e in &entries {
        out.extend(e.tag.to_le_bytes());
        out.extend(e.ty.to_le_bytes());
        out.extend(e.count.to_le_bytes());
        if e.data.len() <= 4 {
            let mut v = e.data.clone();
            v.resize(4, 0);
            out.extend(v);
        } else {
            out.extend((extra_at as u32).to_le_bytes());
            extra.extend(&e.data);
            if e.data.len() % 2 == 1 {
                extra.push(0);
            }
            extra_at += e.data.len().next_multiple_of(2);
        }
    }
    out.extend(0u32.to_le_bytes());
    out.extend(extra);
    out.extend(strip);
    Ok(out)
}

/// `img` as a PNG.
pub(crate) fn png(img: &Raster) -> Result<Vec<u8>, String> {
    let rgba = image::RgbaImage::from_raw(img.width, img.height, img.rgba.clone()).ok_or("the thumbnail image is malformed")?;
    let mut out = vec![];
    rgba.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}
