//! The pages of a PDF to import: opening with a password, the pages picked, the box each page
//! is cropped to ([`CropTo`]) and [`info`].

use craft_pdf::syntax::object::{Dict, Stream};
use craft_pdf::syntax::page::Page;
use craft_pdf::syntax::{DecryptionError, LoadPdfError, Pdf};
use kurbo::{Affine, Rect};
use vectorcraft_doc::setup::MAX_BLEED;

use crate::{Choice, CropTo, ImportOptions, PdfError};

/// What `info` tells about one page.
#[derive(Clone, Debug, PartialEq)]
pub struct PageInfo {
    /// The page's size in points as viewers show it (its crop box, rotated).
    pub width: f64,
    pub height: f64,
    /// The page's rotation in degrees (0, 90, 180 or 270).
    pub rotation: u16,
    /// Every box but Bounding, in PDF user space (y up), clipped to the media box. A missing
    /// bleed, trim or art box is the crop box.
    pub boxes: Vec<(CropTo, Rect)>,
}

/// The pages of a PDF ([`info`]).
#[derive(Clone, Debug, PartialEq)]
pub struct PdfInfo {
    pub pages: Vec<PageInfo>,
}

/// Is this a PostScript file (plain, or an EPS with a binary header) rather than a PDF?
pub fn is_postscript(bytes: &[u8]) -> bool {
    bytes.starts_with(b"%!PS") || bytes.starts_with(&[0xC5, 0xD0, 0xD3, 0xC6])
}

/// The application's preflight threshold for arrays and dictionaries in raw or decoded PDF
/// content. Preserve this 256-level diagnostic guard independently of the shared parser's
/// 128-level direct-container cap; passing this preflight does not guarantee the reader accepts
/// every shallower object.
const MAX_OBJECT_NESTING: usize = 256;

fn too_deep() -> PdfError {
    PdfError::Parse(format!("its arrays and dictionaries are nested more than {MAX_OBJECT_NESTING} deep"))
}

/// Do the arrays and dictionaries in the text of `pdf` nest deeper than [`MAX_OBJECT_NESTING`]?
/// Read as tokens (strings and comments hold none), an object at a time. The data of a stream
/// with a filter is skipped: it isn't PDF text until decoded, which this doesn't do, so nesting
/// inside compressed streams is not seen here.
fn nests_too_deep(pdf: &[u8]) -> bool {
    let after =
        |from: usize, what: &[u8]| pdf.get(from..).and_then(|rest| rest.windows(what.len()).position(|w| w == what)).map(|at| from + at + what.len());
    let word = |at: usize, w: &[u8]| pdf.get(at..).is_some_and(|rest| rest.starts_with(w));
    let (mut depth, mut filtered) = (0usize, false);
    let mut i = 0;
    while let Some(&c) = pdf.get(i) {
        i += 1;
        match c {
            b'%' => i += pdf.get(i..).and_then(|rest| rest.iter().position(|c| matches!(c, b'\n' | b'\r'))).unwrap_or(pdf.len()),
            b'(' => {
                // A string: parentheses nest in it, a backslash takes the next byte.
                let mut open = 1usize;
                while open > 0 {
                    match pdf.get(i) {
                        None => return false,
                        Some(b'\\') => i += 1,
                        Some(b'(') => open += 1,
                        Some(b')') => open -= 1,
                        Some(_) => {}
                    }
                    i += 1;
                }
            }
            b'[' => depth += 1,
            b']' => depth = depth.saturating_sub(1),
            b'<' if pdf.get(i) == Some(&b'<') => (depth, i) = (depth + 1, i + 1),
            b'>' if pdf.get(i) == Some(&b'>') => (depth, i) = (depth.saturating_sub(1), i + 1),
            // A string in hexadecimal.
            b'<' => i = after(i, b">").unwrap_or(pdf.len()),
            b'/' if word(i - 1, b"/Filter") => filtered = true,
            b'e' if word(i - 1, b"endobj") => (depth, filtered) = (0, false),
            b'e' if word(i - 1, b"endstream") => i += 8,
            b's' if word(i - 1, b"stream") => {
                depth = 0;
                if std::mem::take(&mut filtered) {
                    i = after(i, b"endstream").unwrap_or(pdf.len());
                }
            }
            _ => {}
        }
        if depth > MAX_OBJECT_NESTING {
            return true;
        }
    }
    false
}

