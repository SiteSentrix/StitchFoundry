//! File → Package (`file.package`): the document with its linked files and fonts in a folder or a
//! zip, and the Document Info report it shares with Document Info › Save….

use std::collections::BTreeMap;
use std::io::Read;

use serde_json::{Value, json};

use super::tests_links::{BLUE, Folder, RED, open, place, png, save, session, write};
use super::*;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.execute(id, &p).unwrap_or_else(|e| panic!("{id} {p}: {e}"))
}

/// A saved document `dir/poster.vectorcraft` with two linked files (in `dir/art`) and a line of
/// type.
fn poster(dir: &Folder) -> Session {
    let (a, b) = (dir.file("art/red.png"), dir.file("art/blue.png"));
    write(&a, &png(600, 300, RED));
    write(&b, &png(300, 300, BLUE));
    let mut s = session();
    place(&mut s, &a);
    place(&mut s, &b);
    run(&mut s, "text.create", json!({"x": 10, "y": 40, "text": "Poster"}));
    save(&mut s, &dir.file("poster.vectorcraft"));
    s
}

/// The files in a zip archive by name (CRC-checked).
fn unzip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let le16 = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    let le32 = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as usize;
    let end = bytes.len() - 22;
    assert_eq!(le32(end), 0x0605_4b50, "end of central directory");
    let (count, mut i) = (le16(end + 10), le32(end + 16));
    let mut out = BTreeMap::new();
    for _ in 0..count {
        assert_eq!(le32(i), 0x0201_4b50);
        let (method, crc, packed, name_len, offset) = (le16(i + 10), le32(i + 16), le32(i + 20), le16(i + 28), le32(i + 42));
        let name = String::from_utf8(bytes[i + 46..i + 46 + name_len].to_vec()).unwrap();
        i += 46 + name_len + le16(i + 30) + le16(i + 32);
        assert_eq!(le32(offset), 0x0403_4b50);
        let data = &bytes[offset + 30 + le16(offset + 26) + le16(offset + 28)..][..packed];
        let mut content = vec![];
        match method {
            8 => drop(flate2::read::DeflateDecoder::new(data).read_to_end(&mut content).unwrap()),
            _ => content = data.to_vec(),
        }
        let mut c = flate2::Crc::new();
        c.update(&content);
        assert_eq!(c.sum() as usize, crc, "{name}");
        out.insert(name, content);
    }
    out
}

