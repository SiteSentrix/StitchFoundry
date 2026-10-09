//! The layers of an Illustrator EPS or `.ai`.
//!
//! Besides the page it prints, such a file carries the app's own copy of the art: an EPS after its
//! `%%EOF`, between `%AI9_PrivateDataBegin` and `%AI9_PrivateDataEnd` (ASCII85 text of a Zstandard
//! stream, `%AI24_DataStream`; older files: zlib, `%AI9_DataStream`); a PDF-compatible `.ai` in its `AIPrivateData` streams (the Zstandard
//! stream after `%AI24_ZStandard_Data`). It is the document as the legacy Illustrator format writes
//! it (`%AI5_BeginLayer`, `Lb`, `Ln`, `u` … `U`, `*u` … `*U`, `q` … `Q`, `m` `l` `c` `f` `S` …). It
//! has what the page doesn't: the layers with their names, which of them are hidden (and what is on
//! them), the hidden objects, the art outside the artboards, the groups, the compound paths and the
//! clipping groups.
//!
//! The page stays the source of what the art looks like; this copy only gives it its structure. Only
//! the legacy format's own operators and comments are read, never what the app keeps in its private
//! comments (`%_`) and dictionaries. So:
//!
//! - Text objects are stubs here: their characters, fonts and places are in the file's text document
//!   (see `ate`). Type that shows comes from the page, which draws it, into the slot whose text is
//!   where the page has it. Type that doesn't show (a hidden layer or object, or off the page) is made
//!   from the text document; where that can't be read (area type, type on a path) it's left out, with
//!   a warning.
//! - A line with an operator that isn't read (gradients, patterns, images, blends, …) isn't read. If
//!   one is on a layer that shows, the art of this copy would differ from the page's, so the layers
//!   aren't used: the file comes in as its page, as it did before.
//!
//! Sources (no Adobe software was run, no Adobe file is in this repo, and no Adobe specification,
//! SDK or installation was used):
//!
//! - The layer, group, compound-path and clipping vocabulary of the legacy format, the container
//!   (`%AI9_PrivateDataBegin`, `%AI24_DataStream`, `%AI24_ZStandard_Data`, the `AIPrivateData`
//!   streams), the meaning of the `Xw` hidden flag and the text document were worked out from `.eps`
//!   and `.ai` files their users own, used locally and never committed. (Adobe published a
//!   specification of the legacy format, listed by PRONOM at
//!   <https://www.nationalarchives.gov.uk/PRONOM/fmt/423>; it was not used here.)
//! - The wrappers: the text is the standard ASCII85 encoding; the compressed stream is Zstandard,
//!   RFC 8878 (<https://www.rfc-editor.org/rfc/rfc8878>), read with the permissively licensed
//!   `ruzstd` crate (MIT); the older one is zlib, read with `flate2`. The PostScript the layers are
//!   translated into is run by this project's own interpreter (`interp`).
//!
//! Where a file doesn't match what this module expects, it is left alone and its page is imported.

use std::collections::BTreeSet;
use std::io::Read;
use std::sync::Arc;

use vectorcraft_doc::clipnest::nest;
use vectorcraft_doc::{AppearanceItem, Document, LayerColor, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, Point, Rect};

use super::ate::{Story, Texts};
use super::graphics::{GState, Out, slot_of};
use super::interp::Interp;
use super::{Imported, box_of, finish_with, reason};

const BEGIN: &[u8] = b"%AI9_PrivateDataBegin";
const END: &[u8] = b"%AI9_PrivateDataEnd";
const STREAM: &[u8] = b"%AI24_DataStream";
/// The same, in files from before Zstandard.
const STREAM_ZLIB: &[u8] = b"%AI9_DataStream";
/// What a `.ai`'s private data starts with, before its Zstandard stream.
const ZSTD_MARK: &[u8] = b"%AI24_ZStandard_Data";
/// Most bytes the editing copy may have once decompressed.
const MAX_DECODED: u64 = 256 << 20;
/// The notes that say the import left out something the file has (see [`is_loss`]).
const TEXT_LEFT_OUT: &str = "text objects on hidden layers or hidden objects couldn't be read, so they are left out";
const ART_LEFT_OUT: &str = "hidden layers have art this can't read";
const PART_LEFT_OUT: &str = "part of a layer was left out";
const LAYERS_UNREAD: &str = "the file's layers weren't read";

/// Does this import note say that the file has something the document doesn't (hidden text, art or
/// layers that could not be read)? Writing the document over the file would lose it for good.
pub fn is_loss(note: &str) -> bool {
    note.ends_with(TEXT_LEFT_OUT)
        || [ART_LEFT_OUT, PART_LEFT_OUT, LAYERS_UNREAD, super::graphics::TOO_MUCH, super::graphics::FAR_AWAY].iter().any(|n| note.starts_with(n))
}

/// Deepest nesting of layers read (a file with deeper ones comes in as its page).
const MAX_DEPTH: usize = 32;
/// Most layers read.
const MAX_LAYERS: usize = 4096;
/// Most text objects in the layers, and most pieces of type in one place on the page, that are matched
/// to each other (more than this and the layers aren't used).
const MAX_SLOTS: usize = 5000;
const MAX_PIECES: usize = 2000;
/// Most bytes of text in the text objects of the layers (a story named twice counts twice).
const MAX_TEXT: usize = 16 << 20;
/// Most pairs of a text object and a piece of the page's type compared.
const MAX_PAIRS: usize = 4_000_000;

/// What the operators of the format do, as PostScript: paths are the path operators, a fill or a
/// stroke uses the colour the last `k` `K` `g` `G` set (a file's objects each set their own).
const PROLOGUE: &str = "\
/m {moveto} def /l {lineto} def /L {lineto} def /c {curveto} def /C {curveto} def
/v {currentpoint 6 2 roll curveto} def /V {currentpoint 6 2 roll curveto} def
/y {2 copy curveto} def /Y {2 copy curveto} def
/h {closepath} def /N {newpath} def /n {closepath newpath} def
/fc [0 0 0 0] def /sc [0 0 0 1] def /eo false def
/setc {dup length 1 eq {0 get setgray} {dup length 3 eq {aload pop setrgbcolor} {aload pop setcmykcolor} ifelse} ifelse} def
/k {4 array astore /fc exch def} def /K {4 array astore /sc exch def} def
/g {1 exch sub 0 0 0 4 -1 roll 4 array astore /fc exch def} def /G {1 exch sub 0 0 0 4 -1 roll 4 array astore /sc exch def} def
/Xk {pop pop pop 4 array astore /fc exch def} def /XK {pop pop pop 4 array astore /sc exch def} def
/x {pop pop 4 array astore /fc exch def} def /X {pop pop 4 array astore /sc exch def} def
/Xx {6 {pop} repeat 4 array astore /fc exch def} def /XX {6 {pop} repeat 4 array astore /sc exch def} def
/H {} def
/D {1 eq /eo exch def} def
/doF {fc setc eo {eofill} {fill} ifelse} def /doS {sc setc stroke} def
/F {doF} def /f {closepath doF} def /S {doS} def /s {closepath doS} def
/B {gsave doF grestore doS} def /b {closepath B} def
/w {setlinewidth} def /J {setlinecap} def /j {setlinejoin} def /M {setmiterlimit} def /d {setdash} def
/u {} def /U {} def
";

