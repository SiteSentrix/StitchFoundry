//! VectorCraft Image Trace: raster → vector.
//!
//! A clean-room tracer built from the published ideas behind bitmap tracers (the potrace paper
//! and classic colour quantisation), not from any existing implementation:
//!
//! 1. **Quantise** the image into a small palette: a luminance threshold (Black and White), 1-D
//!    k-means over the luminance histogram (Grayscale), or, in Color, over a 15-bit colour
//!    histogram as the [`Palette`] asks: median cut to a colour count followed by weighted k-means
//!    (Limited), median cut until every colour box is tight (Full Tone), one of those two by
//!    whether the image is flat art (Automatic), or the most used colours of a swatch library
//!    (Document Library). Transparent pixels are left out.
//! 2. **Remove noise**: 4-connected same-colour components smaller than `noise` pixels are merged
//!    into their most common neighbouring colour.
//! 3. **Follow boundaries**: for each colour layer, the pixel-crack edges between inside and
//!    outside are linked into closed loops (inside on the right, right turns at saddles, so
//!    diagonal pixels stay separate, matching 4-connectivity). Outer loops run clockwise and
//!    holes counter-clockwise (y down), so the result fills correctly with either fill rule.
//! 4. **Sub-pixel edges**: each crack of a loop is moved to where the image says the edge really
//!    is (an anti-aliased or blurred edge pixel is part one tone, part the other), so the outline
//!    is right to a fraction of a pixel before any fitting; hard-edged images are read as
//!    staircases instead (see `subpixel`).
//! 5. **Curves**: the outline is fitted as Flat Logo mode fits its own (`logo::fit_outline`):
//!    corners the pixels rounded are put back, straight runs become lines, a round outline a
//!    true circle, and the rest as few cubic Béziers as the Paths tolerance allows; Snap Curves To
//!    Lines also straightens slightly curved runs and makes arcs of parts of circles.
//!
//! With **Create: Strokes**, the parts of a colour layer no wider than the stroke width are traced
//! as stroked centre lines instead (see `centerline`); without Fills, wider parts become stroked
//! outlines.
//!
//! Colour layers are traced either *abutting* (each colour's own area; shapes share edges) or
//! *overlapping* (stacked: each layer also covers every layer above it, so no hairline gaps).
#![forbid(unsafe_code)]

mod centerline;
mod contour;
mod fit;
mod logo;
mod mosaic;
mod quantize;
mod subpixel;

use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use vectorcraft_geom::PathData;

pub use contour::{Component, Loop, trace_mask};
pub use mosaic::mosaic;
pub use quantize::{FULL_TONE_MAX, Quantized, TRANSPARENT, denoise, quantize};
use subpixel::Crack;

/// Errors decoding a raster.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("could not decode image: {0}")]
    Decode(String),
    #[error("image is empty")]
    Empty,
    #[error("the image is {width} × {height} pixels, more than Image Trace takes ({max} megapixels)")]
    TooLarge { width: u32, height: u32, max: u64 },
    #[error("this isn't a flat-colour logo ({:.0}% of its solid areas match a few flat colours), so Flat Logo mode would turn its gradients into patches: use Color mode instead", (.fraction * 100.0).floor())]
    NotFlat { fraction: f64 },
    #[error("\"{0}\" is not a colour: write it as #rrggbb")]
    BadPalette(String),
    #[error(
        "this trace would make more than {max} anchor points, more than the document can show smoothly: raise Noise, lower Paths or Colors, or pick a lower-fidelity preset"
    )]
    TooComplex { max: usize },
}

/// The most pixels an image to trace may have (64 megapixels: 256 MB decoded); larger images are
/// refused instead of running out of memory.
pub const MAX_PIXELS: u64 = 64 << 20;

/// The most pixels an image may have for [`Raster::decode_to_fit`] to decode it and scale it down
/// to [`MAX_PIXELS`] (192 megapixels: 768 MB decoded); larger images are refused.
pub const MAX_SOURCE_PIXELS: u64 = 192 << 20;

