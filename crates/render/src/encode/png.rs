//! A PNG writer for raster exports: 8-bit RGBA (RGB when every pixel is opaque), adaptive row
//! filters, the resolution as a `pHYs` chunk and optional Adam7 interlacing (the image builds up
//! progressively while it loads). Chunks are written here; flate2 compresses the image data.

use std::borrow::Cow;
use std::io::Write as _;

use flate2::Crc;
use flate2::write::ZlibEncoder;

/// What goes into the file besides the pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PngOptions {
    /// Pixels per inch, stored as pixels per metre in `pHYs` (`None`: no `pHYs` chunk).
    pub ppi: Option<f64>,
    /// Adam7 interlacing.
    pub interlaced: bool,
}

const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
/// Most image data bytes per `IDAT` chunk.
const IDAT_MAX: usize = 1 << 20;
/// Adam7 passes: (x0, y0, dx, dy).
const ADAM7: [(u32, u32, u32, u32); 7] = [(0, 0, 8, 8), (4, 0, 8, 8), (0, 4, 4, 8), (2, 0, 4, 4), (0, 2, 2, 4), (1, 0, 2, 2), (0, 1, 1, 2)];

/// Pixels per metre for a resolution in pixels per inch (the unit `pHYs` stores).
pub fn pixels_per_metre(ppi: f64) -> u32 {
    (ppi / 0.0254).round().clamp(1.0, u32::MAX as f64) as u32
}

/// A whole `pHYs` chunk (length, type, data, CRC) declaring `x` × `y` pixels per inch.
pub fn phys_chunk(x: f64, y: f64) -> Vec<u8> {
    let [x, y] = [x, y].map(|v| pixels_per_metre(v).to_be_bytes());
    let mut out = Vec::with_capacity(21);
    // x, y, unit 1 = metre.
    chunk(&mut out, b"pHYs", &[x.as_slice(), y.as_slice(), &[1]].concat());
    out
}

/// Encode straight-alpha RGBA8 pixels (`width`×`height`, row-major) as a PNG file.
pub fn encode(rgba: &[u8], width: u32, height: u32, o: &PngOptions) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || width > i32::MAX as u32 || height > i32::MAX as u32 {
        return Err(format!("PNG encoding failed: {width} × {height} pixels is not a valid image size"));
    }
    if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
        return Err("PNG encoding failed: pixel buffer doesn't match the image size".into());
    }
    let px = rgba.as_chunks::<4>().0;
    let opaque = px.iter().all(|p| p[3] == 255);
    let (channels, color_type) = if opaque { (3, 2) } else { (4, 6) };
    let pixels: Cow<[u8]> = if opaque { Cow::Owned(px.iter().flat_map(|p| [p[0], p[1], p[2]]).collect()) } else { Cow::Borrowed(rgba) };

    let mut out = Vec::with_capacity(pixels.len() / 2 + 64);
    out.extend_from_slice(SIGNATURE);
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    // Bit depth 8, colour type, deflate compression, adaptive filtering, interlace method.
    ihdr.extend_from_slice(&[8, color_type, 0, 0, u8::from(o.interlaced)]);
    chunk(&mut out, b"IHDR", &ihdr);
    if let Some(ppi) = o.ppi {
        out.extend_from_slice(&phys_chunk(ppi, ppi));
    }
    let raw = if o.interlaced {
        let mut raw = Vec::with_capacity(pixels.len() + height as usize * 2);
        for pass in ADAM7 {
            let sub = sub_image(&pixels, width, height, channels, pass);
            if let Some((w, h, px)) = sub {
                filter_rows(&px, w, h, channels, &mut raw);
            }
        }
        raw
    } else {
        let mut raw = Vec::with_capacity(pixels.len() + height as usize);
        filter_rows(&pixels, width, height, channels, &mut raw);
        raw
    };
    let mut z = ZlibEncoder::new(Vec::with_capacity(raw.len() / 2), flate2::Compression::default());
    let data = z.write_all(&raw).and_then(|()| z.finish()).map_err(|e| format!("PNG encoding failed: {e}"))?;
    for part in data.chunks(IDAT_MAX) {
        chunk(&mut out, b"IDAT", part);
    }
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

/// Append one chunk: length, type, data, CRC of type and data.
fn chunk(out: &mut Vec<u8>, ty: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(ty);
    out.extend_from_slice(data);
    let mut crc = Crc::new();
    crc.update(ty);
    crc.update(data);
    out.extend_from_slice(&crc.sum().to_be_bytes());
}

/// The pixels of one Adam7 pass as their own image (`None` when the pass is empty).
fn sub_image(px: &[u8], width: u32, height: u32, channels: usize, (x0, y0, dx, dy): (u32, u32, u32, u32)) -> Option<(u32, u32, Vec<u8>)> {
    let w = width.saturating_sub(x0).div_ceil(dx);
    let h = height.saturating_sub(y0).div_ceil(dy);
    if w == 0 || h == 0 {
        return None;
    }
    let mut out = Vec::with_capacity(w as usize * h as usize * channels);
    for y in (y0..height).step_by(dy as usize) {
        let row = &px[y as usize * width as usize * channels..][..width as usize * channels];
        for x in (x0..width).step_by(dx as usize) {
            out.extend_from_slice(&row[x as usize * channels..][..channels]);
        }
    }
    Some((w, h, out))
}

/// Filter every row of a `w`×`h` image into `out` (filter type byte + filtered bytes), picking
/// per row the filter with the smallest sum of absolute values (the usual heuristic).
fn filter_rows(px: &[u8], w: u32, h: u32, bpp: usize, out: &mut Vec<u8>) {
    let stride = w as usize * bpp;
    let zero = vec![0u8; stride];
    let mut cand: [Vec<u8>; 5] = std::array::from_fn(|_| vec![0u8; stride]);
    for y in 0..h as usize {
        let cur = &px[y * stride..][..stride];
        let prev = if y == 0 { &zero[..] } else { &px[(y - 1) * stride..][..stride] };
        // The first pixel has no left neighbour (a = c = 0).
        let (head, tail) = (bpp.min(stride), stride.saturating_sub(bpp));
        let [none, sub, up, avg, pae] = &mut cand;
        none.copy_from_slice(cur);
        sub[..head].copy_from_slice(&cur[..head]);
        for i in 0..stride {
            up[i] = cur[i].wrapping_sub(prev[i]);
        }
        for i in 0..head {
            avg[i] = cur[i].wrapping_sub(prev[i] / 2);
            pae[i] = cur[i].wrapping_sub(paeth(0, prev[i], 0));
        }
        for j in 0..tail {
            let i = j + bpp;
            let (a, b, c) = (cur[j], prev[i], prev[j]);
            sub[i] = cur[i].wrapping_sub(a);
            avg[i] = cur[i].wrapping_sub(((a as u16 + b as u16) / 2) as u8);
            pae[i] = cur[i].wrapping_sub(paeth(a, b, c));
        }
        let cost = |c: &[u8]| c.iter().map(|&v| (v as i8).unsigned_abs() as u64).sum::<u64>();
        let best = (0..5).min_by_key(|&f| cost(&cand[f])).unwrap_or(0);
        out.push(best as u8);
        out.extend_from_slice(&cand[best]);
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = a as i16 + b as i16 - c as i16;
    let (pa, pb, pc) = ((p - a as i16).abs(), (p - b as i16).abs(), (p - c as i16).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}
