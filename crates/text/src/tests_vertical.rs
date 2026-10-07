//! Vertical type (bundled fonts: `§` and `×` stand upright like CJK, letters lie on their side;
//! Japanese text only with craft-fonts' faces, skipped without them).
use super::*;
use kurbo::Shape;
use vectorcraft_doc::{CharStyle, Justify, TextKind};
use vectorcraft_geom::PathData;

fn vertical(text: &str) -> TextObject {
    vertical_styled(text, CharStyle { size: 20.0, ..CharStyle::default() })
}

fn vertical_styled(text: &str, style: CharStyle) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, text, style);
    t.xf = Affine::IDENTITY;
    t.vertical = true;
    t
}

fn ink(l: &TextLayout, i: usize) -> Rect {
    l.glyphs[i].outline.bounding_box()
}

#[test]
fn columns_run_down_and_follow_each_other_to_the_left() {
    let l = layout(FontDb::global(), &vertical("§§\n§"));
    assert!(l.vertical);
    let (a, b, c) = (ink(&l, 0), ink(&l, 1), ink(&l, 2));
    assert!(b.center().y > a.center().y + 15.0, "down the column: {a:?} {b:?}");
    assert!((a.center().x - b.center().x).abs() < 0.5, "one column");
    assert!(c.center().x < a.center().x - 15.0, "the next column is to the left: {c:?}");
    // Point type: the anchor is on the first column's centre line.
    assert!(a.center().x.abs() < 4.0, "{a:?}");
    assert!(l.bounds.contains(a.center()) && l.bounds.contains(c.center()));
}

#[test]
fn upright_marks_stand_and_letters_lie_on_their_side() {
    let h = layout(FontDb::global(), &TextObject::point(Point::ZERO, "§l", CharStyle { size: 20.0, ..CharStyle::default() }));
    let v = layout(FontDb::global(), &vertical("§l"));
    let (hs, hl, vs, vl) = (ink(&h, 0), ink(&h, 1), ink(&v, 0), ink(&v, 1));
    assert!((vs.width() - hs.width()).abs() < 0.01 && (vs.height() - hs.height()).abs() < 0.01, "§ upright: {hs:?} {vs:?}");
    assert!((vl.width() - hl.height()).abs() < 0.01 && (vl.height() - hl.width()).abs() < 0.01, "l turned: {hl:?} {vl:?}");
    assert!(vl.width() > vl.height(), "a tall l lies across the column");
}

#[test]
fn caret_and_selection_turn_with_the_columns() {
    let l = layout(FontDb::global(), &vertical("§§§"));
    let (top, bottom) = caret_position(&l, "§".len());
    assert!((top.y - bottom.y).abs() < 1e-6 && (top.x - bottom.x).abs() > 10.0, "a horizontal caret across the column: {top:?} {bottom:?}");
    let between = (ink(&l, 0).center().y + ink(&l, 1).center().y) / 2.0;
    assert!((top.y - between).abs() < 3.0, "between the first two marks: {top:?} {between} {:?} {:?}", ink(&l, 0), ink(&l, 1));
    let q = selection_quads(&l, 0, "§§".len());
    let r = Rect::from_points(q[0][0], q[0][2]);
    assert!(r.height() > r.width() && r.contains(ink(&l, 0).center()) && r.contains(ink(&l, 1).center()) && !r.contains(ink(&l, 2).center()));
    // Clicking a mark finds it.
    let c = ink(&l, 2).center();
    assert_eq!(hit_byte(&l, c + Vec2::new(0.0, 6.0)), "§§§".len());
    assert_eq!(hit_byte(&l, c - Vec2::new(0.0, 6.0)), "§§".len());
}