#[test]
fn two_links_and_a_font_are_collected_and_relinked() {
    let dir = Folder::new("package");
    let mut s = poster(&dir);
    let out = dir.file("out");
    let r = run(&mut s, "file.package", json!({"folder": out}));
    let files: Vec<&str> = r["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert_eq!(files, ["poster.vectorcraft", "Links/red.png", "Links/blue.png", "Fonts/SourceSans3-Regular.ttf", "poster Report.txt"]);
    assert_eq!((r["links"].as_u64(), r["fonts"].as_u64(), r["missingLinks"].clone()), (Some(2), Some(1), json!([])));
    let root = std::path::Path::new(&out).join("poster Folder");
    assert_eq!(std::path::Path::new(r["folder"].as_str().unwrap()), root);
    for f in &files {
        assert!(root.join(f).is_file(), "{f} written");
    }
    assert_eq!(std::fs::read(root.join("Links/red.png")).unwrap(), png(600, 300, RED));
    let font = std::fs::read(root.join("Fonts/SourceSans3-Regular.ttf")).unwrap();
    assert_eq!(&font[..4], &[0, 1, 0, 0], "a TrueType file");
    // The open document still links to the originals.
    let link = |s: &Session| {
        let mut paths = vec![];
        s.doc().unwrap().doc.visit_images(|_, im| paths.push(im.link.clone().unwrap()));
        paths
    };
    assert!(link(&s).iter().all(|l| l.path.contains("art")));
    // The packaged one opens with the copies, even with the originals gone.
    std::fs::remove_dir_all(dir.0.join("art")).unwrap();
    let r = open(&mut s, &root.join("poster.vectorcraft").to_string_lossy());
    assert_eq!(r["missingLinks"], json!([]), "{r}");
    for l in link(&s) {
        assert!(std::path::Path::new(&l.path).starts_with(root.join("Links")), "{l:?}");
        assert!(l.relative.as_deref().is_some_and(|r| r.starts_with("Links/")));
    }
    assert!(s.doc().unwrap().doc.images.values().all(|b| !b.is_proxy()), "the copies' pixels");
}

#[test]
fn an_unsaved_document_cant_be_packaged() {
    let mut s = session();
    let e = s.execute("file.package", &json!({"folder": std::env::temp_dir()})).unwrap_err().to_string();
    assert!(e.contains("save the document first"), "{e}");
    assert!(Session::new().execute("file.package", &json!({})).is_err(), "no document");
}

#[test]
fn without_a_folder_the_zip_has_the_same_files() {
    let dir = Folder::new("package-zip");
    let mut s = poster(&dir);
    let r = run(&mut s, "file.package", json!({"name": "Poster Kit"}));
    assert_eq!(r["name"], "Poster Kit.zip");
    let zip = unzip(&vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap());
    let want: Vec<String> = r["files"].as_array().unwrap().iter().map(|f| format!("Poster Kit/{}", f.as_str().unwrap())).collect();
    let mut want_sorted = want.clone();
    want_sorted.sort();
    assert_eq!(zip.keys().cloned().collect::<Vec<_>>(), want_sorted);
    assert_eq!(zip["Poster Kit/Links/blue.png"], png(300, 300, BLUE));
    // The packaged document links to the files beside it.
    let doc = vectorcraft_format::load(&zip["Poster Kit/poster.vectorcraft"]).unwrap();
    let mut rel = vec![];
    doc.visit_images(|_, im| rel.push(im.link.clone().unwrap().relative.unwrap()));
    rel.sort();
    assert_eq!(rel, ["Links/blue.png", "Links/red.png"]);
    let report = String::from_utf8(zip["Poster Kit/poster Report.txt"].clone()).unwrap();
    assert!(report.contains("LINKED FILES") && report.contains("→ Links/red.png"), "{report}");
    // The same entries as a folder package.
    let out = dir.file("out");
    let f = run(&mut s, "file.package", json!({"folder": out, "name": "Poster Kit"}));
    assert_eq!(f["files"], r["files"]);
}

#[test]
fn options_leave_out_links_fonts_and_the_report() {
    let dir = Folder::new("package-options");
    let mut s = poster(&dir);
    let r = run(&mut s, "file.package", json!({"linksFolder": false, "copyFonts": false, "report": false}));
    assert_eq!(r["files"], json!(["poster.vectorcraft", "red.png", "blue.png"]));
    let r = run(&mut s, "file.package", json!({"copyLinks": false}));
    assert_eq!(r["files"], json!(["poster.vectorcraft", "Fonts/SourceSans3-Regular.ttf", "poster Report.txt"]));
    let zip = unzip(&vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap());
    let doc = vectorcraft_format::load(&zip["poster Folder/poster.vectorcraft"]).unwrap();
    doc.visit_images(|_, im| assert!(im.link.as_ref().unwrap().path.contains("art"), "not relinked: the originals"));
    let r = run(&mut s, "file.package", json!({"relink": false}));
    let zip = unzip(&vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap());
    assert!(zip.contains_key("poster Folder/Links/red.png"), "copied");
    let doc = vectorcraft_format::load(&zip["poster Folder/poster.vectorcraft"]).unwrap();
    doc.visit_images(|_, im| assert!(im.link.as_ref().unwrap().path.contains("art"), "not relinked"));
    assert!(s.execute("file.package", &json!({"name": "../up"})).is_err());
}

#[test]
fn a_missing_link_is_reported_not_copied() {
    let dir = Folder::new("package-missing");
    let mut s = poster(&dir);
    std::fs::remove_file(dir.file("art/blue.png")).unwrap();
    // Reopened, the missing file's image shows its preview: nothing to copy.
    open(&mut s, &dir.file("poster.vectorcraft"));
    let r = run(&mut s, "file.package", json!({}));
    assert_eq!((r["links"].as_u64(), r["missingLinks"].clone()), (Some(1), json!(["blue.png"])));
    let zip = unzip(&vectorcraft_format::base64_decode(r["dataBase64"].as_str().unwrap()).unwrap());
    let report = String::from_utf8(zip["poster Folder/poster Report.txt"].clone()).unwrap();
    assert!(report.contains("blue.png: not found"), "{report}");
}

#[test]
fn the_document_info_report_has_every_category() {
    let dir = Folder::new("docinfo-report");
    let mut s = poster(&dir);
    s.execute("file.place", &json!({"path": dir.file("art/red.png"), "link": false})).unwrap();
    let text = run(&mut s, "document.info", json!({"format": "text"}))["text"].as_str().unwrap().to_string();
    for h in [
        "DOCUMENT",
        "OBJECTS",
        "GRAPHIC STYLES",
        "SPOT COLORS",
        "PATTERN OBJECTS",
        "GRADIENT SWATCHES",
        "SYMBOLS",
        "FONTS",
        "FONT DETAILS",
        "LINKED IMAGES",
        "EMBEDDED IMAGES",
    ] {
        assert!(text.contains(&format!("\n{h}\n")), "{h}: {text}");
    }
    assert!(text.contains("Images: 3") && text.contains("Source Sans 3 Regular: built in; embedding allowed"), "{text}");
    assert!(text.contains("red.png: 600 × 300 px — ") && text.contains("Color Mode: RGB") && text.contains("\nSwatches: "), "{text}");
    let fonts = run(&mut s, "document.info", json!({"format": "text", "category": "fonts"}))["text"].as_str().unwrap().to_string();
    assert!(fonts.contains("FONTS") && !fonts.contains("OBJECTS"), "{fonts}");
    let i = run(&mut s, "document.info", json!({"category": "linkedImages"}));
    assert_eq!((i["sections"].as_array().unwrap().len(), i["sections"][0]["id"].as_str()), (1, Some("linkedImages")));
    assert_eq!(i["sections"][0]["rows"].as_array().unwrap().len(), 2);
    let all = run(&mut s, "document.info", json!({}));
    assert_eq!(all["sections"].as_array().unwrap().len(), 11);
    assert!(s.execute("document.info", &json!({"category": "brushes"})).is_err());
    assert!(s.execute("document.info", &json!({"format": "xml"})).is_err());
    // The bundled fonts allow embedding.
    let face = vectorcraft_text::FontDb::global().face("Source Sans 3", "Regular").unwrap();
    assert!(face.embeddable() && face.path().is_none());
}


/// A professional handoff: the poster links to a reusable illustration, whose
/// own artwork links to a raster asset. Both dependency levels must travel.
fn poster_with_placed_document(dir: &Folder) -> Session {
    let image = dir.file("art/photo.png");
    write(&image, &png(600, 300, RED));
    let mut part = session();
    place(&mut part, &image);
    let part_path = dir.file("art/part.vectorcraft");
    save(&mut part, &part_path);

    let mut poster = session();
    let placed = run(&mut poster, "file.place", json!({ "path": part_path, "link": true }));
    assert_eq!(placed["linked"], true, "{placed}");
    save(&mut poster, &dir.file("poster.vectorcraft"));
    poster
}

/// Every link must point to a packaged copy, including links inside a linked
/// VectorCraft file. Validate after original assets have been removed.
fn verify_portable_nested_package(folder: &std::path::Path) {
    let main = folder.join("poster.vectorcraft");
    let part = folder.join("Links/part.vectorcraft");
    let image = folder.join("Links/photo.png");
    assert!(main.is_file() && part.is_file() && image.is_file());

    let mut parent = session();
    let opened = open(&mut parent, &main.to_string_lossy());
    assert_eq!(opened["missingLinks"], json!([]), "main package has broken links: {opened}");
    let mut targets = Vec::new();
    parent.doc().unwrap().doc.visit_placed(|_, p| targets.push(p.link.clone()));
    assert_eq!(targets.len(), 1);
    let link = &targets[0];
    assert_eq!(link.relative.as_deref(), Some("Links/part.vectorcraft"));
    let packaged_part = std::fs::read(&part).unwrap();
    let expected_hash = vectorcraft_doc::links::hash_bytes(&packaged_part);
    assert_eq!(link.hash.as_deref(), Some(expected_hash.as_str()));
    let mut nested = session();
    let opened = open(&mut nested, &part.to_string_lossy());
    assert_eq!(opened["missingLinks"], json!([]), "nested document has broken links: {opened}");
    let mut img_links = Vec::new();
    nested.doc().unwrap().doc.visit_images(|_, im| img_links.extend(im.link.clone()));
    assert_eq!(img_links.len(), 1);
    assert_eq!(img_links[0].relative.as_deref(), Some("photo.png"));
    assert_eq!(std::fs::read(&image).unwrap(), png(600, 300, RED));
}

#[test]
fn package_recursively_relinks_placed_documents_and_their_images() {
    let dir = Folder::new("package-placed");
    let mut s = poster_with_placed_document(&dir);
    let out = dir.file("delivery");
    let result = run(&mut s, "file.package", json!({ "folder": out }));
    assert_eq!((result["links"].as_u64(), result["missingLinks"].clone()), (Some(2), json!([])), "{result}");
    let folder = dir.0.join("delivery/poster Folder");
    // The package must work on a second computer without the source tree.
    std::fs::remove_dir_all(dir.0.join("art")).unwrap();
    verify_portable_nested_package(&folder);
}

#[test]
fn zipped_package_recursively_relinks_without_absolute_creator_paths() {
    let dir = Folder::new("package-placed-zip");
    let mut s = poster_with_placed_document(&dir);
    let result = run(&mut s, "file.package", json!({}));
    assert_eq!(result["links"], 2, "{result}");
    let bytes = vectorcraft_format::base64_decode(result["dataBase64"].as_str().unwrap()).unwrap();
    let archive = unzip(&bytes);
    let extraction = dir.file("extracted");
    for (name, bytes) in archive {
        write(&format!("{extraction}/{name}"), &bytes);
    }
    std::fs::remove_dir_all(dir.0.join("art")).unwrap();
    verify_portable_nested_package(&dir.0.join("extracted/poster Folder"));
}

#[test]
fn package_keeps_distinct_linked_documents_with_colliding_file_names() {
    let dir = Folder::new("package-collisions");
    let mut poster = session();
    for (number, color) in [(1, RED), (2, BLUE)] {
        let image = dir.file(&format!("art{number}/picture.png"));
        write(&image, &png(300, 300, color));
        let mut part = session();
        place(&mut part, &image);
        let name = dir.file(&format!("art{number}/part.vectorcraft"));
        save(&mut part, &name);
        run(&mut poster, "file.place", json!({ "path": name, "link": true }));
    }
    save(&mut poster, &dir.file("poster.vectorcraft"));
    let result = run(&mut poster, "file.package", json!({ "folder": dir.file("delivery") }));
    assert_eq!(result["links"], 4, "{result}");
    assert_eq!(result["missingLinks"], json!([]));
    let folder = dir.0.join("delivery/poster Folder");
    let packaged: Vec<_> = result["files"].as_array().unwrap().iter().filter_map(Value::as_str).filter(|name| name.ends_with(".vectorcraft")).collect();
    assert_eq!(packaged.len(), 3, "both nested documents must be retained: {packaged:?}");
    std::fs::remove_dir_all(dir.0.join("art1")).unwrap();
    std::fs::remove_dir_all(dir.0.join("art2")).unwrap();
    let mut check = session();
    assert_eq!(open(&mut check, &folder.join("poster.vectorcraft").to_string_lossy())["missingLinks"], json!([]));
    for name in packaged.into_iter().filter(|n| n.starts_with("Links/")) {
        let mut inner = session();
        let result = open(&mut inner, &folder.join(name).to_string_lossy());
        assert_eq!(result["missingLinks"], json!([]), "{name}: {result}");
    }
}
