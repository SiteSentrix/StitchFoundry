use kurbo::{BezPath, Point, Shape};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Arrowhead, Dash, StrokeLayer, WidthProfile};

use super::*;

fn line(x0: f64, x1: f64) -> BezPath {
    let mut b = BezPath::new();
    b.move_to((x0, 0.0));
    b.line_to((x1, 0.0));
    b
}

fn stroke(width: f64, f: impl FnOnce(&mut StrokeLayer)) -> StrokeLayer {
    let mut st = StrokeLayer::new(Paint::solid(Color::BLACK), width);
    f(&mut st);
    st
}

fn dashed(pattern: &[f64], offset: f64) -> Dash {
    Dash { pattern: pattern.to_vec(), offset, align_corners: false }
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

#[test]
fn line_outline_uses_the_profile_dashes_or_a_plain_stroke() {
    let l = line(0.0, 100.0);
    let plain = line_outline(&l, &stroke(10.0, |_| {}), 10.0, 1e-3);
    assert!(near(plain.area().abs(), 1000.0, 1e-6));
    let lens = line_outline(&l, &stroke(10.0, |s| s.profile = Some(WidthProfile::lens())), 10.0, 1e-3);
    assert!(near(lens.area().abs(), 500.0, 1e-6));
    // Dashes take precedence over a profile (as before).
    let st = stroke(10.0, |s| {
        s.profile = Some(WidthProfile::lens());
        s.dash = Some(dashed(&[25.0, 25.0], 0.0));
    });
    assert!(near(line_outline(&l, &st, 10.0, 1e-3).area().abs(), 500.0, 1e-6));
    assert!(line_outline(&BezPath::new(), &st, 10.0, 1e-3).elements().is_empty());
}

#[test]
fn aligned_width_doubles_only_closed_inside_and_outside_strokes() {
    let mut st = stroke(3.0, |s| s.align = vectorcraft_doc::StrokeAlign::Inside);
    assert_eq!(aligned_width(&st, true), 6.0);
    assert_eq!(aligned_width(&st, false), 3.0);
    st.align = vectorcraft_doc::StrokeAlign::Center;
    assert_eq!(aligned_width(&st, true), 3.0);
}

#[test]
fn no_heads_borrows_the_path() {
    let l = line(0.0, 50.0);
    let p = stroke_pieces(&l, &stroke(2.0, |_| {}));
    assert!(matches!(p.line, Cow::Borrowed(_)) && p.heads.is_empty());
}

#[test]
fn heads_sit_on_the_end_points_and_point_out_of_the_path() {
    for kind in Arrowhead::ALL {
        let st = stroke(2.0, |s| {
            s.start_arrow = Some(kind);
            s.end_arrow = Some(kind);
        });
        let l = line(0.0, 100.0);
        let p = stroke_pieces(&l, &st);
        let [a, b] = &p.heads[..] else { panic!("two heads") };
        assert_eq!((a.tip, b.tip), (Point::new(0.0, 0.0), Point::new(100.0, 0.0)), "{kind:?}");
        assert!(near(a.dir.x, -1.0, 1e-9) && near(b.dir.x, 1.0, 1e-9), "{kind:?}");
        let (ba, bb) = (a.outline.bounding_box(), b.outline.bounding_box());
        assert!(ba.x0 >= -0.01 && bb.x1 <= 100.01, "{kind:?}: nothing past the tips");
        assert!(a.outline.area().abs() > 0.5, "{kind:?}");
    }
    // The start head of a curve points back along its first tangent.
    let mut c = BezPath::new();
    c.move_to((0.0, 0.0));
    c.curve_to((30.0, -40.0), (70.0, -40.0), (100.0, 0.0));
    let p = stroke_pieces(&c, &stroke(2.0, |s| s.start_arrow = Some(Arrowhead::Triangle)));
    assert!(p.heads[0].dir.x < 0.0 && p.heads[0].dir.y > 0.0, "{:?}", p.heads[0].dir);
}
