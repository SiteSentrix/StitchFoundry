use kurbo::{BezPath, PathEl, Point, Rect, Shape, Vec2};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Arrowhead, Dash, LineCap, StrokeLayer, WidthProfile};

use super::*;

fn line(x0: f64, x1: f64) -> BezPath {
    let mut b = BezPath::new();
    b.move_to((x0, 0.0));
    b.line_to((x1, 0.0));
    b
}

fn square(size: f64) -> BezPath {
    Rect::new(0.0, 0.0, size, size).to_path(0.1)
}

fn stroke(width: f64, f: impl FnOnce(&mut StrokeLayer)) -> StrokeLayer {
    let mut st = StrokeLayer::new(Paint::solid(Color::BLACK), width);
    f(&mut st);
    st
}

fn dashed(pattern: &[f64], offset: f64) -> Dash {
    Dash { pattern: pattern.to_vec(), offset, align_corners: false }
}

fn length(bp: &BezPath) -> f64 {
    bp.segments().map(|s| kurbo::ParamCurveArclen::arclen(&s, 1e-9)).sum()
}

fn moves(bp: &BezPath) -> usize {
    bp.elements().iter().filter(|e| matches!(e, PathEl::MoveTo(_))).count()
}

fn near(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

// ---------------------------------------------------------------- dashes and dots

#[test]
fn zero_length_dashes_are_dots_along_the_tangent() {
    let d = dash(&line(0.0, 60.0), &dashed(&[0.0, 6.0], 0.0)).unwrap();
    assert!(d.path.elements().is_empty(), "no dash has length");
    let xs: Vec<f64> = d.dots.iter().map(|p| p.at.x).collect();
    assert_eq!(d.dots.len(), 11, "{xs:?}");
    for (i, dot) in d.dots.iter().enumerate() {
        assert!(near(dot.at.x, 6.0 * i as f64, 1e-9) && near(dot.at.y, 0.0, 1e-12));
        assert!(near(dot.dir.x, 1.0, 1e-12));
    }
    // The offset shifts the dots along the path.
    let d = dash(&line(0.0, 60.0), &dashed(&[0.0, 6.0], 3.0)).unwrap();
    assert_eq!(d.dots.len(), 10);
    assert!(near(d.dots[0].at.x, 3.0, 1e-9));
    // Dots on a curve follow its direction.
    let c = kurbo::Circle::new((0.0, 0.0), 50.0).to_path(1e-6);
    let d = dash(&c, &dashed(&[0.0, 10.0], 0.0)).unwrap();
    for dot in &d.dots {
        assert!(dot.dir.dot(dot.at.to_vec2()).abs() < 1e-3, "tangent ⟂ radius at {:?}", dot.at);
    }
}

#[test]
fn dots_on_a_closed_path_are_not_doubled_at_the_start() {
    let d = dash(&square(60.0), &dashed(&[0.0, 6.0], 0.0)).unwrap();
    assert_eq!(d.dots.len(), 40);
}

#[test]
fn dashes_follow_arc_length_and_join_across_the_start_of_closed_paths() {
    let d = dash(&line(10.0, 90.0), &dashed(&[10.0, 10.0], 0.0)).unwrap();
    assert_eq!(moves(&d.path), 4);
    assert!(near(length(&d.path), 40.0, 1e-6));
    // An odd pattern repeats with dashes and gaps swapped: 10 on, 10 off, 10 on…
    let odd = dash(&line(0.0, 80.0), &dashed(&[10.0], 0.0)).unwrap();
    assert_eq!(moves(&odd.path), 4);
    // Perimeter 400, dashes of 20 starting 10 into the pattern: the dash at 390..410 runs
    // through the start point and is one dash.
    let sq = dash(&square(100.0), &dashed(&[20.0, 20.0], 10.0)).unwrap();
    assert_eq!(moves(&sq.path), 10);
    assert!(near(length(&sq.path), 200.0, 1e-6));
    // A pattern without length is a solid line; one dash round a closed path stays closed.
    assert!(dash(&line(0.0, 10.0), &dashed(&[0.0, 0.0], 0.0)).is_none());
    let whole = dash(&square(10.0), &dashed(&[100.0, 1.0], 0.0)).unwrap();
    assert!(is_closed(&whole.path));
    // A pattern far too fine for the path is drawn solid rather than as millions of dashes.
    assert!(dash(&line(0.0, 1e6), &dashed(&[0.001, 0.001], 0.0)).is_none());
}

#[test]
fn dot_outlines_take_the_cap_shape_and_wind_like_stroked_dashes() {
    let dots = [Dot { at: Point::new(10.0, 0.0), dir: Vec2::new(1.0, 0.0) }];
    let round = dot_outline(&dots, 4.0, LineCap::Round, 1e-4);
    assert!(near(round.area(), std::f64::consts::PI * 4.0, 1e-3));
    let sq = dot_outline(&dots, 4.0, LineCap::Square, 1e-4);
    assert!(near(sq.area(), 16.0, 1e-9));
    assert!(sq.contains(Point::new(11.9, 1.9)) && !round.contains(Point::new(11.9, 1.9)));
    assert!(dot_outline(&dots, 4.0, LineCap::Butt, 1e-4).elements().is_empty());
    // A dot overlapping a dash adds to it under the non-zero rule instead of cancelling it.
    let dash_outline = kurbo::stroke(line(0.0, 10.0).iter(), &kurbo::Stroke::new(4.0), &Default::default(), 1e-4);
    for o in [&round, &sq] {
        assert_eq!(o.area().signum(), dash_outline.area().signum());
    }
    let st = stroke(4.0, |s| {
        s.cap = LineCap::Round;
        s.dash = Some(dashed(&[10.0, 1.0, 0.0, 1.0], 0.0));
    });
    let o = line_outline(&line(0.0, 12.0), &st, 4.0, 1e-3);
    assert!(o.contains(Point::new(10.5, 0.0)), "the dot overlapping the dash end is painted");
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

// ---------------------------------------------------------------- arrowheads

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
