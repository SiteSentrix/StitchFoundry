//! Knockout groups (Transparency panel).

use super::*;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Knockout, Node};
use vectorcraft_geom::shapes;

/// A 50% opaque rectangle: red at 10..60, blue at 40..90 (they overlap at 40..60).
fn half(d: &mut Document, x: f64, rgb: (f32, f32, f32)) -> Node {
    let id = d.alloc_id();
    let mut n = Node::path(
        id,
        shapes::rectangle(Rect::new(x, 10.0, x + 50.0, 90.0)),
        Appearance::basic(Paint::solid(Color::rgb(rgb.0, rgb.1, rgb.2)), Paint::None, 0.0),
    );
    n.opacity = 0.5;
    n
}

/// A document whose layer holds a group (`knockout`) of a 50% red and a 50% blue rectangle,
/// after `tweak` adjusts the group.
fn doc(knockout: Knockout, tweak: impl FnOnce(&mut Document, &mut Node)) -> Document {
    let mut d = Document::new(100.0, 100.0);
    let red = half(&mut d, 10.0, (1.0, 0.0, 0.0));
    let blue = half(&mut d, 40.0, (0.0, 0.0, 1.0));
    let id = d.alloc_id();
    let mut g = Node::group(id, vec![Arc::new(red), Arc::new(blue)]);
    g.knockout = knockout;
    tweak(&mut d, &mut g);
    d.insert(Some(d.layers[0].id), 0, g).unwrap();
    d
}

fn render(d: &Document) -> Rendered {
    Renderer::new().render(d, 100, 100, Affine::IDENTITY, &RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() })
}

fn close(a: [u8; 4], b: [u8; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (*x as i32 - y as i32).abs() <= 3)
}

const BLUE_OVER_WHITE: [u8; 4] = [128, 128, 255, 255];
/// 50% blue over 50% red over white.
const BLUE_OVER_RED: [u8; 4] = [128, 64, 191, 255];

#[test]
fn knockout_group_shows_the_top_object_over_the_backdrop() {
    let on = render(&doc(Knockout::On, |_, _| {}));
    assert!(close(on.pixel(50, 50), BLUE_OVER_WHITE), "overlap: {:?}", on.pixel(50, 50));
    assert!(close(on.pixel(20, 50), [255, 128, 128, 255]), "red alone: {:?}", on.pixel(20, 50));
    for k in [Knockout::Off, Knockout::Neutral] {
        let r = render(&doc(k, |_, _| {}));
        assert!(close(r.pixel(50, 50), BLUE_OVER_RED), "{k:?}: {:?}", r.pixel(50, 50));
    }
}

#[test]
fn neutral_groups_pass_knockout_through() {
    // Red and blue in a neutral group inside a knockout group knock each other out…
    let nested = |inner: Knockout| {
        doc(Knockout::On, |d, g| {
            let children = std::mem::take(g.children_mut().unwrap());
            let mut inner_group = Node::group(d.alloc_id(), children);
            inner_group.knockout = inner;
            *g.children_mut().unwrap() = vec![Arc::new(inner_group)];
        })
    };
    assert!(close(render(&nested(Knockout::Neutral)).pixel(50, 50), BLUE_OVER_WHITE));
    // …an Off group's do not…
    assert!(close(render(&nested(Knockout::Off)).pixel(50, 50), BLUE_OVER_RED));
    // …and with no knockout group around it, a neutral group knocks out nothing.
    let mut plain = nested(Knockout::Neutral);
    Arc::make_mut(&mut Arc::make_mut(&mut plain.layers[0]).children_mut().unwrap()[0]).knockout = Knockout::Neutral;
    assert!(close(render(&plain).pixel(50, 50), BLUE_OVER_RED));
}
