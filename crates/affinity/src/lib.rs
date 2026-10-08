//! Affinity container inspection, without interpreting the compressed document graph.
//!
//! The header layout was researched with MIT-licensed afread (see README). The version-12
//! thumbnail record was observed in synthetic documents saved by Affinity 3.3. Do not scan for
//! PNG signatures: an unrelated resource or an obsolete revision is not a document preview.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use std::fmt;

pub const MAGIC: &[u8; 4] = b"\x00\xffKA";
pub const MAX_PREVIEW_BYTES: usize = 16 << 20;
pub const MAX_PREVIEW_DIMENSION: u32 = 4096;

/// Container failure. The compressed graph is deliberately never decompressed here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Malformed(&'static str),
    Unsupported(&'static str),
    Limit(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, reason) = match self {
            Self::Malformed(s) => ("damaged Affinity file", s),
            Self::Unsupported(s) => ("unsupported Affinity file", s),
            Self::Limit(s) => ("Affinity preview limit exceeded", s),
        };
        write!(f, "{kind}: {reason}; export PNG, PSD or SVG from Affinity for full-resolution artwork")
    }
}

impl std::error::Error for Error {}

/// Header identity; offsets remain private so callers cannot bypass validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Header {
    pub version: u16,
    thumbnail_offset: u64,
}

/// Borrowed embedded thumbnail, at its own dimensions, never the native document dimensions.
#[derive(Debug)]
pub struct Preview<'a> {
    pub png: &'a [u8],
    pub width: u32,
    pub height: u32,
}

pub fn is_affinity(bytes: &[u8]) -> bool {
    bytes.starts_with(MAGIC)
}

fn bytes_at(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], Error> {
    let end = offset.checked_add(length).ok_or(Error::Malformed("offset overflow"))?;
    bytes.get(offset..end).ok_or(Error::Malformed("truncated record"))
}

fn le16(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    let b = bytes_at(bytes, offset, 2)?;
    Ok(u16::from_le_bytes(b.try_into().map_err(|_| Error::Malformed("integer"))?))
}

fn le32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let b = bytes_at(bytes, offset, 4)?;
    Ok(u32::from_le_bytes(b.try_into().map_err(|_| Error::Malformed("integer"))?))
}

fn le64(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    let b = bytes_at(bytes, offset, 8)?;
    Ok(u64::from_le_bytes(b.try_into().map_err(|_| Error::Malformed("integer"))?))
}

fn be32(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    let b = bytes_at(bytes, offset, 4)?;
    Ok(u32::from_be_bytes(b.try_into().map_err(|_| Error::Malformed("PNG integer"))?))
}

/// Recognize documents, rejecting add-ons with the same magic and unknown container versions.
pub fn inspect(bytes: &[u8]) -> Result<Header, Error> {
    if !is_affinity(bytes) {
        return Err(Error::Malformed("missing container signature"));
    }
    let version = le16(bytes, 4)?;
    if !(7..=12).contains(&version) {
        return Err(Error::Unsupported("unverified container version"));
    }
    if le16(bytes, 6)? != 0 {
        return Err(Error::Unsupported("container flags"));
    }
    // afread's Prsn document class, stored little-endian. Brushes, macros and palettes
    // share the magic but must not be opened as documents.
    if bytes_at(bytes, 8, 4)? != b"nsrP" {
        return Err(Error::Unsupported("this container is not an Affinity document"));
    }
    if bytes_at(bytes, 12, 4)? != b"#Inf" {
        return Err(Error::Malformed("missing information header"));
    }
    bytes_at(bytes, 0, if version == 7 { 64 } else { 72 })?;
    if version > 7 && bytes_at(bytes, 64, 4)? != b"Prot" {
        return Err(Error::Malformed("missing protocol header"));
    }
    Ok(Header { version, thumbnail_offset: le64(bytes, 24)? })
}

