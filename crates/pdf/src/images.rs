//! Images the writer makes itself: pixels it samples (freeform gradients) as PDF image
//! XObjects.

use std::sync::Arc;

use krilla::image::{BitsPerComponent, CustomImage, Image, ImageColorspace};

/// Raw 8-bit pixels: colour samples (RGB, or grey with `luma`) and an optional alpha channel.
#[derive(Clone, Hash)]
struct Raw {
    color: Arc<Vec<u8>>,
    alpha: Option<Arc<Vec<u8>>>,
    width: u32,
    height: u32,
    luma: bool,
}

impl CustomImage for Raw {
    fn color_channel(&self) -> &[u8] {
        &self.color
    }

    fn alpha_channel(&self) -> Option<&[u8]> {
        self.alpha.as_deref().map(Vec::as_slice)
    }

    fn bits_per_component(&self) -> BitsPerComponent {
        BitsPerComponent::Eight
    }

    fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    fn icc_profile(&self) -> Option<&[u8]> {
        None
    }

    fn color_space(&self) -> ImageColorspace {
        if self.luma { ImageColorspace::Luma } else { ImageColorspace::Rgb }
    }
}

/// A `width` × `height` image of straight (not premultiplied) RGBA pixels, written as RGB, or as
/// grey when every pixel is grey, with an alpha channel only when some pixel isn't opaque.
/// `interpolate` asks viewers to smooth it when enlarged (PDF/A forbids it). `None` when `rgba`
/// doesn't hold that many pixels.
pub(crate) fn from_rgba(rgba: &[u8], width: u32, height: u32, interpolate: bool) -> Option<Image> {
    let px = rgba.as_chunks::<4>().0;
    if px.len() as u64 != width as u64 * height as u64 || px.is_empty() {
        return None;
    }
    let luma = px.iter().all(|p| p[0] == p[1] && p[1] == p[2]);
    let color = if luma { px.iter().map(|p| p[0]).collect() } else { px.iter().flat_map(|p| [p[0], p[1], p[2]]).collect() };
    let alpha = px.iter().any(|p| p[3] < 255).then(|| Arc::new(px.iter().map(|p| p[3]).collect()));
    Image::from_custom(Raw { color: Arc::new(color), alpha, width, height, luma }, interpolate).ok()
}
