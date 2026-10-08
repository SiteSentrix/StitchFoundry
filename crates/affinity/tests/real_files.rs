//! Public Affinity documents, when present: `AFFINITY_SAMPLES=<dir>` points at a folder of
//! `.af`/`.afdesign`/`.afphoto`/`.afpub` files (searched recursively). Without it this test does
//! nothing; `cargo xtask corpus` fetches the pinned public set (see the crate README).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};

use vectorcraft_affinity::{Archive, Limits, stream};

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            files(&p, out);
        } else if p.extension().and_then(|e| e.to_str()).is_some_and(|e| ["af", "afdesign", "afphoto", "afpub", "aftemplate"].contains(&e)) {
            out.push(p);
        }
    }
}

#[test]
fn public_documents_parse_completely() {
    let Some(dir) = std::env::var_os("AFFINITY_SAMPLES") else { return };
    let mut paths = Vec::new();
    files(Path::new(&dir), &mut paths);
    assert!(!paths.is_empty(), "no Affinity documents under {dir:?}");
    let mut failures = Vec::new();
    for p in &paths {
        let bytes = std::fs::read(p).unwrap();
        let result = Archive::open(&bytes, Limits::default()).and_then(|mut a| {
            let doc = a.read("doc.dat")?;
            stream::parse(&doc).map(|s| s.objects.len())
        });
        match result {
            Ok(n) => assert!(n > 0),
            Err(e) => failures.push(format!("{}: {e}", p.display())),
        }
    }
    assert!(failures.is_empty(), "{} of {} failed:\n{}", failures.len(), paths.len(), failures.join("\n"));
    eprintln!("{} Affinity documents parsed", paths.len());
}
