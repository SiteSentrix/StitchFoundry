//! Vertical type (bundled fonts only: `§` and `×` stand upright like CJK, letters lie on their side).
use super::*;
use kurbo::Shape;
use vectorcraft_doc::{CharStyle, TextKind};
use vectorcraft_geom::PathData;

fn vertical(text: &str) -> TextObject {
    let mut t = TextObject::point(Point::ZERO, text, CharStyle { size: 20.0, ..CharStyle::default() });
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
