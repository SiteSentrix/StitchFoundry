//! The TIFF preview (little-endian, PackBits strips, written by the raster exports' TIFF writer):
//! 1-bit black and white, or 8-bit RGB with or without an alpha channel. Also the PNG thumbnail.

use std::borrow::Cow;

use vectorcraft_render::encode::on_white;
use vectorcraft_render::encode::tiff::{self, ByteOrder, Compression, Image, Layout, Photometric};

use crate::Raster;

/// `img` as a TIFF: one bit a pixel (`bw`: black where darker than mid-grey on white), else RGB
/// over white or, with `alpha`, RGBA (unassociated alpha).
pub(crate) fn encode(img: &Raster, bw: bool, alpha: bool) -> Result<Vec<u8>, String> {
    let w = img.width as usize;
    if w == 0 || img.height == 0 || w.checked_mul(img.height as usize).and_then(|n| n.checked_mul(4)) != Some(img.rgba.len()) {
        return Err("the preview image is empty".into());
    }
    let px = img.rgba.as_chunks::<4>().0;
    let (data, photometric, samples, bits): (Cow<[u8]>, _, u16, u16) = if bw {
        let mut data = Vec::with_capacity(w.div_ceil(8) * img.height as usize);
        for line in px.chunks(w) {
            let at = data.len();
            data.resize(at + w.div_ceil(8), 0);
            for (x, p) in line.iter().enumerate() {
                let [r, g, b] = on_white(p);
                let luma = 299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b);
                // WhiteIsZero: a set bit is black.
                if luma < 128_000
                    && let Some(byte) = data.get_mut(at + x / 8)
                {
                    *byte |= 0x80 >> (x % 8);
                }
            }
        }
        (Cow::Owned(data), Photometric::WhiteIsZero, 1, 1)
    } else if alpha {
        (Cow::Borrowed(&img.rgba), Photometric::Rgb, 4, 8)
    } else {
        (px.iter().flat_map(on_white).collect(), Photometric::Rgb, 3, 8)
    };
    let image = Image { width: img.width, height: img.height, data: &data, photometric, samples, bits, alpha: alpha && !bw };
    tiff::write(&image, &Layout { byte_order: ByteOrder::Little, compression: Compression::PackBits, ppi: 72.0, icc: None })
        .map_err(|e| format!("the preview: {e}"))
}

/// `img` as a PNG.
pub(crate) fn png(img: &Raster) -> Result<Vec<u8>, String> {
    let rgba = image::RgbaImage::from_raw(img.width, img.height, img.rgba.clone()).ok_or("the thumbnail image is malformed")?;
    let mut out = vec![];
    rgba.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(out)
}
