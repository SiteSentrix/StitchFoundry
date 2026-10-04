//! Shaping: text runs -> positioned glyphs in points (before line breaking).

use std::ops::Range;
use std::sync::Arc;

use harfrust::{Direction, Feature, ShapeOptions, UnicodeBuffer};
use skrifa::MetadataProvider;
use skrifa::instance::{LocationRef, Size};
use vectorcraft_doc::CharStyle;

use crate::features::OtFeatures;
use crate::fontdb::{FontDb, FontFace};

/// A shaped glyph with all character-style effects resolved, in points (y down).
#[derive(Clone, Debug)]
pub(crate) struct SGlyph {
    pub face: Arc<FontFace>,
    pub gid: u32,
    /// Byte offset of the cluster in the plain text, and its length in bytes.
    pub byte: usize,
    pub len: usize,
    pub run: usize,
    /// Advance in points (tracking, manual kerning and horizontal scale applied).
    pub adv: f64,
    /// Offset from the pen position, y down.
    pub dx: f64,
    pub dy: f64,
    /// Outline scale (font units -> points).
    pub sx: f64,
    pub sy: f64,
    /// Baseline shift in points (positive = up).
    pub bshift: f64,
    /// Character rotation in degrees (counter-clockwise).
    pub rotation: f64,
    pub ascent: f64,
    pub descent: f64,
    pub leading: f64,
    /// Cap height and x height in points.
    pub cap: f64,
    pub xh: f64,
    /// First source character of the cluster.
    pub ch: char,
}

impl SGlyph {
    pub fn is_space(&self) -> bool {
        matches!(self.ch, ' ' | '\t' | '\u{3000}' | '\u{2002}'..='\u{200B}')
    }
    /// A line may break after this glyph.
    pub fn break_after(&self) -> bool {
        self.is_space() || matches!(self.ch, '-' | '\u{2010}' | '\u{2013}' | '\u{2014}' | '/' | SOFT_HYPHEN) || is_cjk(self.ch)
    }
    /// A soft (discretionary) hyphen: invisible unless a line breaks after it.
    pub fn is_soft_hyphen(&self) -> bool {
        self.ch == SOFT_HYPHEN
    }
    /// Part of a word that may be hyphenated (letters and apostrophes).
    pub fn is_letter(&self) -> bool {
        self.ch.is_alphabetic() || matches!(self.ch, '\'' | '’')
    }
}

pub(crate) const SOFT_HYPHEN: char = '\u{00AD}';

/// A visible hyphen in the face and size of `g`, placed at the end of `g`'s cluster (zero source
/// length) for a line broken inside a word.
pub(crate) fn hyphen_glyph(g: &SGlyph) -> SGlyph {
    let gid = ['-', '\u{2010}', SOFT_HYPHEN].into_iter().map(|c| g.face.glyph_for(c)).find(|&id| id != 0).unwrap_or(0);
    let mut h = g.clone();
    h.gid = gid;
    h.adv = g.face.advance(gid) * g.sx;
    h.dx = 0.0;
    h.dy = 0.0;
    h.byte = g.byte + g.len;
    h.len = 0;
    h.ch = '-';
    h
}

fn is_cjk(c: char) -> bool {
    matches!(c as u32, 0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x20000..=0x2FFFF)
}

/// Vertical metrics (points) of a style's resolved face: (ascent, descent, leading).
pub(crate) fn style_metrics(db: &FontDb, st: &CharStyle) -> (f64, f64, f64) {
    let vs = st.v_scale / 100.0;
    let Some(face) = db.face(&st.font_family, &st.font_style) else { return (st.size * 0.8 * vs, st.size * 0.2 * vs, st.effective_leading()) };
    let k = st.size / face.upem;
    (face.ascent * k * vs, face.descent * k * vs, st.effective_leading())
}

/// Cap height and x height (points) of a style's resolved face.
pub(crate) fn cap_x_heights(db: &FontDb, st: &CharStyle) -> (f64, f64) {
    let vs = st.v_scale / 100.0;
    let Some(face) = db.face(&st.font_family, &st.font_style) else { return (st.size * 0.7 * vs, st.size * 0.5 * vs) };
    let k = st.size / face.upem * vs;
    (face.cap_height * k, face.x_height * k)
}

