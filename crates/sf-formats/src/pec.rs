//! Brother PEC: the block Brother machines sew from, stored inside PES files
//! ([`crate::pes`]) or alone as a `.pec` file (`#PEC0001` followed by the block).
//!
//! Block layout (offsets from the start of the block):
//!
//! | offset | content |
//! |---|---|
//! | 0x000 | `LA:` label (16 characters, space padded) CR |
//! | 0x014 | 12 fixed bytes `20 20 20 20 64 20 00 20 00 20 20 20`, then `FF 00` |
//! | 0x022 | thumbnail row size in bytes (6 = 48 px) and row count (38) |
//! | 0x024 | 12 bytes `20` |
//! | 0x030 | colour block count − 1 |
//! | 0x031 | Brother palette index of each block ([`pec_chart`]), padded with `20` to 463 bytes |
//! | 0x200 | `00 00`, u24 LE offset from 0x200 to the thumbnails, `31 FF F0` |
//! | 0x208 | u16 LE width, u16 LE height (0.1 mm), u16 LE 0x1E0, u16 LE 0x1B0 |
//! | 0x210 | stitch data, `FF` |
//! | … | thumbnails: 48×38 px, 1 bit per pixel, 6 bytes per row, bit 0 = leftmost pixel; the whole design, then one per colour block, each with the usual rounded frame |
//!
//! Stitch data, 0.1 mm, +y **down** (screen orientation). Each record is x then
//! y; a coordinate is either one byte, 7-bit two's complement (−64..63), or two
//! bytes whose first has bit 7 set: a 12-bit two's complement value
//! (−2048..2047) plus flags `0x10` (jump) and `0x20` (trim). The data starts
//! with a jump of (−min x, −min y) that puts the design's top-left corner at
//! 0,0; the reader skips it, so a design reads back where it was written.
//!
//! * stitches use the short form when both coordinates fit, else the long form;
//! * jumps use the long form with the jump flag;
//! * a trim sets the trim flag on the jump after it (a zero move with the trim
//!   flag when no jump follows); the reader turns a trim-flagged record into a
//!   trim and, if it moves, a jump;
//! * `FE B0 xx` changes colour (xx alternates 2, 1, 2, …); stops are written as
//!   colour changes;
//! * `FF` ends the design.
//!
//! Uncertain: the meaning of the fixed bytes at 0x014 and of 0x1E0/0x1B0; we
//! write the values commonly found in files and readers ignore them.

use crate::bytes::{Cursor, Put, ascii_label, read_label, sat_u16};
use crate::plan::{Decoder, Plan, PlanCfg, build_plan};
use crate::{Format, FormatError, WriteOptions};
use sf_core::threads::pec_chart;
use sf_core::{Cmd, Pattern, Thread};

/// Longest move per axis (12-bit long form).
const MAX_STEP: i64 = 2047;
const JUMP_FLAG: u8 = 0x10;
const TRIM_FLAG: u8 = 0x20;
/// Thumbnail size in pixels and bytes per row.
pub(crate) const THUMB_W: usize = 48;
pub(crate) const THUMB_H: usize = 38;
const THUMB_ROW: usize = THUMB_W / 8;
pub(crate) const THUMB_BYTES: usize = THUMB_ROW * THUMB_H;
/// Offset of the stitch section header within the block.
const STITCH_SECTION: usize = 0x200;
/// Offset of the stitch data within the block.
const STITCH_DATA: usize = 0x210;
/// Size of the colour index area (0x31..0x200).
const COLOR_AREA: usize = STITCH_SECTION - 0x31;

pub(crate) fn plan(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg), FormatError> {
    plan_with_indices(pattern, opts).map(|(plan, cfg, _)| (plan, cfg))
}

