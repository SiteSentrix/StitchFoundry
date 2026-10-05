//! Untrusted files never crash the app: garbage, truncated, mutated and hostile SVG and PDF input,
//! and swatch (`.vcswatches`, `.gpl`), graphic style (`.vcstyles`) and flattener preset
//! (`.vcflattener`) libraries, must load as an error or as a document that then renders and
//! exports, without a panic.
//!
//! `PROPTEST_CASES=20000 cargo test -p vectorcraft-engine --test import_fuzz` runs a deeper search.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use proptest::prelude::*;
use serde_json::{Value, json};
use vectorcraft_doc::Document;
use vectorcraft_testkit::catch_quiet;
use vectorcraft_testkit::fixtures::rich_session;

/// 64 cases each, unless `PROPTEST_CASES` asks for more.
fn config() -> ProptestConfig {
    let mut c = ProptestConfig { failure_persistence: None, ..ProptestConfig::default() };
    if std::env::var_os("PROPTEST_CASES").is_none() {
        c.cases = 64;
    }
    c
}

/// Import must not panic; whatever comes back must render and export without panicking either.
fn survive(what: &str, import: impl FnOnce() -> Option<Document>) -> Result<(), TestCaseError> {
    let r = catch_quiet(|| {
        if let Some(d) = import() {
            let mut r = vectorcraft_render::Renderer::new();
            if let Some(ab) = d.artboards.first() {
                let scale = (256.0 / ab.rect.width().max(ab.rect.height()).max(1.0)).min(1.0);
                if vectorcraft_render::raster_size(ab.rect, scale).is_ok() {
                    let _ = r.render_region(&d, ab.rect, scale, true).to_png();
                }
            }
            let _ = vectorcraft_svg::export(&d, &vectorcraft_svg::ExportOptions::default());
            let _ = vectorcraft_engine::export_pdf(&d, &vectorcraft_pdf::PdfOptions::default());
            let _ = vectorcraft_format::save_file(&d);
        }
    });
    r.map_err(|msg| TestCaseError::fail(format!("{what}: panicked: {msg}")))
}

fn rich_doc() -> Document {
    (*rich_session().doc().unwrap().doc).clone()
}

fn rich_svg() -> String {
    vectorcraft_svg::export(&rich_doc(), &vectorcraft_svg::ExportOptions::default())
}

fn rich_pdf() -> Vec<u8> {
    vectorcraft_engine::export_pdf(&rich_doc(), &vectorcraft_pdf::PdfOptions::uncompressed()).unwrap()
}

/// Numbers that tend to break arithmetic: zero, negatives, huge, tiny, exponents, junk.
fn arb_num() -> impl Strategy<Value = String> {
    prop_oneof![
        (-1000.0f64..1000.0).prop_map(|v| format!("{v:.2}")),
        Just("0".to_string()),
        Just("-0".to_string()),
        Just("1e308".to_string()),
        Just("-1e308".to_string()),
        Just("1e-308".to_string()),
        Just("1e15".to_string()),
        Just("4294967296".to_string()),
        Just("-2147483649".to_string()),
        Just("NaN".to_string()),
        Just("inf".to_string()),
        Just("".to_string()),
        Just("1e".to_string()),
        Just("--1".to_string()),
    ]
}

// ---------- SVG ----------

