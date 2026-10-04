//! Soft proofing (View → Proof Colors), Overprint Preview and Separations Preview.
//!
//! * **Proof Colors** post-processes the rendered frame: every pixel goes display sRGB → proof
//!   space (CMYK press or RGB/colour-blindness simulation) → display, through a cached 17³ LUT.
//! * **Separations Preview** recolours the *document* before rendering: each paint colour is split
//!   into process inks (colour-managed) plus a spot ink when it's linked to a spot swatch. With one
//!   plate visible the plate renders as greyscale ink coverage (black = 100%); with several, the
//!   visible inks are composited through the proof profile, spots multiplied on top.
//!   Placed raster images are not separated (limitation).
//! * **Overprint Preview** (and Separations Preview, which implies it): fills and strokes that
//!   overprint ([`vectorcraft_doc::FillLayer::overprint`], characters' too) are drawn with
//!   Multiply, which approximates their inks printing over the inks below: a zero ink lets the
//!   inks below show through, where a knockout would replace them.
//!
//! The app's current view state lives in [`view`] / [`set_view`]; the canvas copies it into
//! [`crate::RenderOptions`] with [`active_proof`] and [`overprint_preview_on`].

use std::borrow::Cow;
use std::sync::{Arc, RwLock};

use vectorcraft_color::cms::{self, Cms, PROCESS_PLATES};
pub use vectorcraft_color::cms::{Intent, ProofSetup, ProofTarget};
use vectorcraft_color::{BlendMode, Color, Paint};
use vectorcraft_doc::{AppearanceItem, Document, Node, NodeKind};

use crate::RenderOptions;

/// Ink coverage of one paint colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Inks {
    /// Process inks C, M, Y, K (0..1).
    pub cmyk: [f32; 4],
    /// Spot ink (swatch name, tint 0..1).
    pub spot: Option<(String, f32)>,
}

/// A printing plate.
#[derive(Clone, Debug, PartialEq)]
pub struct Plate {
    pub name: String,
    pub spot: bool,
    /// Display colour of the ink (for the panel swatch).
    pub rgb: [f32; 3],
}

/// The plates of `doc`: the four process plates plus one per spot swatch.
pub fn plates(doc: &Document) -> Vec<Plate> {
    let c = cms::active();
    let mut v: Vec<Plate> = PROCESS_PLATES
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut ink = [0.0; 4];
            ink[i] = 1.0;
            Plate { name: (*n).into(), spot: false, rgb: c.cmyk_to_srgb(ink, false) }
        })
        .collect();
    for sw in doc.swatches_iter() {
        if let (true, Paint::Solid { color, .. }) = (sw.spot, &sw.paint)
            && !v.iter().any(|p| p.name == sw.name)
        {
            v.push(Plate { name: sw.name.clone(), spot: true, rgb: c.display_rgb(color) });
        }
    }
    v
}

fn spot_swatch<'a>(doc: &'a Document, name: &str) -> Option<&'a Color> {
    doc.swatch(name).filter(|s| s.spot).and_then(|s| match &s.paint {
        Paint::Solid { color, .. } => Some(color),
        _ => None,
    })
}

/// Separate one colour into inks. Colours linked to a spot swatch print on that plate only, with
/// the tint given by the colour's ink total relative to the swatch's.
pub fn inks(doc: &Document, c: &Cms, color: &Color, swatch: Option<&str>, intent: Intent) -> Inks {
    if let Some(name) = swatch
        && let Some(sc) = spot_swatch(doc, name)
    {
        let full: f32 = c.to_cmyk(sc, intent).iter().sum();
        let this: f32 = c.to_cmyk(color, intent).iter().sum();
        let tint = if full <= 1e-4 { 1.0 } else { (this / full).clamp(0.0, 1.0) };
        return Inks { cmyk: [0.0; 4], spot: Some((name.to_string(), tint)) };
    }
    Inks { cmyk: c.to_cmyk(color, intent), spot: None }
}