/// Operators that draw, set the state they draw with, or group (`q` `Q` `W` `*u` `*U` are the
/// interpreter's own, see `Interp::native_op`).
const KEEP: &[&str] = &[
    "m", "l", "L", "c", "C", "v", "V", "y", "Y", "h", "n", "N", "f", "F", "s", "S", "b", "B", "k", "K", "g", "G", "Xk", "XK", "x", "X", "Xx", "XX",
    "H", "w", "J", "j", "M", "d", "D", "u", "U", "*u", "*U", "q", "Q", "W", "Xw",
];
/// Operators that only describe an object to the app (its lock, its overprint, its name's id, …):
/// the line is left out.
const IGNORE: &[&str] = &["A", "Ae", "AE", "As", "Ap", "O", "R", "i", "Xd", "Xy", "XR", "XW", "XP"];

/// One token of a line.
enum Tok<'a> {
    Num(f64),
    Str(Vec<u8>),
    Name(&'a str),
}

/// The tokens of `line`: numbers, strings, names (the brackets of an array are left out); `None`
/// for a string that doesn't end.
fn tokens(line: &str) -> Option<Vec<Tok<'_>>> {
    let b = line.as_bytes();
    let mut out = vec![];
    let mut i = 0;
    while let Some(&c) = b.get(i) {
        match c {
            c if c.is_ascii_whitespace() || c == b'[' || c == b']' => i += 1,
            b'(' => {
                let (mut depth, mut s) = (1, vec![]);
                i += 1;
                loop {
                    let c = *b.get(i)?;
                    i += 1;
                    match c {
                        b'\\' => {
                            let e = *b.get(i)?;
                            i += 1;
                            s.push(match e {
                                b'n' => b'\n',
                                b'r' => b'\r',
                                b't' => b'\t',
                                b'0'..=b'7' => {
                                    let mut v = u32::from(e - b'0');
                                    for _ in 0..2 {
                                        match b.get(i) {
                                            Some(d @ b'0'..=b'7') => {
                                                v = v * 8 + u32::from(d - b'0');
                                                i += 1;
                                            }
                                            _ => break,
                                        }
                                    }
                                    (v & 0xff) as u8
                                }
                                other => other,
                            });
                        }
                        b'(' => {
                            depth += 1;
                            s.push(c);
                        }
                        b')' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            s.push(c);
                        }
                        _ => s.push(c),
                    }
                }
                out.push(Tok::Str(s));
            }
            _ => {
                let start = i;
                while b.get(i).is_some_and(|c| !c.is_ascii_whitespace() && !matches!(c, b'[' | b']' | b'(')) {
                    i += 1;
                }
                let word = line.get(start..i)?;
                out.push(word.parse::<f64>().map_or(Tok::Name(word), Tok::Num));
            }
        }
    }
    Some(out)
}

/// What the editing copy holds of a layer.
#[derive(Default)]
struct Layer {
    name: String,
    visible: bool,
    color: Option<[u8; 3]>,
    items: Vec<Item>,
    /// The code read since the last sublayer.
    code: String,
    /// Operators on this layer that aren't read.
    unsupported: BTreeSet<String>,
}

enum Item {
    Code(String),
    Sub(Layer),
}

impl Layer {
    fn flush(&mut self) {
        if !self.code.is_empty() {
            self.items.push(Item::Code(std::mem::take(&mut self.code)));
        }
    }
}

struct Parsed {
    /// The art's bounding box `llx lly urx ury`.
    bbox: [f64; 4],
    /// The first artboard (`%AI3_Cropmarks`), in the same space.
    artboard: Option<[f64; 4]>,
    /// The template box (`%AI3_TemplateBox`): its centre is the centre of the app's canvas.
    template: Option<[f64; 4]>,
    layers: Vec<Layer>,
}

/// The editing copy of the art, decompressed.
fn decode(ps: &[u8]) -> Option<Vec<u8>> {
    fn find(h: &[u8], n: &[u8], from: usize) -> Option<usize> {
        h.get(from..)?.windows(n.len()).position(|w| w == n).map(|p| p + from)
    }
    let begin = find(ps, BEGIN, 0)?;
    let end = find(ps, END, begin).unwrap_or(ps.len());
    let block = ps.get(begin..end)?;
    // Newer files compress with Zstandard, older ones (CS-era) with zlib.
    let (at, zstd) = match find(block, STREAM, 0) {
        Some(at) => (at + STREAM.len(), true),
        None => (find(block, STREAM_ZLIB, 0)? + STREAM_ZLIB.len(), false),
    };
    // The stream's lines each start with a `%`.
    let text: String = block
        .get(at..)?
        .split(|b| matches!(b, b'\r' | b'\n'))
        .filter(|l| !l.is_empty())
        .flat_map(|l| l.strip_prefix(b"%").unwrap_or(l).iter())
        .map(|b| char::from(*b))
        .collect();
    let packed = crate::ps::ascii85_decode(&text)?;
    if zstd { unzstd(&packed) } else { unzlib(&packed) }
}

/// The editing copy of a PDF-compatible `.ai`'s `AIPrivateData` streams, joined, decompressed.
fn decode_ai(private: &[u8]) -> Option<Vec<u8>> {
    unzstd(private.strip_prefix(ZSTD_MARK)?)
}

/// `packed` as a zlib stream decompressed (up to [`MAX_DECODED`] bytes).
fn unzlib(packed: &[u8]) -> Option<Vec<u8>> {
    let mut data = vec![];
    flate2::read::ZlibDecoder::new(packed).take(MAX_DECODED).read_to_end(&mut data).ok()?;
    (data.len() as u64 != MAX_DECODED).then_some(data)
}

/// `packed` as a Zstandard stream decompressed (up to [`MAX_DECODED`] bytes).
fn unzstd(packed: &[u8]) -> Option<Vec<u8>> {
    let mut zstd = ruzstd::decoding::StreamingDecoder::new(packed).ok()?;
    let mut data = vec![];
    (&mut zstd).take(MAX_DECODED).read_to_end(&mut data).ok()?;
    (data.len() as u64 != MAX_DECODED).then_some(data)
}

/// The stories that text objects written plainly (not as comments) name.
fn plain_stories(data: &[u8]) -> BTreeSet<u32> {
    data.split(|b| matches!(b, b'\r' | b'\n'))
        .filter_map(|raw| {
            let line = String::from_utf8_lossy(raw);
            line.trim().strip_suffix("/StoryIndex ,").and_then(|n| n.trim().parse().ok())
        })
        .collect()
}

