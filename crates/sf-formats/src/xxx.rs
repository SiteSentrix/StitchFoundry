//! Singer XXX.
//!
//! A 0x100-byte little-endian header:
//!
//! | offset | type | field |
//! |---|---|---|
//! | 0x00 | 23 bytes | zero |
//! | 0x17 | u32 | number of stitch data records |
//! | 0x1B | 12 bytes | zero |
//! | 0x27 | u16 | colour (block) count |
//! | 0x29 | u16 | zero |
//! | 0x2B | u16, u16 | width and height (0.1 mm) |
//! | 0x2F | 4 bytes | zero |
//! | 0x33 | u16, u16 | extents left of and above the start point (0.1 mm) |
//! | 0xFC | u32 | offset of the end record |
//!
//! Stitch data from 0x100: signed 8-bit dx dy pairs in 0.1 mm, +y up, the x
//! byte never 0x7D–0x7F (so at most ±124 per axis). `0x7F` escapes a 4-byte
//! command: `7F 01 dx dy` jump, `7F 03 00 00` trim, `7F 08 00 00` colour change
//! (also stops), `7F 7F 02 14` end. After the end record come two zero bytes and
//! the colour table: one `00 RR GG BB` entry per block, zero entries up to 21,
//! then `00 FF FF FF` and `00 01`.
//!
//! The reader also accepts `7F 02` as a trim, `7F 0A`–`7F 17` as colour
//! changes, `7F 18` as the end and `7D`/`7E` + i16 dx + i16 dy (LE) as a long
//! jump. Uncertain: most header fields and the exact long-jump layout; readers
//! only use the colour count and the colour table after the end record.

use crate::bytes::{Cursor, Put, sat_u16, u16_le_at, u32_le_at};
use crate::exp::{byte, signed};
use crate::plan::{Decoder, Plan, PlanCfg, build_plan};
use crate::{FormatError, WriteOptions};
use sf_core::{Cmd, Pattern, Rgb, Thread};

const HEADER: usize = 0x100;
const MAX_STEP: i64 = 124;
const ESC: u8 = 0x7F;
/// Readers refuse more colours than this.
const MAX_COLORS: usize = 10_000;

pub(crate) fn plan(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg), FormatError> {
    let cfg = PlanCfg::tenth_mm(true, MAX_STEP, MAX_STEP, opts.center);
    let mut plan = build_plan(pattern, &cfg.with(opts))?;
    // Only the colour is stored.
    for t in &mut plan.threads {
        *t = Thread { color: t.color, ..Thread::default() };
    }
    if plan.threads.len() > usize::from(u16::MAX) {
        return Err(FormatError::TooLarge);
    }
    Ok((plan, cfg))
}

pub(crate) fn write(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let (plan, _) = plan(pattern, opts)?;
    let mut data = Vec::with_capacity(plan.recs.len() * 2 + 16);
    let mut cur = (0i64, 0i64);
    let (mut min, mut max) = ((0i64, 0i64), (0i64, 0i64));
    for r in &plan.recs {
        let (dx, dy) = (r.x - cur.0, r.y - cur.1);
        match r.cmd {
            Cmd::Stitch => data.extend_from_slice(&[byte(dx), byte(dy)]),
            Cmd::Jump => data.extend_from_slice(&[ESC, 0x01, byte(dx), byte(dy)]),
            Cmd::Trim => data.extend_from_slice(&[ESC, 0x03, 0x00, 0x00]),
            Cmd::ColorChange | Cmd::Stop => data.extend_from_slice(&[ESC, 0x08, 0x00, 0x00]),
            Cmd::End => data.extend_from_slice(&[ESC, ESC, 0x02, 0x14, 0x00, 0x00]),
        }
        cur = (r.x, r.y);
        min = (min.0.min(r.x), min.1.min(r.y));
        max = (max.0.max(r.x), max.1.max(r.y));
    }
    // The end record (7F 7F 02 14) and its two zero bytes close the stitch data.
    let end = HEADER + data.len().saturating_sub(6);

    let mut out = vec![0u8; 0x17];
    out.put_u32_le(u32::try_from(plan.recs.len()).map_err(|_| FormatError::TooLarge)?);
    out.put(&[0; 12]);
    out.put_u16_le(plan.threads.len() as u16);
    out.put_u16_le(0);
    out.put_u16_le(sat_u16(max.0 - min.0));
    out.put_u16_le(sat_u16(max.1 - min.1));
    out.put(&[0; 4]);
    out.put_u16_le(sat_u16(-min.0));
    out.put_u16_le(sat_u16(max.1));
    out.resize(0xFC, 0);
    out.put_u32_le(u32::try_from(end).map_err(|_| FormatError::TooLarge)?);
    out.extend_from_slice(&data);
    for t in &plan.threads {
        out.put(&[0, t.color.r, t.color.g, t.color.b]);
    }
    for _ in plan.threads.len()..21 {
        out.put(&[0; 4]);
    }
    out.put(&[0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x01]);
    Ok(out)
}

