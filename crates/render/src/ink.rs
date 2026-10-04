//! Blending in CMYK documents.
//!
//! In a CMYK document blend modes and opacity composite inks, not screen colours: Multiply of cyan
//! over magenta prints both inks (blue, C100 M100 through the CMYK profile), where screen RGB would
//! give the product of the two display colours. The renderer composites additive values, so a CMYK
//! document that shows transparency is drawn twice, once per ink plane
//! ([`vectorcraft_color::blend::cmyk_planes`]): every colour painted as its complemented C, M and
//! Y ([`Ink::Cmy`]), then as its complemented K, a grey ([`Ink::K`]). Blend modes, opacity,
//! groups, knockouts and effects composite the planes exactly as they composite RGB, which is CMYK
//! blending. [`compose`] reads each pixel's inks back and shows them through the working CMYK
//! profile (a cached 4-D lookup table). Opacity masks keep taking their luminance from screen
//! colours; RGB colours and placed images are separated into the working CMYK space first.
//!
//! RGB documents, and CMYK documents without transparency, draw screen colours ([`Ink::Display`]).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use vectorcraft_color::Color;
use vectorcraft_color::blend::{cmyk_planes, planes_cmyk};
use vectorcraft_color::cms::{self, Cms, ProofLut, lab};
use vectorcraft_doc::{ColorMode, Document};
use vello_cpu::Pixmap;
use vello_cpu::peniko;

use crate::{RenderOptions, Renderer};

/// What a frame paints for colours.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub(crate) enum Ink {
    /// Screen colours.
    #[default]
    Display,
    /// The complemented C, M and Y inks.
    Cmy,
    /// The complemented K ink, as a grey.
    K,
}

impl Ink {
    /// The three values painted for colour `c`.
    pub(crate) fn rgb(self, c: &Color) -> [f32; 3] {
        match self {
            Ink::Display => c.to_rgb(),
            _ => self.plane(inks(c)),
        }
    }

    /// [`Self::rgb`] as a paint, at `alpha`.
    pub(crate) fn color(self, c: &Color, alpha: f32) -> peniko::Color {
        let [r, g, b] = self.rgb(c);
        peniko::Color::from_rgba8(q(r), q(g), q(b), q(alpha))
    }

    /// A fixed screen colour (pasteboard, guides, tile edges) as a paint.
    pub(crate) fn fixed(self, rgba: [u8; 4]) -> peniko::Color {
        let [r, g, b, a] = self.fixed8(rgba);
        peniko::Color::from_rgba8(r, g, b, a)
    }

    fn fixed8(self, [r, g, b, a]: [u8; 4]) -> [u8; 4] {
        match self {
            Ink::Display => [r, g, b, a],
            _ => {
                let lab = lab::srgb_to_lab([r, g, b].map(|v| v as f32 / 255.0));
                let [r, g, b] = self.plane(inks(&Color::Lab { l: lab.l, a: lab.a, b: lab.b })).map(q);
                [r, g, b, a]
            }
        }
    }

    fn plane(self, inks: [f32; 4]) -> [f32; 3] {
        cmyk_planes(inks)[(self == Ink::K) as usize]
    }
}