/// The layers of the editing copy `data`.
fn parse(data: &[u8]) -> Option<Parsed> {
    let (mut bbox, mut hires, mut artboard, mut template) = (None, None, None, None);
    let mut stack: Vec<Layer> = vec![];
    let mut done: Vec<Layer> = vec![];
    let mut dict_depth = 0usize;
    // In a text object's dictionary: the story it names, once the dictionary says.
    let mut text_story: Option<Option<u32>> = None;
    // Whether that text object is written as comments, which the file does for some of its stories
    // twice (once plain, once commented) and for others only commented.
    let mut text_commented = false;
    let plain = plain_stories(data);
    let mut count = 0usize;
    for raw in data.split(|b| matches!(b, b'\r' | b'\n')) {
        let line = String::from_utf8_lossy(raw);
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // A text object's dictionary may be written as comments (`%_`), which a reader that doesn't
        // know text objects skips: what that reader skips is read here, for the text object only.
        let line = match line.strip_prefix("%_") {
            Some(rest) if dict_depth > 0 || rest.starts_with("/AI11Text") && rest.ends_with(':') => {
                if dict_depth == 0 {
                    text_commented = true;
                }
                rest.trim()
            }
            _ => {
                if dict_depth == 0 {
                    text_commented = false;
                }
                line
            }
        };
        if line.starts_with('%') {
            match line {
                "%AI5_BeginLayer" => {
                    count += 1;
                    // Too many, too deep, or in the middle of a dictionary: not a file this reads.
                    if count > MAX_LAYERS || stack.len() > MAX_DEPTH || dict_depth > 0 {
                        return None;
                    }
                    if let Some(parent) = stack.last_mut() {
                        parent.flush();
                    }
                    stack.push(Layer { visible: true, ..Layer::default() });
                }
                "%AI5_EndLayer--" => {
                    if dict_depth > 0 {
                        return None;
                    }
                    let mut l = stack.pop()?;
                    l.flush();
                    match stack.last_mut() {
                        Some(parent) => parent.items.push(Item::Sub(l)),
                        None => done.push(l),
                    }
                }
                _ if stack.is_empty() => {
                    if let Some(v) = line.strip_prefix("%%HiResBoundingBox:") {
                        hires = box_of(v.trim()).or(hires);
                    } else if let Some(v) = line.strip_prefix("%%BoundingBox:") {
                        bbox = box_of(v.trim()).or(bbox);
                    } else if let Some(v) = line.strip_prefix("%AI3_Cropmarks:") {
                        artboard = box_of(v.trim()).or(artboard);
                    } else if let Some(v) = line.strip_prefix("%AI3_TemplateBox:") {
                        template = four_numbers(v).or(template);
                    }
                }
                _ => {}
            }
            continue;
        }
        let Some(layer) = stack.last_mut() else { continue };
        // The app's dictionaries (`/Name :` … `;`): a text object's is a slot for its text.
        if dict_depth > 0 {
            // A dictionary ends with `;`, which may have its key after it (`; /ConfiningPath ,`).
            if line == ";" || line.starts_with("; /") {
                dict_depth -= 1;
                if dict_depth == 0
                    && let Some(story) = text_story.take()
                {
                    // A commented copy of a story that has a plain text object is not another object.
                    if !(text_commented && story.is_some_and(|n| plain.contains(&n))) {
                        layer.code.push_str(&format!("{} __txt\n", story.map_or(-1, i64::from)));
                    }
                }
            } else if line.ends_with(':') {
                dict_depth += 1;
            } else if text_story.is_some()
                && let Some(n) = line.strip_suffix("/StoryIndex ,")
            {
                text_story = Some(n.trim().parse().ok());
            }
            continue;
        }
        if line.starts_with('/') {
            if line.ends_with(':') {
                dict_depth = 1;
                if line.starts_with("/AI11Text") {
                    text_story = Some(None);
                } else {
                    layer.unsupported.insert("an object with its own dictionary".into());
                }
            } else {
                layer.unsupported.insert("an unknown object".into());
            }
            continue;
        }
        let Some(toks) = tokens(line) else {
            layer.unsupported.insert("an unreadable line".into());
            continue;
        };
        let last = toks.last().and_then(|t| if let Tok::Name(n) = t { Some(*n) } else { None });
        match last {
            Some("Lb") => {
                let n: Vec<f64> = toks.iter().filter_map(|t| if let Tok::Num(v) = t { Some(*v) } else { None }).collect();
                layer.visible = n.first().is_none_or(|v| *v != 0.0);
                if let [r, g, b] = n.get(8..11).unwrap_or_default() {
                    let byte = |v: &f64| (v.is_finite() && (0.0..=255.0).contains(v)).then_some(*v as u8);
                    layer.color = byte(r).zip(byte(g)).zip(byte(b)).map(|((r, g), b)| [r, g, b]);
                }
                continue;
            }
            Some("Ln") => {
                if let Some(Tok::Str(s)) = toks.first() {
                    layer.name = String::from_utf8_lossy(s).into_owned();
                }
                continue;
            }
            Some("LB") => continue,
            _ => {}
        }
        let mut ignored = false;
        let mut missing: Option<&str> = None;
        for t in &toks {
            if let Tok::Name(n) = t {
                if IGNORE.contains(n) {
                    ignored = true;
                } else if !KEEP.contains(n) {
                    missing = missing.or(Some(n));
                }
            }
        }
        if let Some(n) = missing {
            let shown = n.len() <= 8 && n.chars().all(|c| c.is_ascii_graphic());
            layer.unsupported.insert(if shown { format!("`{n}`") } else { "binary data".to_string() });
        } else if !ignored {
            layer.code.push_str(line);
            layer.code.push('\n');
        }
    }
    if dict_depth > 0 {
        return None;
    }
    while let Some(mut l) = stack.pop() {
        l.flush();
        match stack.last_mut() {
            Some(parent) => parent.items.push(Item::Sub(l)),
            None => done.push(l),
        }
    }
    Some(Parsed { bbox: hires.or(bbox)?, artboard, template, layers: done })
}

/// Four numbers (a box that may be a point).
fn four_numbers(v: &str) -> Option<[f64; 4]> {
    let n: Vec<f64> = v.split_whitespace().map_while(|w| w.parse::<f64>().ok()).collect();
    let [a, b, c, d] = n.as_slice() else { return None };
    [a, b, c, d].iter().all(|v| v.is_finite() && v.abs() <= 1e7).then_some([*a, *b, *c, *d])
}

/// `names` for a note: the first few, and how many more.
fn few(names: BTreeSet<String>) -> String {
    const SHOWN: usize = 5;
    let more = names.len().saturating_sub(SHOWN);
    let mut list: Vec<String> = names.into_iter().take(SHOWN).collect();
    if more > 0 {
        list.push(format!("{more} more"));
    }
    list.join(", ")
}

