//! Janome JEF.
//!
//! Little-endian header:
//!
//! | offset | type | field |
//! |---|---|---|
//! | 0x00 | u32 | offset of the stitch data: 0x74 + 8 × colours |
//! | 0x04 | u32 | flags (0x14) |
//! | 0x08 | 14 + 2 bytes | date `yyyymmddhhmmss`, two zero bytes |
//! | 0x18 | u32 | colour (block) count |
//! | 0x1C | u32 | point count: 2-byte words of stitch data |
//! | 0x20 | u32 | hoop code |
//! | 0x24 | 4 × i32 | extents from the start point: left, top, right, bottom (0.1 mm) |
//! | 0x34 | 4 × 4 × i32 | room left to the edges of the 110×110, 50×50, 140×200 and 126×110 mm hoops, in the same order (all −1 when the design does not fit) |
//! | 0x74 | colours × i32 | Janome palette index of each block ([`jef_chart`]) |
//! | … | colours × i32 | thread type of each block (0x0D) |
//!
//! Stitch data: signed 8-bit dx dy pairs in 0.1 mm, +y up, at most ±127 per
//! axis; `0x80` escapes: `80 01 dx dy` colour change, `80 02 dx dy` move
//! (jump), `80 10` end. JEF has no trim command: a trim is written as a move of
//! zero length (`80 02 00 00`, a common convention) and a zero-length move
//! reads back as a trim; zero-length jumps are never written otherwise. Stops
//! become colour changes.
//!
//! Uncertain: the hoop codes (we write 0 = 110×110, 1 = 50×50, 2 = 140×200,
//! 3 = 126×110, 4 = 200×200), the hoops of the edge-distance blocks, the
//! meaning of the flags and how machines treat the zero-length trim move;
//! readers ignore the hoop code, flags, date and extents.

use crate::bytes::{Cursor, Put, sat_i32, u32_le_at};
use crate::exp::{byte, signed};
use crate::plan::{Decoder, Plan, PlanCfg, build_plan};
use crate::{FormatError, Timestamp, WriteOptions};
use sf_core::threads::jef_chart;
use sf_core::{Cmd, Pattern, Thread};

const MAX_STEP: i64 = 127;
const TABLES: usize = 0x74;
/// Hoops (code, half width, half height in 0.1 mm), smallest first.
const HOOPS: [(u32, i64, i64); 5] = [(1, 250, 250), (0, 550, 550), (3, 630, 550), (2, 700, 1000), (4, 1000, 1000)];
/// The hoops of the four edge-distance blocks, in file order.
const EDGE_HOOPS: [(i64, i64); 4] = [(550, 550), (250, 250), (700, 1000), (630, 550)];
/// Readers refuse more colours than this.
const MAX_COLORS: usize = 10_000;

pub(crate) fn plan(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg), FormatError> {
    plan_with_indices(pattern, opts).map(|(plan, cfg, _)| (plan, cfg))
}

/// The plan, with its threads mapped to the Janome palette, and their palette indices.
fn plan_with_indices(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg, Vec<usize>), FormatError> {
    let cfg = PlanCfg::tenth_mm(true, MAX_STEP, MAX_STEP, opts.center);
    let mut plan = build_plan(pattern, &cfg.with(opts))?;
    let indices = plan.map_to_chart(&jef_chart());
    Ok((plan, cfg, indices))
}

fn date(t: Timestamp) -> String {
    format!("{:04}{:02}{:02}{:02}{:02}{:02}", t.year.min(9_999), t.month.min(99), t.day.min(99), t.hour.min(99), t.minute.min(99), t.second.min(99))
}