/// Read only the indexed version-12 `Thmb` record, checking every PNG chunk CRC. Compressed
/// pixels must still be validated by the consuming image decoder, with allocation limits.
pub fn preview(bytes: &[u8]) -> Result<Preview<'_>, Error> {
    let h = inspect(bytes)?;
    if h.version != 12 {
        return Err(Error::Unsupported("legacy preview records have not been verified"));
    }
    if h.thumbnail_offset == 0 {
        return Err(Error::Unsupported("no embedded preview; native layer decoding is not implemented"));
    }
    let offset = usize::try_from(h.thumbnail_offset).map_err(|_| Error::Malformed("thumbnail offset overflow"))?;
    if offset < 72 {
        return Err(Error::Malformed("thumbnail overlaps the file header"));
    }
    let record = bytes.get(offset..).ok_or(Error::Malformed("thumbnail offset outside file"))?;
    if bytes_at(record, 0, 8)? != b"\xff\xff\xff\xffThmb" {
        return Err(Error::Malformed("missing indexed thumbnail"));
    }
    if le32(record, 8)? != 1 || bytes_at(record, 28, 1)? != [1] {
        return Err(Error::Unsupported("thumbnail record version or encoding"));
    }
    let size = usize::try_from(le32(record, 24)?).map_err(|_| Error::Limit("encoded size"))?;
    if size > MAX_PREVIEW_BYTES {
        return Err(Error::Limit("encoded preview exceeds 16 MiB"));
    }
    if le32(record, 16)? != 29 || le32(record, 20)? != 0 || u64::from(le32(record, 12)?) != size as u64 + 13 {
        return Err(Error::Malformed("inconsistent thumbnail record lengths"));
    }
    let png = bytes_at(record, 29, size)?;
    if bytes_at(png, 0, 8)? != b"\x89PNG\r\n\x1a\n" || be32(png, 8)? != 13 || bytes_at(png, 12, 4)? != b"IHDR" {
        return Err(Error::Malformed("thumbnail is not a PNG"));
    }
    let width = be32(png, 16)?;
    let height = be32(png, 20)?;
    if width == 0 || height == 0 || width > MAX_PREVIEW_DIMENSION || height > MAX_PREVIEW_DIMENSION {
        return Err(Error::Limit("preview dimensions must be 1..4096"));
    }
    // Bound work by bytes, not a file-supplied chunk count, and require exactly one complete PNG.
    let mut pos = 8usize;
    let (mut palette, mut image_data, mut image_data_ended) = (false, false, false);
    loop {
        let length = usize::try_from(be32(png, pos)?).map_err(|_| Error::Limit("PNG chunk length"))?;
        let chunk_size = length.checked_add(12).ok_or(Error::Malformed("PNG chunk overflow"))?;
        let chunk = bytes_at(png, pos, chunk_size)?;
        let checksum_input = bytes_at(chunk, 4, chunk_size - 8)?;
        if crc32fast::hash(checksum_input) != be32(chunk, chunk_size - 4)? {
            return Err(Error::Malformed("PNG chunk checksum"));
        }
        let kind = bytes_at(chunk, 4, 4)?;
        if !kind.iter().all(u8::is_ascii_alphabetic) {
            return Err(Error::Malformed("PNG chunk type"));
        }
        match kind {
            b"IHDR" if pos == 8 => {}
            b"IHDR" => return Err(Error::Malformed("duplicate PNG header")),
            b"PLTE" => {
                if palette || image_data {
                    return Err(Error::Malformed("PNG palette order"));
                }
                palette = true;
            }
            b"IDAT" => {
                if image_data_ended {
                    return Err(Error::Malformed("non-contiguous PNG image data"));
                }
                image_data = true;
            }
            b"IEND" => {
                if !image_data {
                    return Err(Error::Malformed("PNG has no image data"));
                }
            }
            _ if kind.first().is_some_and(u8::is_ascii_uppercase) => {
                return Err(Error::Unsupported("unknown critical PNG chunk"));
            }
            _ => {}
        }
        if image_data && kind != b"IDAT" {
            image_data_ended = true;
        }
        // Native sample previews are plain, single-frame PNGs. Pixel allocation limits do not
        // necessarily bound aggregate compressed metadata; leave these layouts unsupported.
        if matches!(kind, b"zTXt" | b"iTXt" | b"iCCP") {
            return Err(Error::Unsupported("compressed PNG metadata in the preview"));
        }
        if matches!(kind, b"acTL" | b"fcTL" | b"fdAT") {
            return Err(Error::Unsupported("animated PNG preview"));
        }
        pos = pos.checked_add(chunk_size).ok_or(Error::Malformed("PNG offset overflow"))?;
        if kind == b"IEND" {
            if length != 0 || pos != png.len() {
                return Err(Error::Malformed("PNG end or trailing data"));
            }
            break;
        }
    }
    Ok(Preview { png, width, height })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn fixture() -> Vec<u8> {
        let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
        png.extend(13u32.to_be_bytes());
        png.extend(b"IHDR");
        png.extend(2u32.to_be_bytes());
        png.extend(1u32.to_be_bytes());
        png.extend([8, 6, 0, 0, 0]);
        png.extend(crc32fast::hash(&png[12..]).to_be_bytes());
        png.extend([0; 4]);
        png.extend(b"IDAT");
        png.extend(crc32fast::hash(b"IDAT").to_be_bytes());
        png.extend([0; 4]);
        png.extend(b"IEND");
        png.extend(crc32fast::hash(b"IEND").to_be_bytes());
        let mut b = vec![0; 72];
        b[..4].copy_from_slice(MAGIC);
        b[4..6].copy_from_slice(&12u16.to_le_bytes());
        b[8..12].copy_from_slice(b"nsrP");
        b[12..16].copy_from_slice(b"#Inf");
        b[24..32].copy_from_slice(&72u64.to_le_bytes());
        b[64..68].copy_from_slice(b"Prot");
        b.extend(b"\xff\xff\xff\xffThmb");
        b.extend(1u32.to_le_bytes());
        b.extend((png.len() as u32 + 13).to_le_bytes());
        b.extend(29u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend((png.len() as u32).to_le_bytes());
        b.push(1);
        b.extend(png);
        b
    }

    #[test]
    fn indexed_preview_and_header() {
        let b = fixture();
        assert_eq!(inspect(&b).unwrap().version, 12);
        let p = preview(&b).unwrap();
        assert_eq!((p.width, p.height), (2, 1));
        assert_eq!(p.png, &b[101..]);
    }

    #[test]
    fn every_truncation_is_rejected() {
        let b = fixture();
        for end in 0..b.len() {
            assert!(preview(&b[..end]).is_err(), "{end}");
        }
    }

    #[test]
    fn rejects_assets_versions_offsets_lengths_and_bombs() {
        let b = fixture();
        for (offset, value) in [
            (4, 13u64.to_le_bytes().to_vec()),
            (8, b"urBR".to_vec()),
            (24, u64::MAX.to_le_bytes().to_vec()),
            (24, 1u64.to_le_bytes().to_vec()),
            (24, 0u64.to_le_bytes().to_vec()),
            (84, u32::MAX.to_le_bytes().to_vec()),
            (88, 30u32.to_le_bytes().to_vec()),
            (96, u32::MAX.to_le_bytes().to_vec()),
            (117, 5000u32.to_be_bytes().to_vec()),
        ] {
            let mut broken = b.clone();
            broken[offset..offset + value.len()].copy_from_slice(&value);
            assert!(preview(&broken).is_err(), "offset {offset}");
        }
        for version in 7..12 {
            let mut legacy = b.clone();
            legacy[4..6].copy_from_slice(&(version as u16).to_le_bytes());
            assert!(inspect(&legacy).is_ok());
            assert!(matches!(preview(&legacy), Err(Error::Unsupported(_))));
        }
    }

    #[test]
    fn an_unindexed_png_or_resource_is_never_used() {
        let mut b = fixture();
        b[24..32].copy_from_slice(&0u64.to_le_bytes());
        assert!(preview(&b).is_err());
        b[24..32].copy_from_slice(&72u64.to_le_bytes());
        b[76..80].copy_from_slice(b"Meta");
        assert!(preview(&b).is_err());
    }

    #[test]
    fn every_chunk_checksum_including_the_end_is_checked() {
        let b = fixture();
        for at in [130, b.len() - 1] {
            let mut damaged = b.clone();
            damaged[at] ^= 1;
            assert_eq!(preview(&damaged).unwrap_err(), Error::Malformed("PNG chunk checksum"));
        }
        let mut ancillary = vec![0; 4];
        ancillary.extend(b"tEXt");
        ancillary.extend(crc32fast::hash(b"tEXt").to_be_bytes());
        let mut with_ancillary = b.clone();
        let at = b.len() - 12;
        with_ancillary.splice(at..at, ancillary);
        let png_length = (with_ancillary.len() - 101) as u32;
        with_ancillary[84..88].copy_from_slice(&(png_length + 13).to_le_bytes());
        with_ancillary[96..100].copy_from_slice(&png_length.to_le_bytes());
        assert!(preview(&with_ancillary).is_ok());
        with_ancillary[at + 8] ^= 1;
        assert_eq!(preview(&with_ancillary).unwrap_err(), Error::Malformed("PNG chunk checksum"));
    }

    #[test]
    fn compressed_metadata_and_animation_are_not_preview_support() {
        for kind in [b"zTXt", b"iTXt", b"iCCP", b"acTL", b"fcTL", b"fdAT"] {
            let mut extra = vec![0; 4];
            extra.extend(kind);
            extra.extend(crc32fast::hash(kind).to_be_bytes());
            let mut b = fixture();
            let at = b.len() - 12;
            b.splice(at..at, extra);
            let length = (b.len() - 101) as u32;
            b[84..88].copy_from_slice(&(length + 13).to_le_bytes());
            b[96..100].copy_from_slice(&length.to_le_bytes());
            assert!(matches!(preview(&b), Err(Error::Unsupported(_))));
        }
    }

    #[test]
    fn critical_chunks_follow_a_single_complete_png_layout() {
        fn with_chunks(kinds: &[&[u8; 4]]) -> Vec<u8> {
            let original = fixture();
            let mut b = original[..134].to_vec(); // File, record and complete first IHDR.
            for kind in kinds {
                b.extend([0; 4]);
                b.extend(*kind);
                b.extend(crc32fast::hash(*kind).to_be_bytes());
            }
            let length = (b.len() - 101) as u32;
            b[84..88].copy_from_slice(&(length + 13).to_le_bytes());
            b[96..100].copy_from_slice(&length.to_le_bytes());
            b
        }
        assert!(preview(&with_chunks(&[b"IDAT", b"IDAT", b"IEND"])).is_ok());
        // PNG readers must not reject an otherwise valid unknown ancillary chunk just
        // because its reserved (third-letter) bit is set; future versions can define it.
        assert!(preview(&with_chunks(&[b"abct", b"IDAT", b"IEND"])).is_ok());
        for kinds in [
            vec![b"IEND"],
            vec![b"IDAT", b"IHDR", b"IEND"],
            vec![b"PLTE", b"PLTE", b"IDAT", b"IEND"],
            vec![b"IDAT", b"PLTE", b"IEND"],
            vec![b"IDAT", b"tEXt", b"IDAT", b"IEND"],
            vec![b"ABCD", b"IDAT", b"IEND"],
            vec![b"IDAT", b"IEND", b"tEXt"],
        ] {
            assert!(preview(&with_chunks(&kinds)).is_err(), "{kinds:?}");
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(256))]
        #[test]
        fn arbitrary_input_never_panics(b in prop::collection::vec(any::<u8>(), 0..2048)) {
            let _ = inspect(&b);
            let _ = preview(&b);
        }
        #[test]
        fn mutations_never_panic(edits in prop::collection::vec((0usize..200, any::<u8>()), 0..32)) {
            let mut b = fixture();
            for (i, value) in edits { if let Some(v) = b.get_mut(i) { *v = value; } }
            let _ = preview(&b);
        }
    }
}