/// What the operators not read are, on layers that show and on layers that don't.
fn unread(layers: &[Layer], shown: bool, on: &mut BTreeSet<String>, off: &mut BTreeSet<String>) {
    for l in layers {
        let shown = shown && l.visible;
        let set = if shown { &mut *on } else { &mut *off };
        set.extend(l.unsupported.iter().cloned());
        let subs: Vec<&Layer> = l.items.iter().filter_map(|i| if let Item::Sub(s) = i { Some(s) } else { None }).collect();
        for s in subs {
            unread(std::slice::from_ref(s), shown, on, off);
        }
    }
}

/// A text slot of the layers.
struct SlotInfo {
    story: Option<u32>,
    /// It, its layer and the objects round it show.
    shown: bool,
}

/// The text slots of the layers and objects, in order.
fn collect_slots(nodes: &[Arc<Node>], shown: bool, out: &mut Vec<SlotInfo>) {
    for n in nodes {
        let shown = shown && n.visible;
        if let Some(story) = slot_of(n.name.as_deref()) {
            out.push(SlotInfo { story, shown });
        } else if let Some(c) = n.children().filter(|_| !matches!(n.kind, NodeKind::Compound { .. })) {
            collect_slots(c, shown, out);
        }
    }
}

/// Builds the layers' nodes by running each stretch of their code.
struct Builder {
    out: Option<Out>,
}

impl Builder {
    /// What `code` draws, as the nodes of a layer.
    fn run(&mut self, code: &str) -> Vec<Arc<Node>> {
        let Some(out) = self.out.take() else { return vec![] };
        let src = format!("{PROLOGUE}{code}");
        let mut out = out;
        out.hidden = false;
        let mut it = Interp::new(src.as_bytes(), GState::default(), out);
        it.illustrator = true;
        it.native = true;
        let result = it.run();
        it.release();
        let fault = it.fault.take();
        let mut out = it.out;
        if let Err(e) = result {
            out.warn(&format!("{PART_LEFT_OUT}: {}", reason(&e, fault.as_ref())));
        }
        let drawn = std::mem::take(&mut out.drawn);
        let nodes = nest(&mut out.doc, drawn);
        self.out = Some(out);
        nodes
    }

    fn layer(&mut self, l: &Layer, depth: usize) -> Option<Arc<Node>> {
        let mut children = vec![];
        for item in &l.items {
            match item {
                Item::Code(code) => children.extend(self.run(code)),
                Item::Sub(sub) if depth < MAX_DEPTH => children.extend(self.layer(sub, depth + 1)),
                Item::Sub(_) => {}
            }
        }
        let out = self.out.as_mut()?;
        let name = if l.name.is_empty() { "Layer" } else { &l.name };
        let mut node = Node::layer(out.doc.alloc_id(), name, l.color.map_or(LayerColor::Preset(0), LayerColor::Custom));
        node.visible = l.visible;
        if let Some(c) = node.children_mut() {
            *c = children;
        }
        Some(Arc::new(node))
    }
}

/// How far from a text object's own extent the strokes drawn around it reach (points).
const OUTLINE_REACH: f64 = 6.0;

/// Does `n` look like part of a text object's outline: a path with a stroke, inside `around`?
fn outlines(n: &Node, around: Rect) -> bool {
    matches!(n.kind, NodeKind::Path { .. } | NodeKind::Compound { .. })
        && n.appearance.items.iter().any(|i| matches!(i, AppearanceItem::Stroke(s) if s.visible && s.width > 0.0))
        && n.visual_bounds().is_some_and(|b| {
            let a = around.inflate(OUTLINE_REACH, OUTLINE_REACH);
            b.x0 >= a.x0 && b.y0 >= a.y0 && b.x1 <= a.x1 && b.y1 <= a.y1
        })
}

/// How far apart (points) the baselines of two text nodes may be for them to be on one line.
const SAME_LINE: f64 = 0.5;

/// Where `n`'s baseline lies across the direction it runs in (points), if it is text.
fn baseline(n: &Node) -> Option<f64> {
    let NodeKind::Text(t) = &n.kind else { return None };
    let [a, b, _, _, e, f] = t.xf.as_coeffs();
    let len = a.hypot(b);
    (len > 1e-9).then(|| (a * f - b * e) / len)
}

/// The text objects the page paints, each as the run of nodes that make it up (its text, once for
/// each fill and stroke it has, with the paths of its strokes), in painting order: those that
/// show in `shown`, those of hidden layers and objects (a PDF has them) in `hidden`.
fn text_objects(nodes: &[Arc<Node>], on: bool, shown: &mut Vec<Vec<Arc<Node>>>, hidden: &mut Vec<Vec<Arc<Node>>>) {
    let is_text = |n: &Node| matches!(n.kind, NodeKind::Text(_));
    let (mut i, mut free) = (0, 0);
    while let Some(n) = nodes.get(i) {
        if !is_text(n) {
            if let Some(c) = n.children().filter(|_| !matches!(n.kind, NodeKind::Compound { .. })) {
                text_objects(c, on && n.visible, shown, hidden);
                free = i + 1;
            }
            i += 1;
            continue;
        }
        let mut bounds = n.visual_bounds().unwrap_or_default();
        let (mut start, mut end) = (i, i);
        while start > free
            && let Some(p) = nodes.get(start - 1)
            && outlines(p, bounds)
        {
            start -= 1;
            bounds = bounds.union(p.visual_bounds().unwrap_or_default());
        }
        while let Some(next) = nodes.get(end + 1)
            && let Some(b) = next.visual_bounds()
            && (is_text(next)
                && !b.intersect(bounds.inflate(OUTLINE_REACH, OUTLINE_REACH)).is_zero_area()
                && baseline(next)
                    .zip(nodes.get(start..=end).unwrap_or_default().iter().find_map(|m| baseline(m)))
                    .is_some_and(|(x, y)| (x - y).abs() <= SAME_LINE)
                || outlines(next, bounds))
        {
            end += 1;
            bounds = bounds.union(b);
        }
        let object = nodes.get(start..=end).unwrap_or_default().to_vec();
        let into = if on && n.visible { &mut *shown } else { &mut *hidden };
        into.push(object);
        i = end + 1;
        free = i;
    }
}

/// `n` and what it holds with ids of `doc`.
fn reid(doc: &mut Document, n: &Arc<Node>) -> Arc<Node> {
    let mut n = n.as_ref().clone();
    n.id = doc.alloc_id();
    if let Some(children) = n.children_mut() {
        for c in children.iter_mut() {
            *c = reid(doc, c);
        }
    }
    Arc::new(n)
}

