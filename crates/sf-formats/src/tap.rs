//! Happy `.tap`: the stitch records of a Tajima DST file without its 512-byte header (public
//! description: https://edutechwiki.unige.ch/en/Embroidery_format). No colours or label: colour
//! changes are DST colour-change records, trims DST's three cancelling jumps.

use crate::plan::{Plan, PlanCfg};
use crate::{FormatError, WriteOptions, dst};
use sf_core::Pattern;

/// The DST header TAP leaves out.
const DST_HEADER: usize = 512;

pub(crate) fn plan(pattern: &Pattern, opts: &WriteOptions) -> Result<(Plan, PlanCfg), FormatError> {
    dst::plan(pattern, opts)
}

pub(crate) fn write(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let dst = dst::write(pattern, opts)?;
    Ok(dst.get(DST_HEADER..).unwrap_or_default().to_vec())
}

pub(crate) fn read(bytes: &[u8]) -> Result<Pattern, FormatError> {
    if bytes.len() < 3 {
        return Err(FormatError::Truncated);
    }
    dst::read_records(bytes, String::new())
}

/// Whole 3-byte records ending with DST's end record, and no DST header.
pub(crate) fn sniff(bytes: &[u8]) -> bool {
    bytes.len() >= 6 && bytes.len().is_multiple_of(3) && bytes.ends_with(&[0x00, 0x00, 0xF3]) && !bytes.starts_with(b"LA:")
}