#[test]
fn area_type_starts_at_the_right_edge_and_wraps_to_the_left() {
    let mut t = vertical(&"§".repeat(12));
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, 100.0, 105.0).to_path(0.1)) };
    let l = layout(FontDb::global(), &t);
    let first = ink(&l, 0);
    assert!(first.x1 <= 100.0 + 1e-6 && first.x1 > 80.0 && first.y0 >= 0.0, "top right: {first:?}");
    let cols: Vec<f64> = l.glyphs.iter().map(|g| (g.outline.bounding_box().center().x * 10.0).round()).collect();
    let mut distinct = cols.clone();
    distinct.dedup();
    assert!(distinct.len() >= 2 && distinct.windows(2).all(|w| w[1] < w[0]), "columns right to left: {distinct:?}");
    assert!(l.glyphs.iter().all(|g| g.outline.bounding_box().y1 <= 105.0 + 1e-6), "inside the frame");
}

#[test]
fn type_on_a_path_stays_horizontal() {
    let mut t = vertical("ab");
    t.kind = TextKind::OnPath { path: PathData::from_bezpath(&kurbo::Line::new((0.0, 0.0), (100.0, 0.0)).to_path(0.1)), start: 0.0 };
    assert!(!layout(FontDb::global(), &t).vertical);
}

#[test]
fn three_digits_stand_across_the_column_squeezed_into_one_em() {
    let l = layout(FontDb::global(), &vertical("§100§"));
    let block: Vec<Rect> = (1..4).map(|i| ink(&l, i)).collect();
    assert!(block.windows(2).all(|w| w[1].center().x > w[0].center().x + 2.0 && (w[1].center().y - w[0].center().y).abs() < 1.0), "{block:?}");
    let span = block[0].union(block[2]);
    assert!(span.width() <= 20.0 + 1e-6, "squeezed into the 20 pt em: {span:?}");
    assert!(ink(&l, 4).center().y - ink(&l, 0).center().y < 45.0, "one em of the column");
}

#[test]
fn two_digits_stand_across_the_column_and_longer_numbers_lie_on_their_side() {
    let l = layout(FontDb::global(), &vertical("§12§2026"));
    let (one, two) = (ink(&l, 1), ink(&l, 2));
    assert!((one.center().y - two.center().y).abs() < 1.0 && two.center().x > one.center().x + 3.0, "12 side by side: {one:?} {two:?}");
    assert!(one.height() > one.width(), "upright digits are taller than wide: {one:?}");
    let (before, after) = (ink(&l, 0), ink(&l, 3));
    assert!(after.center().y - before.center().y < 45.0, "the block takes one em (20 pt) of the column");
    let year: Vec<Rect> = (4..8).map(|i| ink(&l, i)).collect();
    assert!(year.windows(2).all(|w| w[1].center().y > w[0].center().y + 5.0), "2026 runs down the column on its side");
}

/// Each line's text, from the layout's line ranges.
fn line_texts(t: &TextObject, l: &TextLayout) -> Vec<String> {
    let text = t.plain_text();
    l.lines.iter().map(|li| text.get(li.start..li.end).unwrap_or("").trim_end().to_string()).collect()
}

#[test]
fn kinsoku_keeps_closing_marks_off_line_starts_and_opening_brackets_off_line_ends() {
    for vertical_type in [false, true] {
        // Five characters fit a line: without kinsoku "。" would start the second line.
        let text = "あいうえお。かきくけ「こさしすせそ」";
        let mut t = TextObject::point(Point::ZERO, text, CharStyle { size: 20.0, ..CharStyle::default() });
        t.xf = Affine::IDENTITY;
        t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, 101.0, 400.0).to_path(0.1)) };
        if vertical_type {
            t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, 400.0, 101.0).to_path(0.1)) };
            t.vertical = true;
        }
        let l = layout(FontDb::global(), &t);
        let lines = line_texts(&t, &l);
        assert!(lines.len() > 2, "{lines:?}");
        for (i, line) in lines.iter().enumerate() {
            if i > 0 {
                let first = line.chars().next().unwrap_or(' ');
                assert!(!crate::shape::no_line_start(first), "line {i} starts with {first}: {lines:?}");
            }
            let last = line.chars().last().unwrap_or(' ');
            assert!(!crate::shape::no_line_end(last), "line {i} ends with {last}: {lines:?}");
        }
    }
}