/// The page's text objects given to each of `slots` text slots: in painting order, and when there
/// are more of one than of the other (a label may be a few text objects, or one with several
/// strokes), each goes where the share of the order it is at falls.
fn assign(objects: &[Vec<Arc<Node>>], slots: usize) -> Vec<Vec<&[Arc<Node>]>> {
    let mut given: Vec<Vec<&[Arc<Node>]>> = vec![vec![]; slots];
    for (j, o) in objects.iter().enumerate() {
        if let Some(at) = given.get_mut(((2 * j + 1) * slots / (2 * objects.len())).min(slots.saturating_sub(1))) {
            at.push(o);
        }
    }
    given
}

/// What goes into a text slot.
enum Content {
    /// Text objects (with the strokes round them) of the page.
    Page(Vec<Arc<Node>>),
    /// A text object made from the file's text document.
    Made(Box<Node>),
    /// Nothing the file lets this read.
    Empty,
    /// Its type is on the page with another text object's; there is nothing to put here.
    Merged,
}

/// Replace the text slots in `nodes` with what `content` has for each, in order (a slot given none is
/// left out). `empty` counts those.
fn fill_slots(doc: &mut Document, nodes: &mut Vec<Arc<Node>>, content: &mut std::vec::IntoIter<Content>, empty: &mut usize) {
    let mut i = 0;
    while i < nodes.len() {
        let Some(n) = nodes.get(i) else { break };
        if slot_of(n.name.as_deref()).is_some() {
            let visible = n.visible;
            let replacement = match content.next() {
                Some(Content::Page(parts)) if !parts.is_empty() => {
                    let children = parts.iter().map(|p| reid(doc, p)).collect();
                    let mut group = Node::group(doc.alloc_id(), children);
                    group.visible = visible;
                    Some(group)
                }
                Some(Content::Made(mut text)) => {
                    text.id = doc.alloc_id();
                    text.visible = visible;
                    text.name = Some(MADE.into());
                    Some(*text)
                }
                Some(Content::Merged) => {
                    nodes.remove(i);
                    continue;
                }
                _ => None,
            };
            match replacement {
                Some(node) => {
                    if let Some(slot) = nodes.get_mut(i) {
                        *slot = Arc::new(node);
                    }
                    i += 1;
                }
                None => {
                    *empty += 1;
                    nodes.remove(i);
                }
            }
        } else {
            if n.children().is_some()
                && !matches!(n.kind, NodeKind::Compound { .. })
                && let Some(c) = nodes.get_mut(i).map(Arc::make_mut).and_then(Node::children_mut)
            {
                fill_slots(doc, c, content, empty);
            }
            i += 1;
        }
    }
}

/// How far, in points, the page's type may be from where the file's text document puts it for the
/// two to be the same type.
const SAME_PLACE: f64 = 0.75;

/// Its letters and digits, which is what is compared of type (the page may spell a mark another way).
fn letters(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).collect()
}

/// Does `n` hold type that is where `story` lays one of its lines out: the line, or a piece of it
/// (the page splits a line where its kerning or its styles change), on that line's baseline and
/// after its start? How far from where the line starts it is, if so.
fn same_type(n: &Node, lines: &[(String, Point)]) -> Option<f64> {
    let NodeKind::Text(t) = &n.kind else { return None };
    let have = letters(&t.plain_text());
    let [a, b, _, _, e, f] = t.xf.as_coeffs();
    let len = a.hypot(b);
    if have.is_empty() || len < 1e-9 {
        return None;
    }
    let (ux, uy) = (a / len, b / len);
    // About how far a character of it advances, to say how far along a line a piece may be.
    let advance = n.visual_bounds().map_or(0.0, |r| r.width().max(r.height())) / have.len() as f64;
    lines
        .iter()
        .filter_map(|(text, at)| {
            let line = letters(text);
            let skipped = line.find(&have)?;
            if skipped == 0 {
                return Some(((at.x - e).powi(2) + (at.y - f).powi(2)).sqrt()).filter(|d| *d <= SAME_PLACE);
            }
            let (dx, dy) = (e - at.x, f - at.y);
            let (along, across) = (dx * ux + dy * uy, (dx * uy - dy * ux).abs());
            ((have.len() > 1 || skipped + 1 == line.len())
                && across <= SAME_PLACE
                && along >= -SAME_PLACE
                && along <= line.len() as f64 * advance * 2.0 + 4.0)
                .then_some(across + 0.001 * along)
        })
        .min_by(f64::total_cmp)
}

/// The page draws a line in pieces where its kerning or styles change ("swe", "ep"), and the file
/// has it as one line of one text object: the pieces that on one baseline add up to a line of
/// `lines` become one text (each keeping its own style, the line's spaces between them).
fn join_pieces(parts: &mut Vec<Arc<Node>>, lines: &[(String, Point)]) {
    let text_of = |n: &Arc<Node>| if let NodeKind::Text(t) = &n.kind { Some(t.plain_text()) } else { None };
    let texts: Vec<usize> = parts.iter().enumerate().filter(|(_, n)| matches!(n.kind, NodeKind::Text(_))).map(|(i, _)| i).collect();
    // A page with this many pieces of type in one place isn't one of lines of a story.
    if texts.len() > MAX_PIECES {
        return;
    }
    let mut taken = vec![false; parts.len()];
    // The first piece of each line to join, the others, and the spaces to put after each piece.
    let mut joins: Vec<(usize, Vec<usize>, Vec<String>)> = vec![];
    for &i in &texts {
        if taken.get(i).copied().unwrap_or(true) {
            continue;
        }
        let Some(line) = parts.get(i).and_then(|n| baseline(n)) else { continue };
        let group: Vec<usize> = texts
            .iter()
            .copied()
            .filter(|j| {
                !taken.get(*j).copied().unwrap_or(true) && parts.get(*j).and_then(|n| baseline(n)).is_some_and(|b| (b - line).abs() <= SAME_LINE)
            })
            .collect();
        for j in &group {
            if let Some(t) = taken.get_mut(*j) {
                *t = true;
            }
        }
        let pieces: Vec<String> = group.iter().filter_map(|j| parts.get(*j).and_then(text_of)).collect();
        let places: Vec<(f64, f64)> = group
            .iter()
            .filter_map(|j| parts.get(*j))
            .filter_map(|n| if let NodeKind::Text(t) = &n.kind { Some((t.xf.as_coeffs()[4], t.xf.as_coeffs()[5])) } else { None })
            .collect();
        let distinct = places.iter().enumerate().all(|(a, p)| places.iter().skip(a + 1).all(|q| (p.0 - q.0).hypot(p.1 - q.1) > 0.01));
        let joined: String = pieces.iter().map(|p| letters(p)).collect();
        if group.len() < 2 || !distinct || joined.is_empty() {
            continue;
        }
        let Some((whole, _)) = lines.iter().find(|(text, _)| letters(text) == joined) else { continue };
        // Where the line has spaces after a piece's last character that the page doesn't draw.
        let line_chars: Vec<char> = whole.chars().collect();
        let mut at = 0;
        let mut gaps = vec![];
        for (k, piece) in pieces.iter().enumerate() {
            for _ in piece.chars().filter(|c| !c.is_whitespace()) {
                while line_chars.get(at).is_some_and(|c| c.is_whitespace()) {
                    at += 1;
                }
                at += 1;
            }
            let space: String = line_chars.iter().skip(at).take_while(|c| c.is_whitespace()).collect();
            let next = pieces.get(k + 1);
            let drawn = piece.ends_with(char::is_whitespace) || next.is_none_or(|n| n.starts_with(char::is_whitespace));
            gaps.push(if drawn { String::new() } else { space });
        }
        if at <= line_chars.len() {
            joins.push((group[0], group[1..].to_vec(), gaps));
        }
    }
    let mut gone: BTreeSet<usize> = BTreeSet::new();
    for (first, rest, gaps) in &joins {
        let mut runs: Vec<Vec<vectorcraft_doc::TextRun>> = [first]
            .into_iter()
            .chain(rest)
            .filter_map(|j| parts.get(*j))
            .filter_map(|n| if let NodeKind::Text(t) = &n.kind { Some(t.runs.clone()) } else { None })
            .collect();
        for (r, gap) in runs.iter_mut().zip(gaps) {
            if let Some(last) = r.last_mut() {
                last.text.push_str(gap);
            }
        }
        if let Some(node) = parts.get_mut(*first).map(Arc::make_mut)
            && let NodeKind::Text(t) = &mut node.kind
        {
            t.runs = runs.into_iter().flatten().collect();
            // The layout cached for the first piece is not this text's.
            t.cached_bounds = None;
            t.cached_baselines.clear();
        }
        gone.extend(rest);
    }
    // The pieces that went into a text, removed once the joins are done: removing as each is made
    // would move the places of the pieces that the others still name.
    let mut keep = 0;
    parts.retain(|_| {
        keep += 1;
        !gone.contains(&(keep - 1))
    });
}

