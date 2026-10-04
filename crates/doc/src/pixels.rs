//! Pixel access for embedded images: a stable content key and recolouring the pixels.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::Arc;

use crate::ImageBlob;

impl ImageBlob {
    /// A PNG blob of `png` bytes.
    pub fn png(png: Vec<u8>) -> Self {
        Self { mime: "image/png".into(), bytes: Arc::new(png) }
    }

    /// A key derived from the bytes (FNV-1a), the same for identical images.
    pub fn content_key(&self) -> String {
        let h = self.bytes.iter().fold(0xcbf29ce484222325u64, |h, x| (h ^ *x as u64).wrapping_mul(0x100000001b3));
        format!("img{h:016x}")
    }

    /// The image with `f` applied to each pixel's (straight) RGB, alpha kept and fully transparent
    /// pixels left alone, re-encoded as PNG. `f` runs once per distinct colour. `None` when the
    /// image can't be decoded or no pixel changed.
    pub fn map_rgb(&self, mut f: impl FnMut([u8; 3]) -> [u8; 3]) -> Option<ImageBlob> {
        let mut img = image::load_from_memory(&self.bytes).ok()?.to_rgba8();
        let mut memo: HashMap<[u8; 3], [u8; 3]> = HashMap::new();
        let mut changed = false;
        for px in img.pixels_mut().filter(|p| p.0[3] > 0) {
            let rgb = [px.0[0], px.0[1], px.0[2]];
            let out = *memo.entry(rgb).or_insert_with(|| f(rgb));
            if out != rgb {
                px.0[..3].copy_from_slice(&out);
                changed = true;
            }
        }
        if !changed {
            return None;
        }
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
        Some(Self::png(png))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn blob(px: &[[u8; 4]], w: u32, h: u32) -> ImageBlob {
        let img = image::RgbaImage::from_raw(w, h, px.concat()).unwrap();
        let mut png = Vec::new();
        img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        ImageBlob::png(png)
    }

    fn pixels(b: &ImageBlob) -> Vec<[u8; 4]> {
        image::load_from_memory(&b.bytes).unwrap().to_rgba8().pixels().map(|p| p.0).collect()
    }

    #[test]
    fn maps_each_colour_once_and_keeps_alpha() {
        let b = blob(&[[255, 0, 0, 255], [255, 0, 0, 128], [0, 0, 255, 255], [10, 20, 30, 0]], 2, 2);
        let mut calls = 0;
        let out = b
            .map_rgb(|[r, g, bl]| {
                calls += 1;
                [255 - r, 255 - g, 255 - bl]
            })
            .unwrap();
        assert_eq!(calls, 2, "red and blue, once each; the transparent pixel is skipped");
        assert_eq!(pixels(&out), [[0, 255, 255, 255], [0, 255, 255, 128], [255, 255, 0, 255], [10, 20, 30, 0]]);
        assert_ne!(out.content_key(), b.content_key());
        assert_eq!(b.content_key(), blob(&[[255, 0, 0, 255], [255, 0, 0, 128], [0, 0, 255, 255], [10, 20, 30, 0]], 2, 2).content_key());
        assert!(b.map_rgb(|c| c).is_none(), "nothing changed");
        assert!(ImageBlob::png(vec![1, 2, 3]).map_rgb(|c| c).is_none(), "undecodable");
    }
}