fn arb_svg_attr() -> impl Strategy<Value = String> {
    let names = prop::sample::select(vec![
        "x",
        "y",
        "width",
        "height",
        "rx",
        "ry",
        "r",
        "cx",
        "cy",
        "x1",
        "y1",
        "x2",
        "y2",
        "dx",
        "dy",
        "font-size",
        "stroke-width",
        "stroke-dasharray",
        "stroke-dashoffset",
        "stroke-miterlimit",
        "opacity",
        "fill-opacity",
        "offset",
        "startOffset",
        "letter-spacing",
        "rotate",
        "textLength",
        "fx",
        "fy",
        "fr",
        "points",
        "viewBox",
        "transform",
        "d",
        "fill",
        "stroke",
        "stop-color",
        "mask-type",
        "data-vectorcraft-mask",
        "href",
        "baseline-shift",
        "word-spacing",
        "kerning",
        "writing-mode",
        "font-weight",
        "text-anchor",
        "side",
        "path",
    ]);
    let value = prop_oneof![
        arb_num(),
        prop::collection::vec(arb_num(), 0..7).prop_map(|v| v.join(" ")),
        prop::collection::vec(arb_num(), 0..7).prop_map(|v| format!("matrix({})", v.join(" "))),
        prop::collection::vec(arb_num(), 0..3).prop_map(|v| format!("rotate({}) scale({})", v.join(" "), v.join(","))),
        prop::collection::vec((prop::sample::select(vec!["M", "L", "C", "Q", "A", "Z", "H", "V", "S", "T", "m", "a"]), arb_num()), 0..12)
            .prop_map(|v| v.iter().map(|(c, n)| format!("{c}{n} {n}")).collect::<Vec<_>>().join(" ")),
        prop::collection::vec(arb_num(), 0..2).prop_map(|v| format!("{}%", v.join(""))),
        // Colours (Lab too), mask options and links.
        prop::collection::vec(arb_num(), 0..4).prop_map(|v| format!("lab({})", v.join(" "))),
        prop::sample::select(vec!["noclip invert", "invert", "noclip", "alpha", "luminance", "url(#e)", "#e", "#zz", "none", "currentColor"])
            .prop_map(str::to_string),
    ];
    (names, value).prop_map(|(n, v)| format!(" {n}=\"{v}\""))
}

fn arb_svg_element(depth: u32) -> BoxedStrategy<String> {
    let tag = prop::sample::select(vec![
        "rect",
        "circle",
        "ellipse",
        "line",
        "polyline",
        "polygon",
        "path",
        "text",
        "tspan",
        "textPath",
        "g",
        "use",
        "image",
        "linearGradient",
        "radialGradient",
        "stop",
        "pattern",
        "clipPath",
        "mask",
        "symbol",
        "defs",
        "svg",
        "marker",
        "a",
    ]);
    let attrs = || prop::collection::vec(arb_svg_attr(), 0..6).prop_map(|v| v.concat());
    let leaf = (tag.clone(), attrs(), "[a-z ]{0,6}").prop_map(|(t, a, txt)| format!("<{t} id=\"e\"{a} href=\"#e\">{txt}</{t}>"));
    if depth == 0 {
        return leaf.boxed();
    }
    prop_oneof![
        leaf,
        (tag, attrs(), prop::collection::vec(arb_svg_element(depth - 1), 0..4)).prop_map(|(t, a, ch)| format!("<{t}{a}>{}</{t}>", ch.concat()))
    ]
    .boxed()
}

/// A character-level edit that keeps the string valid UTF-8 (the importer takes `&str`).
fn mutate_text(src: &str, cut: usize, edits: &[(usize, char)]) -> String {
    let mut chars: Vec<char> = src.chars().collect();
    for &(i, c) in edits {
        if !chars.is_empty() {
            let n = chars.len();
            chars[i % n] = c;
        }
    }
    chars.truncate(cut.min(chars.len()));
    chars.into_iter().collect()
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn svg_garbage_never_panics(s in ".{0,300}") {
        survive("svg garbage", || vectorcraft_svg::import(&s).ok())?;
    }

    #[test]
    fn svg_hostile_markup_never_panics(
        root in prop::collection::vec(arb_svg_attr(), 0..4),
        body in prop::collection::vec(arb_svg_element(2), 0..6),
    ) {
        let svg = format!("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\"{}>{}</svg>", root.concat(), body.concat());
        survive(&svg, || vectorcraft_svg::import(&svg).ok())?;
    }

    #[test]
    fn svg_hostile_css_never_panics(
        sheet in r#"[a-z#.>*\[\]=:;{}@ !"'/,-]{0,160}"#,
        decls in r#"[a-z0-9.%: ;!-]{0,60}"#,
        body in prop::collection::vec(arb_svg_element(1), 0..3),
    ) {
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>{sheet}</style><g class=\"a b\"><text id=\"t\" class=\"a\" x=\"1 2 3\" y=\"9\" rotate=\"5\" style=\"{decls}\">ab<tspan dy=\"3\">c</tspan></text></g>{}</svg>",
            body.concat()
        );
        survive(&svg, || vectorcraft_svg::import(&svg).ok())?;
    }

    #[test]
    fn svg_mutated_export_never_panics(
        cut in 0usize..20_000,
        edits in prop::collection::vec((0usize..20_000, prop::sample::select(vec!['<', '>', '"', '/', '-', '9', 'e', '.', ' ', '#', '%', '&', ';', 'x'])), 0..10),
    ) {
        let svg = mutate_text(&rich_svg(), cut, &edits);
        survive("mutated svg", || vectorcraft_svg::import(&svg).ok())?;
    }
}