/// The largest size with `w : h`'s aspect of at most `max` pixels (each side at least 1).
fn fit_size(w: u32, h: u32, max: u64) -> (u32, u32) {
    let pixels = u64::from(w) * u64::from(h);
    if pixels <= max || pixels == 0 {
        return (w, h);
    }
    let k = (max as f64 / pixels as f64).sqrt();
    let side = |v: u32| ((f64::from(v) * k).floor() as u32).clamp(1, v);
    let (mut nw, mut nh) = (side(w), side(h));
    // A side held at 1 pixel leaves the other too long: cut it to what fits.
    if u64::from(nw) * u64::from(nh) > max {
        if nw >= nh {
            nw = u32::try_from(max / u64::from(nh)).unwrap_or(nw).clamp(1, nw);
        } else {
            nh = u32::try_from(max / u64::from(nw)).unwrap_or(nh).clamp(1, nh);
        }
    }
    (nw, nh)
}

/// An RGBA8 raster (row-major, top row first).
#[derive(Clone, Debug, PartialEq)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Raster {
    /// A raster from RGBA bytes (`width * height * 4` of them).
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert_eq!(rgba.len(), width as usize * height as usize * 4, "rgba length must be width*height*4");
        Self { width, height, rgba }
    }
    /// A raster whose pixel `(x, y)` is `f(x, y)`.
    pub fn from_fn(width: u32, height: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Self {
        let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
        for y in 0..height {
            for x in 0..width {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Self { width, height, rgba }
    }
    /// Decode PNG / JPEG / WebP / GIF bytes (at most [`MAX_PIXELS`], checked before decoding).
    pub fn decode(bytes: &[u8]) -> Result<Self, TraceError> {
        let decode_err = |e: image::ImageError| TraceError::Decode(e.to_string());
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| TraceError::Decode(e.to_string()))?;
        let (w, h) = reader.into_dimensions().map_err(decode_err)?;
        if w == 0 || h == 0 {
            return Err(TraceError::Empty);
        }
        if u64::from(w) * u64::from(h) > MAX_PIXELS {
            return Err(TraceError::TooLarge { width: w, height: h, max: MAX_PIXELS >> 20 });
        }
        let img = image::load_from_memory(bytes).map_err(decode_err)?.to_rgba8();
        let (w, h) = img.dimensions();
        Ok(Self { width: w, height: h, rgba: img.into_raw() })
    }
    /// Decode as [`Raster::decode`], but an image of more than [`MAX_PIXELS`] (up to
    /// [`MAX_SOURCE_PIXELS`]) is scaled down, aspect kept, to the largest size within
    /// [`MAX_PIXELS`] instead of being refused: a 300 ppi scan of a poster traces at slightly
    /// less than its own resolution. The caller maps the result back with `width` / `height`.
    pub fn decode_to_fit(bytes: &[u8]) -> Result<Self, TraceError> {
        let decode_err = |e: image::ImageError| TraceError::Decode(e.to_string());
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| TraceError::Decode(e.to_string()))?;
        let (w, h) = reader.into_dimensions().map_err(decode_err)?;
        if w == 0 || h == 0 {
            return Err(TraceError::Empty);
        }
        let pixels = u64::from(w) * u64::from(h);
        if pixels <= MAX_PIXELS {
            return Self::decode(bytes);
        }
        if pixels > MAX_SOURCE_PIXELS {
            return Err(TraceError::TooLarge { width: w, height: h, max: MAX_SOURCE_PIXELS >> 20 });
        }
        // Decoded within the allocation the source limit needs; the web keeps file import's 512 MB.
        // A deeper image (16 bits a channel) that needs more fails with a decode error.
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| TraceError::Decode(e.to_string()))?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(if cfg!(target_arch = "wasm32") { 512 << 20 } else { MAX_SOURCE_PIXELS * 4 });
        reader.limits(limits);
        let img = reader.decode().map_err(decode_err)?.into_rgba8();
        let (w, h) = img.dimensions();
        let (nw, nh) = fit_size(w, h, MAX_PIXELS);
        // Box-filtered: every source pixel counts, so thin lines and anti-aliased edges survive.
        let small = image::imageops::thumbnail(&img, nw, nh);
        Ok(Self { width: nw, height: nh, rgba: small.into_raw() })
    }
    /// Encode as PNG.
    pub fn encode_png(&self) -> Vec<u8> {
        let mut out = Vec::new();
        if let Some(img) = image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone()) {
            let _ = img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png);
        }
        out
    }
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = (y as usize * self.width as usize + x as usize) * 4;
        [self.rgba[i], self.rgba[i + 1], self.rgba[i + 2], self.rgba[i + 3]]
    }
}

