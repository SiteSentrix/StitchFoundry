//! `document.open` PDF option `textAs`.

use serde_json::{Value, json};
use vectorcraft_pdf::TextAs;
use vectorcraft_testkit::pdf::{PdfPage, pdf};

use super::*;

fn open(s: &mut Session, bytes: &[u8], extra: Value) -> crate::Result<Value> {
    let mut p = json!({"name": "art.pdf", "dataBase64": vectorcraft_format::base64_encode(bytes)});
    if let (Some(o), Value::Object(e)) = (p.as_object_mut(), extra) {
        o.extend(e);
    }
    s.execute("document.open", &p)
}

fn kinds(s: &Session) -> Vec<&'static str> {
    let mut out = vec![];
    s.active().unwrap().doc.walk(|n| out.push(n.kind_label()));
    out
}

#[test]
fn text_opens_as_type_or_outlines() {
    let page = PdfPage {
        resources: "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>".into(),
        ..PdfPage::new(200.0, 100.0, "BT /F1 12 Tf 10 50 Td (Hello) Tj ET")
    };
    let bytes = pdf(&[page], None);
    let mut s = Session::new();
    open(&mut s, &bytes, json!({})).unwrap();
    assert!(kinds(&s).contains(&"Type"), "{:?}", kinds(&s));
    let r = open(&mut s, &bytes, json!({"textAs": "outlines"})).unwrap();
    assert!(!kinds(&s).contains(&"Type"));
    assert!(r["warnings"].to_string().contains("outlines"), "{r}");
    let e = open(&mut s, &bytes, json!({"textAs": "glyphs"})).unwrap_err().to_string();
    assert!(e.contains("textAs must be one of text, outlines"), "{e}");
}

#[test]
fn load_options_read_text() {
    let o = LoadOptions::from_params("x", &json!({"textAs": "Outlines"})).unwrap();
    assert_eq!(o.text_as, TextAs::Outlines);
    assert!(!o.is_partial());
    assert_eq!(LoadOptions::default().text_as, TextAs::Text);
}
