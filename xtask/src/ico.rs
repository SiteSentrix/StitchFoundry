//! `cargo xtask ico <out.ico> <png>...`: pack square PNGs (each at most 256 px) into a Windows `.ico`.
//! `cargo xtask icns <out.icns> <png>...`: pack square PNGs (16 to 1024 px) into a macOS `.icns`.
//!
//! Every entry is stored as PNG data, which Windows Vista and later, and macOS 10.7 and later, read
//! at every size. Used by `packaging/icons.sh` (the `.icns` without `iconutil`, so on any system).

use std::path::Path;

/// Width and height from a PNG's IHDR chunk.
fn png_size(png: &[u8]) -> Result<(u32, u32), String> {
    if png.len() < 24 || &png[..8] != b"\x89PNG\r\n\x1a\n" || &png[12..16] != b"IHDR" {
        return Err("not a PNG".into());
    }
    let be = |i: usize| u32::from_be_bytes([png[i], png[i + 1], png[i + 2], png[i + 3]]);
    Ok((be(16), be(20)))
}

/// Build the `.ico` bytes from PNG images.
pub fn pack(pngs: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let count = u16::try_from(pngs.len()).map_err(|_| "too many images")?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * pngs.len() as u32;
    for png in pngs {
        let (w, h) = png_size(png)?;
        if w > 256 || h > 256 {
            return Err(format!("{w}x{h} is larger than 256 px"));
        }
        // 256 is stored as 0.
        out.push((w % 256) as u8);
        out.push((h % 256) as u8);
        out.extend_from_slice(&[0, 0]); // palette size, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in pngs {
        out.extend_from_slice(png);
    }
    Ok(out)
}

pub fn run(args: &[&str]) -> Result<(), String> {
    let [out, inputs @ ..] = args else {
        return Err("usage: cargo xtask ico <out.ico> <png>...".into());
    };
    if inputs.is_empty() {
        return Err("usage: cargo xtask ico <out.ico> <png>...".into());
    }
    let pngs = inputs.iter().map(|p| std::fs::read(p).map_err(|e| format!("{p}: {e}"))).collect::<Result<Vec<_>, _>>()?;
    std::fs::write(Path::new(out), pack(&pngs)?).map_err(|e| format!("{out}: {e}"))
}

/// The `.icns` entry types that take a PNG of `size` pixels (the 1x and 2x slots of `iconutil`).
fn icns_types(size: u32) -> &'static [&'static [u8; 4]] {
    match size {
        16 => &[b"icp4"],
        32 => &[b"icp5", b"ic11"],
        64 => &[b"ic12"],
        128 => &[b"ic07"],
        256 => &[b"ic08", b"ic13"],
        512 => &[b"ic09", b"ic14"],
        1024 => &[b"ic10"],
        _ => &[],
    }
}

/// Build the `.icns` bytes from square PNG images of 16, 32, 64, 128, 256, 512 or 1024 px.
pub fn pack_icns(pngs: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    let mut body = Vec::new();
    for png in pngs {
        let (w, h) = png_size(png)?;
        let types = icns_types(w);
        if w != h || types.is_empty() {
            return Err(format!("{w}x{h}: icns takes square 16, 32, 64, 128, 256, 512 or 1024 px images"));
        }
        let len = u32::try_from(png.len() + 8).map_err(|_| "image too large")?;
        for t in types {
            body.extend_from_slice(*t);
            body.extend_from_slice(&len.to_be_bytes());
            body.extend_from_slice(png);
        }
    }
    let total = u32::try_from(body.len() + 8).map_err(|_| "icon set too large")?;
    let mut out = b"icns".to_vec();
    out.extend_from_slice(&total.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

pub fn run_icns(args: &[&str]) -> Result<(), String> {
    let [out, inputs @ ..] = args else {
        return Err("usage: cargo xtask icns <out.icns> <png>...".into());
    };
    if inputs.is_empty() {
        return Err("usage: cargo xtask icns <out.icns> <png>...".into());
    }
    let pngs = inputs.iter().map(|p| std::fs::read(p).map_err(|e| format!("{p}: {e}"))).collect::<Result<Vec<_>, _>>()?;
    std::fs::write(Path::new(out), pack_icns(&pngs)?).map_err(|e| format!("{out}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_png() {
        assert!(pack(&[b"nope".to_vec()]).is_err());
        assert!(pack_icns(&[b"nope".to_vec()]).is_err());
    }

    /// A PNG signature and IHDR claiming `size` px: enough for the packers, which copy the data.
    fn fake_png(size: u32) -> Vec<u8> {
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR".to_vec();
        v.extend_from_slice(&size.to_be_bytes());
        v.extend_from_slice(&size.to_be_bytes());
        v
    }

    #[test]
    fn icns_has_a_slot_per_size() {
        let out = pack_icns(&[fake_png(16), fake_png(256)]).unwrap();
        assert_eq!(&out[..4], b"icns");
        assert_eq!(u32::from_be_bytes([out[4], out[5], out[6], out[7]]) as usize, out.len());
        // 16 px fills one slot, 256 px two (the 256 and the 128@2x)
        let entry = 8 + 24;
        assert_eq!(out.len(), 8 + 3 * entry);
        assert_eq!(&out[8..12], b"icp4");
        assert_eq!(&out[8 + entry..12 + entry], b"ic08");
        assert_eq!(&out[8 + 2 * entry..12 + 2 * entry], b"ic13");
        assert!(pack_icns(&[fake_png(100)]).is_err());
    }
}