// ---------- PDF ----------

fn handmade_pdf(content: &str, resources: &str, media: &str) -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox {media} /Contents 4 0 R /Resources {resources} >>"),
        format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len() + 1),
    ];
    let mut out = b"%PDF-1.7\n".to_vec();
    let mut offsets = vec![];
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objs.len() + 1).as_bytes());
    out
}

/// One content-stream operation with junk operands.
fn arb_pdf_op() -> impl Strategy<Value = String> {
    let op = prop::sample::select(vec![
        "m", "l", "c", "v", "y", "h", "re", "f", "f*", "F", "S", "s", "B", "B*", "b", "n", "W", "W*", "q", "Q", "cm", "w", "J", "j", "M", "d", "ri",
        "i", "gs", "g", "G", "rg", "RG", "k", "K", "cs", "CS", "sc", "SC", "scn", "SCN", "sh", "Do", "BT", "ET", "Tf", "Td", "TD", "Tm", "T*", "Tc",
        "Tw", "Tz", "TL", "Tr", "Ts", "Tj", "TJ", "'", "\"", "BMC", "BDC", "EMC", "d0", "d1",
    ]);
    let operand = prop_oneof![
        arb_num(),
        prop::collection::vec(arb_num(), 0..5).prop_map(|v| format!("[{}]", v.join(" "))),
        Just("/F1".to_string()),
        Just("/Sh0".to_string()),
        Just("/Im0".to_string()),
        Just("/GS0".to_string()),
        Just("/DeviceRGB".to_string()),
        Just("/Pattern".to_string()),
        Just("(Hi)".to_string()),
        Just("<00ff>".to_string()),
        Just("[(a) -250 (b)]".to_string()),
    ];
    (prop::collection::vec(operand, 0..7), op).prop_map(|(args, op)| format!("{} {op}", args.join(" ")))
}

fn arb_resources() -> impl Strategy<Value = String> {
    prop::sample::select(vec![
        "<< >>".to_string(),
        "<< /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >>".to_string(),
        "<< /ExtGState << /GS0 << /CA 0.5 /ca -3 /BM /Multiply /SMask /None >> >> >>".to_string(),
        "<< /Shading << /Sh0 << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 1e308 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >> >> >>".to_string(),
        "<< /Shading << /Sh0 << /ShadingType 3 /ColorSpace /DeviceRGB /Coords [0 0 0 0 0 -5] /Function << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [] /N -1 >> >> >> >>".to_string(),
        "<< /XObject << /Im0 << /Type /XObject /Subtype /Image /Width 4294967295 /Height 0 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Length 3 >> >> >>".to_string(),
        "<< /Pattern << /P0 << /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 0 0] /XStep 0 /YStep -0 /Resources << >> >> >> >>".to_string(),
    ])
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn pdf_garbage_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..600)) {
        let mut b = b"%PDF-1.7\n".to_vec();
        b.extend(bytes);
        survive("pdf garbage", || vectorcraft_pdf::import(&b).ok())?;
    }

    #[test]
    fn pdf_hostile_content_never_panics(
        ops in prop::collection::vec(arb_pdf_op(), 0..40),
        resources in arb_resources(),
        media in prop::sample::select(vec!["[0 0 100 100]", "[0 0 0 0]", "[0 0 1e308 1e308]", "[100 100 0 0]", "[-5 -5 -1 -1]", "[0 0 14400 14400]", "[]", "[0 0 NaN 5]"]),
    ) {
        let pdf = handmade_pdf(&ops.join("\n"), &resources, media);
        survive(&ops.join(" "), || vectorcraft_pdf::import(&pdf).ok())?;
    }

    #[test]
    fn pdf_mutated_export_never_panics(
        cut in 0usize..40_000,
        flips in prop::collection::vec((0usize..40_000, prop::sample::select(vec![b'0', b'9', b'-', b'.', b' ', b'[', b']', b'<', b'>', b'/', b'(', b'e', 0u8, 0xff])), 0..10),
    ) {
        let mut b = rich_pdf();
        for (i, c) in flips {
            let n = b.len();
            b[i % n] = c;
        }
        b.truncate(cut.min(b.len()).max(9));
        survive("mutated pdf", || vectorcraft_pdf::import(&b).ok())?;
    }
}

