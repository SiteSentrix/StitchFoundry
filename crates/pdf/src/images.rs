//! Images in the PDF: the document's images as the Compression settings say (resampled above a
//! resolution, compressed with ZIP or JPEG) and converted as the Output settings say
//! ([`Pixels`]), and pixels the writer makes itself (freeform gradients).
//!
//! Every image is classified as the settings are split: colour, greyscale (a grey colour type) or
//! monochrome (greyscale that is only black and white). An image already as asked (not resampled,
//! a JPEG kept as JPEG or another kept lossless) is embedded as it is; the others are decoded,
//! resampled and encoded again. JPEG needs opaque pixels: images with transparency stay lossless.

use std::io::Cursor;
use std::sync::Arc;

use image::buffer::ConvertBuffer;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, ImageDecoder, ImageFormat, ImageReader, RgbaImage};
use krilla::image::{BitsPerComponent, CustomImage, Image, ImageColorspace};

use crate::output::Pixels;
use crate::{CompressionSettings, Downsample, ImageCodec, JpegQuality, MonoCodec};

/// Raw 8-bit pixels: colour samples (RGB, grey or CMYK) and an optional alpha channel.
#[derive(Clone, Hash)]
struct Raw {
    color: Arc<Vec<u8>>,
    alpha: Option<Arc<Vec<u8>>>,
    width: u32,
    height: u32,
    space: ImageColorspace,
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
        self.space
    }
}

/// A `width` × `height` image of straight (not premultiplied) RGBA pixels, written as RGB, or as
/// grey when every pixel is grey, with an alpha channel only when some pixel isn't opaque.
/// `interpolate` asks viewers to smooth it when enlarged (PDF/A forbids it). `None` when `rgba`
/// doesn't hold that many pixels.
pub(crate) fn from_rgba(rgba: &[u8], width: u32, height: u32, interpolate: bool) -> Option<Image> {
    from_rgba_as(rgba, width, height, interpolate, None)
}

/// [`from_rgba`], colour pixels written as CMYK when `pixels` converts to CMYK (grey ones stay
/// grey).
fn from_rgba_as(rgba: &[u8], width: u32, height: u32, interpolate: bool, pixels: Option<&Pixels>) -> Option<Image> {
    let px = rgba.as_chunks::<4>().0;
    if px.len() as u64 != width as u64 * height as u64 || px.is_empty() {
        return None;
    }
    let luma = px.iter().all(|p| p[0] == p[1] && p[1] == p[2]);
    let (color, space) = if luma {
        (px.iter().map(|p| p[0]).collect(), ImageColorspace::Luma)
    } else if let Some(cmyk) = pixels.and_then(|p| p.cmyk(rgba)) {
        (cmyk, ImageColorspace::Cmyk)
    } else {
        (px.iter().flat_map(|p| [p[0], p[1], p[2]]).collect(), ImageColorspace::Rgb)
    };
    let alpha = px.iter().any(|p| p[3] < 255).then(|| Arc::new(px.iter().map(|p| p[3]).collect()));
    Image::from_custom(Raw { color: Arc::new(color), alpha, width, height, space }, interpolate).ok()
}

/// [`from_rgba`] of pixels converted by `pixels` (when given).
pub(crate) fn from_rgba_in(mut rgba: Vec<u8>, width: u32, height: u32, interpolate: bool, pixels: Option<&Pixels>) -> Option<Image> {
    if let Some(p) = pixels {
        p.rgb_in_place(&mut rgba);
    }
    from_rgba_as(&rgba, width, height, interpolate, pixels)
}

/// The kinds of images the Compression section has settings for.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Kind {
    Color,
    Gray,
    Mono,
}

/// How an image's pixels are compressed in the file.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Codec {
    /// JPEG images stay JPEG (at this quality when they are resampled); the others are lossless.
    Auto(u8),
    Zip,
    Jpeg(u8),
}