/// Tracing mode (Illustrator's Mode popup).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    #[serde(alias = "bw", alias = "blackWhite", alias = "Black and White")]
    BlackAndWhite,
    #[serde(alias = "gray", alias = "Grayscale")]
    Grayscale,
    #[serde(alias = "Color")]
    Color,
    /// Flat-colour logos: sub-pixel edges, sharp corners restored, true lines and circles.
    #[serde(alias = "Logo", alias = "flat", alias = "flatLogo")]
    Logo,
}

/// How colour layers relate (Advanced → Method).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Method {
    /// Each colour traced on its own; neighbouring shapes share their edge.
    #[default]
    Abutting,
    /// Stacked: every layer extends under the layers above it.
    Overlapping,
}

/// Where Color mode's colours come from (the Palette popup).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Palette {
    /// The `colors` colours that best represent the image.
    #[default]
    #[serde(alias = "Limited")]
    Limited,
    /// As many colours as the image's tones need, up to [`FULL_TONE_MAX`] (photos, gradients):
    /// `color_detail` sets how close two colours may be and stay apart.
    #[serde(alias = "Full Tone")]
    FullTone,
    /// Flat art (a few colours cover nearly all of it) traces with exactly those colours, anything
    /// else as Full Tone.
    #[serde(alias = "Automatic")]
    Automatic,
    /// The most used colours of a swatch library ([`TraceParams::swatches`]), at most `colors` of
    /// them, exactly as they are.
    #[serde(alias = "library", alias = "Document Library")]
    DocumentLibrary,
}

/// Image Trace parameters (the Image Trace panel).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TraceParams {
    pub mode: Mode,
    /// Black and White: pixels darker than this (luminance 0–255) are black.
    pub threshold: u8,
    /// Color: where the colours come from.
    pub palette: Palette,
    /// Palette size for Color (Limited; at most this many for Document Library), number of grays
    /// for Grayscale (2–256).
    pub colors: u32,
    /// Full Tone and Automatic: how many colours, 0 (few) – 100 (many).
    pub color_detail: f64,
    /// Document Library: the swatch library, `"document"` (the document's swatches) or a library id
    /// or name. The caller looks its colours up into `swatches`.
    pub library: String,
    /// Document Library: the library's colours (sRGB). Not saved: they are looked up from
    /// `library` at every trace.
    #[serde(skip)]
    pub swatches: Vec<[u8; 3]>,
    /// Paths (fidelity) 0–100: higher follows the pixels more tightly.
    pub paths: f64,
    /// Corners 0–100: higher keeps more corners.
    pub corners: f64,
    /// Noise: areas smaller than this many pixels are ignored. (Flat Logo mode: areas smaller than
    /// `noise / 10` pixels of the source image.)
    pub noise: u32,
    pub method: Method,
    /// Drop white areas (no white background shape).
    pub ignore_white: bool,
    /// Replace nearly-straight curves with straight lines. In Logo mode this also snaps tight arcs
    /// to true circles.
    pub snap_curves_to_lines: bool,
    /// Create: trace areas as filled shapes.
    pub fills: bool,
    /// Create: trace lines no wider than `stroke_width` as stroked centre lines.
    pub strokes: bool,
    /// Stroke: the widest feature (px) traced as a stroke.
    pub stroke_width: f64,
    /// Flat Logo mode: the flat colours (`#rrggbb`) to use; empty finds them automatically. White is
    /// always traced on its own and cannot be named, colours closer than about 12 (RGB distance)
    /// count as one, and at most ten are used. (Not `palette`: that is Color mode's colour source.)
    pub logo_colors: Vec<String>,
}

