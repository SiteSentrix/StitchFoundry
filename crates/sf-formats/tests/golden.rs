//! Byte-level structure of the written files: headers, offsets, signatures.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use common::{options, sample, threads_for};
use sf_formats::{Format, read, write};

fn file(format: Format) -> Vec<u8> {
    write(format, &sample(format), &options(true)).unwrap()
}

fn u32_le(b: &[u8], off: usize) -> usize {
    u32::from_le_bytes(b[off..off + 4].try_into().unwrap()) as usize
}

fn u16_le(b: &[u8], off: usize) -> usize {
    usize::from(u16::from_le_bytes(b[off..off + 2].try_into().unwrap()))
}

#[test]
fn dst_header_is_512_bytes_and_the_file_ends_with_the_end_record() {
    let b = file(Format::Dst);
    assert!(b.len() > 512 && (b.len() - 512).is_multiple_of(3));
    assert_eq!(&b[..3], b"LA:");
    assert_eq!(&b[3..20], b"Sample 1        \r");
    let header = std::str::from_utf8(&b[..125]).unwrap();
    let fields: Vec<&str> = header.split('\r').collect();
    let names: Vec<&str> = fields.iter().map(|f| f.get(..3).unwrap_or(f)).collect();
    assert_eq!(names, ["LA:", "ST:", "CO:", "+X:", "-X:", "+Y:", "-Y:", "AX:", "AY:", "MX:", "MY:", "PD:", "\u{1a}"]);
    let num = |i: usize| fields[i][3..].trim().replace(' ', "").parse::<i64>().unwrap();
    assert_eq!(fields[1].len(), 10, "ST: has 7 digits");
    assert_eq!(num(1) as usize, (b.len() - 512) / 3, "ST: counts every record");
    assert_eq!(num(2), 3, "CO: colour changes");
    assert_eq!(&fields[9..12], ["MX:+    0", "MY:+    0", "PD:******"]);
    assert!(b[125..512].iter().all(|&c| c == b' '));
    assert!(b.ends_with(&[0x00, 0x00, 0xF3]));

    // The extents (0.1 mm, +y up) agree with the design (+y down); trim markers may reach 0.2 mm further.
    let ext = read(Format::Dst, &b).unwrap().extents();
    let close = |field: i64, mm: f64| (field - (mm * 10.0).round() as i64).abs() <= 2;
    assert!(close(num(3), ext.max.x) && close(num(4), -ext.min.x), "{fields:?} {ext:?}");
    assert!(close(num(5), -ext.min.y) && close(num(6), ext.max.y), "{fields:?} {ext:?}");
}

#[test]
fn pes_header_points_at_the_pec_block() {
    let pes = file(Format::Pes);
    assert_eq!(&pes[..8], b"#PES0001");
    let off = u32_le(&pes, 8);
    assert_eq!(&pes[off..off + 3], b"LA:");
    // The same PEC block as in a .pec file.
    let pec = file(Format::Pec);
    assert_eq!(&pec[..8], b"#PEC0001");
    assert_eq!(&pes[off..], &pec[8..]);
}

