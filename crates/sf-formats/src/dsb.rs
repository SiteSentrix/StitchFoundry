//! Barudan `.dsb`: a 512-byte DST header (label, counts and extents, see [`crate::dst`]), then
//! the 3-byte records of [`crate::u01`]: a control byte, then the y and x moves (0.1 mm, +y up,
//! at most 127). Control byte: bit 6 makes y negative, bit 5 makes x negative, the low five bits
//! hold the function (stitch 0x00, jump 0x01, trim 0x07, stop 0x08, needle 1–15 0x09–0x17) and
//! the end is `F8 00 00`. No colours: the reader colours the blocks from a generic palette.
//!
//! Public description: the format notes of the MIT-licensed pyembroidery project
//! (https://github.com/EmbroidePy/pyembroidery). Uncertain: the header fields Barudan machines
//! need; we write DST's.

use crate::plan::{Plan, PlanCfg};
use crate::{FormatError, WriteOptions, dst, u01};
use sf_core::Pattern;

const HEADER: usize = 512;

pub(crate) fn plan(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg), FormatError> {
    u01::plan(pattern, opts)
}

pub(crate) fn write(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let (plan, _) = plan(pattern, opts)?;
    let (body, extents) = u01::records(&plan);
    let mut out = dst::header(opts.label(pattern), &extents)?;
    out.extend_from_slice(&body);
    Ok(out)
}

pub(crate) fn read(bytes: &[u8]) -> Result<Pattern, FormatError> {
    let body = bytes.get(HEADER..).ok_or(FormatError::Truncated)?;
    u01::read_records(body, dst::header_label(bytes))
}

/// A DST header followed by U01 records ending with the end record.
pub(crate) fn sniff(bytes: &[u8]) -> bool {
    dst::sniff(bytes) && bytes.get(HEADER..).is_some_and(u01::sniff_records)
}