impl Default for TraceParams {
    /// Illustrator's [Default]: black and white, threshold 128.
    fn default() -> Self {
        Self {
            mode: Mode::BlackAndWhite,
            threshold: 128,
            palette: Palette::Limited,
            colors: 6,
            color_detail: 50.0,
            library: "document".into(),
            swatches: vec![],
            paths: 50.0,
            corners: 75.0,
            noise: 25,
            method: Method::Abutting,
            ignore_white: false,
            snap_curves_to_lines: false,
            fills: true,
            strokes: false,
            stroke_width: 10.0,
            logo_colors: Vec::new(),
        }
    }
}

impl TraceParams {
    /// Maximum distance (px) of polygon vertices from the pixel boundary.
    fn polygon_tolerance(&self) -> f64 {
        let f = (self.paths / 100.0).clamp(0.0, 1.0);
        0.55 + 0.75 * (1.0 - f)
    }
    /// Curve fitting tolerance (px).
    fn fit_tolerance(&self) -> f64 {
        let f = (self.paths / 100.0).clamp(0.0, 1.0);
        0.2 + 1.6 * (1.0 - f)
    }
    /// How outlines are fitted: Paths sets how closely the curves follow the edge (0.08 px at 100,
    /// 0.33 px at 50, 0.78 px at 0), Corners the turn that makes a corner (35° at 100, 90° at 0).
    /// Straight edges are always lines; Snap Curves To Lines also straightens slightly curved ones
    /// and makes parts of circles true arcs.
    fn outline_style(&self) -> logo::Style {
        // Parameters come from users and agents: a NaN or infinity must not reach the geometry.
        let unit = |v: f64, default: f64| if v.is_finite() { (v / 100.0).clamp(0.0, 1.0) } else { default };
        let snap = self.snap_curves_to_lines;
        logo::Style {
            step: 0.25,
            tol: 0.08 + 0.7 * (1.0 - unit(self.paths, 0.5)).powf(1.5),
            corner_deg: 90.0 - 55.0 * unit(self.corners, 0.75),
            snap: true,
            arcs: snap,
            line_tol: if snap { 0.5 } else { 0.2 },
            circle_tol: if snap { 0.3 } else { 0.15 },
            restore: Some(logo::RestoreParams::default()),
            check_step: 0.15,
            ..logo::Style::default()
        }
    }
    /// Turn angle (degrees) above which a polygon vertex stays a corner.
    fn corner_angle(&self) -> f64 {
        let c = (self.corners / 100.0).clamp(0.0, 1.0);
        150.0 - 115.0 * c
    }
}

/// Built-in preset names in panel order (our own parameter sets).
pub const PRESET_NAMES: &[&str] = &[
    "Default",
    "High Fidelity Photo",
    "Low Fidelity Photo",
    "3 Colors",
    "6 Colors",
    "16 Colors",
    "Shades of Gray",
    "Black and White Logo",
    "Sketched Art",
    "Silhouettes",
    "Line Art",
    "Technical Drawing",
    "Flat Logo",
];