fn map_paint(p: &mut Paint, f: &mut dyn FnMut(&Color, Option<&str>) -> Color) {
    match p {
        Paint::Solid { color, swatch } => *color = f(color, swatch.as_deref()),
        Paint::Gradient(g) => {
            for s in &mut g.gradient.stops {
                s.color = f(&s.color, None);
            }
        }
        _ => {}
    }
}

/// Apply `f` to every colour of a node tree (fills, strokes, gradient stops, text runs, mesh
/// points), keeping swatch links.
pub fn map_node_colors(n: &mut Node, f: &mut dyn FnMut(&Color, Option<&str>) -> Color) {
    for it in &mut n.appearance.items {
        match it {
            AppearanceItem::Fill(l) => map_paint(&mut l.paint, f),
            AppearanceItem::Stroke(l) => map_paint(&mut l.paint, f),
        }
    }
    match &mut n.kind {
        NodeKind::Text(t) => {
            for r in &mut t.runs {
                map_paint(&mut r.style.fill, f);
                map_paint(&mut r.style.stroke, f);
            }
        }
        NodeKind::Mesh(m) => {
            for p in &mut m.points {
                p.color = f(&p.color, None);
            }
        }
        _ => {}
    }
    if let Some(ch) = n.children_mut() {
        for c in ch.iter_mut() {
            map_node_colors(Arc::make_mut(c), f);
        }
    }
}

/// Apply `f` to every colour in the document's art and symbol definitions.
pub fn map_document_colors(doc: &mut Document, f: &mut dyn FnMut(&Color, Option<&str>) -> Color) {
    for l in &mut doc.layers {
        map_node_colors(Arc::make_mut(l), f);
    }
    for s in &mut doc.symbols {
        map_node_colors(Arc::make_mut(&mut s.art), f);
    }
}

/// Whether overprinting shows: Overprint Preview, or Separations Preview (which implies it).
pub(crate) fn overprints(opts: &RenderOptions) -> bool {
    opts.overprint_preview || opts.proof.as_ref().is_some_and(|p| p.separations.is_some())
}

/// Draw the overprinting fills and strokes of `a`'s subtree with Multiply (those with a blend
/// mode of their own keep it), copying only the nodes on the way to them.
fn multiply_overprints(a: &mut Arc<Node>) {
    if !a.has_overprint() {
        return;
    }
    let n = Arc::make_mut(a);
    for it in &mut n.appearance.items {
        match it {
            AppearanceItem::Fill(l) if l.overprint && l.blend == BlendMode::Normal => l.blend = BlendMode::Multiply,
            AppearanceItem::Stroke(l) if l.overprint && l.blend == BlendMode::Normal => l.blend = BlendMode::Multiply,
            _ => {}
        }
    }
    for c in n.children_mut().into_iter().flatten() {
        multiply_overprints(c);
    }
}

fn plate_color(doc: &Document, c: &Cms, proof: &ProofSetup, visible: &[String], color: &Color, swatch: Option<&str>) -> Color {
    let ink = inks(doc, c, color, swatch, proof.intent);
    let rgb = if visible.len() == 1 {
        let name = &visible[0];
        let v = match PROCESS_PLATES.iter().position(|p| p == name) {
            Some(i) => ink.cmyk[i],
            None => ink.spot.as_ref().filter(|(n, _)| n == name).map_or(0.0, |s| s.1),
        };
        [1.0 - v; 3]
    } else {
        let mut cmyk = ink.cmyk;
        for (i, p) in PROCESS_PLATES.iter().enumerate() {
            if !visible.iter().any(|v| v == p) {
                cmyk[i] = 0.0;
            }
        }
        let mut rgb = c.proof_cmyk_to_srgb(cmyk, proof);
        if let Some((name, t)) = &ink.spot
            && visible.contains(name)
            && let Some(sc) = spot_swatch(doc, name)
        {
            let s = c.display_rgb(sc);
            for i in 0..3 {
                rgb[i] *= 1.0 - t * (1.0 - s[i]);
            }
        }
        rgb
    };
    let [r, g, b] = c.srgb_to_rgb(rgb);
    Color::Rgb { r, g, b }
}

