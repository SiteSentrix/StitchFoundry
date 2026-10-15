//! Melco expanded EXP.
//!
//! No header. 2-byte records: signed 8-bit dx and dy in 0.1 mm, +y up, at most
//! ±127 per axis. `0x80` in the first byte escapes a command of four bytes:
//!
//! * `80 01 dx dy` — colour change (written with a 0,0 move); also used for
//!   stops, which therefore read back as colour changes;
//! * `80 04 dx dy` — jump;
//! * `80 80 07 00` — trim (the convention we write; the reader takes any
//!   `80 80 ..` as a trim, and `80 02 dx dy` as a trim followed by a jump).
//!
//! There is no end record: the design ends with the file. Unknown escapes are
//! skipped. Uncertain: the two bytes after `80 80` (we write `07 00`).

use crate::bytes::Cursor;
use crate::plan::{Decoder, Plan, PlanCfg, build_plan};
use crate::{FormatError, WriteOptions};
use sf_core::{Cmd, Pattern};

const MAX_STEP: i64 = 127;
const ESC: u8 = 0x80;
const COLOR_CHANGE: u8 = 0x01;
const TRIM_JUMP: u8 = 0x02;
const JUMP: u8 = 0x04;
const TRIM: u8 = 0x80;

/// A signed byte (two's complement) from a move known to fit.
pub(crate) fn byte(v: i64) -> u8 {
    v as i8 as u8
}

/// The signed value of a byte.
pub(crate) fn signed(b: u8) -> i64 {
    i64::from(b as i8)
}

pub(crate) fn plan(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg), FormatError> {
    let cfg = PlanCfg::tenth_mm(true, MAX_STEP, MAX_STEP, opts.center);
    let mut plan = build_plan(pattern, &cfg.with(opts))?;
    plan.use_generic_threads();
    Ok((plan, cfg))
}

pub(crate) fn write(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let (plan, _) = plan(pattern, opts)?;
    let mut out = Vec::with_capacity(plan.recs.len() * 2 + 16);
    let mut cur = (0i64, 0i64);
    for r in &plan.recs {
        let (dx, dy) = (r.x - cur.0, r.y - cur.1);
        match r.cmd {
            Cmd::Stitch => out.extend_from_slice(&[byte(dx), byte(dy)]),
            Cmd::Jump => out.extend_from_slice(&[ESC, JUMP, byte(dx), byte(dy)]),
            Cmd::Trim => out.extend_from_slice(&[ESC, TRIM, 0x07, 0x00]),
            Cmd::ColorChange | Cmd::Stop => out.extend_from_slice(&[ESC, COLOR_CHANGE, 0x00, 0x00]),
            Cmd::End => {}
        }
        cur = (r.x, r.y);
    }
    Ok(out)
}

pub(crate) fn read(bytes: &[u8]) -> Result<Pattern, FormatError> {
    let mut dec = Decoder::new(10.0, true);
    let mut c = Cursor::new(bytes);
    while c.remaining() >= 2 {
        let [a, b] = c.array::<2>()?;
        if a != ESC {
            dec.stitch(signed(a), signed(b))?;
            continue;
        }
        // A command cut off by the end of the file ends the design.
        let Ok([x, y]) = c.array::<2>() else { break };
        let (dx, dy) = (signed(x), signed(y));
        match b {
            COLOR_CHANGE => {
                dec.command(Cmd::ColorChange)?;
                if (dx, dy) != (0, 0) {
                    dec.jump(dx, dy)?;
                }
            }
            TRIM_JUMP => {
                dec.command(Cmd::Trim)?;
                if (dx, dy) != (0, 0) {
                    dec.jump(dx, dy)?;
                }
            }
            JUMP => dec.jump(dx, dy)?,
            TRIM => dec.command(Cmd::Trim)?,
            _ => {}
        }
    }
    dec.finish(Vec::new(), String::new())
}

/// Whole records only, known escapes only, at least one stitch.
pub(crate) fn sniff(bytes: &[u8]) -> bool {
    if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
        return false;
    }
    let mut c = Cursor::new(bytes);
    let mut stitches = 0usize;
    while let Ok([a, b]) = c.array::<2>() {
        if a == ESC {
            if !matches!(b, COLOR_CHANGE | TRIM_JUMP | JUMP | TRIM) || c.skip(2).is_err() {
                return false;
            }
        } else {
            stitches += 1;
        }
    }
    stitches > 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_bytes() {
        for v in -128..=127 {
            assert_eq!(signed(byte(v)), v);
        }
    }

    #[test]
    fn sniffing() {
        assert!(sniff(&[1, 2, 0x80, 0x04, 5, 5, 3, 3]));
        assert!(!sniff(&[1, 2, 0x80, 0x33, 5, 5]));
        assert!(!sniff(&[1, 2, 3]));
        assert!(!sniff(&[0x80, 0x01, 0, 0]));
    }
}