/// Each glyph's cell down the (single) column: from its origin over its advance. A glyph that
/// doesn't advance (the rest of a tate-chu-yoko block) shares the cell of the glyph before it.
fn column_cells(l: &TextLayout) -> Vec<(f64, f64)> {
    let mut cells: Vec<(f64, f64)> = Vec::with_capacity(l.glyphs.len());
    for g in &l.glyphs {
        let cell = match cells.last() {
            Some(&prev) if g.advance == 0.0 => prev,
            _ => (g.origin.y, g.origin.y + g.advance),
        };
        cells.push(cell);
    }
    cells
}

/// One column, cells one after the other, each glyph's ink inside its own cell (within `tol`)
/// and no two glyphs' ink overlapping.
fn assert_own_cells(l: &TextLayout, text: &str, tol: f64) {
    let label = |i: usize| &text[l.glyphs[i].byte..l.glyphs[i].byte + l.glyphs[i].len];
    let cells = column_cells(l);
    for (i, g) in l.glyphs.iter().enumerate() {
        assert!((g.origin.x - l.glyphs[0].origin.x).abs() < 1e-9, "{:?} leaves the column: {g:?}", label(i));
        if let Some(next) = l.glyphs.get(i + 1) {
            assert!((next.origin.y - (g.origin.y + g.advance)).abs() < 1e-9, "{:?} starts where {:?} ends", label(i + 1), label(i));
        }
        if g.outline.elements().is_empty() {
            continue;
        }
        let (r, (top, bottom)) = (ink(l, i), cells[i]);
        assert!(r.y0 >= top - tol && r.y1 <= bottom + tol, "{:?}: ink {r:?} outside its cell {top}..{bottom}", label(i));
        for j in (0..i).filter(|&j| !l.glyphs[j].outline.elements().is_empty()) {
            let o = ink(l, j);
            let (ox, oy) = (r.x1.min(o.x1) - r.x0.max(o.x0), r.y1.min(o.y1) - r.y0.max(o.y0));
            assert!(ox <= tol || oy <= tol, "{:?} {r:?} overlaps {:?} {o:?}", label(i), label(j));
        }
    }
}

/// Index of the glyph of `needle`'s first byte in `text`.
fn glyph_of(l: &TextLayout, text: &str, needle: &str) -> usize {
    let b = text.find(needle).unwrap();
    l.glyphs.iter().position(|g| g.byte == b).unwrap()
}

/// Issue #260 with bundled glyphs (`§` stands in for the upright CJK characters).
#[test]
fn numbers_keep_their_own_cells_down_the_column() {
    let text = "§§ §§§ 10§22§§ 1§ 100§ 2026§";
    let l = layout(FontDb::global(), &vertical(text));
    assert_own_cells(&l, text, 0.25);
    // Tate-chu-yoko: one em of the column, centred on the column's centre line (x = 0), the next
    // character right after it.
    for block in ["10§", "22§", "100§"] {
        let first = glyph_of(&l, text, block);
        let digits = block.len() - '§'.len_utf8();
        let after = first + digits;
        let g = &l.glyphs[first];
        assert!((g.advance - 20.0).abs() < 1e-9, "{block}: {g:?}");
        assert!(l.glyphs[first + 1..after].iter().all(|d| d.advance == 0.0), "{block}");
        assert!((l.glyphs[after].origin.y - (g.origin.y + 20.0)).abs() < 1e-9, "{block}: the next character follows the block's em");
        let inks = (first..after).map(|i| ink(&l, i)).reduce(|a, b| a.union(b)).unwrap();
        assert!(inks.center().x.abs() < 1.0, "{block} centred on the column: {inks:?}");
        assert!((inks.center().y - (g.origin.y + 10.0)).abs() < 1.5, "{block} centred in its em: {inks:?} {g:?}");
    }
    // A single digit and longer numbers lie on their side, advancing by their own length.
    let one = glyph_of(&l, text, "1§");
    let r = ink(&l, one);
    assert!(r.width() > r.height() && l.glyphs[one].advance < 20.0, "1 on its side: {r:?}");
    let year = glyph_of(&l, text, "2026");
    let len: f64 = l.glyphs[year..year + 4].iter().map(|g| g.advance).sum();
    assert!(len > 30.0 && (l.glyphs[year + 4].origin.y - l.glyphs[year].origin.y - len).abs() < 1e-9, "2026 takes its length: {len}");
    // Half-width spaces take their own (narrow) width.
    for (i, g) in l.glyphs.iter().enumerate().filter(|(_, g)| text[g.byte..].starts_with(' ')) {
        assert!(g.advance > 2.0 && g.advance < 10.0, "space {i}: {g:?}");
    }
}