#[test]
fn pec_block_layout_and_48_by_38_thumbnails() {
    let f = file(Format::Pec);
    let blk = &f[8..];
    assert_eq!(&blk[..20], b"LA:Sample 1        \r");
    assert_eq!((usize::from(blk[0x22]) * 8, blk[0x23]), (48, 38), "thumbnail size");
    let colors = usize::from(blk[0x30]) + 1;
    assert_eq!(colors, 4);
    assert_eq!(&blk[0x31..0x35], &[5, 13, 2, 37], "palette indices");
    assert!(blk[0x35..0x200].iter().all(|&c| c == 0x20), "colour area padded to 0x200");
    assert_eq!(&blk[0x205..0x208], &[0x31, 0xFF, 0xF0]);

    let thumbs = 0x200 + (usize::from(blk[0x202]) | usize::from(blk[0x203]) << 8 | usize::from(blk[0x204]) << 16);
    assert_eq!(blk[thumbs - 1], 0xFF, "the stitch data ends right before the thumbnails");
    assert_eq!(blk.len() - thumbs, (colors + 1) * 6 * 38, "one image for the design and one per colour");
    // The frame alone sets 148 pixels; every image also shows stitches.
    for img in blk[thumbs..].chunks(6 * 38) {
        assert_eq!(&img[6..12], &[0xF0, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F], "frame top");
        assert_eq!(&img[18..24], &[0x04, 0, 0, 0, 0, 0x20], "frame sides");
        assert!(img.iter().map(|b| b.count_ones()).sum::<u32>() > 148);
    }

    // Width and height in 0.1 mm cover the design.
    let ext = read(Format::Pec, &f).unwrap().extents();
    assert!(u16_le(blk, 0x208) as f64 >= (ext.width() * 10.0).round());
    assert!(u16_le(blk, 0x20A) as f64 >= (ext.height() * 10.0).round());
    // The stitch data starts with the long-form jump to the design's corner.
    assert_eq!(blk[0x210] & 0xF0, 0x90);
    assert_eq!(blk[0x212] & 0xF0, 0x90);
}

#[test]
fn jef_stitch_offset_matches_the_start_of_the_stitch_data() {
    let b = file(Format::Jef);
    let colors = u32_le(&b, 0x18);
    assert_eq!(colors, 4);
    let offset = u32_le(&b, 0);
    assert_eq!(offset, 0x74 + 8 * colors);
    assert_eq!(&b[8..24], b"20261010120000\0\0");
    let indices: Vec<usize> = (0..colors).map(|i| u32_le(&b, 0x74 + 4 * i)).collect();
    assert_eq!(indices, vec![10, 3, 12, 6], "Janome palette indices");
    assert!((0..colors).all(|i| u32_le(&b, 0x74 + 4 * (colors + i)) == 0x0D), "thread types end at the offset");
    // The data at the offset is the jump from the origin; it ends with the end code.
    assert_eq!(&b[offset..offset + 2], &[0x80, 0x02]);
    assert!(b.ends_with(&[0x80, 0x10]));
    assert_eq!(u32_le(&b, 0x1C), (b.len() - offset) / 2, "point count");
}

#[test]
fn other_formats_have_their_structure() {
    let vp3 = file(Format::Vp3);
    assert!(vp3.starts_with(b"%vsm%\0"));
    assert!(vp3.windows(4).any(|w| w == b"xxPP"));
    assert_eq!(vp3.windows(3).filter(|w| *w == [0x00, 0x05, 0x00]).count(), 4, "one colour block per thread");

    let u01 = file(Format::U01);
    assert!(u01.len() > 0x100 && (u01.len() - 0x100).is_multiple_of(3));
    assert!(u01.ends_with(&[0xF8, 0x00, 0x00]));

    let xxx = file(Format::Xxx);
    assert_eq!(u16_le(&xxx, 0x27), 4);
    // The header points at the end record; the colour table follows it and two zero bytes,
    // padded to 21 entries and closed by 00 FF FF FF 00 01.
    let end = u32_le(&xxx, 0xFC);
    assert_eq!(&xxx[end..end + 6], &[0x7F, 0x7F, 0x02, 0x14, 0, 0], "the header points at the end record");
    let expected: Vec<[u8; 4]> = threads_for(Format::Xxx).iter().map(|t| [0, t.color.r, t.color.g, t.color.b]).collect();
    let colours: Vec<[u8; 4]> = xxx[end + 6..end + 6 + 4 * expected.len()].chunks(4).map(|c| c.try_into().unwrap()).collect();
    assert_eq!(colours, expected);
    assert_eq!(xxx.len(), end + 6 + 4 * 21 + 6);
    assert!(xxx.ends_with(&[0x00, 0xFF, 0xFF, 0xFF, 0x00, 0x01]));

    let exp = file(Format::Exp);
    assert!(exp.len().is_multiple_of(2));
}