/// Read a PDF, decrypting it with `password` (none: the empty user password). The permissions
/// password opens a file too.
pub(crate) fn open(bytes: &[u8], password: Option<&str>) -> Result<Pdf, PdfError> {
    open_with_options(bytes, password, &open_options())
}

/// Preserve the application's existing accepted file sizes. Ordinary stream decoding still
/// uses the shared reader's immutable per-document limits, and its input copy is fallible.
pub(crate) fn open_options() -> craft_pdf::OpenOptions {
    craft_pdf::OpenOptions { max_input_bytes: usize::MAX, ..Default::default() }
}

pub(crate) fn open_with_options(bytes: &[u8], password: Option<&str>, options: &craft_pdf::OpenOptions) -> Result<Pdf, PdfError> {
    if is_postscript(bytes) {
        return Err(PdfError::PostScript);
    }
    if nests_too_deep(bytes) {
        return Err(too_deep());
    }
    let password = password.unwrap_or_default();
    let read = |pw: &str| craft_pdf::open_slice_with_options(bytes, Some(pw), options);
    read(password)
        .or_else(|e| match e {
            // The reader takes only the open password of RC4 and 128-bit AES files: the
            // permissions password gives it.
            craft_pdf::OpenError::Load(LoadPdfError::Decryption(DecryptionError::PasswordProtected)) if !password.is_empty() => {
                crate::encrypt::user_password(bytes, password).map_or(Err(e), |user| read(&user))
            }
            e => Err(e),
        })
        .map_err(|e| match e {
            craft_pdf::OpenError::Load(LoadPdfError::Decryption(DecryptionError::PasswordProtected)) if password.is_empty() => {
                PdfError::NeedsPassword
            }
            craft_pdf::OpenError::Load(LoadPdfError::Decryption(DecryptionError::PasswordProtected)) => PdfError::WrongPassword,
            e => PdfError::Parse(e.to_string()),
        })
}