/// The settings for one kind of image.
struct Rule {
    downsample: Downsample,
    /// Target resolution, and the resolution above which images are resampled (ppi).
    ppi: f64,
    above: f64,
    codec: Codec,
    /// Why the codec differs from the one asked for (that one isn't available).
    instead: Option<&'static str>,
}

const UNCOMPRESSED: &str = "uncompressed images are not written: images are compressed with ZIP";
const CMYK_ZIP: &str = "images converted to CMYK are compressed with ZIP, not JPEG";

/// JPEG quality (1–100) of a quality setting.
fn quality(q: JpegQuality) -> u8 {
    match q {
        JpegQuality::Minimum => 10,
        JpegQuality::Low => 30,
        JpegQuality::Medium => 55,
        JpegQuality::High => 80,
        JpegQuality::Maximum => 95,
    }
}

fn rule(c: &CompressionSettings, kind: Kind) -> Rule {
    if kind == Kind::Mono {
        let m = &c.mono;
        let instead = match m.compression {
            MonoCodec::Zip => None,
            MonoCodec::None => Some(UNCOMPRESSED),
            MonoCodec::CcittG3 | MonoCodec::CcittG4 => Some("CCITT compression is not supported: monochrome images are compressed with ZIP"),
            MonoCodec::RunLength => Some("Run Length compression is not supported: monochrome images are compressed with ZIP"),
        };
        return Rule { downsample: m.downsample, ppi: m.ppi, above: m.above_ppi, codec: Codec::Zip, instead };
    }
    let i = if kind == Kind::Color { &c.color } else { &c.gray };
    let (codec, instead) = match i.compression {
        ImageCodec::Auto => (Codec::Auto(quality(i.quality)), None),
        ImageCodec::Zip => (Codec::Zip, None),
        ImageCodec::Jpeg => (Codec::Jpeg(quality(i.quality)), None),
        ImageCodec::None => (Codec::Zip, Some(UNCOMPRESSED)),
        ImageCodec::Jpeg2000 => (Codec::Zip, Some("JPEG 2000 compression is not supported: those images are compressed with ZIP")),
    };
    Rule { downsample: i.downsample, ppi: i.ppi, above: i.above_ppi, codec, instead }
}

/// What [`recode`] makes of an image.
pub(crate) struct Recoded {
    /// The image to write; `None` keeps the source image as it is.
    pub image: Option<Image>,
    /// Set when the image isn't compressed as asked.
    pub warning: Option<&'static str>,
}

/// Is `img` (a grey image) only black and white?
fn bilevel(img: &DynamicImage) -> bool {
    img.to_luma_alpha8().pixels().all(|p| p[0] == 0 || p[0] == 255)
}

