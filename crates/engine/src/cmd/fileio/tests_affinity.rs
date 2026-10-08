//! Affinity is explicitly a preview-only reader, through the same commands and save policy.

use super::*;
use proptest::prelude::*;
use serde_json::json;
use std::io::Cursor;

fn png() -> Vec<u8> {
    let im = image::RgbaImage::from_fn(2, 1, |x, _| if x == 0 { image::Rgba([220, 40, 60, 255]) } else { image::Rgba([0, 0, 0, 0]) });
    let mut bytes = Vec::new();
    im.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    bytes
}

/// Synthetic envelope without a native object graph; not a native Affinity writer.
fn file(png: &[u8]) -> Vec<u8> {
    let mut b = vec![0; 72];
    b[..4].copy_from_slice(vectorcraft_affinity::MAGIC);
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
fn preview_opens_with_a_warning_and_never_saves_to_the_source() {
    let mut s = Session::new();
    let bytes = file(&png());
    for name in ["drawing.af", "renamed.png", "unknown"] {
        let result = open_bytes(&mut s, name, &bytes, Some(format!("/source/{name}"))).unwrap();
        assert_eq!(result["format"], "affinity");
        assert!(result["warnings"][0].as_str().unwrap().contains("2×1"));
        assert!(result["warnings"][0].as_str().unwrap().contains("Native vectors"));
        let st = s.doc().unwrap();
        assert!(st.path.is_none());
        assert_eq!(st.format, "vectorcraft");
        assert_eq!((st.doc.artboards[0].rect.width(), st.doc.artboards[0].rect.height()), (2.0, 1.0));
        assert_eq!(st.doc.images.values().next().unwrap().bytes.as_slice(), png());
        let saved = vectorcraft_format::save_file(&st.doc);
        let reopened = vectorcraft_format::load_file(&saved).unwrap();
        assert_eq!(reopened.doc.images, st.doc.images);
        let plan = save_plan(&s, SaveMode::Save, &json!({})).unwrap();
        assert!(plan.path.is_none(), "Save must ask for a new destination");
        assert_eq!(plan.format.id, "vectorcraft");
        assert!(plan.name.ends_with(".vectorcraft"));
    }
    let f = format("affinity").unwrap();
    assert!(f.read && !f.write);
    assert!(OPEN_EXTS.contains(&"af"));
    assert!(!PLACE_EXTS.contains(&"af"));
    assert!(!SAVE_FORMATS.contains(&"affinity"));
    assert!(s.execute("document.serialize", &json!({"format":"af"})).is_err());
    assert!(s.execute("document.export", &json!({"format":"affinity"})).is_err());
    assert!(save_format(None, Some("drawing.af")).is_err());
}

#[test]
fn preview_reaches_the_document_open_command() {
    let mut s = Session::new();
    let r = s.execute("document.open", &json!({"name":"test.af", "dataBase64":vectorcraft_format::base64_encode(&file(&png()))})).unwrap();
    assert_eq!(r["format"], "affinity");
    assert_eq!(r["warnings"].as_array().unwrap().len(), 1);
}

#[test]
fn placing_a_preview_is_rejected_even_when_renamed_or_queued() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":100,"height":100})).unwrap();
    let original = s.doc().unwrap().doc.clone();
    for name in ["test.af", "renamed.png", "renamed.txt"] {
        let p = json!({"name":name, "dataBase64":vectorcraft_format::base64_encode(&file(&png())), "link":true});
        for cmd in ["file.place", "file.place.info"] {
            let e = s.execute(cmd, &p).unwrap_err().to_string();
            assert!(e.contains("File › Open") && e.contains("not supported"), "{e}");
        }
        let e = s.execute("file.place.queue", &json!({"files":[p]})).unwrap_err().to_string();
        assert!(e.contains("File › Open") && e.contains("not supported"), "{e}");
    }
    assert_eq!(s.doc().unwrap().doc, original);
    assert!(place_filters().all(|(_, exts)| !exts.contains(&"af")));
}

