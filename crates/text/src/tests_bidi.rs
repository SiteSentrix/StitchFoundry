//! Direction, source-cluster and editing regressions; no RTL font assets required.
use super::*;
use kurbo::{Point, Rect, Shape};
use vectorcraft_doc::{CharStyle, TextKind};

fn text_layout(s: &str) -> TextLayout {
    layout(FontDb::global(), &TextObject::point(Point::ZERO, s, CharStyle::default()))
}

#[test]
fn hebrew_and_arabic_are_visually_rtl_with_exact_source_clusters() {
    for s in ["שלום", "مرحبا"] {
        let l = text_layout(s);
        assert!(!l.glyphs.is_empty());
        assert!(l.glyphs.iter().all(|g| g.rtl));
        let bytes: Vec<_> = l.glyphs.iter().map(|g| g.byte).collect();
        assert!(bytes.windows(2).all(|w| w[0] >= w[1]), "{s}: {bytes:?}");
        for g in &l.glyphs {
            assert!(s.get(g.byte..g.byte + g.len).is_some(), "{g:?}");
            assert!(g.len <= s.len());
        }
        assert!(caret_position(&l, 0).0.x > caret_position(&l, s.len()).0.x);
    }
}

#[test]
fn mixed_text_preserves_latin_and_number_order() {
    let s = "שלום Rust 123";
    let l = text_layout(s);
    let visible: String = l.glyphs.iter().filter_map(|g| s.get(g.byte..)?.chars().next()).collect();
    assert_eq!(visible, "Rust 123 םולש");
    assert_eq!(s, "שלום Rust 123", "layout must not rewrite source text");
}

#[test]
fn rtl_hit_testing_arrows_and_selection_follow_visual_positions() {
    let s = "שלום";
    let l = text_layout(s);
    for g in &l.glyphs {
        let p = Point::new(g.origin.x + g.advance * 0.9, g.origin.y);
        assert_eq!(hit_byte(&l, p), g.byte);
        let p = Point::new(g.origin.x + g.advance * 0.1, g.origin.y);
        assert_eq!(hit_byte(&l, p), g.byte + g.len);
    }
    assert_eq!(caret_horizontal(&l, 0, false), 2);
    assert_eq!(caret_horizontal(&l, 2, true), 0);
    assert!(!selection_quads(&l, 0, s.len()).is_empty());
}

#[test]
fn wrapping_retains_logical_line_ranges_and_reorders_each_line() {
    let s = "שלום עולם שלום עולם שלום עולם";
    let mut t = TextObject::point(Point::ZERO, s, CharStyle::default());
    t.kind = TextKind::Area { frame: vectorcraft_geom::PathData::from_bezpath(&Rect::new(0.0, 0.0, 55.0, 400.0).to_path(0.1)) };
    let l = layout(FontDb::global(), &t);
    assert!(l.lines.len() > 1);
    assert!(!l.overflow);
    for line in &l.lines {
        assert!(s.get(line.start..line.end).is_some());
        for g in &l.glyphs[line.glyph_start..line.glyph_end] {
            assert!(g.byte >= line.start && g.byte + g.len <= line.end);
        }
    }
}

#[test]
fn paragraph_direction_is_independent_and_neutrals_do_not_force_ltr() {
    let l = text_layout("123 שלום\nEnglish\nمرحبا");
    assert!(l.glyphs.iter().any(|g| g.line == 0 && g.rtl));
    assert!(l.glyphs.iter().filter(|g| g.line == 1).all(|g| !g.rtl));
    assert!(l.glyphs.iter().filter(|g| g.line == 2).all(|g| g.rtl));
}

#[test]
fn hebrew_marks_stay_with_base_clusters() {
    let s = "שָׁלוֹם";
    let l = text_layout(s);
    for g in &l.glyphs {
        assert!(s.get(g.byte..g.byte + g.len).is_some());
    }
    assert!(l.glyphs.iter().all(|g| g.rtl));
}