/// The box round `parts`.
fn extent(parts: &[Arc<Node>]) -> Option<Rect> {
    parts.iter().filter_map(|n| n.visual_bounds()).reduce(|a, b| a.union(b))
}

/// What goes into each of `infos`' slots: the page's type that the file's text document puts where the
/// page has it, else type made from the text document, else (for a file whose text document can't say)
/// the page's type in painting order.
fn plan(
    infos: &[SlotInfo],
    stories: &[Option<Arc<Story>>],
    template: Option<(f64, f64)>,
    to_doc: Affine,
    pages: [&[Vec<Arc<Node>>]; 2],
    warn: &mut Vec<String>,
) -> Vec<Content> {
    let [shown, hidden] = pages;
    let lines: Vec<Option<Vec<(String, Point)>>> = stories
        .iter()
        .map(|s| {
            let (s, t) = (s.as_ref()?, template?);
            Some(s.line_starts(t).into_iter().map(|(text, (x, y))| (text, to_doc * Point::new(x, y))).collect())
        })
        .collect();
    // The page's objects that are where a shown slot's lines are, nearest first.
    let mut pairs: Vec<(f64, usize, usize)> = vec![];
    for (k, info) in infos.iter().enumerate() {
        let Some(l) = lines.get(k).and_then(Option::as_ref).filter(|_| info.shown) else { continue };
        for (j, object) in shown.iter().enumerate() {
            if let Some(d) = object.iter().filter_map(|n| same_type(n, l)).min_by(f64::total_cmp) {
                pairs.push((d, k, j));
            }
        }
    }
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut owner: Vec<Option<usize>> = vec![None; shown.len()];
    for (_, k, j) in pairs {
        if let Some(o) = owner.get_mut(j).filter(|o| o.is_none()) {
            *o = Some(k);
        }
    }
    // The file's text document is trusted where it agrees with the page about something, or where the
    // page has no type to disagree with.
    let trusted = owner.iter().any(Option::is_some) || shown.is_empty();
    let readable = |k: usize| trusted && lines.get(k).is_some_and(Option::is_some);
    let mut content: Vec<Content> = infos.iter().map(|_| Content::Empty).collect();
    let mut spare: Vec<Vec<Arc<Node>>> = vec![];
    for (j, object) in shown.iter().enumerate() {
        match owner.get(j).copied().flatten().filter(|_| trusted) {
            Some(k) => match content.get_mut(k) {
                Some(Content::Page(p)) => p.extend(object.iter().cloned()),
                Some(c) => *c = Content::Page(object.clone()),
                None => {}
            },
            None => spare.push(object.clone()),
        }
    }
    for (k, c) in content.iter_mut().enumerate() {
        if let (Content::Page(parts), Some(Some(l))) = (c, lines.get(k)) {
            join_pieces(parts, l);
        }
    }
    // Type of the text document where the page has none: hidden type, and shown type off the page.
    for (k, c) in content.iter_mut().enumerate() {
        if matches!(c, Content::Empty)
            && readable(k)
            && let (Some(Some(story)), Some(t)) = (stories.get(k), template)
            && let Some(node) = story.node(NodeId(0), t, to_doc)
        {
            *c = Content::Made(Box::new(node));
        }
    }
    // What is left of the page goes by painting order into the shown slots the file can't say.
    let loose: Vec<usize> = infos
        .iter()
        .enumerate()
        .filter(|(k, i)| i.shown && matches!(content.get(*k), Some(Content::Empty)) && !readable(*k))
        .map(|(k, _)| k)
        .collect();
    if loose.is_empty() {
        // Type of the page that no text object says it owns goes with the nearest text object that has
        // type of the page; with none, it is left out.
        let mut left_out = false;
        for object in &spare {
            let (centre, near) = (extent(object).map(|r| r.center()), |c: &Content| match c {
                Content::Page(parts) => extent(parts).map(|r| r.center()),
                _ => None,
            });
            let nearest = content
                .iter_mut()
                .filter_map(|c| near(c).zip(centre).map(|(p, q)| ((p.x - q.x).hypot(p.y - q.y), c)))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            match nearest {
                Some((_, Content::Page(parts))) => parts.extend(object.iter().cloned()),
                _ => left_out = true,
            }
        }
        if !spare.is_empty() {
            warn.push(
                if left_out {
                    "some of the page's type has no text object in the layers, so it is left out"
                } else {
                    "some of the page's type has no text object of its own in the layers, so it is with another's"
                }
                .into(),
            );
        }
    } else {
        for (slot, parts) in loose.iter().zip(assign(&spare, loose.len())) {
            if let Some(c) = content.get_mut(*slot) {
                // A text object the page shows as part of another's has nothing to hold of its own.
                *c = if parts.is_empty() && !spare.is_empty() {
                    Content::Merged
                } else {
                    Content::Page(parts.into_iter().flat_map(|p| p.iter().cloned()).collect())
                };
            }
        }
        if spare.len() != loose.len() {
            warn.push("the text is on the layers in the order the page paints it, so a text object may be in another group than it was".into());
        }
    }
    // The same for the slots that don't show.
    let unread: Vec<usize> =
        infos.iter().enumerate().filter(|(k, i)| !i.shown && matches!(content.get(*k), Some(Content::Empty))).map(|(k, _)| k).collect();
    for (slot, parts) in unread.iter().zip(assign(hidden, unread.len())) {
        if let Some(c) = content.get_mut(*slot) {
            *c = Content::Page(parts.into_iter().flat_map(|p| p.iter().cloned()).collect());
        }
    }
    content
}

