//! Raster export: render a document region with [`RasterExportOptions`] (resolution, background,
//! anti-aliasing) and encode it as PNG ([`png`]: resolution in `pHYs`, Adam7 interlacing), JPEG
//! (resolution in the JFIF header) or lossless WebP.

pub mod png;

use vectorcraft_doc::{Document, Node, NodeKind};
use vectorcraft_geom::Rect;

use crate::{AntiAlias, RenderOptions, Rendered, Renderer, fx};

/// A raster file format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterFormat {
    Png,
    Jpeg,
    WebP,
}

/// How a raster export renders and encodes.
#[derive(Clone, Debug, PartialEq)]
pub struct RasterExportOptions {
    /// Resolution in pixels per inch: 72 renders one pixel per point. Stored in the file.
    pub ppi: f64,
    /// Opaque background colour (RGB); `None` = transparent (JPEG has no alpha: white).
    pub background: Option<[u8; 3]>,
    pub anti_alias: AntiAlias,
    /// PNG: Adam7 interlacing.
    pub interlaced: bool,
    /// JPEG quality 1–100.
    pub quality: u8,
}

impl Default for RasterExportOptions {
    fn default() -> Self {
        Self { ppi: 72.0, background: None, anti_alias: AntiAlias::Art, interlaced: false, quality: 90 }
    }
}

impl RasterExportOptions {
    /// Pixels per point.
    pub fn scale(&self) -> f64 {
        self.ppi / 72.0
    }

    /// What the renderer draws for `format`: template layers left out, over the background
    /// (white for a JPEG without one).
    pub fn render_options(&self, format: RasterFormat) -> RenderOptions {
        let background = self.background.or((format == RasterFormat::Jpeg).then_some([255; 3]));
        RenderOptions {
            background: background.map(|[r, g, b]| [r, g, b, 255]),
            skip_templates: true,
            anti_alias: self.anti_alias,
            ..Default::default()
        }
    }

    /// Encode a rendered image as `format`.
    pub fn encode(&self, img: &Rendered, format: RasterFormat) -> Result<Vec<u8>, String> {
        match format {
            RasterFormat::Png => {
                png::encode(&img.to_straight(), img.width, img.height, &png::PngOptions { ppi: Some(self.ppi), interlaced: self.interlaced })
            }
            RasterFormat::Jpeg => jpeg(img, self.quality, Some(self.ppi)),
            RasterFormat::WebP => webp(img),
        }
    }
}

impl Renderer {
    /// Render `region` of `doc` (an artboard or any rect) as exported and encode it as `format`.
    /// Callers check the size first ([`crate::raster_size`]).
    pub fn export_region(&mut self, doc: &Document, region: Rect, format: RasterFormat, opts: &RasterExportOptions) -> Result<Vec<u8>, String> {
        let img = self.render_region_with(doc, region, opts.scale(), &opts.render_options(format));
        opts.encode(&img, format)
    }
}

/// JPEG at `quality` 1–100, partly transparent pixels flattened on white, with the resolution
/// `ppi` (if any) in the JFIF header.
pub(crate) fn jpeg(img: &Rendered, quality: u8, ppi: Option<f64>) -> Result<Vec<u8>, String> {
    let rgb: Vec<u8> = img
        .to_straight()
        .as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = p[3] as u32;
            let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
            [mix(p[0]), mix(p[1]), mix(p[2])]
        })
        .collect();
    let mut buf = Vec::new();
    let mut enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality.clamp(1, 100));
    if let Some(ppi) = ppi {
        enc.set_pixel_density(image::codecs::jpeg::PixelDensity::dpi(ppi.round().clamp(1.0, u16::MAX as f64) as u16));
    }
    image::ImageEncoder::write_image(enc, &rgb, img.width, img.height, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("JPEG encoding failed: {e}"))?;
    Ok(buf)
}

/// Lossless WebP.
pub(crate) fn webp(img: &Rendered) -> Result<Vec<u8>, String> {
    let mut buf = Vec::new();
    let enc = image::codecs::webp::WebPEncoder::new_lossless(&mut buf);
    image::ImageEncoder::write_image(enc, &img.to_straight(), img.width, img.height, image::ExtendedColorType::Rgba8)
        .map_err(|e| format!("WebP encoding failed: {e}"))?;
    Ok(buf)
}

/// Bounds of the art an export draws: visible objects off template layers (guides left out),
/// with their strokes and live effects. `None` when nothing would be drawn.
pub fn art_bounds(doc: &Document) -> Option<Rect> {
    doc.layers.iter().fold(None, |acc, l| vectorcraft_geom::union_opt(acc, drawn_bounds(l)))
}

fn drawn_bounds(n: &Node) -> Option<Rect> {
    if !n.visible {
        return None;
    }
    match &n.kind {
        NodeKind::Layer { template: true, .. } | NodeKind::Path { guide: true, .. } => None,
        NodeKind::Layer { children, .. } | NodeKind::Group { children, clip: false } => {
            children.iter().fold(None, |acc, c| vectorcraft_geom::union_opt(acc, drawn_bounds(c)))
        }
        _ if fx::has_fx(n) => fx::visual_bounds(n),
        _ => n.visual_bounds(),
    }
}

#[cfg(test)]
mod tests;