#[test]
fn arabic_contextual_forms_and_lam_alef_use_the_shaper() {
    let db = FontDb::global();
    // Installed fonts are optional; no proprietary or test font is copied into the repo.
    let face =
        ["Geeza Pro", "Arial", "Noto Sans Arabic"].into_iter().filter_map(|name| db.face(name, "Regular")).find(|f| f.covers('ل') && f.covers('ا'));
    let Some(face) = face else { return };
    let st = CharStyle { font_family: face.family.clone(), font_style: face.style.clone(), ..CharStyle::default() };
    for s in ["سلام", "שלום سلام"] {
        let l = layout(db, &TextObject::point(Point::ZERO, s, st.clone()));
        let arabic: Vec<_> = l.glyphs.iter().filter(|g| s.get(g.byte..).is_some_and(|t| t.starts_with(['س', 'ل', 'ا', 'م']))).collect();
        assert!(
            arabic.iter().any(|g| s.get(g.byte..).and_then(|t| t.chars().next()).is_some_and(|c| g.gid != face.glyph_for(c))),
            "Arabic must use contextual forms: {s}"
        );
        assert!(arabic.iter().all(|g| g.rtl));
    }
}

#[test]
fn bidi_isolates_numbers_and_brackets_keep_valid_clusters() {
    for s in ["שלום (123)", "مرحبا (Rust 42)", "English \u{2067}שלום 123\u{2069} end"] {
        let l = text_layout(s);
        for g in &l.glyphs {
            assert!(s.get(g.byte..g.byte + g.len).is_some());
        }
        assert!(!selection_quads(&l, 0, s.len()).is_empty());
    }
}

#[test]
fn right_aligned_wrapped_rtl_keeps_trailing_spaces_outside_content() {
    let mut t = TextObject::point(Point::ZERO, "שלום עולם שלום עולם שלום עולם", CharStyle::default());
    t.kind = TextKind::Area { frame: vectorcraft_geom::PathData::from_bezpath(&Rect::new(0.0, 0.0, 80.0, 400.0).to_path(0.1)) };
    t.para.justify = vectorcraft_doc::Justify::Right;
    let l = layout(FontDb::global(), &t);
    assert!(l.lines.len() > 1);
    for line in &l.lines {
        assert!((line.x1 - 80.0).abs() < 1e-6, "{line:?}");
    }
}

#[test]
fn visual_arrows_cross_rtl_paragraph_boundaries() {
    let l = text_layout("שלום\nעולם");
    assert!(l.lines.iter().all(|l| l.rtl));
    assert_eq!(caret_horizontal(&l, 8, false), 9);
    assert_eq!(caret_horizontal(&l, 9, true), 8);
}

#[test]
fn automatic_alignment_follows_each_paragraph_and_explicit_choices_win() {
    let db = FontDb::global();
    for s in ["שלום", "مرحبا", "123 שלום"] {
        let mut t = TextObject::point(Point::ZERO, s, CharStyle::default());
        assert_eq!(t.para.justify, vectorcraft_doc::Justify::Auto);
        let l = layout(db, &t);
        assert!((l.lines[0].x1).abs() < 1e-6, "RTL point type grows left from its anchor: {s}");
        t.kind = TextKind::Area { frame: vectorcraft_geom::PathData::from_bezpath(&Rect::new(0.0, 0.0, 200.0, 200.0).to_path(0.1)) };
        let l = layout(db, &t);
        assert!((l.lines[0].x1 - 200.0).abs() < 1e-6);
        t.para.justify = vectorcraft_doc::Justify::Left;
        assert_eq!(layout(db, &t).lines[0].x0, 0.0, "explicit left alignment is preserved");
        t.para.justify = vectorcraft_doc::Justify::Center;
        let l = layout(db, &t);
        assert!((l.lines[0].x0 + l.lines[0].x1 - 200.0).abs() < 1e-6);
    }
    let t = TextObject::point(Point::ZERO, "English\nשלום\nمرحبا\nEnglish again", CharStyle::default());
    let l = layout(db, &t);
    assert_eq!(l.lines[0].x0, 0.0);
    assert!(l.lines[1].x0 < 0.0 && l.lines[1].x1.abs() < 1e-6);
    assert!(l.lines[2].x0 < 0.0 && l.lines[2].x1.abs() < 1e-6);
    assert_eq!(l.lines[3].x0, 0.0);
}