/// How much of `page` two documents draw differently (0 to 1), each seen on white.
fn difference(a: &Document, b: &Document, page: Rect) -> f64 {
    let scale = (400.0 / page.width().max(page.height()).max(1.0)).min(4.0);
    let mut r = vectorcraft_render::Renderer::new();
    let (x, y) = (r.render_region(a, page, scale, true), r.render_region(b, page, scale, true));
    if x.width != y.width || x.height != y.height {
        return 1.0;
    }
    let differing = x
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .zip(y.pixels.as_chunks::<4>().0)
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(a, b)| a.abs_diff(*b) > 48))
        .count();
    differing as f64 / (f64::from(x.width) * f64::from(x.height)).max(1.0)
}

/// More of the page than this differing between the layers' art and the page's own, and the
/// layers aren't used.
const MAX_DIFFERENCE: f64 = 0.05;
/// More than this and the import says so.
const NOTABLE_DIFFERENCE: f64 = 0.002;

/// Which box of the editing copy is the page.
#[derive(Clone, Copy)]
enum Page {
    /// An EPS's page is its art's bounding box.
    Art,
    /// A `.ai` is read on its (first and only) artboard.
    Artboard,
}

/// What [`build`] made: the layers, and the drawing state they were made in (swatches, warnings).
struct Built {
    out: Out,
    layers: Vec<Arc<Node>>,
}

/// The layers of the editing copy `data`, with the text (and strokes round it) of `visible`, the
/// file read as its page, put on them. `base` is the document they go in.
fn build(data: &[u8], visible: &Document, base: Document, page: Page) -> Result<Built, String> {
    let parsed = parse(data).ok_or("its editing data is damaged")?;
    if parsed.layers.is_empty() {
        return Err("its editing data has no layers".into());
    }
    let [llx, lly, urx, ury] = match page {
        Page::Art => parsed.bbox,
        Page::Artboard => parsed.artboard.ok_or("its editing data doesn't say where its artboard is")?,
    };
    let artboard = visible.artboards.first().map(|a| a.rect).ok_or("it has no page")?;
    if matches!(page, Page::Artboard) && visible.artboards.len() != 1 {
        return Err("it has more than one artboard".into());
    }
    if ((urx - llx) - artboard.width()).abs() > 0.5 || ((ury - lly) - artboard.height()).abs() > 0.5 {
        return Err("its editing data is for a different page".into());
    }
    let (mut on, mut off) = (BTreeSet::new(), BTreeSet::new());
    unread(&parsed.layers, true, &mut on, &mut off);
    if !on.is_empty() {
        return Err(format!("its layers use {}, which aren't read yet", few(on)));
    }
    let (mut shown, mut hidden) = (vec![], vec![]);
    for l in &visible.layers {
        if let Some(c) = l.children() {
            text_objects(c, l.visible, &mut shown, &mut hidden);
        }
    }
    let mut doc = base;
    doc.layers.clear();
    let to_doc = Affine::translate((artboard.x0, artboard.y0)) * Affine::new([1.0, 0.0, 0.0, -1.0, -llx, ury]);
    let mut b = Builder { out: Some(Out::new(doc, to_doc, artboard)) };
    let mut layers: Vec<Arc<Node>> = parsed.layers.iter().filter_map(|l| b.layer(l, 0)).collect();
    let mut out = b.out.ok_or("a layer couldn't be read")?;
    let mut infos = vec![];
    collect_slots(&layers, true, &mut infos);
    if infos.iter().all(|i| !i.shown) && !shown.is_empty() {
        return Err("its page has text, and its layers have no text objects to put it in".into());
    }
    // The page's type is compared with every text object of the layers: that has its limits.
    if infos.len() > MAX_SLOTS || infos.len().saturating_mul(shown.len()) > MAX_PAIRS {
        return Err("it has too many text objects".into());
    }
    let texts = Texts::read(data);
    // Each story is read once, however many text objects name it.
    let mut read: std::collections::BTreeMap<u32, Option<Arc<Story>>> = Default::default();
    let stories: Vec<Option<Arc<Story>>> = infos
        .iter()
        .map(|i| {
            let n = i.story?;
            read.entry(n).or_insert_with(|| texts.as_ref().and_then(|t| t.story(n as usize)).map(Arc::new)).clone()
        })
        .collect();
    if stories.iter().flatten().map(|s| s.len()).sum::<usize>() > MAX_TEXT {
        return Err("it has too much text".into());
    }
    let template = parsed.template.map(|t| ((t[0] + t[2]) / 2.0, (t[1] + t[3]) / 2.0));
    let mut notes = vec![];
    let content = plan(&infos, &stories, template, to_doc, [&shown, &hidden], &mut notes);
    let mut empty = 0;
    fill_slots(&mut out.doc, &mut layers, &mut content.into_iter(), &mut empty);
    for n in notes {
        out.warn(&n);
    }
    if empty > 0 {
        out.warn(&format!("{empty} {TEXT_LEFT_OUT}"));
    }
    if !off.is_empty() {
        out.warn(&format!("{ART_LEFT_OUT} ({}), left out of them", few(off)));
    }
    Ok(Built { out, layers })
}

/// Marks (in a node's name, until [`prune_unseen`] has looked) the text objects made from the file's
/// text document.
const MADE: &str = "\u{1}made";
/// How many such objects are weighed against the page (each takes a render of the page's size).
const MAX_WEIGHED: usize = 12;
/// The text objects the file keeps that the page doesn't draw.
const UNSEEN_TEXT: &str = "text objects that the file keeps but its page doesn't draw (what is left of type turned to outlines) are left out";
/// How much more of the page (0 to 1) the layers may differ without a text object and still look no worse.
const UNSEEN_MARGIN: f64 = 5e-5;

/// Clear the marks in `nodes` and say which marked objects show, by id.
fn unmark(nodes: &mut [Arc<Node>], shown: bool, found: &mut Vec<(NodeId, Option<Rect>)>) {
    for n in nodes {
        let shown = shown && n.visible;
        if n.name.as_deref() == Some(MADE) {
            if shown {
                found.push((n.id, n.visual_bounds()));
            }
            Arc::make_mut(n).name = None;
        } else if !matches!(n.kind, NodeKind::Compound { .. })
            && let Some(c) = Arc::make_mut(n).children_mut()
        {
            unmark(c, shown, found);
        }
    }
}