/// A built-in preset by name (case-insensitive; `[Default]` is accepted).
pub fn preset(name: &str) -> Option<TraceParams> {
    let key = name.trim().trim_start_matches('[').trim_end_matches(']').to_ascii_lowercase();
    let d = TraceParams::default();
    let color = |colors: u32, paths: f64, corners: f64, noise: u32, method: Method| TraceParams {
        mode: Mode::Color,
        colors,
        paths,
        corners,
        noise,
        method,
        ..TraceParams::default()
    };
    let bw = |threshold: u8, paths: f64, corners: f64, noise: u32, snap: bool| TraceParams {
        mode: Mode::BlackAndWhite,
        threshold,
        paths,
        corners,
        noise,
        ignore_white: true,
        snap_curves_to_lines: snap,
        ..TraceParams::default()
    };
    Some(match key.as_str() {
        "default" => d,
        "high fidelity photo" => color(64, 90.0, 25.0, 4, Method::Overlapping),
        "low fidelity photo" => color(20, 60.0, 50.0, 12, Method::Overlapping),
        "3 colors" => color(3, 60.0, 60.0, 20, Method::Abutting),
        "6 colors" => color(6, 65.0, 60.0, 16, Method::Abutting),
        "16 colors" => color(16, 70.0, 55.0, 10, Method::Abutting),
        "shades of gray" => TraceParams { mode: Mode::Grayscale, colors: 8, paths: 60.0, corners: 50.0, noise: 10, method: Method::Overlapping, ..d },
        "black and white logo" => bw(128, 95.0, 80.0, 8, true),
        "sketched art" => bw(150, 50.0, 50.0, 100, false),
        "silhouettes" => bw(200, 40.0, 60.0, 30, false),
        "line art" => bw(128, 80.0, 70.0, 10, false),
        "technical drawing" => bw(128, 95.0, 90.0, 4, true),
        // Flat-colour logos: colours found automatically, edges placed to a fraction of a pixel.
        "flat logo" => TraceParams { mode: Mode::Logo, colors: 8, paths: 60.0, corners: 50.0, noise: 6, snap_curves_to_lines: true, ..d },
        _ => return None,
    })
}

/// All built-in presets in panel order.
pub fn presets() -> Vec<(&'static str, TraceParams)> {
    PRESET_NAMES.iter().filter_map(|n| Some((*n, preset(n)?))).collect()
}

/// One traced shape: an outer contour plus its holes, filled with `color`, or (with `stroke`)
/// lines stroked with it.
#[derive(Clone, Debug, PartialEq)]
pub struct TracedPath {
    /// In pixel coordinates (x right, y down; the image spans `0..width × 0..height`).
    pub path: PathData,
    pub color: [u8; 3],
    /// Filled pixel count of the component the path came from.
    pub pixels: usize,
    /// Create Strokes: the path is stroked this wide (px), not filled (centre lines, or the
    /// outlines of wider areas without Fills).
    pub stroke: Option<f64>,
}

/// The traced result, bottom-most path first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TraceResult {
    pub paths: Vec<TracedPath>,
    /// Colours actually used.
    pub palette: Vec<[u8; 3]>,
}

impl TraceResult {
    pub fn anchor_count(&self) -> usize {
        self.paths.iter().map(|p| p.path.anchor_count()).sum()
    }
}

fn is_white(c: [u8; 3]) -> bool {
    c.iter().all(|&v| v >= 245)
}

/// Trace `img` with `params`. Failures (an image Logo mode refuses, say) give an empty result: use
/// [`trace_within`] to see why.
pub fn trace(img: &Raster, params: &TraceParams) -> TraceResult {
    // No trace has more than `usize::MAX` anchors.
    trace_within(img, params, usize::MAX).unwrap_or_default()
}