/// Decodes the stitch data; returns the decoder and the offset after the end record.
fn decode(bytes: &[u8]) -> Result<(Decoder, Option<usize>), FormatError> {
    let mut dec = Decoder::new(10.0, true);
    let mut c = Cursor::at(bytes, HEADER);
    if c.remaining() == 0 {
        return Err(FormatError::Truncated);
    }
    // A record cut off by the end of the file ends the design.
    while let Ok([a, b]) = c.array::<2>() {
        match a {
            0x7D | 0x7E => {
                let Ok(rest) = c.array::<3>() else { break };
                let x = i16::from_le_bytes([b, rest[0]]);
                let y = i16::from_le_bytes([rest[1], rest[2]]);
                dec.jump(i64::from(x), i64::from(y))?;
            }
            ESC => {
                let Ok([x, y]) = c.array::<2>() else { break };
                let (dx, dy) = (signed(x), signed(y));
                match b {
                    0x01 => dec.jump(dx, dy)?,
                    0x02 | 0x03 => {
                        dec.command(Cmd::Trim)?;
                        if (dx, dy) != (0, 0) {
                            dec.jump(dx, dy)?;
                        }
                    }
                    0x08 | 0x0A..=0x17 => dec.command(Cmd::ColorChange)?,
                    0x7F | 0x18 => return Ok((dec, Some(c.pos()))),
                    _ => {}
                }
            }
            _ => dec.stitch(signed(a), signed(b))?,
        }
    }
    Ok((dec, None))
}

pub(crate) fn read(bytes: &[u8]) -> Result<Pattern, FormatError> {
    let colors = usize::from(u16_le_at(bytes, 0x27).ok_or(FormatError::Truncated)?);
    let (dec, end) = decode(bytes)?;
    let colors = colors.min(dec.color_changes() + 1).min(MAX_COLORS);
    let fits = |off: usize| off.checked_add(4 * colors).is_some_and(|e| e <= bytes.len());
    // The table follows the end record and two zero bytes; the header points at the end record.
    let pointed = u32_le_at(bytes, 0xFC).map(|o| o as usize).filter(|&o| o >= HEADER && bytes.get(o..o + 2) == Some(&[ESC, ESC]));
    let table = pointed.map(|o| o + 6).or_else(|| end.map(|e| e + 2)).filter(|&o| fits(o));
    let threads: Vec<Thread> = match table {
        Some(off) => (0..colors)
            .filter_map(|i| bytes.get(off + 4 * i + 1..off + 4 * i + 4))
            .map(|c| Thread::new(Rgb::new(c.first().copied().unwrap_or(0), c.get(1).copied().unwrap_or(0), c.get(2).copied().unwrap_or(0)), ""))
            .collect(),
        None => Vec::new(),
    };
    dec.finish(threads, String::new())
}

/// A zero header start, a colour count and stitch data that reaches an end record.
pub(crate) fn sniff(bytes: &[u8]) -> bool {
    let Some(start) = bytes.get(..0x17) else { return false };
    let colors = u16_le_at(bytes, 0x27).unwrap_or(0);
    start.iter().all(|&b| b == 0) && colors >= 1 && matches!(decode(bytes), Ok((_, Some(_))))
}