/// Take object `id` out of `nodes` (`hide`: leave it, hidden); whether it was there.
fn take_out(nodes: &mut Vec<Arc<Node>>, id: NodeId, hide: bool) -> bool {
    if let Some(i) = nodes.iter().position(|n| n.id == id) {
        if hide {
            if let Some(n) = nodes.get_mut(i) {
                Arc::make_mut(n).visible = false;
            }
        } else {
            nodes.remove(i);
        }
        return true;
    }
    nodes.iter_mut().any(|n| !matches!(n.kind, NodeKind::Compound { .. }) && Arc::make_mut(n).children_mut().is_some_and(|c| take_out(c, id, hide)))
}

/// The text objects made from the file's text document that show on the page's area, where the page
/// has no type to match them, and that the layers look no worse without, are not drawn by the app
/// that wrote the file (which paints every shown text object; it keeps the type of what was turned
/// to outlines): they are taken out. How many.
fn prune_unseen(page: &Document, doc: &mut Document) -> usize {
    let mut found = vec![];
    unmark(&mut doc.layers, true, &mut found);
    let Some(rect) = page.artboards.first().map(|a| a.rect) else { return 0 };
    if found.is_empty() || found.len() > MAX_WEIGHED {
        return 0;
    }
    let mut with = difference(page, doc, rect);
    let mut removed = 0;
    for (id, bounds) in found {
        if !bounds.is_some_and(|b| !b.intersect(rect).is_zero_area()) {
            continue;
        }
        let mut without = doc.clone();
        if !take_out(&mut without.layers, id, true) {
            continue;
        }
        let d = difference(page, &without, rect);
        if d - with <= UNSEEN_MARGIN {
            take_out(&mut doc.layers, id, false);
            with = d;
            removed += 1;
        }
    }
    removed
}

/// Do the layers' art and the page's own look alike? An error says they don't; `Ok` has the note
/// to give when they differ a little.
fn compare(page: &Document, layered: &Document) -> Result<Option<String>, String> {
    let Some(rect) = page.artboards.first().map(|a| a.rect) else { return Ok(None) };
    let diff = difference(page, layered, rect);
    if diff > MAX_DIFFERENCE {
        return Err(format!("the art on them differs from the page's in {:.0}% of it", diff * 100.0));
    }
    Ok((diff > NOTABLE_DIFFERENCE).then(|| {
        format!(
            "the art on the layers differs from the file's own page in about {:.1}% of it (brush strokes, live effects and the like that the layers' data doesn't describe)",
            diff * 100.0
        )
    }))
}

/// The EPS `visible` (read as its page) read through its editing copy instead: its layers (the
/// hidden ones too), groups, compound paths and clipping groups, with the text and the strokes
/// around it of the page. When the file has no editing copy this reads, `visible` as it was; when
/// it has one that can't be used, `visible` with a warning that says why.
pub(super) fn layered(ps: &[u8], mut visible: Imported) -> Imported {
    let Some(data) = decode(ps) else {
        // A file that says it has the editing copy but has none that can be read is a loss too.
        let has = |n: &[u8]| ps.windows(n.len()).any(|w| w == n);
        if has(BEGIN) && (has(STREAM) || has(STREAM_ZLIB)) {
            visible.warnings.push(format!("{LAYERS_UNREAD} (its editing data is damaged or too large): it comes in as its page, in one layer"));
        }
        return visible;
    };
    let Some(page) = visible.document.artboards.first().map(|a| a.rect) else { return visible };
    let base = Document::new(page.width(), page.height());
    let result = build(&data, &visible.document, base, Page::Art).and_then(|built| {
        let mut done = finish_with(built.out, built.layers);
        let unseen = prune_unseen(&visible.document, &mut done.document);
        if unseen > 0 {
            done.warnings.push(format!("{unseen} {UNSEEN_TEXT}"));
        }
        let note = compare(&visible.document, &done.document)?;
        done.warnings.extend(note);
        Ok(done)
    });
    match result {
        Ok(mut done) => {
            for w in &visible.warnings {
                if !done.warnings.contains(w) {
                    done.warnings.push(w.clone());
                }
            }
            done
        }
        Err(why) => {
            let mut v = visible;
            v.warnings.push(format!("{LAYERS_UNREAD} ({why}): it comes in as its page, in one layer"));
            v
        }
    }
}

/// The PDF-compatible `.ai` `visible` (read as its PDF part) read through its editing copy
/// (`private`, the joined `AIPrivateData` streams) instead, as [`layered`] reads an EPS: with the
/// layers and objects of the file, and the art that lies outside its artboard, which its PDF part
/// doesn't have. `warnings` are the notes of reading the PDF part; the notes returned are those,
/// and the layers'.
pub fn layered_ai(private: &[u8], visible: Document, warnings: Vec<String>) -> (Document, Vec<String>) {
    let Some(data) = decode_ai(private) else { return (visible, warnings) };
    let result = build(&data, &visible, visible.clone(), Page::Artboard).and_then(|built| {
        let mut notes = built.out.warnings.clone();
        let mut done = finish_layers(built.out, built.layers);
        let unseen = prune_unseen(&visible, &mut done);
        if unseen > 0 {
            notes.push(format!("{unseen} {UNSEEN_TEXT}"));
        }
        notes.extend(compare(&visible, &done)?);
        Ok((done, notes))
    });
    match result {
        Ok((done, notes)) => {
            let mut all = warnings;
            all.extend(notes.into_iter().filter(|n| !all.contains(n)).collect::<Vec<_>>());
            (done, all)
        }
        Err(why) => {
            let mut all = warnings;
            all.push(format!("{LAYERS_UNREAD} from its editing data ({why}): they come from its PDF part"));
            (visible, all)
        }
    }
}

fn finish_layers(out: Out, layers: Vec<Arc<Node>>) -> Document {
    let mut doc = out.doc;
    doc.layers = layers;
    doc
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_doc::{CharStyle, TextObject};

    fn piece(text: &str, x: f64, y: f64) -> Arc<Node> {
        Arc::new(Node::new(NodeId(0), NodeKind::Text(Box::new(TextObject::point(Point::new(x, y), text, CharStyle::default())))))
    }

    /// Lines that the page draws in pieces, the pieces of one between those of the other, are each
    /// joined, and only their own pieces go.
    #[test]
    fn interleaved_lines_are_joined_without_taking_each_others_pieces() {
        let mut parts = vec![piece("He", 0.0, 0.0), piece("ab", 0.0, -20.0), piece("cd", 16.0, -20.0), piece("llo", 20.0, 0.0)];
        let lines = [("Hello".to_string(), Point::new(0.0, 0.0)), ("abcd".to_string(), Point::new(0.0, -20.0))];
        join_pieces(&mut parts, &lines);
        let texts: Vec<String> = parts.iter().filter_map(|n| if let NodeKind::Text(t) = &n.kind { Some(t.plain_text()) } else { None }).collect();
        assert_eq!(texts, ["Hello", "abcd"]);
    }
}