/// The plan with its threads mapped to the Brother palette, and their palette indices.
fn plan_with_indices(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg, Vec<usize>), FormatError> {
    let cfg = PlanCfg::tenth_mm(false, MAX_STEP, MAX_STEP, opts.center);
    let mut plan = build_plan(pattern, &cfg.with(opts))?;
    let indices = plan.map_to_chart(&pec_chart());
    // The count is stored as one byte (count − 1).
    if indices.len() > 256 {
        return Err(FormatError::TooLarge);
    }
    let (min, max) = extents(&plan);
    // The leading jump to (−min x, −min y) must fit the 12-bit form; width and height are u16.
    if -min.0 > MAX_STEP || -min.1 > MAX_STEP || max.0 - min.0 > i64::from(u16::MAX) || max.1 - min.1 > i64::from(u16::MAX) {
        return Err(FormatError::TooLarge);
    }
    Ok((plan, cfg, indices))
}

/// Extents of the moves, including the start point (the origin).
fn extents(plan: &Plan) -> ((i64, i64), (i64, i64)) {
    plan.recs
        .iter()
        .filter(|r| matches!(r.cmd, Cmd::Stitch | Cmd::Jump))
        .fold(((0, 0), (0, 0)), |(min, max), r| ((min.0.min(r.x), min.1.min(r.y)), (max.0.max(r.x), max.1.max(r.y))))
}

/// A coordinate in the 12-bit long form with `flags`.
fn long(v: i64, flags: u8) -> [u8; 2] {
    let v = (v & 0x0FFF) as u16;
    [0x80 | flags | (v >> 8) as u8, (v & 0xFF) as u8]
}

/// The stitch data (without the leading positioning jump), ending with `FF`.
fn encode_stitches(plan: &Plan) -> Vec<u8> {
    let mut out = Vec::with_capacity(plan.recs.len() * 2 + 16);
    let mut cur = (0i64, 0i64);
    let mut pending_trim = false;
    let mut color_flag = 2u8;
    let short = |v: i64| (-64..=63).contains(&v);
    for (i, r) in plan.recs.iter().enumerate() {
        let (dx, dy) = (r.x - cur.0, r.y - cur.1);
        match r.cmd {
            Cmd::Stitch => {
                if short(dx) && short(dy) {
                    out.extend_from_slice(&[(dx & 0x7F) as u8, (dy & 0x7F) as u8]);
                } else {
                    out.extend_from_slice(&long(dx, 0));
                    out.extend_from_slice(&long(dy, 0));
                }
            }
            Cmd::Jump => {
                let flag = if pending_trim { TRIM_FLAG } else { JUMP_FLAG };
                pending_trim = false;
                out.extend_from_slice(&long(dx, flag));
                out.extend_from_slice(&long(dy, flag));
            }
            Cmd::Trim => {
                if plan.recs.get(i + 1).is_some_and(|n| n.cmd == Cmd::Jump) {
                    pending_trim = true;
                } else {
                    out.extend_from_slice(&long(0, TRIM_FLAG));
                    out.extend_from_slice(&long(0, TRIM_FLAG));
                }
            }
            Cmd::ColorChange | Cmd::Stop => {
                out.extend_from_slice(&[0xFE, 0xB0, color_flag]);
                color_flag = if color_flag == 2 { 1 } else { 2 };
            }
            Cmd::End => out.push(0xFF),
        }
        cur = (r.x, r.y);
    }
    out
}

/// A 1-bit thumbnail with the usual rounded frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Thumb(pub [u8; THUMB_BYTES]);

impl Thumb {
    fn framed() -> Thumb {
        let mut t = Thumb([0; THUMB_BYTES]);
        for x in 4..=43 {
            t.set(x, 1);
            t.set(x, 36);
        }
        for y in 3..=34 {
            t.set(2, y);
            t.set(45, y);
        }
        for (x, y) in [(3, 2), (44, 2), (3, 35), (44, 35)] {
            t.set(x, y);
        }
        t
    }

    #[cfg(test)]
    pub fn get(&self, x: i64, y: i64) -> bool {
        Self::index(x, y).and_then(|(i, bit)| self.0.get(i).map(|b| b & bit != 0)).unwrap_or(false)
    }

    fn index(x: i64, y: i64) -> Option<(usize, u8)> {
        let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
        (x < THUMB_W && y < THUMB_H).then(|| (y * THUMB_ROW + x / 8, 1u8 << (x % 8)))
    }