fn q(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Whether `doc` is drawn by ink planes: a CMYK document that shows transparency, unless drawn
/// in Outline view or Separations Preview (which paints plates).
pub(crate) fn blends_in_cmyk(doc: &Document, opts: &RenderOptions) -> bool {
    doc.color_mode == ColorMode::Cmyk
        && !opts.outline
        && opts.proof.as_ref().is_none_or(|p| p.separations.is_none())
        && (doc.layers.iter().any(|l| l.shows_transparency())
            || doc.symbols.iter().any(|s| s.art.shows_transparency())
            || doc.patterns.iter().any(|p| p.art.iter().any(|a| a.shows_transparency())))
}

/// The inks of `c` in the working CMYK space: CMYK as it is, grey as black ink, RGB and Lab
/// separated with the colour settings' intent (cached: separating searches the profile).
fn inks(c: &Color) -> [f32; 4] {
    let key = match *c {
        Color::Cmyk { c, m, y, k } => return [c, m, y, k],
        Color::Gray { k } => return [0.0, 0.0, 0.0, k],
        Color::Rgb { r, g, b } => [0, r.to_bits(), g.to_bits(), b.to_bits()],
        Color::Lab { l, a, b } => [1, l.to_bits(), a.to_bits(), b.to_bits()],
    };
    let cms = cms::active();
    SEPARATED.with(|s| {
        let (owner, seen) = &mut *s.borrow_mut();
        if !owner.as_ref().is_some_and(|o| Arc::ptr_eq(o, &cms)) || seen.len() >= 4096 {
            seen.clear();
            *owner = Some(cms.clone());
        }
        *seen.entry(key).or_insert_with(|| cms.to_cmyk(c, cms.settings().intent))
    })
}

/// Separated colours, for the colour settings they were separated with.
type Separated = (Option<Arc<Cms>>, HashMap<[u32; 4], [f32; 4]>);

thread_local! {
    static SEPARATED: RefCell<Separated> = RefCell::default();
}

/// Grid points per ink of [`InkLut`].
const LUT_N: usize = 17;

/// Inks → display sRGB through the working CMYK profile: a 17⁴ lookup table of XYZ, interpolated
/// quadrilinearly (inks mix close to linearly in light, and out-of-gamut colours are clipped
/// after interpolating, not before).
struct InkLut {
    data: Vec<[f32; 3]>,
}

impl InkLut {
    fn build(cms: &Cms) -> Self {
        let s = (LUT_N - 1) as f32;
        let mut data = Vec::with_capacity(LUT_N.pow(4));
        for c in 0..LUT_N {
            for m in 0..LUT_N {
                for y in 0..LUT_N {
                    for k in 0..LUT_N {
                        data.push(lab::lab_to_xyz(cms.cmyk_to_lab([c, m, y, k].map(|i| i as f32 / s))));
                    }
                }
            }
        }
        Self { data }
    }

    fn apply(&self, inks: [f32; 4]) -> [f32; 3] {
        const STRIDE: [usize; 4] = [LUT_N * LUT_N * LUT_N, LUT_N * LUT_N, LUT_N, 1];
        let p = inks.map(|v| v.clamp(0.0, 1.0) * (LUT_N - 1) as f32);
        let i = p.map(|v| (v as usize).min(LUT_N - 2));
        let base: usize = (0..4).map(|d| i[d] * STRIDE[d]).sum();
        let mut out = [0.0f32; 3];
        for corner in 0..16 {
            let (mut w, mut at) = (1.0f32, base);
            for d in 0..4 {
                let t = p[d] - i[d] as f32;
                if corner >> d & 1 == 1 {
                    w *= t;
                    at += STRIDE[d];
                } else {
                    w *= 1.0 - t;
                }
            }
            if w > 0.0 {
                for (o, v) in out.iter_mut().zip(self.data[at]) {
                    *o += w * v;
                }
            }
        }
        lab::xyz_to_srgb(out)
    }
}

/// Placed images as painted with [`Ink::Cmy`] and [`Ink::K`], per image key, with the tables they
/// were separated by.
pub(crate) type InkImages = HashMap<String, (Arc<[ProofLut; 2]>, [Arc<Pixmap>; 2])>;

/// A table built for the colour settings it was built with.
type Cached<T> = Mutex<Option<(Arc<Cms>, Arc<T>)>>;

static DISPLAY: Cached<InkLut> = Mutex::new(None);
/// Screen colours → [`Ink::Cmy`] and [`Ink::K`] values (placed images).
static SEPARATE: Cached<[ProofLut; 2]> = Mutex::new(None);

/// The table in `slot` for the active colour settings, built by `build` when they changed.
fn cached<T>(slot: &Cached<T>, build: impl FnOnce(&Cms) -> T) -> Arc<T> {
    let cms = cms::active();
    let mut s = slot.lock().unwrap_or_else(|e| e.into_inner());
    match &*s {
        Some((owner, t)) if Arc::ptr_eq(owner, &cms) => t.clone(),
        _ => {
            let t = Arc::new(build(&cms));
            *s = Some((cms, t.clone()));
            t
        }
    }
}

/// The picture of a CMYK document from its ink planes (premultiplied RGBA8 drawn with
/// [`Ink::Cmy`] and [`Ink::K`]): each pixel's inks shown through the working CMYK profile. Where
/// only the frame's `background` shows it keeps its exact screen colour.
pub(crate) fn compose(cmy: &[u8], k: &[u8], background: Option<[u8; 4]>) -> Vec<u8> {
    let lut = cached(&DISPLAY, InkLut::build);
    let bg = background.map(|b| (Ink::Cmy.fixed8(b), Ink::K.fixed8(b), b));
    let mut out = Vec::with_capacity(cmy.len());
    // Runs of one colour convert once.
    let mut last: Option<([u8; 8], [u8; 4])> = None;
    for (p, q) in cmy.as_chunks::<4>().0.iter().zip(k.as_chunks::<4>().0) {
        let key = [p[0], p[1], p[2], p[3], q[0], q[1], q[2], q[3]];
        let px = match (&last, &bg) {
            _ if p[3] == 0 => [0; 4],
            (Some((k0, v)), _) if *k0 == key => *v,
            (_, Some((bc, bk, b))) if p == bc && q == bk => *b,
            _ => {
                let un = |v: u8, a: u8| if a == 0 { 1.0 } else { (v as f32 / a as f32).min(1.0) };
                let a = p[3];
                let rgb = lut.apply(planes_cmyk([un(p[0], a), un(p[1], a), un(p[2], a)], un(q[0], q[3])));
                let pm = |v: f32| (v.clamp(0.0, 1.0) * a as f32).round() as u8;
                let v = [pm(rgb[0]), pm(rgb[1]), pm(rgb[2]), a];
                last = Some((key, v));
                v
            }
        };
        out.extend_from_slice(&px);
    }
    out
}

impl Renderer {
    /// Placed image `key` (decoded as `pm`) as painted with `ink`, cached.
    pub(crate) fn ink_image(&mut self, key: &str, pm: &Arc<Pixmap>, ink: Ink) -> Arc<Pixmap> {
        if ink == Ink::Display {
            return pm.clone();
        }
        let luts = cached(&SEPARATE, |cms| {
            let intent = cms.settings().intent;
            [Ink::Cmy, Ink::K].map(|ink| ProofLut::build(|rgb| ink.plane(cms.srgb_to_cmyk(rgb, intent))))
        });
        let at = (ink == Ink::K) as usize;
        if let Some((owner, planes)) = self.ink_images.get(key)
            && Arc::ptr_eq(owner, &luts)
        {
            return planes[at].clone();
        }
        let planes = [0, 1].map(|i| {
            let mut out = Pixmap::new(pm.width(), pm.height());
            for (o, s) in out.data_mut().iter_mut().zip(pm.data()) {
                let a = s.a;
                if a == 0 {
                    continue;
                }
                let un = |v: u8| (v as f32 / a as f32).min(1.0);
                let [r, g, b] = luts[i].apply([un(s.r), un(s.g), un(s.b)]).map(|v| (v.clamp(0.0, 1.0) * a as f32).round() as u8);
                *o = vello_cpu::color::PremulRgba8 { r, g, b, a };
            }
            Arc::new(out)
        });
        let out = planes[at].clone();
        self.ink_images.insert(key.to_string(), (luts, planes));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ink_table_matches_the_profile() {
        let cms = cms::active();
        let lut = InkLut::build(&cms);
        let mut worst = 0.0f32;
        // Between the grid points (where interpolation errs most) and on them.
        for i in 0..=20 {
            for j in 0..=20 {
                let inks = [i as f32 / 20.0, j as f32 / 20.0, ((i * 7 + j * 3) % 21) as f32 / 20.0, ((i + j * 5) % 21) as f32 / 20.0];
                let (a, b) = (lut.apply(inks), cms.cmyk_to_srgb(inks, false));
                worst = worst.max((0..3).map(|c| (a[c] - b[c]).abs()).fold(0.0, f32::max));
            }
        }
        // Within two 8-bit levels: far below a visible difference.
        assert!(worst * 255.0 < 2.5, "worst error {} levels", worst * 255.0);
    }

    #[test]
    fn fixed_colours_and_paper() {
        // White is paper on both planes; the display plane keeps screen colours exactly.
        assert_eq!(Ink::Cmy.fixed8([255; 4]), [255; 4]);
        assert_eq!(Ink::K.fixed8([255; 4]), [255; 4]);
        assert_eq!(Ink::Display.fixed8([12, 34, 56, 78]), [12, 34, 56, 78]);
        assert_eq!(Ink::K.rgb(&Color::gray(0.25)), [0.75; 3]);
        assert_eq!(Ink::Cmy.rgb(&Color::cmyk(0.1, 0.2, 0.3, 0.4)), [0.9, 0.8, 0.7]);
    }
}