/// Trace `img` with `params`, giving up with [`TraceError::TooComplex`] once the paths have more
/// than `max_anchors` anchor points (a photo traced at high fidelity can make millions). The
/// colour layers are traced in parallel where threads are available.
pub fn trace_within(img: &Raster, params: &TraceParams, max_anchors: usize) -> Result<TraceResult, TraceError> {
    let (w, h) = (img.width as usize, img.height as usize);
    if w == 0 || h == 0 || img.rgba.len() != w * h * 4 {
        return Ok(TraceResult::default());
    }
    if params.mode == Mode::Logo {
        return logo::trace_logo(img, params, max_anchors);
    }
    let mut q = quantize(img, params);
    denoise(&mut q.labels, w, h, params.noise as usize);
    // Pixel count per palette entry.
    let mut counts = vec![0usize; q.palette.len()];
    for &l in &q.labels {
        if let Some(c) = counts.get_mut(l as usize) {
            *c += 1;
        }
    }
    // Layers bottom → top: largest area first.
    let mut order: Vec<usize> = (0..q.palette.len()).filter(|&i| counts[i] > 0).collect();
    order.sort_by(|a, b| counts[*b].cmp(&counts[*a]).then(a.cmp(b)));
    let mut rank = vec![usize::MAX; q.palette.len()];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    let layers: Vec<(usize, usize)> =
        order.iter().copied().enumerate().filter(|&(_, ci)| !(params.ignore_white && is_white(q.palette[ci]))).collect();
    let job = Layers {
        q: &q,
        rank: &rank,
        params,
        img,
        bw_tones: bw_tones(img, &q),
        style: params.outline_style(),
        opts: fit::FitOptions {
            polygon_tol: params.polygon_tolerance(),
            fit_tol: params.fit_tolerance(),
            corner_angle: params.corner_angle(),
            snap_lines: params.snap_curves_to_lines,
        },
        w,
        h,
        anchors: AtomicUsize::new(0),
        max_anchors,
    };
    let mut out = TraceResult::default();
    for (paths, &(_, ci)) in parallel_map(&layers, |&(r, ci)| job.layer(r, ci)).into_iter().zip(&layers) {
        out.paths.extend(paths.ok_or(TraceError::TooComplex { max: max_anchors })?);
        let color = q.palette[ci];
        if !out.palette.contains(&color) {
            out.palette.push(color);
        }
    }
    Ok(out)
}

/// The mean luminance of the pixels labelled 0 (white) and 1 (black) of a Black and White
/// quantisation: pure white and black where a side has no pixel.
fn bw_tones(img: &Raster, q: &Quantized) -> [f64; 2] {
    let (mut sum, mut n) = ([0.0f64; 2], [0.0f64; 2]);
    for (p, &l) in img.rgba.as_chunks::<4>().0.iter().zip(&q.labels) {
        if let (Some(s), Some(n)) = (sum.get_mut(l as usize), n.get_mut(l as usize)) {
            *s += (f64::from(p[0]) * 299.0 + f64::from(p[1]) * 587.0 + f64::from(p[2]) * 114.0) / 1000.0;
            *n += 1.0;
        }
    }
    [if n[0] > 0.0 { sum[0] / n[0] } else { 255.0 }, if n[1] > 0.0 { sum[1] / n[1] } else { 0.0 }]
}

/// What tracing one colour layer needs.
struct Layers<'a> {
    q: &'a Quantized,
    /// Each palette entry's layer, bottom (0) up.
    rank: &'a [usize],
    params: &'a TraceParams,
    img: &'a Raster,
    /// Black and White: the mean luminance of the white pixels and of the black ones.
    bw_tones: [f64; 2],
    /// How outlines are fitted.
    style: logo::Style,
    /// How centre lines are fitted.
    opts: fit::FitOptions,
    w: usize,
    h: usize,
    /// Anchor points made so far, by every layer.
    anchors: AtomicUsize,
    max_anchors: usize,
}

