//! Brother PES.
//!
//! Written as version 1 in the widely used short form: the signature
//! `#PES0001`, the u32 LE offset of the PEC block (22), ten zero bytes (hoop
//! size, design-area flag and a segment block count of 0, i.e. no
//! CEmbOne/CSewSeg sections), then the PEC block ([`crate::pec`]). Machines sew
//! from the PEC block; editing software rebuilds its own sections from it.
//!
//! The reader accepts every PES version: it follows the PEC offset at byte 8
//! and decodes the PEC block. Thread colours come from the PEC palette indices
//! (version 5 and later also store RGB thread colours in the PES section; they
//! are not read).

use crate::bytes::{Put, u32_le_at};
use crate::{Format, FormatError, WriteOptions, pec};
use sf_core::Pattern;

/// Offset of the PEC block in the files we write.
pub(crate) const PEC_OFFSET: usize = 22;

pub(crate) fn write(pattern: &Pattern, opts: &WriteOptions) -> Result<Vec<u8>, FormatError> {
    let block = pec::block(pattern, opts)?;
    let mut out = Vec::with_capacity(PEC_OFFSET + block.len());
    out.put(b"#PES0001");
    out.put_u32_le(PEC_OFFSET as u32);
    out.put(&[0; 10]);
    out.put(&block);
    Ok(out)
}

pub(crate) fn read(bytes: &[u8]) -> Result<Pattern, FormatError> {
    if !bytes.starts_with(b"#PES") {
        return Err(if bytes.len() < 4 { FormatError::Truncated } else { FormatError::BadMagic(Format::Pes) });
    }
    let offset = u32_le_at(bytes, 8).ok_or(FormatError::Truncated)? as usize;
    if offset < 12 || offset >= bytes.len() {
        return Err(FormatError::Invalid("PES: PEC block offset out of range".into()));
    }
    pec::read_block(bytes, offset)
}