pub(crate) fn write(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let (plan, _, indices) = plan_with_indices(pattern, opts)?;

    let mut data = Vec::with_capacity(plan.recs.len() * 2 + 16);
    let mut cur = (0i64, 0i64);
    let (mut min, mut max) = ((0i64, 0i64), (0i64, 0i64));
    for r in &plan.recs {
        let (dx, dy) = (r.x - cur.0, r.y - cur.1);
        match r.cmd {
            Cmd::Stitch => data.extend_from_slice(&[byte(dx), byte(dy)]),
            Cmd::Jump => data.extend_from_slice(&[0x80, 0x02, byte(dx), byte(dy)]),
            Cmd::Trim => data.extend_from_slice(&[0x80, 0x02, 0x00, 0x00]),
            Cmd::ColorChange | Cmd::Stop => data.extend_from_slice(&[0x80, 0x01, 0x00, 0x00]),
            Cmd::End => data.extend_from_slice(&[0x80, 0x10]),
        }
        cur = (r.x, r.y);
        min = (min.0.min(r.x), min.1.min(r.y));
        max = (max.0.max(r.x), max.1.max(r.y));
    }

    let colors = indices.len();
    let offset = TABLES + 8 * colors;
    let (left, top, right, bottom) = (-min.0, max.1, max.0, -min.1);
    let fits = |hw: i64, hh: i64| left.max(right) <= hw && top.max(bottom) <= hh;
    let hoop = HOOPS.iter().find(|h| fits(h.1, h.2)).map_or(4, |h| h.0);

    let mut out = Vec::with_capacity(offset + data.len());
    out.put_u32_le(u32::try_from(offset).map_err(|_| FormatError::TooLarge)?);
    out.put_u32_le(0x14);
    out.put(date(opts.timestamp.unwrap_or_else(Timestamp::now)).as_bytes());
    out.put(&[0, 0]);
    out.put_u32_le(u32::try_from(colors).map_err(|_| FormatError::TooLarge)?);
    out.put_u32_le(u32::try_from(data.len() / 2).map_err(|_| FormatError::TooLarge)?);
    out.put_u32_le(hoop);
    for v in [left, top, right, bottom] {
        out.put_i32_le(sat_i32(v));
    }
    for (hw, hh) in EDGE_HOOPS {
        let room = [hw - left, hh - top, hw - right, hh - bottom];
        let fit = room.iter().all(|&v| v >= 0);
        for v in room {
            out.put_i32_le(if fit { sat_i32(v) } else { -1 });
        }
    }
    for &i in &indices {
        out.put_u32_le(u32::try_from(i).unwrap_or(0));
    }
    for _ in 0..colors {
        out.put_u32_le(0x0D);
    }
    out.extend_from_slice(&data);
    Ok(out)
}

pub(crate) fn read(bytes: &[u8]) -> Result<Pattern, FormatError> {
    let offset = u32_le_at(bytes, 0).ok_or(FormatError::Truncated)? as usize;
    let colors = u32_le_at(bytes, 0x18).ok_or(FormatError::Truncated)? as usize;
    if offset < TABLES || offset > bytes.len() {
        return Err(FormatError::Invalid("JEF stitch data offset out of range".into()));
    }
    if colors > MAX_COLORS || TABLES + 4 * colors > offset {
        return Err(FormatError::Invalid("JEF colour table does not fit the header".into()));
    }
    let chart = jef_chart();
    let threads: Vec<Thread> = (0..colors)
        .map(|i| {
            let idx = u32_le_at(bytes, TABLES + 4 * i).unwrap_or(0) as usize;
            chart.threads.get(idx).filter(|_| idx > 0).cloned().unwrap_or_else(|| Pattern::new().thread(i))
        })
        .collect();

    let mut dec = Decoder::new(10.0, true);
    let mut c = Cursor::at(bytes, offset);
    // A file cut off before its end record ends where it stops.
    while let Ok([a, b]) = c.array::<2>() {
        if a != 0x80 {
            dec.stitch(signed(a), signed(b))?;
            continue;
        }
        match b {
            0x01 => {
                let Ok([x, y]) = c.array::<2>() else { break };
                dec.command(Cmd::ColorChange)?;
                if (x, y) != (0, 0) {
                    dec.jump(signed(x), signed(y))?;
                }
            }
            0x02 => {
                let Ok([x, y]) = c.array::<2>() else { break };
                if (x, y) == (0, 0) {
                    dec.command(Cmd::Trim)?;
                } else {
                    dec.jump(signed(x), signed(y))?;
                }
            }
            0x10 => break,
            _ => {}
        }
    }
    dec.finish(threads, String::new())
}

/// The stitch data offset agrees with the colour count.
pub(crate) fn sniff(bytes: &[u8]) -> bool {
    let (Some(offset), Some(colors)) = (u32_le_at(bytes, 0), u32_le_at(bytes, 0x18)) else { return false };
    (1..=MAX_COLORS as u32).contains(&colors) && u64::from(offset) == TABLES as u64 + 8 * u64::from(colors) && offset as usize <= bytes.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_fourteen_digits() {
        let t = Timestamp { year: 2026, month: 10, day: 9, hour: 8, minute: 7, second: 6 };
        assert_eq!(date(t), "20261009080706");
        let wild = Timestamp { year: 65_000, month: 200, day: 200, hour: 200, minute: 200, second: 200 };
        assert_eq!(date(wild).len(), 14);
    }
}
