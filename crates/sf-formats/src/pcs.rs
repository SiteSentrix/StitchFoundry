//! Pfaff `.pcs` (also PCD and PCQ, which share the layout): experimental, not yet confirmed on a
//! machine. Public descriptions: the reader in pyembroidery (MIT licence, read to confirm the
//! layout, no code taken: https://github.com/EmbroidePy/pyembroidery, `PcsReader.py`) and the
//! format overview at https://edutechwiki.unige.ch/en/Embroidery_format.
//!
//! Little-endian, uncompressed:
//!
//! | offset | type | field |
//! |---|---|---|
//! | 0x00 | u8 | version (written 0; readers ignore it) |
//! | 0x01 | u8 | hoop: 0 PCD, 1 PCQ, 2 small (80 × 80 mm), 3 large |
//! | 0x02 | u16 | colour count C (at most 16) |
//! | 0x04 | 4 × C | colours: R, G, B, 0 |
//! | 0x04 + 4C | u16 | record count |
//! | … | 9 × count | records: `0, x i24, 0, y i24, flags` |
//!
//! Positions are absolute, in 1/6 mm, +y up. Flags: 0 stitch, `0x01` colour change, `0x04`
//! move (jump). There is no trim: a trim is written as a move to where the needle is, and read
//! back as a trim; the record count ends the design.

use crate::bytes::{Cursor, Put};
use crate::plan::{Decoder, Plan, PlanCfg, build_plan};
use crate::{FormatError, WriteOptions};
use sf_core::{Cmd, Pattern, Rgb, Thread};

/// File units per millimetre.
const UNITS: f64 = 6.0;
/// The longest stitch and move written, in units (12 mm).
const MAX_STEP: i64 = 72;
/// Colours a file holds at most.
const MAX_COLORS: usize = 16;
/// A small hoop's field (units either way from the centre: 40 mm).
const SMALL_HOOP: i64 = 240;
const COLOR_CHANGE: u8 = 0x01;
const MOVE: u8 = 0x04;

pub(crate) fn plan(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg), FormatError> {
    // 10 m at most, as the other formats.
    let cfg = PlanCfg { units_per_mm: UNITS, max_coord: 60_000, ..PlanCfg::tenth_mm(true, MAX_STEP, MAX_STEP, opts.center) };
    let mut plan = build_plan(pattern, &cfg.with(opts))?;
    // Only the colour is stored.
    for t in &mut plan.threads {
        *t = Thread { color: t.color, ..Thread::default() };
    }
    if plan.threads.len() > MAX_COLORS {
        return Err(FormatError::Invalid(format!("a PCS file holds {MAX_COLORS} colours at most, the design has {}", plan.threads.len())));
    }
    Ok((plan, cfg))
}

/// `v` as a little-endian 24-bit two's complement number.
fn i24(v: i64) -> [u8; 3] {
    let b = (v as i32).to_le_bytes();
    [b[0], b[1], b[2]]
}

pub(crate) fn write(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let (plan, _) = plan(pattern, opts)?;
    let mut recs = Vec::with_capacity(plan.recs.len() * 9);
    let (mut count, mut at, mut far) = (0usize, (0i64, 0i64), 0i64);
    for r in &plan.recs {
        let flags = match r.cmd {
            Cmd::Stitch => 0,
            Cmd::Jump => MOVE,
            Cmd::ColorChange | Cmd::Stop => COLOR_CHANGE,
            // a move to where the needle is
            Cmd::Trim => MOVE,
            Cmd::End => continue,
        };
        let (x, y) = if r.cmd == Cmd::Trim { at } else { (r.x, r.y) };
        recs.push(0);
        recs.extend_from_slice(&i24(x));
        recs.push(0);
        recs.extend_from_slice(&i24(y));
        recs.push(flags);
        at = (x, y);
        far = far.max(x.abs()).max(y.abs());
        count += 1;
    }
    let count = u16::try_from(count).map_err(|_| FormatError::TooLarge)?;
    let mut out = Vec::with_capacity(recs.len() + 8 + 4 * plan.threads.len());
    out.push(0);
    out.push(if far <= SMALL_HOOP { 2 } else { 3 });
    out.put_u16_le(plan.threads.len() as u16);
    for t in &plan.threads {
        out.put(&[t.color.r, t.color.g, t.color.b, 0]);
    }
    out.put_u16_le(count);
    out.extend_from_slice(&recs);
    Ok(out)
}

/// The header: hoop byte, colour count and the colour table's end.
fn header(bytes: &[u8]) -> Result<(usize, usize, usize), FormatError> {
    let mut c = Cursor::at(bytes, 0);
    let [_, hoop] = c.array::<2>().map_err(|_| FormatError::Truncated)?;
    let colors = usize::from(u16::from_le_bytes(c.array::<2>().map_err(|_| FormatError::Truncated)?));
    if hoop > 3 || colors > MAX_COLORS {
        return Err(FormatError::Invalid("not a PCS header".into()));
    }
    let table_end = 4 + 4 * colors;
    let n = bytes.get(table_end..table_end + 2).ok_or(FormatError::Truncated)?;
    let records = usize::from(u16::from_le_bytes([n[0], n[1]]));
    Ok((colors, table_end + 2, records))
}

fn i24_at(b: &[u8]) -> i64 {
    let v = i32::from_le_bytes([b.first().copied().unwrap_or(0), b.get(1).copied().unwrap_or(0), b.get(2).copied().unwrap_or(0), 0]);
    i64::from((v << 8) >> 8)
}

pub(crate) fn read(bytes: &[u8]) -> Result<Pattern, FormatError> {
    let (colors, start, records) = header(bytes)?;
    let threads: Vec<Thread> =
        (0..colors).filter_map(|i| bytes.get(4 + 4 * i..4 + 4 * i + 3)).map(|c| Thread::new(Rgb::new(c[0], c[1], c[2]), "")).collect();
    let mut dec = Decoder::new(UNITS, true);
    let mut at = (0i64, 0i64);
    // A record cut off by the end of the file ends the design.
    for k in 0..records {
        let Some(r) = bytes.get(start + 9 * k..start + 9 * k + 9) else { break };
        let (x, y, flags) = (i24_at(&r[1..4]), i24_at(&r[5..8]), r[8]);
        if flags & COLOR_CHANGE != 0 {
            dec.command(Cmd::ColorChange)?;
            if (x, y) != at {
                dec.jump(x - at.0, y - at.1)?;
            }
        } else if flags & MOVE != 0 {
            if (x, y) == at {
                dec.command(Cmd::Trim)?;
            } else {
                dec.jump(x - at.0, y - at.1)?;
            }
        } else {
            dec.stitch(x - at.0, y - at.1)?;
        }
        at = (x, y);
    }
    dec.finish(threads, String::new())
}

/// A sane header whose colour table and records fill the file exactly.
pub(crate) fn sniff(bytes: &[u8]) -> bool {
    matches!(header(bytes), Ok((_, start, records)) if records > 0 && start + 9 * records == bytes.len())
}
