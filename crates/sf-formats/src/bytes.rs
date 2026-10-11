//! Bounds-checked byte reading and little helpers for writing.

use crate::FormatError;

/// A read cursor over untrusted bytes: every read is bounds-checked and
/// returns [`FormatError::Truncated`] instead of panicking.
#[derive(Clone, Debug)]
pub(crate) struct Cursor<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Cursor { data, pos: 0 }
    }

    /// A cursor at `pos` (which may be past the end: every read then fails).
    pub fn at(data: &'a [u8], pos: usize) -> Self {
        Cursor { data, pos }
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn remaining(&self) -> usize {
        self.data.len().saturating_sub(self.pos)
    }

    /// The next `n` bytes, consumed.
    pub fn take(&mut self, n: usize) -> Result<&'a [u8], FormatError> {
        let end = self.pos.checked_add(n).ok_or(FormatError::Truncated)?;
        let s = self.data.get(self.pos..end).ok_or(FormatError::Truncated)?;
        self.pos = end;
        Ok(s)
    }

    /// The next `n` bytes, not consumed.
    pub fn peek(&self, n: usize) -> Option<&'a [u8]> {
        self.data.get(self.pos..self.pos.checked_add(n)?)
    }

    pub fn skip(&mut self, n: usize) -> Result<(), FormatError> {
        self.take(n).map(|_| ())
    }

    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], FormatError> {
        let mut a = [0u8; N];
        a.copy_from_slice(self.take(N)?);
        Ok(a)
    }

    pub fn u8(&mut self) -> Result<u8, FormatError> {
        Ok(self.array::<1>()?[0])
    }

    pub fn u16_be(&mut self) -> Result<u16, FormatError> {
        Ok(u16::from_be_bytes(self.array()?))
    }

    pub fn i16_be(&mut self) -> Result<i16, FormatError> {
        Ok(i16::from_be_bytes(self.array()?))
    }

    pub fn u32_be(&mut self) -> Result<u32, FormatError> {
        Ok(u32::from_be_bytes(self.array()?))
    }

    pub fn i32_be(&mut self) -> Result<i32, FormatError> {
        Ok(i32::from_be_bytes(self.array()?))
    }
}

/// Little-endian `u32` at `off`, if the bytes are there.
pub(crate) fn u32_le_at(data: &[u8], off: usize) -> Option<u32> {
    let b: [u8; 4] = data.get(off..off.checked_add(4)?)?.try_into().ok()?;
    Some(u32::from_le_bytes(b))
}

/// Little-endian `u16` at `off`, if the bytes are there.
pub(crate) fn u16_le_at(data: &[u8], off: usize) -> Option<u16> {
    let b: [u8; 2] = data.get(off..off.checked_add(2)?)?.try_into().ok()?;
    Some(u16::from_le_bytes(b))
}

/// Writing helpers on a byte vector.
pub(crate) trait Put {
    fn put(&mut self, b: &[u8]);
    fn put_u16_le(&mut self, v: u16) {
        self.put(&v.to_le_bytes());
    }
    fn put_u16_be(&mut self, v: u16) {
        self.put(&v.to_be_bytes());
    }
    fn put_i16_be(&mut self, v: i16) {
        self.put(&v.to_be_bytes());
    }
    fn put_u32_le(&mut self, v: u32) {
        self.put(&v.to_le_bytes());
    }
    fn put_i32_le(&mut self, v: i32) {
        self.put(&v.to_le_bytes());
    }
    fn put_u32_be(&mut self, v: u32) {
        self.put(&v.to_be_bytes());
    }
    fn put_i32_be(&mut self, v: i32) {
        self.put(&v.to_be_bytes());
    }
}

impl Put for Vec<u8> {
    fn put(&mut self, b: &[u8]) {
        self.extend_from_slice(b);
    }
}

/// A design label as printable ASCII: other characters become `_`, at most
/// `max` characters.
pub(crate) fn ascii_label(name: &str, max: usize) -> String {
    name.chars().filter(|c| !c.is_control()).take(max).map(|c| if c.is_ascii_graphic() || c == ' ' { c } else { '_' }).collect()
}

/// A label read from a file: bytes up to the first NUL or CR, trailing spaces
/// removed, non-ASCII bytes replaced.
pub(crate) fn read_label(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0 || b == b'\r').unwrap_or(bytes.len());
    let s: String = bytes.iter().take(end).map(|&b| if b.is_ascii_graphic() || b == b' ' { b as char } else { '_' }).collect();
    s.trim_end().to_string()
}

/// `i32` from an `i64` known to be in range (saturating otherwise).
pub(crate) fn sat_i32(v: i64) -> i32 {
    v.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

/// `u16` from an `i64`, saturating.
pub(crate) fn sat_u16(v: i64) -> u16 {
    v.clamp(0, i64::from(u16::MAX)) as u16
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_never_reads_past_the_end() {
        let data = [1u8, 2, 3];
        let mut c = Cursor::new(&data);
        assert_eq!(c.u16_be().unwrap(), 0x0102);
        assert_eq!(c.u16_be(), Err(FormatError::Truncated));
        assert_eq!(c.u8().unwrap(), 3);
        assert_eq!(c.remaining(), 0);
        assert_eq!(c.take(usize::MAX), Err(FormatError::Truncated));
        let mut far = Cursor::at(&data, usize::MAX);
        assert_eq!(far.u8(), Err(FormatError::Truncated));
        assert_eq!(far.peek(1), None);
        assert_eq!(u32_le_at(&data, usize::MAX - 1), None);
    }

    #[test]
    fn labels() {
        assert_eq!(ascii_label("Rosé\tgarden flowers 2026", 16), "Ros_garden flowe");
        assert_eq!(read_label(b"Hello   \rjunk"), "Hello");
    }
}