impl Layers<'_> {
    /// How far the edge lies from a crack between two pixels, towards the outside one (−0.5 to 0.5).
    ///
    /// Each pixel of the column across the crack is read as a blend of the tones on its two sides
    /// (the two pixels' palette colours; in Black and White the mean luminance of the black and of
    /// the white pixels). The edge is where the blends, summed, put it: as far out as there is
    /// inside tone in the outside pixels, less the outside tone in the inside ones (two pixels each
    /// way, while they are on the same side and no more blended than the pixel before: more would be
    /// another edge's doing).
    ///
    /// Where the inside is at most three pixels deep with the outside's colour behind it (a thin
    /// line), its two edges share those pixels and cannot be read apart; the inside tone of the
    /// whole column gives the line's middle (its centroid) and width (its sum) instead. A thin gap
    /// between two parts of the inside is read the same way, with the tones swapped.
    ///
    /// Black and White then moves the edge to where the luminance crosses the threshold: by the
    /// threshold's distance from the middle tone, over how steep the edge is (nothing for a
    /// threshold in the middle, more the softer the edge).
    fn shift(&self, c: Crack) -> f64 {
        let label = |k: i32| c.at(k).and_then(|i| self.q.labels.get(i).copied()).filter(|&l| l != TRANSPARENT);
        let (Some(la), Some(lb)) = (label(0), label(1)) else { return 0.0 };
        if la == lb {
            return 0.0;
        }
        let bw = self.params.mode == Mode::BlackAndWhite;
        let (ca, cb) = if bw {
            let tone = |l: u16| [self.bw_tones[usize::from(l == 1)]; 3];
            (tone(la), tone(lb))
        } else {
            let (Some(ca), Some(cb)) = (self.q.palette.get(la as usize), self.q.palette.get(lb as usize)) else { return 0.0 };
            (ca.map(f64::from), cb.map(f64::from))
        };
        let d = [0, 1, 2].map(|k| cb[k] - ca[k]);
        let len2: f64 = d.iter().map(|v| v * v).sum();
        if len2 <= 0.0 {
            return 0.0;
        }
        // How much of the inside tone (0 to 1) the pixel `k` steps out has; none off the image.
        let inside = |k: i32| {
            let Some(p) = c.at(k).and_then(|i| self.img.rgba.get(i * 4..i * 4 + 3)) else { return 0.0 };
            let p = [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])];
            let p = if bw { [(p[0] * 299.0 + p[1] * 587.0 + p[2] * 114.0) / 1000.0; 3] } else { p };
            1.0 - ((0..3).map(|k| (p[k] - ca[k]) * d[k]).sum::<f64>() / len2).clamp(0.0, 1.0)
        };
        // The edge from a thin run of `n` pixels starting at pixel `from` and going `dir`, by the
        // amounts `tone` of the run's own tone in it and in the two pixels at either end of it.
        // Returns the run's middle and width, measured from the crack towards the outside.
        let thin = |from: i32, dir: i32, n: i32, tone: &dyn Fn(i32) -> f64| {
            let (near, far) = (from - dir, from + n * dir);
            let mut cells: Vec<(i32, f64)> = (0..n).map(|j| from + j * dir).chain([near, far]).map(|k| (k, tone(k))).collect();
            for (edge, out) in [(near, -dir), (far, dir)] {
                if label(edge + out) == label(edge) {
                    cells.push((edge + out, tone(edge + out).min(tone(edge))));
                }
            }
            let width: f64 = cells.iter().map(|c| c.1).sum();
            // Pixel `k`'s centre is `k - 0.5` from the crack.
            (width > 1e-6).then(|| (cells.iter().map(|c| c.1 * (f64::from(c.0) - 0.5)).sum::<f64>() / width, width))
        };
        // How deep each side is here: up to three pixels of its own label, then the other's.
        let depth = |dir: i32, own: u16, other: u16| {
            let start = if dir < 0 { 0 } else { 1 };
            let n = (1..=3).take_while(|j| label(start + j * dir) == Some(own)).count() as i32 + 1;
            (n <= 3 && label(start + n * dir) == Some(other)).then_some(n)
        };
        let mut steep = 1.0;
        let mut shift = if let Some((middle, width)) = depth(-1, la, lb).and_then(|n| thin(0, -1, n, &inside)) {
            middle + width / 2.0
        } else if let Some((middle, width)) = depth(1, lb, la).and_then(|n| thin(1, 1, n, &|k| 1.0 - inside(k))) {
            middle - width / 2.0
        } else {
            let (a1, b1) = (1.0 - inside(0), inside(1));
            let a2 = (label(-1) == Some(la)).then(|| (1.0 - inside(-1)).min(a1));
            let b2 = (label(2) == Some(lb)).then(|| inside(2).min(b1));
            // The edge's steepness per pixel: the square of the change over two pixels (1 for an
            // anti-aliased edge, less for a blurred one).
            let change = [a2.map(|v| 1.0 - v - b1), b2.map(|v| 1.0 - a1 - v)].into_iter().flatten().fold(f64::NAN, f64::max);
            if !change.is_nan() {
                steep = (change * change).clamp(0.35, 1.0);
            }
            b1 + b2.unwrap_or(0.0) - a1 - a2.unwrap_or(0.0)
        };
        if bw {
            // The share of black at which a pixel turns black.
            let (white, black) = (self.bw_tones[0], self.bw_tones[1]);
            let turn = ((white - f64::from(self.params.threshold)) / (white - black).max(1.0)).clamp(0.02, 0.98);
            shift += if la == 1 { 1.0 } else { -1.0 } * (0.5 - turn) / steep;
        }
        shift
    }

    /// The paths of layer `r` (palette entry `ci`), or `None` once the anchors made pass the
    /// maximum.
    fn layer(&self, r: usize, ci: usize) -> Option<Vec<TracedPath>> {
        let (params, opts) = (self.params, &self.opts);
        let color = *self.q.palette.get(ci)?;
        let overlapping = params.method == Method::Overlapping;
        let (w, h) = (self.w, self.h);
        // Create Strokes: this colour's lines, left out of its areas.
        let (thin, lines) = if params.strokes {
            centerline::lines(&self.q.labels, u16::try_from(ci).unwrap_or(TRANSPARENT), w, h, params.stroke_width, opts)
        } else {
            (vec![], vec![])
        };
        let mask: Vec<bool> = self
            .q
            .labels
            .iter()
            .enumerate()
            .map(|(i, &l)| {
                l != TRANSPARENT
                    && !thin.get(i).copied().unwrap_or(false)
                    && if overlapping { self.rank.get(l as usize).is_some_and(|&lr| lr >= r) } else { l as usize == ci }
            })
            .collect();
        // Without Fills, areas are outlined with a 1 px stroke.
        let area_stroke = (params.strokes && !params.fills).then_some(1.0);
        let min_hole = params.noise.max(1) as i64;
        let mut paths = vec![];
        let mut add = |path: PathData, pixels: usize, stroke: Option<f64>| {
            let n = path.anchor_count();
            if self.anchors.fetch_add(n, Ordering::Relaxed).saturating_add(n) > self.max_anchors {
                return false;
            }
            paths.push(TracedPath { path, color, pixels, stroke });
            true
        };
        for comp in trace_mask(&mask, w, h) {
            let fit = |l: &Loop| fit::fit_loop(l, w, h, &|c| self.shift(c), &self.style);
            let Some(outer) = fit(&comp.outer) else { continue };
            let mut subs = vec![outer];
            for hole in &comp.holes {
                if hole.area2.abs() / 2 < min_hole && params.noise > 0 {
                    continue;
                }
                subs.extend(fit(hole));
            }
            if !add(PathData::new(subs), comp.pixels, area_stroke) {
                return None;
            }
        }
        for line in lines {
            if !add(line.path, line.pixels, Some(line.width)) {
                return None;
            }
        }
        Some(paths)
    }
}

/// `f` of each of `items`, in order, spread over the available cores.
#[cfg(not(target_arch = "wasm32"))]
fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(items.len());
    if threads <= 1 {
        return items.iter().map(f).collect();
    }
    let next = AtomicUsize::new(0);
    let mut done: Vec<(usize, R)> = std::thread::scope(|s| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                s.spawn(|| {
                    let mut mine = vec![];
                    loop {
                        let i = next.fetch_add(1, Ordering::Relaxed);
                        let Some(item) = items.get(i) else { break };
                        mine.push((i, f(item)));
                    }
                    mine
                })
            })
            .collect();
        // A worker's panic goes on in this thread, as if it had run here (the engine's guard
        // reports it).
        workers.into_iter().flat_map(|w| w.join().unwrap_or_else(|e| std::panic::resume_unwind(e))).collect()
    });
    done.sort_by_key(|(i, _)| *i);
    done.into_iter().map(|(_, r)| r).collect()
}

/// `f` of each of `items`, in order (no threads on the web).
#[cfg(target_arch = "wasm32")]
fn parallel_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    items.iter().map(f).collect()
}

#[cfg(test)]
mod tests;