/// Encoded image `bytes` placed `size` points wide and high, as the Compression settings `c` ask:
/// resampled when its resolution is above the threshold, compressed with ZIP or JPEG, its colours
/// converted by `convert` (colour images). `interpolate` lets viewers smooth it. An image that
/// can't be decoded is kept as it is.
pub(crate) fn recode(bytes: &[u8], size: (f64, f64), c: &CompressionSettings, interpolate: bool, convert: Option<&Pixels>) -> Recoded {
    let keep = Recoded { image: None, warning: None };
    let Ok(reader) = ImageReader::new(Cursor::new(bytes)).with_guessed_format() else { return keep };
    let jpeg = reader.format() == Some(ImageFormat::Jpeg);
    let Ok(decoder) = reader.into_decoder() else { return keep };
    let (w, h) = decoder.dimensions();
    // Resolution along each side as placed; above the threshold, a side goes down to the target.
    let ppi = |px: u32, pt: f64| px as f64 * 72.0 / pt.abs();
    let (px, py) = (ppi(w, size.0), ppi(h, size.1));
    let sized = |r: &Rule| {
        let resample = r.downsample != Downsample::None && [px, py].iter().any(|p| p.is_finite() && *p > r.above);
        let side = |n: u32, p: f64| if resample && p.is_finite() && p > r.ppi { ((n as f64 * r.ppi / p).round() as u32).clamp(1, n) } else { n };
        (side(w, px), side(h, py))
    };
    // Already as asked: not resampled, and lossless or a JPEG kept as JPEG.
    let kept = |r: &Rule| {
        sized(r) == (w, h)
            && match r.codec {
                Codec::Auto(_) => true,
                Codec::Zip => !jpeg,
                Codec::Jpeg(_) => jpeg,
            }
    };
    use ExtendedColorType as T;
    let grey = matches!(decoder.original_color_type(), T::L1 | T::La1 | T::L2 | T::La2 | T::L4 | T::La4 | T::L8 | T::La8 | T::L16 | T::La16);
    let (kind, pixels) = if grey {
        // Only the pixels tell monochrome images from greyscale ones: decoded unless both keep it.
        let (g, m) = (rule(c, Kind::Gray), rule(c, Kind::Mono));
        if kept(&g) && kept(&m) && g.instead == m.instead {
            return Recoded { image: None, warning: g.instead };
        }
        let Ok(img) = DynamicImage::from_decoder(decoder) else { return keep };
        (if bilevel(&img) { Kind::Mono } else { Kind::Gray }, Some(img))
    } else {
        (Kind::Color, None)
    };
    let rule = rule(c, kind);
    let convert = convert.filter(|_| kind == Kind::Color);
    if kept(&rule) && convert.is_none() {
        return Recoded { image: None, warning: rule.instead };
    }
    let Some(img) = pixels.or_else(|| image::load_from_memory(bytes).ok()) else { return keep };
    let (nw, nh) = sized(&rule);
    let img = if (nw, nh) == (w, h) {
        img
    } else {
        match rule.downsample {
            Downsample::Average => img.thumbnail_exact(nw, nh),
            Downsample::Subsample => img.resize_exact(nw, nh, FilterType::Nearest),
            Downsample::Bicubic | Downsample::None => img.resize_exact(nw, nh, FilterType::CatmullRom),
        }
    };
    let mut rgba = img.to_rgba8();
    if kind == Kind::Mono {
        // Resampling greys the edges: back to black and white.
        for p in rgba.pixels_mut() {
            let v = if p[0] >= 128 { 255 } else { 0 };
            (p[0], p[1], p[2]) = (v, v, v);
        }
    }
    let quality = match rule.codec {
        Codec::Jpeg(q) => Some(q),
        Codec::Auto(q) if jpeg => Some(q),
        _ => None,
    };
    // CMYK images are written losslessly (the PDF writer's CMYK JPEGs need the reader's
    // conventions).
    let cmyk = convert.is_some_and(|p| p.is_cmyk());
    let (width, height) = rgba.dimensions();
    let opaque = rgba.pixels().all(|p| p[3] == 255);
    let image = match quality.filter(|_| opaque && !cmyk) {
        Some(q) => {
            if let Some(p) = convert {
                p.rgb_in_place(&mut rgba);
            }
            to_jpeg(&rgba, kind != Kind::Color, q, interpolate).or_else(|| from_rgba(rgba.as_raw(), width, height, interpolate))
        }
        None => from_rgba_in(rgba.into_raw(), width, height, interpolate, convert),
    };
    let warning = rule.instead.or((cmyk && quality.is_some() && opaque).then_some(CMYK_ZIP));
    Recoded { image, warning }
}

/// Opaque `rgba` as a JPEG image at `quality` (a greyscale JPEG with `grey`).
fn to_jpeg(rgba: &RgbaImage, grey: bool, quality: u8, interpolate: bool) -> Option<Image> {
    let img = if grey { DynamicImage::ImageLuma8(rgba.convert()) } else { DynamicImage::ImageRgb8(rgba.convert()) };
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality).encode_image(&img).ok()?;
    Image::from_jpeg(out.into(), interpolate).ok()
}