#[test]
fn justified_columns_keep_tate_chu_yoko_blocks_whole_and_marks_centred() {
    let text = "§10§§";
    let mut t = vertical(text);
    t.kind = TextKind::Area { frame: PathData::from_bezpath(&Rect::new(0.0, 0.0, 40.0, 200.0).to_path(0.1)) };
    t.para.justify = Justify::JustifyAll;
    let l = layout(FontDb::global(), &t);
    assert_eq!(l.lines.len(), 1);
    // 200 pt of column, four 20 pt cells: three gaps of 40 pt between them, none inside "10".
    let (one, zero) = (ink(&l, 1), ink(&l, 2));
    assert!((one.center().y - zero.center().y).abs() < 0.5 && zero.x0 > one.x1, "10 side by side: {one:?} {zero:?}");
    let marks: Vec<Rect> = [0, 3, 4].iter().map(|&i| ink(&l, i)).collect();
    assert!((marks[1].center().y - marks[0].center().y - 120.0).abs() < 1e-6, "{marks:?}");
    assert!((marks[2].center().y - marks[1].center().y - 60.0).abs() < 1e-6, "{marks:?}");
    assert!(marks.iter().all(|m| (m.center().x - marks[0].center().x).abs() < 1e-6), "upright marks stay on the centre line: {marks:?}");
    assert!(marks[2].y1 <= 200.0 + 1e-6, "inside the frame: {marks:?}");
}

#[test]
fn tracking_spaces_upright_marks_without_moving_them_off_the_centre_line() {
    let plain = layout(FontDb::global(), &vertical("§§"));
    let tracked = layout(FontDb::global(), &vertical_styled("§§", CharStyle { size: 20.0, tracking: 200.0, ..CharStyle::default() }));
    for i in 0..2 {
        assert!((ink(&tracked, i).center().x - ink(&plain, i).center().x).abs() < 1e-6, "{:?} {:?}", ink(&tracked, i), ink(&plain, i));
    }
    let step = ink(&tracked, 1).center().y - ink(&tracked, 0).center().y;
    assert!((step - 24.0).abs() < 1e-6, "one em plus 200/1000 em of tracking: {step}");
}

/// Issue #260 as reported, with a Japanese face of craft-fonts (whose own digits are proportional).
#[test]
fn japanese_numbers_keep_their_own_cells_down_the_column() {
    let Some(face) = crate::craft_fonts::japanese_ui_fonts(false).into_iter().next() else {
        eprintln!("skipped: built without craft-fonts (set CRAFT_FONTS_DIR to a craft-fonts checkout to run it)");
        return;
    };
    let db = FontDb::with_font_dirs(vec![]);
    let text = "はただ商店 秋のセール 10月22日まで";
    let t = vertical_styled(text, CharStyle { size: 28.0, font_family: face.family.into(), font_style: face.style.into(), ..CharStyle::default() });
    let l = layout(&db, &t);
    assert!(l.glyphs.iter().filter(|g| !text[g.byte..].starts_with(' ')).all(|g| g.gid != 0), "real glyphs, no tofu");
    assert_own_cells(&l, text, 0.5);
    for block in ["10月", "22日"] {
        let first = glyph_of(&l, text, block);
        let g = &l.glyphs[first];
        assert!((g.advance - 28.0).abs() < 1e-9 && l.glyphs[first + 1].advance == 0.0, "{block}: one em");
        assert!((l.glyphs[first + 2].origin.y - (g.origin.y + 28.0)).abs() < 1e-9, "{block}: the next character follows the em");
        let inks = ink(&l, first).union(ink(&l, first + 1));
        assert!(inks.center().x.abs() < 2.8, "{block} centred on the column: {inks:?}");
    }
}