/// The document as it should be drawn for these options (overprints, separations).
pub(crate) fn prepare<'a>(doc: &'a Document, opts: &RenderOptions) -> Cow<'a, Document> {
    let seps = opts.proof.as_ref().and_then(|p| p.separations.as_ref().map(|s| (p, s)));
    let overprint = overprints(opts) && (doc.layers.iter().any(|l| l.has_overprint()) || doc.symbols.iter().any(|s| s.art.has_overprint()));
    if !overprint && seps.is_none() {
        return Cow::Borrowed(doc);
    }
    let mut d = doc.clone();
    if overprint {
        for l in &mut d.layers {
            multiply_overprints(l);
        }
        for s in &mut d.symbols {
            multiply_overprints(&mut s.art);
        }
    }
    if let Some((proof, visible)) = seps {
        let c = cms::active();
        let src = doc.clone();
        map_document_colors(&mut d, &mut |col, sw| plate_color(&src, &c, proof, visible, col, sw));
    }
    Cow::Owned(d)
}

/// Soft-proof the rendered (premultiplied RGBA8) pixels in place.
pub(crate) fn post(pixels: &mut [u8], opts: &RenderOptions) {
    let Some(proof) = opts.proof.as_ref() else { return };
    if proof.separations.is_some() || matches!(proof.target, ProofTarget::MonitorRgb | ProofTarget::Srgb) {
        return;
    }
    let lut = cms::active().proof_lut(proof);
    let mut last_in = [0u8; 4];
    let mut last_out = [0u8; 4];
    for px in pixels.chunks_exact_mut(4) {
        let a = px[3];
        if a == 0 {
            continue;
        }
        if *px == last_in {
            px.copy_from_slice(&last_out);
            continue;
        }
        last_in.copy_from_slice(px);
        let un = |c: u8| if a == 255 { c } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
        let out = lut.apply8([un(px[0]), un(px[1]), un(px[2])]);
        for i in 0..3 {
            px[i] = if a == 255 { out[i] } else { ((out[i] as u32 * a as u32 + 127) / 255) as u8 };
        }
        last_out.copy_from_slice(px);
    }
}

/// View-level proof state (View → Proof Setup / Proof Colors / Overprint Preview, Separations
/// Preview panel). Process-wide: the app has one proof state for all windows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProofView {
    /// View → Proof Setup.
    pub setup: ProofSetup,
    /// View → Proof Colors.
    pub proof_colors: bool,
    /// View → Overprint Preview.
    pub overprint: bool,
    /// Separations Preview: `Some(visible plates)` when on.
    pub separations: Option<Vec<String>>,
}

static VIEW: RwLock<Option<ProofView>> = RwLock::new(None);

pub fn view() -> ProofView {
    VIEW.read().unwrap_or_else(|e| e.into_inner()).clone().unwrap_or_default()
}

pub fn set_view(v: ProofView) {
    *VIEW.write().unwrap_or_else(|e| e.into_inner()) = Some(v);
}

/// The proof to pass in [`RenderOptions::proof`] for the current view state.
pub fn active_proof() -> Option<ProofSetup> {
    let v = view();
    match v.separations {
        Some(s) => Some(ProofSetup { separations: Some(s), ..v.setup }),
        None if v.proof_colors => Some(ProofSetup { separations: None, ..v.setup }),
        None => None,
    }
}

/// Whether [`RenderOptions::overprint_preview`] should be on (Separations Preview implies it).
pub fn overprint_preview_on() -> bool {
    let v = view();
    v.overprint || v.separations.is_some()
}