/// The error of a page whose decoded operands nest too deeply. Read the same content as the
/// scanner and interpreter: bare Flate streams can exceed an ordinary materialization limit.
pub(crate) fn content_too_deep(page: &Page<'_>) -> Result<(), PdfError> {
    const WINDOW: usize = 64 << 10;
    let mut content = page.content_reader();
    let mut window = Vec::new();
    window.try_reserve_exact(WINDOW).map_err(|_| crate::import_scan::incomplete_content())?;
    let mut nesting = ContentNesting::default();
    loop {
        window.clear();
        if content.read_into(&mut window, WINDOW) == 0 {
            break;
        }
        if nesting.feed(&window) {
            return Err(too_deep());
        }
    }
    if content.truncated() {
        return Err(crate::import_scan::incomplete_content());
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
enum ContentToken {
    #[default]
    Normal,
    Comment,
    Literal {
        depth: usize,
        escaped: bool,
    },
    Hex,
    Less,
    Greater,
}

/// Only lexical nesting is needed before the recursive operand parser runs. Keep state across
/// content windows and streams, including a dictionary delimiter split between the two bytes.
#[derive(Default)]
struct ContentNesting {
    depth: usize,
    token: ContentToken,
}

impl ContentNesting {
    fn feed(&mut self, bytes: &[u8]) -> bool {
        for &byte in bytes {
            match self.token {
                ContentToken::Comment => {
                    if matches!(byte, b'\r' | b'\n') {
                        self.token = ContentToken::Normal;
                    }
                    continue;
                }
                ContentToken::Literal { mut depth, escaped } => {
                    if escaped {
                        self.token = ContentToken::Literal { depth, escaped: false };
                    } else if byte == b'\\' {
                        self.token = ContentToken::Literal { depth, escaped: true };
                    } else {
                        if byte == b'(' {
                            depth = depth.saturating_add(1);
                        } else if byte == b')' {
                            depth = depth.saturating_sub(1);
                        }
                        self.token = if depth == 0 { ContentToken::Normal } else { ContentToken::Literal { depth, escaped: false } };
                    }
                    continue;
                }
                ContentToken::Hex => {
                    if byte == b'>' {
                        self.token = ContentToken::Normal;
                    }
                    continue;
                }
                ContentToken::Less => {
                    self.token = if byte == b'<' || byte == b'>' { ContentToken::Normal } else { ContentToken::Hex };
                    if byte == b'<' {
                        self.depth = self.depth.saturating_add(1);
                        if self.depth > MAX_OBJECT_NESTING {
                            return true;
                        }
                    }
                    continue;
                }
                ContentToken::Greater => {
                    self.token = ContentToken::Normal;
                    if byte == b'>' {
                        self.depth = self.depth.saturating_sub(1);
                        continue;
                    }
                }
                ContentToken::Normal => {}
            }
            match byte {
                b'%' => self.token = ContentToken::Comment,
                b'(' => self.token = ContentToken::Literal { depth: 1, escaped: false },
                b'<' => self.token = ContentToken::Less,
                b'>' => self.token = ContentToken::Greater,
                b'[' => self.depth = self.depth.saturating_add(1),
                b']' => self.depth = self.depth.saturating_sub(1),
                _ => {}
            }
            if self.depth > MAX_OBJECT_NESTING {
                return true;
            }
        }
        false
    }
}

/// The pages and their boxes. An encrypted PDF needs its `password`
/// ([`PdfError::NeedsPassword`] / [`PdfError::WrongPassword`]).
pub fn info(bytes: &[u8], password: Option<&str>) -> Result<PdfInfo, PdfError> {
    let pdf = open(bytes, password)?;
    let pages = pdf
        .pages()
        .iter()
        .map(|page| {
            let (w, h) = page.render_dimensions();
            let rotation = match page.rotation() {
                craft_pdf::syntax::page::Rotation::None => 0,
                craft_pdf::syntax::page::Rotation::Horizontal => 90,
                craft_pdf::syntax::page::Rotation::Flipped => 180,
                craft_pdf::syntax::page::Rotation::FlippedHorizontal => 270,
            };
            let boxes = CropTo::ALL.iter().filter(|c| **c != CropTo::Bounding).map(|c| (*c, page_box(page, *c))).collect();
            PageInfo { width: w as f64, height: h as f64, rotation, boxes }
        })
        .collect();
    Ok(PdfInfo { pages })
}

/// The 0-based pages `opts` pick out of `count`.
pub(crate) fn picked(opts: &ImportOptions, count: usize) -> Result<Vec<usize>, PdfError> {
    let mut v = match &opts.pages {
        Some(p) => {
            if let Some(bad) = p.iter().find(|i| **i >= count) {
                return Err(PdfError::BadPage(bad.saturating_add(1), count));
            }
            p.clone()
        }
        None => (0..count).collect(),
    };
    if let Some(m) = opts.max_pages {
        v.truncate(m);
    }
    if v.is_empty() {
        return Err(PdfError::NoPages);
    }
    Ok(v)
}

fn rect(r: craft_pdf::syntax::object::Rect) -> Rect {
    Rect::new(r.x0, r.y0, r.x1, r.y1).abs()
}

/// Box `which` of `page` in PDF user space, clipped to the media box (Bounding: the crop box,
/// which the art's bounds replace after import). A missing or empty box is the crop box.
pub(crate) fn page_box(page: &Page<'_>, which: CropTo) -> Rect {
    let media = rect(page.media_box());
    let crop = rect(page.intersected_crop_box());
    let key: &[u8] = match which {
        CropTo::Bounding | CropTo::Crop => return crop,
        CropTo::Media if media.area() > 0.0 => return media,
        CropTo::Media => return crop,
        CropTo::Bleed => b"BleedBox",
        CropTo::Trim => b"TrimBox",
        CropTo::Art => b"ArtBox",
    };
    page.raw().get::<craft_pdf::syntax::object::Rect>(key).map(|r| rect(r).intersect(media)).filter(|r| r.area() > 0.0).unwrap_or(crop)
}

/// Where `page` draws on its artboard: the page transform (PDF user space → y-down page space
/// with the crop box at the origin, rotation applied) and `which` box in that space (a page with
/// no area gets the size viewers give it).
pub(crate) fn frame(page: &Page<'_>, which: CropTo) -> (Affine, Rect) {
    let init = Affine::new(page.initial_transform(true).as_coeffs());
    let b = init.transform_rect_bbox(page_box(page, which));
    let (w, h) = page.render_dimensions();
    (init, if b.is_finite() && b.area() > 0.0 { b } else { Rect::new(0.0, 0.0, w as f64, h as f64) })
}

/// How far `page`'s bleed box reaches past its trim box as the page shows (rotated): the bleed
/// `[top, bottom, left, right]` in points, kept to 0–[`MAX_BLEED`].
pub(crate) fn bleed(page: &Page<'_>) -> [f64; 4] {
    let init = Affine::new(page.initial_transform(true).as_coeffs());
    let (trim, bleed) = (init.transform_rect_bbox(page_box(page, CropTo::Trim)), init.transform_rect_bbox(page_box(page, CropTo::Bleed)));
    [trim.y0 - bleed.y0, bleed.y1 - trim.y1, trim.x0 - bleed.x0, bleed.x1 - trim.x1]
        .map(|b| if b.is_finite() { b.clamp(0.0, MAX_BLEED) } else { 0.0 })
}

/// The `PieceInfo` key such files keep their private data under.
const PRIVATE_DATA_OWNER: &[u8] = b"Illustrator"; // brand-ok: the key the files use

/// Most bytes of an editor's private data read.
const MAX_PRIVATE: usize = 256 << 20;

/// The private data of an Illustrator `.ai` (its first page's `PieceInfo`): the `AIPrivateData`
/// streams in order, joined. A PDF saved with the editing data has its streams under
/// `AIPDFPrivateData1`, `AIPDFPrivateData2`, … instead: those are read when there is no
/// `AIPrivateData1`. `None` for a PDF without either (or with a stream that can't be read).
/// What it holds is the editor's own copy of the art (see `vectorcraft_eps::layered_ai`).
pub fn illustrator_data(bytes: &[u8], password: Option<&str>) -> Option<Vec<u8>> {
    let pdf = open(bytes, password).ok()?;
    let page = pdf.pages().first()?;
    let private = page.raw().get::<Dict<'_>>(b"PieceInfo")?.get::<Dict<'_>>(PRIVATE_DATA_OWNER)?.get::<Dict<'_>>(b"Private")?;
    let key = if private.contains_key(b"AIPrivateData1") { "AIPrivateData" } else { "AIPDFPrivateData" };
    let mut data = vec![];
    for n in 1..=100_000u32 {
        let Some(stream) = private.get::<Stream<'_>>(format!("{key}{n}").as_bytes()) else { break };
        data.extend_from_slice(&inflated(&stream, MAX_PRIVATE - data.len())?);
    }
    (!data.is_empty()).then_some(data)
}

/// Does the PDF's first page carry an editor's copy of the art: a PDF-compatible `.ai`
/// (`AIPrivateData` keys) or a PDF saved with its editing data (`AIPDFPrivateData`, #1345)? Without
/// reading the data. Either's trim boxes are its editor's artboards.
pub fn has_illustrator_data(bytes: &[u8], password: Option<&str>) -> bool {
    open(bytes, password).is_ok_and(|pdf| pdf.pages().first().is_some_and(|p| piece_info_has(p, &[b"AIPrivateData", b"AIPDFPrivateData"])))
}

/// `stream` decoded, up to `most` bytes: the decoder is run here rather than by the PDF reader's
/// `decoded()`, which has no limit, so a stream of zeros can't grow to the size of the memory. Only a
/// stream that is plain or only deflated is read (the private data is no other); else, or when it
/// is larger once decoded, `None`.
fn inflated(stream: &Stream<'_>, most: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    if stream.dict().get::<Dict<'_>>(b"DecodeParms").is_some() {
        return None;
    }
    let raw = stream.raw_data();
    let mut out = vec![];
    let limit = u64::try_from(most).ok()?.checked_add(1)?;
    match stream.filters().len() {
        0 => out.extend_from_slice(&raw),
        1 => {
            flate2::read::ZlibDecoder::new(&*raw).take(limit).read_to_end(&mut out).ok()?;
        }
        _ => return None,
    }
    (out.len() <= most).then_some(out)
}

/// Does `page` carry an editor's private data (`PieceInfo` keys starting with `AIPrivateData`)?
/// A file saved without PDF compatibility keeps its art there and shows a placeholder page.
pub(crate) fn has_private_data(page: &Page<'_>) -> bool {
    piece_info_has(page, &[b"AIPrivateData"])
}

/// Does `page` state where its art is: an art, trim or bleed box (ISO 32000-1 §14.11.2, the
/// extent of the page's meaningful content and of the finished page)? A page written from an
/// artboard has them; the notice a file saved without its PDF part shows instead of its art is
/// the same page whatever the document, with no art to state the place of.
pub(crate) fn states_art(page: &Page<'_>) -> bool {
    [&b"ArtBox"[..], b"TrimBox", b"BleedBox"].iter().any(|k| page.raw().get::<[f32; 4]>(*k).is_some())
}

/// Does `page`'s `PieceInfo` hold a key starting with one of `prefixes`, up to four dictionaries in?
fn piece_info_has(page: &Page<'_>, prefixes: &[&[u8]]) -> bool {
    fn walk(d: &Dict<'_>, prefixes: &[&[u8]], depth: u32) -> bool {
        depth < 4
            && d.keys().any(|k| {
                prefixes.iter().any(|p| k.starts_with(p)) || d.get::<Dict<'_>>(k.as_ref()).is_some_and(|sub| walk(&sub, prefixes, depth + 1))
            })
    }
    page.raw().get::<Dict<'_>>(b"PieceInfo").is_some_and(|d| walk(&d, prefixes, 0))
}

#[cfg(test)]
mod content_depth_tests {
    use super::{ContentNesting, MAX_OBJECT_NESTING, nests_too_deep};

    #[test]
    fn lexical_nesting_survives_every_two_piece_split() {
        let n = MAX_OBJECT_NESTING;
        let cases = [
            (format!("{}0{}", "[".repeat(n), "]".repeat(n)), false),
            (format!("{}0{}", "[".repeat(n + 1), "]".repeat(n + 1)), true),
            (format!("{}0{}", "<< /A ".repeat(n), ">> ".repeat(n)), false),
            (format!("{}0{}", "<< /A ".repeat(n + 1), ">> ".repeat(n + 1)), true),
            (format!("% {}\r\n[0]", "[<<".repeat(n + 1)), false),
            (format!("% {}\r{}0{}", "[<<".repeat(n + 1), "[".repeat(n + 1), "]".repeat(n + 1)), true),
            (format!("({}\\({}\\) (nested) \\\\) [0]", "[<<".repeat(n + 1), ">>]".repeat(n + 1)), false),
            (format!("<{}> [0]", "3c3c5b".repeat(n + 1)), false),
            (format!("<> {}0{}", "[".repeat(n + 1), "]".repeat(n + 1)), true),
        ];
        for (text, expected) in cases {
            let bytes = text.as_bytes();
            assert_eq!(nests_too_deep(bytes), expected, "raw guard agreement");
            for split in 0..=bytes.len() {
                let (first, last) = bytes.split_at(split);
                let mut nesting = ContentNesting::default();
                assert_eq!(nesting.feed(first) || nesting.feed(last), expected, "split={split}");
            }
            for size in [1, 2, 3, 7, 31, 64] {
                let mut nesting = ContentNesting::default();
                assert_eq!(bytes.chunks(size).any(|part| nesting.feed(part)), expected, "chunk={size}");
            }
        }
    }
}