#[test]
fn templates_and_libraries_cannot_silently_extract_preview_content() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":100,"height":100})).unwrap();
    let original = s.doc().unwrap().doc.clone();
    for name in ["test.af", "renamed.svg"] {
        let p = json!({"name":name, "dataBase64":vectorcraft_format::base64_encode(&file(&png()))});
        for cmd in ["file.newFromTemplate", "swatch.library.load", "graphicStyle.loadLibrary"] {
            let e = s.execute(cmd, &p).unwrap_err().to_string();
            assert!(e.contains("File › Open"), "{cmd}: {e}");
        }
    }
    assert_eq!(s.documents().len(), 1);
    assert_eq!(s.doc().unwrap().doc, original);
}

#[test]
fn malformed_crc_and_every_truncation_fail() {
    let b = file(&png());
    for end in 0..b.len() {
        assert!(load("bad.af", &b[..end]).is_err(), "{end}");
    }
    let mut crc = b.clone();
    crc[130] ^= 1;
    assert!(load("bad.af", &crc).is_err());
    let mut legacy = b.clone();
    legacy[4..6].copy_from_slice(&10u16.to_le_bytes());
    assert!(load("old.af", &legacy).err().unwrap().to_string().contains("legacy preview"));
}

#[test]
fn end_and_post_image_chunk_crcs_cannot_escape_pixel_decoding() {
    let mut end_crc = png();
    *end_crc.last_mut().unwrap() ^= 1;
    assert!(load("bad-end.af", &file(&end_crc)).is_err());

    let body = b"Note\0Synthetic";
    let mut text = (body.len() as u32).to_be_bytes().to_vec();
    text.extend(b"tEXt");
    text.extend(body);
    let mut h = crc32fast::Hasher::new();
    h.update(b"tEXt");
    h.update(body);
    text.extend(h.finalize().to_be_bytes());
    let mut post_image = png();
    let at = post_image.len() - 12;
    post_image.splice(at..at, text);
    assert!(load("valid-text.af", &file(&post_image)).is_ok());
    post_image[at + 8 + body.len()] ^= 1;
    assert!(load("bad-text.af", &file(&post_image)).is_err());
}

#[test]
fn a_previewless_file_fails_without_replacing_the_active_document() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width":100,"height":100})).unwrap();
    let original = s.doc().unwrap().doc.clone();
    let mut bytes = file(&png());
    bytes[24..32].copy_from_slice(&0u64.to_le_bytes());
    let e = s.execute("document.open", &json!({"name":"no-preview.af", "dataBase64":vectorcraft_format::base64_encode(&bytes)})).unwrap_err();
    assert!(e.to_string().contains("no embedded preview"));
    assert_eq!(s.documents().len(), 1);
    assert_eq!(s.doc().unwrap().doc, original);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn bounded_mutations_do_not_panic(edits in prop::collection::vec((0usize..300, any::<u8>()), 0..16)) {
        let mut b = file(&png());
        for (i, value) in edits { if let Some(byte) = b.get_mut(i) { *byte = value; } }
        let _ = load("mutated.af", &b);
    }
}

#[test]
#[ignore = "set AFFINITY_ORACLE_DIR to synthetic Affinity .af and exported PNG files"]
fn native_affinity_oracle() {
    let dir = std::env::var("AFFINITY_ORACLE_DIR").unwrap();
    for (name, size) in [("synthetic-rectangle", (64.0, 48.0)), ("synthetic-large", (1024.0, 768.0))] {
        let bytes = std::fs::read(format!("{dir}/{name}.af")).unwrap();
        let r = load(&format!("{name}.af"), &bytes).unwrap();
        let ab = r.doc.artboards[0].rect;
        assert_eq!((ab.width(), ab.height()), size);
        let preview = vectorcraft_affinity::preview(&bytes).unwrap();
        assert_eq!(r.doc.images.values().next().unwrap().bytes.as_slice(), preview.png);
        if name == "synthetic-rectangle" {
            let source = image::load_from_memory(&std::fs::read(format!("{dir}/{name}.png")).unwrap()).unwrap().to_rgba8();
            let decoded = image::load_from_memory(preview.png).unwrap().to_rgba8();
            assert_eq!(decoded, source);
        }
    }
}
