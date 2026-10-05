//! The built-in PDF presets: generated in code, valid, described, and round-tripping as JSON.

use crate::*;

#[test]
fn built_in_presets() {
    let all = builtin_presets();
    assert_eq!(all[0].name, DEFAULT_PRESET);
    assert!(all[0].settings.preserve_editing, "the app default keeps the document editable");
    assert_eq!(all.len(), 7);
    for p in &all {
        assert_eq!(p.settings.check_values(), Ok(()), "{}", p.name);
        assert!(!p.description.is_empty());
        let back: PdfPreset = serde_json::from_value(serde_json::to_value(p).unwrap()).unwrap();
        assert_eq!(&back, p, "round-trips");
    }
    // PDF/X presets name standards the writer doesn't produce yet.
    let x4 = builtin_preset("pdf/x-4:2010").unwrap();
    assert!(matches!(x4.settings.check(), Err(PdfError::Unsupported(_))));
    assert_eq!(builtin_preset("default").unwrap().name, DEFAULT_PRESET);
    assert!(builtin_preset("Nope").is_none());
}