// ---------- library files ----------

/// The `data` of a library file `cmd` writes for the rich document.
fn saved(cmd: &str, params: Value) -> String {
    rich_session().execute(cmd, &params).unwrap()["data"].as_str().unwrap().to_string()
}

/// Library file `data` loaded with `load`; what it holds is then used on the rich document
/// (`then`, given the load's result), which renders and exports.
fn survive_library(what: &str, load: &str, data: &str, then: impl FnOnce(&mut vectorcraft_engine::Session, &Value)) -> Result<(), TestCaseError> {
    survive(what, || {
        let mut s = rich_session();
        let r = s.execute(load, &json!({"data": data, "name": "fuzz"})).ok()?;
        then(&mut s, &r);
        Some((*s.doc().ok()?.doc).clone())
    })
}

fn swatches(what: &str, data: &str) -> Result<(), TestCaseError> {
    survive_library(what, "swatch.library.load", data, |s, r| {
        let _ = s.execute("swatch.library.add", &json!({"library": r["library"], "apply": "fill"}));
    })
}

fn styles(what: &str, data: &str) -> Result<(), TestCaseError> {
    survive_library(what, "graphicStyle.loadLibrary", data, |s, r| {
        let _ = s.execute("select.all", &json!({}));
        let _ = s.execute("graphicStyle.addFromLibrary", &json!({"library": r["library"], "apply": true}));
    })
}

fn flattener_presets(what: &str, data: &str) -> Result<(), TestCaseError> {
    survive_library(what, "flattener.presets.import", data, |s, r| {
        let _ = s.execute("select.all", &json!({}));
        if let Some(name) = r["imported"].get(0) {
            let _ = s.execute("flattener.preview", &json!({"preset": name, "highlight": "allAffected"}));
            let _ = s.execute("object.flattenTransparency", &json!({"preset": name, "lineArtPpi": 36, "gradientPpi": 36}));
        }
    })
}

/// Characters that break JSON and GPL palettes.
fn arb_edit() -> impl Strategy<Value = (usize, char)> {
    (0usize..20_000, prop::sample::select(vec!['{', '}', '[', ']', '"', ':', ',', '-', '9', 'e', '.', ' ', '\n', '#', 'n', 'x', '\t']))
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn library_garbage_never_panics(s in ".{0,300}", head in prop::sample::select(vec!["", "GIMP Palette\n","{\"format\": \"vcswatches\", ", "{\"format\": \"vcstyles\", ", "{\"format\": \"vcflattener\", "])) {
        let data = format!("{head}{s}");
        swatches("swatch library garbage", &data)?;
        styles("style library garbage", &data)?;
        flattener_presets("flattener preset garbage", &data)?;
    }

    #[test]
    fn mutated_swatch_libraries_never_panic(cut in 0usize..20_000, edits in prop::collection::vec(arb_edit(), 0..10), gpl in any::<bool>()) {
        let text = saved("swatch.library.save", json!({"format": if gpl { "gpl" } else { "vcswatches" }}));
        swatches("mutated swatch library", &mutate_text(&text, cut, &edits))?;
    }

    #[test]
    fn mutated_style_libraries_never_panic(cut in 0usize..20_000, edits in prop::collection::vec(arb_edit(), 0..10)) {
        let text = saved("graphicStyle.saveLibrary", json!({}));
        styles("mutated style library", &mutate_text(&text, cut, &edits))?;
    }

    #[test]
    fn mutated_flattener_presets_never_panic(cut in 0usize..5_000, edits in prop::collection::vec(arb_edit(), 0..10)) {
        let text = saved("flattener.presets.export", json!({"names": ["high", "medium", "low"]}));
        flattener_presets("mutated flattener presets", &mutate_text(&text, cut, &edits))?;
    }
}
