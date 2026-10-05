//! Export for Screens: files returned for download, one by one or as a ZIP.

use serde_json::{Value, json};

use super::*;

fn b64(v: &Value) -> Vec<u8> {
    vectorcraft_format::base64_decode(v["dataBase64"].as_str().expect("dataBase64")).unwrap()
}

/// A document of `artboards` 40×30 pt artboards side by side with a 20 pt square on the first.
fn session(artboards: usize) -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 40, "height": 30, "artboards": artboards})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 5, "y": 5, "width": 20, "height": 20})).unwrap();
    s
}

fn export(s: &mut Session, p: Value) -> Value {
    s.execute("document.exportForScreens", &p).unwrap()
}

/// `(name, bytes)` of each returned file.
fn files(r: &Value) -> Vec<(String, Vec<u8>)> {
    r["files"].as_array().unwrap().iter().map(|f| (f["name"].as_str().unwrap().to_string(), b64(f))).collect()
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_zip_is_written_into_the_folder() {
    let mut s = session(1);
    let dir = std::env::temp_dir().join(format!("vc-screens-zip-{}", std::process::id()));
    let folder = dir.to_string_lossy().replace('\\', "/");
    let r = export(&mut s, json!({"folder": folder, "zip": true, "formats": [{"format": "png"}, {"format": "svg"}]}));
    let zip = std::fs::read(r["path"].as_str().unwrap()).unwrap();
    assert_eq!(super::zip::entries(&zip).unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn zip_downloads_round_trip() {
    let mut s = session(2);
    let p = json!({"formats": [{"format": "png"}, {"format": "png", "scale": 2}, {"format": "pdf"}]});
    let plain = files(&export(&mut s, p.clone()));
    let mut q = p;
    q["zip"] = json!(true);
    let r = export(&mut s, q);
    let title = super::file_stem(&s.doc().unwrap().doc.title);
    assert_eq!(r["name"], format!("{title}.zip"));
    let zip = b64(&r);
    assert_eq!(r["bytes"].as_u64(), Some(zip.len() as u64));
    let entries = super::zip::entries(&zip).unwrap();
    let names = |files: &[(String, Vec<u8>)]| files.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>();
    assert_eq!(names(&entries), names(&plain), "the same files, sub-folders in their names");
    for ((name, zipped), (_, returned)) in entries.iter().zip(&plain) {
        if name.ends_with(".pdf") {
            // A PDF carries the time it was written.
            assert_eq!(vectorcraft_pdf::import(zipped).unwrap().artboards.len(), 1, "{name}");
        } else {
            assert_eq!(zipped, returned, "{name}");
        }
    }
    assert_eq!(r["files"].as_array().unwrap().len(), 6);
}

#[test]
fn zip_archives_store_their_files() {
    let files = [("a.txt", b"hello".to_vec()), ("sub/ünï.png", vec![0u8, 1, 2, 255]), ("empty", vec![])];
    let zip = super::zip::store(&files).unwrap();
    let back = super::zip::entries(&zip).unwrap();
    assert_eq!(back.len(), 3);
    for ((n, b), (m, c)) in files.iter().zip(&back) {
        assert_eq!((*n, b), (m.as_str(), c));
    }
    assert_eq!(super::zip::entries(&super::zip::store::<&str, Vec<u8>>(&[]).unwrap()).unwrap(), vec![]);
    let mut broken = zip.clone();
    let at = broken.windows(5).position(|w| w == b"hello").unwrap();
    broken[at] = b'j';
    assert!(super::zip::entries(&broken).is_err(), "the CRC catches a changed byte");
}
