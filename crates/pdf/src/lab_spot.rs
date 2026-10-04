//! Lab alternate spaces for spot colours defined in Lab.
//!
//! The PDF writer gives a Separation colour space a device alternate only, so the export writes
//! Lab spot colours with their CMYK equivalent and [`lab_alternates`] then rewrites those spaces
//! to `[/Separation /Name [/Lab …] <tint function>]`, the tint function running from paper white
//! (L 100, a = b = 0) to the colour's Lab values, and moves the cross-reference offsets after the
//! rewritten bytes.

use vectorcraft_color::cms::Lab;
use vectorcraft_color::cms::lab::D50;

/// `name` as the PDF writer writes a name object (`/` then the bytes, irregular ones as `#XX`).
fn pdf_name(name: &str) -> Vec<u8> {
    let mut out = vec![b'/'];
    for &b in name.as_bytes() {
        let regular = !b"\0\t\n\x0C\r ()<>[]{}/%".contains(&b);
        if b != b'#' && (b'!'..=b'~').contains(&b) && regular {
            out.push(b);
        } else {
            out.extend(format!("#{b:02X}").bytes());
        }
    }
    out
}

/// A number for PDF syntax: at most 4 decimals, no trailing zeros.
fn num(v: f32) -> String {
    let s = format!("{:.4}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// The Separation array of spot colour `name` with a Lab alternate.
fn lab_separation(name: &str, lab: Lab) -> Vec<u8> {
    let mut out = b"[/Separation".to_vec();
    out.extend(pdf_name(name));
    let [x, y, z] = D50.map(num);
    let (l, a, b) = (num(lab.l.clamp(0.0, 100.0)), num(lab.a.clamp(-128.0, 127.0)), num(lab.b.clamp(-128.0, 127.0)));
    out.extend(
        format!(
            "[/Lab<</WhitePoint[{x} {y} {z}]/Range[-128 127 -128 127]>>]<</FunctionType 2/Domain[0 1]/Range[0 100 -128 127 -128 127]/C0[100 0 0]/C1[{l} {a} {b}]/N 1>>]"
        )
        .bytes(),
    );
    out
}

fn find(hay: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    hay.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|i| i + from)
}

/// Rewrite the DeviceCMYK Separation spaces of `spots` (colorant name, Lab values) in `pdf` (as
/// written by the export: uncompressed objects, a cross-reference table) to Lab alternates.
/// Spaces written another way are left as they are.
pub(crate) fn lab_alternates(pdf: Vec<u8>, spots: &[(String, Lab)]) -> Vec<u8> {
    // (start, end, replacement) of each rewritten array, in file order.
    let mut edits: Vec<(usize, usize, Vec<u8>)> = vec![];
    for (name, lab) in spots {
        let mut head = b"[/Separation".to_vec();
        head.extend(pdf_name(name));
        head.extend(b"/DeviceCMYK<<");
        let mut from = 0;
        while let Some(start) = find(&pdf, &head, from) {
            let Some(end) = find(&pdf, b">>]", start).map(|e| e + 3) else { break };
            edits.push((start, end, lab_separation(name, *lab)));
            from = end;
        }
    }
    let Some(xref) = xref_offset(&pdf) else { return pdf };
    if edits.is_empty() || edits.iter().any(|e| e.1 > xref) {
        return pdf;
    }
    edits.sort_by_key(|e| e.0);
    // Where a byte offset of the old file lands in the new one.
    let moved = |off: usize| -> usize {
        edits.iter().filter(|e| e.1 <= off).fold(off as isize, |o, e| o + e.2.len() as isize - (e.1 - e.0) as isize) as usize
    };
    let mut out = Vec::with_capacity(pdf.len() + 256 * edits.len());
    let mut at = 0;
    for (start, end, new) in &edits {
        out.extend_from_slice(&pdf[at..*start]);
        out.extend_from_slice(new);
        at = *end;
    }
    out.extend_from_slice(&pdf[at..xref]);
    // The cross-reference table: `xref\n0 N\n` then N 20-byte entries; in-use ones hold offsets.
    let tail = &pdf[xref..];
    let Some(entries) = find(tail, b"\n", 5).map(|i| i + 1) else { return pdf };
    let Some(count) = std::str::from_utf8(&tail[5..entries - 1]).ok().and_then(|s| s.split_whitespace().nth(1)?.parse::<usize>().ok()) else {
        return pdf;
    };
    let table_end = entries + 20 * count;
    if tail.len() < table_end {
        return pdf;
    }
    out.extend_from_slice(&tail[..entries]);
    for e in tail[entries..table_end].chunks(20) {
        match std::str::from_utf8(&e[..10]).ok().and_then(|s| s.parse::<usize>().ok()) {
            Some(off) if e[17] == b'n' => {
                out.extend(format!("{:010}", moved(off)).bytes());
                out.extend_from_slice(&e[10..]);
            }
            _ => out.extend_from_slice(e),
        }
    }
    let rest = &tail[table_end..];
    let Some(sx) = rfind(rest, b"startxref\n") else { return pdf };
    out.extend_from_slice(&rest[..sx]);
    out.extend(format!("startxref\n{}\n%%EOF", moved(xref)).bytes());
    out
}

fn rfind(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).rposition(|w| w == needle)
}

/// The offset `startxref` gives, when it points at a cross-reference table.
fn xref_offset(pdf: &[u8]) -> Option<usize> {
    let sx = rfind(pdf, b"startxref")?;
    let off: usize = std::str::from_utf8(&pdf[sx + 9..]).ok()?.split_whitespace().next()?.parse().ok()?;
    pdf.get(off..)?.starts_with(b"xref").then_some(off)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_written_like_the_pdf_writer_writes_them() {
        assert_eq!(pdf_name("Ink"), b"/Ink");
        assert_eq!(pdf_name("PMSish 123"), b"/PMSish#20123");
        assert_eq!(pdf_name("A#(b)"), b"/A#23#28b#29");
        assert_eq!((num(0.5), num(-0.0), num(100.0), num(-12.34567)), ("0.5".into(), "0".into(), "100".into(), "-12.3457".into()));
    }
}