    fn set(&mut self, x: i64, y: i64) {
        if let Some((i, bit)) = Self::index(x, y)
            && let Some(b) = self.0.get_mut(i)
        {
            *b |= bit;
        }
    }

    /// Bresenham line between two pixels (both inside the image).
    fn line(&mut self, a: (i64, i64), b: (i64, i64)) {
        let (dx, dy) = ((b.0 - a.0).abs(), -(b.1 - a.1).abs());
        let (sx, sy) = (if a.0 < b.0 { 1 } else { -1 }, if a.1 < b.1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (a.0, a.1, dx + dy);
        loop {
            self.set(x, y);
            if (x, y) == b {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }
}

/// Thumbnails of the whole design and of each colour block, the stitches drawn
/// as lines at one common scale inside the frame.
fn thumbnails(plan: &Plan, blocks: usize) -> Vec<Thumb> {
    let mut all = Thumb::framed();
    let mut per_block = vec![Thumb::framed(); blocks];
    let stitches = || plan.recs.iter().filter(|r| r.cmd == Cmd::Stitch);
    let (min_x, max_x) = (stitches().map(|r| r.x).min().unwrap_or(0), stitches().map(|r| r.x).max().unwrap_or(0));
    let (min_y, max_y) = (stitches().map(|r| r.y).min().unwrap_or(0), stitches().map(|r| r.y).max().unwrap_or(0));
    // Drawing area inside the frame: x 5..=42, y 4..=33.
    let (w, h) = ((max_x - min_x) as f64, (max_y - min_y) as f64);
    let scale = [37.0 / w, 29.0 / h].into_iter().filter(|s| s.is_finite()).fold(f64::INFINITY, f64::min);
    let scale = if scale.is_finite() { scale } else { 0.0 };
    let (cx, cy) = ((min_x + max_x) as f64 / 2.0, (min_y + max_y) as f64 / 2.0);
    let px = |x: i64, y: i64| {
        let fx = 23.5 + (x as f64 - cx) * scale;
        let fy = 18.5 + (y as f64 - cy) * scale;
        (fx.round().clamp(5.0, 42.0) as i64, fy.round().clamp(4.0, 33.0) as i64)
    };
    let mut block = 0usize;
    let mut prev: Option<(i64, i64)> = None;
    for r in &plan.recs {
        match r.cmd {
            Cmd::Stitch => {
                let p = px(r.x, r.y);
                let from = prev.unwrap_or(p);
                all.line(from, p);
                if let Some(t) = per_block.get_mut(block) {
                    t.line(from, p);
                }
                prev = Some(p);
            }
            Cmd::ColorChange | Cmd::Stop => {
                block += 1;
                prev = None;
            }
            Cmd::Jump | Cmd::Trim | Cmd::End => prev = None,
        }
    }
    let mut out = vec![all];
    out.extend(per_block);
    out
}

/// The PEC block.
pub(crate) fn block(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let (plan, _, indices) = plan_with_indices(pattern, opts)?;
    let (min, max) = extents(&plan);
    let mut out = Vec::with_capacity(0x400 + plan.recs.len() * 2);
    out.put(format!("LA:{:<16}\r", ascii_label(opts.label(pattern), 16)).as_bytes());
    out.put(&[0x20, 0x20, 0x20, 0x20, 0x64, 0x20, 0x00, 0x20, 0x00, 0x20, 0x20, 0x20, 0xFF, 0x00]);
    out.put(&[THUMB_ROW as u8, THUMB_H as u8]);
    out.put(&[0x20; 12]);
    out.push(indices.len().saturating_sub(1) as u8);
    let mut colors: Vec<u8> = indices.iter().map(|&i| i as u8).collect();
    colors.resize(COLOR_AREA, 0x20);
    out.put(&colors);

    let mut data = Vec::new();
    data.extend_from_slice(&long(-min.0, JUMP_FLAG));
    data.extend_from_slice(&long(-min.1, JUMP_FLAG));
    data.extend_from_slice(&encode_stitches(&plan));
    let thumbs_at = STITCH_DATA - STITCH_SECTION + data.len();
    if thumbs_at > 0xFF_FFFF {
        return Err(FormatError::TooLarge);
    }
    out.put(&[0x00, 0x00]);
    out.put(&(thumbs_at as u32).to_le_bytes()[..3]);
    out.put(&[0x31, 0xFF, 0xF0]);
    out.put_u16_le(sat_u16(max.0 - min.0));
    out.put_u16_le(sat_u16(max.1 - min.1));
    out.put_u16_le(0x1E0);
    out.put_u16_le(0x1B0);
    out.put(&data);
    for t in thumbnails(&plan, indices.len()) {
        out.put(&t.0);
    }
    Ok(out)
}

/// A `.pec` file: `#PEC0001` and the block.
pub(crate) fn write_file(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let mut out = b"#PEC0001".to_vec();
    out.extend_from_slice(&block(pattern, opts)?);
    Ok(out)
}

/// One coordinate: its value and long-form flags.
fn coord(c: &mut Cursor) -> Result<(i64, u8), FormatError> {
    let b = c.u8()?;
    if b & 0x80 == 0 {
        let v = i64::from(b & 0x7F);
        return Ok((if v >= 0x40 { v - 0x80 } else { v }, 0));
    }
    let lo = c.u8()?;
    let v = (i64::from(b & 0x0F) << 8) | i64::from(lo);
    Ok((if v >= 0x800 { v - 0x1000 } else { v }, b & 0x70))
}

/// Decodes a PEC block starting at `base` in `bytes`.
pub(crate) fn read_block(bytes: &[u8], base: usize) -> Result<Pattern, FormatError> {
    let blk = bytes.get(base..).ok_or(FormatError::Truncated)?;
    if !blk.starts_with(b"LA:") {
        return Err(if blk.len() < 3 { FormatError::Truncated } else { FormatError::Invalid("PEC block does not start with LA:".into()) });
    }
    let name = read_label(blk.get(3..19).unwrap_or_default());
    let mut c = Cursor::at(blk, 0x30);
    let count = usize::from(c.u8()?) + 1;
    let chart = pec_chart();
    let threads: Vec<Thread> = c
        .take(count)?
        .iter()
        .enumerate()
        .map(|(i, &idx)| chart.threads.get(usize::from(idx)).filter(|_| idx > 0).cloned().unwrap_or_else(|| Pattern::new().thread(i)))
        .collect();

    // The stitch data ends where the thumbnails start (when that offset is sane).
    let mut c = Cursor::at(blk, STITCH_SECTION + 2);
    let thumbs = c.take(3).map(|b| b.iter().rev().fold(0usize, |a, &v| (a << 8) | usize::from(v)))?;
    let end = STITCH_SECTION.checked_add(thumbs).filter(|&e| (STITCH_DATA..=blk.len()).contains(&e)).unwrap_or(blk.len());
    let data = blk.get(..end).filter(|d| d.len() > STITCH_DATA).ok_or(FormatError::Truncated)?;
    let mut c = Cursor::at(data, STITCH_DATA);

    // Skip the positioning jump.
    if let Some(&[hx, _, hy, _]) = c.peek(4)
        && !matches!(hx, 0xFE | 0xFF)
        && hx & 0x90 == 0x90
        && hy & 0x90 == 0x90
    {
        c.skip(4)?;
    }
    let mut dec = Decoder::new(10.0, false);
    // A record cut off by the end of the data ends the design.
    while let Some(&[first]) = c.peek(1) {
        if first == 0xFF {
            break;
        }
        if c.peek(2) == Some(&[0xFE, 0xB0][..]) {
            if c.skip(3).is_err() {
                break;
            }
            dec.command(Cmd::ColorChange)?;
            continue;
        }
        let (Ok((dx, fx)), Ok((dy, fy))) = (coord(&mut c), coord(&mut c)) else { break };
        let flags = fx | fy;
        if flags & TRIM_FLAG != 0 {
            dec.command(Cmd::Trim)?;
            if (dx, dy) != (0, 0) {
                dec.jump(dx, dy)?;
            }
        } else if flags & JUMP_FLAG != 0 {
            dec.jump(dx, dy)?;
        } else {
            dec.stitch(dx, dy)?;
        }
    }
    dec.finish(threads, name)
}

pub(crate) fn read_file(bytes: &[u8]) -> Result<Pattern, FormatError> {
    if !bytes.starts_with(b"#PEC") {
        return Err(if bytes.len() < 4 { FormatError::Truncated } else { FormatError::BadMagic(Format::Pec) });
    }
    read_block(bytes, 8)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thumbnails of a PEC block (the whole design first), for checks and previews.
    fn read_thumbnails(bytes: &[u8], base: usize) -> Option<Vec<Thumb>> {
        let blk = bytes.get(base..)?;
        let count = usize::from(*blk.get(0x30)?) + 1;
        let mut c = Cursor::at(blk, STITCH_SECTION + 2);
        let thumbs = c.take(3).ok()?.iter().rev().fold(0usize, |a, &v| (a << 8) | usize::from(v));
        let mut c = Cursor::at(blk, STITCH_SECTION.checked_add(thumbs)?);
        (0..=count).map(|_| c.array::<THUMB_BYTES>().ok().map(Thumb)).collect()
    }

    #[test]
    fn coordinates_round_trip() {
        for v in [-2048i64, -2047, -65, -64, -1, 0, 1, 63, 64, 2047] {
            let enc = long(v, JUMP_FLAG);
            let mut c = Cursor::new(&enc);
            assert_eq!(coord(&mut c).unwrap(), (v, JUMP_FLAG));
        }
        for v in -64i64..=63 {
            let enc = [(v & 0x7F) as u8];
            let mut c = Cursor::new(&enc);
            assert_eq!(coord(&mut c).unwrap(), (v, 0));
        }
    }

    #[test]
    fn frame_matches_the_usual_template() {
        let t = Thumb::framed();
        assert_eq!(&t.0[0..6], &[0, 0, 0, 0, 0, 0]);
        assert_eq!(&t.0[6..12], &[0xF0, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
        assert_eq!(&t.0[12..18], &[0x08, 0, 0, 0, 0, 0x10]);
        assert_eq!(&t.0[18..24], &[0x04, 0, 0, 0, 0, 0x20]);
        assert_eq!(&t.0[THUMB_BYTES - 12..THUMB_BYTES - 6], &[0xF0, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
        assert!(t.get(2, 20) && t.get(45, 20) && !t.get(20, 20));
    }

    #[test]
    fn thumbnails_show_each_block() {
        let mut p = Pattern::new();
        for i in 0..20 {
            p.stitch(sf_core::Point::new(f64::from(i), if i % 2 == 0 { 0.0 } else { 8.0 }));
        }
        p.color_change();
        p.stitch(sf_core::Point::new(-10.0, 20.0));
        p.stitch(sf_core::Point::new(-10.0, 30.0));
        let blk = block(&p, &WriteOptions::default()).unwrap();
        let thumbs = read_thumbnails(&blk, 0).unwrap();
        assert_eq!(thumbs.len(), 3);
        let frame = Thumb::framed();
        let ink = |t: &Thumb| t.0.iter().zip(frame.0.iter()).map(|(a, f)| (a & !f).count_ones()).sum::<u32>();
        assert!(ink(&thumbs[1]) > 0 && ink(&thumbs[2]) > 0);
        assert_eq!(ink(&thumbs[0]), ink(&thumbs[1]) + ink(&thumbs[2]));
        let back = read_block(&blk, 0).unwrap();
        assert_eq!(back.count(Cmd::ColorChange), 1);
    }

    #[test]
    fn lines_connect_their_ends() {
        let mut t = Thumb([0; THUMB_BYTES]);
        t.line((5, 4), (42, 33));
        assert!(t.get(5, 4) && t.get(42, 33));
        assert_eq!(t.0.iter().map(|b| b.count_ones()).sum::<u32>(), 38);
    }
}