/// Shape `text[range]`, where `runs` gives each run's byte range in `text` and style.
pub(crate) fn shape_range(
    db: &FontDb,
    text: &str,
    range: Range<usize>,
    runs: &[(Range<usize>, &CharStyle)],
    feats: &OtFeatures,
    out: &mut Vec<SGlyph>,
) {
    for (ri, (rr, st)) in runs.iter().enumerate() {
        let a = rr.start.max(range.start);
        let b = rr.end.min(range.end);
        if a >= b {
            continue;
        }
        let Some(primary) = db.face(&st.font_family, &st.font_style) else { continue };
        let pmap = primary.skrifa().map(|f| f.charmap());
        // Split into segments by font coverage.
        let mut seg_start = a;
        let mut seg_face = primary.clone();
        let mut cache: Vec<(char, Arc<FontFace>)> = Vec::new();
        for (i, c) in text[a..b].char_indices() {
            let i = a + i;
            let covered = c.is_whitespace() || c.is_control() || pmap.as_ref().is_none_or(|m| m.map(c).is_some());
            let face = if covered {
                primary.clone()
            } else if let Some((_, f)) = cache.iter().find(|(k, _)| *k == c) {
                f.clone()
            } else {
                let f = db.fallback_for(c, primary.id()).unwrap_or_else(|| primary.clone());
                cache.push((c, f.clone()));
                f
            };
            // Combining marks stay with their base.
            if face.id() != seg_face.id() && !is_mark(c) {
                if i > seg_start {
                    shape_segment(text, seg_start..i, ri, st, &seg_face, feats, out);
                }
                seg_start = i;
                seg_face = face;
            }
        }
        if b > seg_start {
            shape_segment(text, seg_start..b, ri, st, &seg_face, feats, out);
        }
    }
}

fn is_mark(c: char) -> bool {
    matches!(c as u32, 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F | 0x200D | 0xFE00..=0xFE0F)
}

fn shape_segment(text: &str, range: Range<usize>, run: usize, st: &CharStyle, face: &Arc<FontFace>, feats: &OtFeatures, out: &mut Vec<SGlyph>) {
    let seg = &text[range.clone()];
    let size = st.size.max(0.0);
    let k = size / face.upem;
    let hs = st.h_scale / 100.0;
    let vs = st.v_scale / 100.0;
    let tracking = st.tracking / 1000.0 * size;
    let manual_kern = st.kerning.map(|v| v / 1000.0 * size).unwrap_or(0.0);
    let ascent = face.ascent * k * vs;
    let descent = face.descent * k * vs;
    let leading = st.effective_leading();
    let cap = face.cap_height * k * vs;
    let xh = face.x_height * k * vs;
    let first_char = |byte: usize| text[byte..].chars().next().unwrap_or(' ');

    let mut raw: Vec<(u32, u32, i32, i32, i32)> = Vec::with_capacity(seg.len()); // gid, cluster, xadv, xoff, yoff
    let shaped = face.hb().map(|hb| {
        let shaper = face.shaper.shaper(&hb).build();
        let mut buf = UnicodeBuffer::new();
        for (i, c) in seg.char_indices() {
            let cl = (range.start + i) as u32;
            if st.all_caps {
                for u in c.to_uppercase() {
                    buf.add(u, cl);
                }
            } else {
                buf.add(c, cl);
            }
        }
        buf.set_direction(Direction::LeftToRight);
        buf.guess_segment_properties();
        let feats: Vec<Feature> = feats.resolve(st);
        let gb = shaper.shape(buf, ShapeOptions::new().features(&feats));
        for (info, pos) in gb.glyph_infos().iter().zip(gb.glyph_positions()) {
            raw.push((info.glyph_id, info.cluster, pos.x_advance, pos.x_offset, pos.y_offset));
        }
    });
    if shaped.is_none() {
        // Fallback: nominal glyphs and hmtx advances, no shaping.
        if let Some(f) = face.skrifa() {
            let cmap = f.charmap();
            let gm = f.glyph_metrics(Size::unscaled(), LocationRef::default());
            for (i, c) in seg.char_indices() {
                let cl = (range.start + i) as u32;
                let chars: Vec<char> = if st.all_caps { c.to_uppercase().collect() } else { vec![c] };
                for u in chars {
                    let g = cmap.map(u).unwrap_or_default();
                    let adv = gm.advance_width(g).unwrap_or(face.upem as f32 * 0.5);
                    raw.push((g.to_u32(), cl, adv.round() as i32, 0, 0));
                }
            }
        }
    }
    let n = raw.len();
    for (gi, &(gid, cl, xa, xo, yo)) in raw.iter().enumerate() {
        let cl = cl as usize;
        // Cluster end: the next larger cluster value in the segment, else the segment end.
        let end = raw[gi + 1..].iter().map(|r| r.1 as usize).find(|&c| c > cl).unwrap_or(range.end);
        let last_in_cluster = gi + 1 == n || raw[gi + 1].1 as usize != cl;
        let ch = first_char(cl);
        let mut adv = xa as f64 * k * hs;
        if ch == SOFT_HYPHEN {
            adv = 0.0;
        } else if last_in_cluster {
            adv += tracking + manual_kern;
        }
        out.push(SGlyph {
            face: face.clone(),
            gid,
            byte: cl,
            len: end.saturating_sub(cl).max(1),
            run,
            adv,
            dx: xo as f64 * k * hs,
            dy: -(yo as f64) * k * vs,
            sx: k * hs,
            sy: k * vs,
            bshift: st.baseline_shift,
            rotation: st.rotation,
            ascent,
            descent,
            leading,
            cap,
            xh,
            ch,
        });
    }
}
