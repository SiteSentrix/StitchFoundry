//! The harness inputs: ground-truth vector art (drawn in a 256-unit square, rasterised at several
//! sizes) and two continuous-tone images computed per pixel.

use kurbo::{Affine, BezPath, Circle, Ellipse, Point, RoundedRect, Shape};
use vectorcraft_trace::Raster;

use crate::raster::{self, noise};

/// What an input tells about a trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Curved one-colour shapes: a circle, ellipses, a rounded badge with a hole, a letter.
    Shape,
    /// Straight-edged one-colour shapes: polygons, a star, a letter.
    Poly,
    /// Thin lines, 0.5 to 5 units wide.
    Lines,
    /// Straight-edged shapes and lines a degree or two off level and upright, as a scan or a
    /// screenshot taken a little askew has them.
    Askew,
    /// Flat art in several colours, with regions that touch.
    Flat,
    /// Continuous tone: a gradient and a photo-like image.
    Tone,
}

pub const CLASSES: [Class; 6] = [Class::Shape, Class::Poly, Class::Lines, Class::Askew, Class::Flat, Class::Tone];

impl Class {
    pub fn name(self) -> &'static str {
        match self {
            Class::Shape => "curved shapes",
            Class::Poly => "polygons",
            Class::Lines => "thin lines",
            Class::Askew => "askew shapes",
            Class::Flat => "flat colour art",
            Class::Tone => "gradient, photo",
        }
    }
}

/// How an input is degraded after it is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    Clean,
    /// One hard sample per pixel: no anti-aliasing (a GIF, a 1-bit scan).
    Hard,
    Blur,
    Noise,
    Scan,
}

pub const VARIANTS: [Variant; 5] = [Variant::Clean, Variant::Hard, Variant::Blur, Variant::Noise, Variant::Scan];

impl Variant {
    pub fn name(self) -> &'static str {
        match self {
            Variant::Clean => "clean",
            Variant::Hard => "aliased",
            Variant::Blur => "blurred",
            Variant::Noise => "noisy",
            Variant::Scan => "scanned",
        }
    }
    /// The colour a flat `c` of the clean image has in this variant (noise and shading aside).
    pub fn color(self, c: [u8; 3]) -> [u8; 3] {
        if self == Variant::Scan { raster::scan_color(c) } else { c }
    }
}

/// A colour of a continuous-tone input at `(u, v)` in the unit square; `x`, `y` are the pixel.
type Tone = fn(f64, f64, u32, u32) -> [f64; 3];

pub struct Input {
    pub name: &'static str,
    pub class: Class,
    /// The art, bottom first, in 256-unit coordinates. Shapes nest or abut, never partly overlap,
    /// so every outline is a visible edge.
    pub shapes: Vec<(BezPath, [u8; 3])>,
    pub bg: [u8; 3],
    /// Anchor points the art itself has.
    pub anchors: usize,
    /// Its sharp corners.
    pub corners: Vec<Point>,
    tone: Option<Tone>,
}

const WHITE: [u8; 3] = [255, 255, 255];
const INK: [u8; 3] = [0, 0, 0];

fn poly(pts: &[(f64, f64)]) -> BezPath {
    let mut p = BezPath::new();
    for (i, &pt) in pts.iter().enumerate() {
        if i == 0 { p.move_to(pt) } else { p.line_to(pt) }
    }
    p.close_path();
    p
}

fn regular(cx: f64, cy: f64, r: f64, n: usize, rot: f64) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let a = rot + std::f64::consts::TAU * i as f64 / n as f64;
            (cx + r * a.cos(), cy + r * a.sin())
        })
        .collect()
}

fn points(pts: &[(f64, f64)]) -> Vec<Point> {
    pts.iter().map(|&p| p.into()).collect()
}

/// `outer` with `holes` cut out (wound the other way: the rasteriser fills non-zero).
fn with_holes(outer: BezPath, holes: &[BezPath]) -> BezPath {
    let mut p = outer;
    for h in holes {
        p.extend(h.reverse_subpaths());
    }
    p
}

/// Half of a disc, left or right of the vertical line through its centre (3 anchors).
fn half_disc(c: Point, r: f64, left: bool) -> BezPath {
    let (s, k) = (if left { -r } else { r }, 0.552_284_75 * r);
    let ks = if left { -k } else { k };
    let mut p = BezPath::new();
    p.move_to((c.x, c.y - r));
    p.curve_to((c.x + ks, c.y - r), (c.x + s, c.y - k), (c.x + s, c.y));
    p.curve_to((c.x + s, c.y + k), (c.x + ks, c.y + r), (c.x, c.y + r));
    p.close_path();
    p
}

fn gradient(u: f64, v: f64, _: u32, _: u32) -> [f64; 3] {
    let glow = (-((u - 0.7).powi(2) + (v - 0.3).powi(2)) / 0.05).exp();
    [40.0 + 190.0 * u + 25.0 * glow, 70.0 + 120.0 * v + 60.0 * glow, 200.0 - 150.0 * u * v + 40.0 * glow]
}

/// Photo-like: sky to ground across a soft horizon, soft-edged objects, shading, texture and
/// sensor noise.
fn photo(u: f64, v: f64, x: u32, y: u32) -> [f64; 3] {
    let soft = |d: f64, width: f64| 1.0 / (1.0 + (-d / width).exp());
    let mix = |a: [f64; 3], b: [f64; 3], t: f64| [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t);
    let sky = mix([70.0, 120.0, 200.0], [190.0, 210.0, 235.0], v / 0.55);
    let ground = mix([95.0, 120.0, 60.0], [50.0, 70.0, 40.0], (v - 0.55) / 0.45);
    let mut c = mix(sky, ground, soft(v - 0.55 - 0.03 * (u * 9.0).sin(), 0.006));
    // Objects: (centre, radius, colour, edge softness).
    let blobs = [
        (0.28, 0.42, 0.17, [200.0, 80.0, 60.0], 0.004),
        (0.66, 0.50, 0.22, [230.0, 190.0, 120.0], 0.012),
        (0.80, 0.22, 0.08, [250.0, 240.0, 200.0], 0.02),
        (0.45, 0.78, 0.12, [60.0, 50.0, 90.0], 0.003),
    ];
    for (bx, by, r, col, edge) in blobs {
        let d = ((u - bx).powi(2) + (v - by).powi(2)).sqrt();
        // Shaded like a lit ball.
        let lit = 1.0 - 0.45 * ((u - bx + 0.4 * r) / r).clamp(-1.0, 1.0).abs();
        c = mix(c, col.map(|k| k * lit), soft(r - d, edge));
    }
    let texture = 6.0 * ((u * 61.0).sin() * (v * 47.0).cos()) + 4.0 * ((u + v) * 173.0).sin();
    [0, 1, 2].map(|i| c[i] + texture + 5.0 * noise(x, y, 31 + i as u32))
}

/// Every input, in report order.
pub fn inputs() -> Vec<Input> {
    let art = |name, class, shapes: Vec<(BezPath, [u8; 3])>, anchors, corners: Vec<Point>| Input {
        name,
        class,
        shapes,
        bg: WHITE,
        anchors,
        corners,
        tone: None,
    };
    let tone = |name, f: Tone| Input { name, class: Class::Tone, shapes: vec![], bg: WHITE, anchors: 0, corners: vec![], tone: Some(f) };
    let ink = |p: BezPath| vec![(p, INK)];

    let mut ellipses = Ellipse::new((90.3, 99.6), (60.0, 30.0), 0.0).to_path(1e-3);
    ellipses.extend(Ellipse::new((165.4, 190.2), (70.0, 25.0), 0.6).to_path(1e-3));
    let badge = with_holes(RoundedRect::new(40.5, 56.5, 215.5, 199.5, 28.0).to_path(1e-3), &[Circle::new((128.2, 127.7), 36.0).to_path(1e-3)]);
    // A "D": a straight stem joined smoothly to a bowl, with its counter.
    let mut d = BezPath::new();
    d.move_to((60.4, 60.3));
    d.line_to((110.0, 60.3));
    d.curve_to((170.0, 60.3), (200.0, 86.0), (200.0, 128.0));
    d.curve_to((200.0, 170.0), (170.0, 195.7), (110.0, 195.7));
    d.line_to((60.4, 195.7));
    d.close_path();
    let mut counter = BezPath::new();
    counter.move_to((84.4, 84.3));
    counter.line_to((108.0, 84.3));
    counter.curve_to((150.0, 84.3), (172.0, 100.0), (172.0, 128.0));
    counter.curve_to((172.0, 156.0), (150.0, 171.7), (108.0, 171.7));
    counter.line_to((84.4, 171.7));
    counter.close_path();
    let d_corners = points(&[(60.4, 60.3), (60.4, 195.7), (84.4, 84.3), (84.4, 171.7)]);

    let tri = [(40.3, 40.6), (120.7, 60.2), (60.4, 120.8)];
    let square = regular(180.3, 80.6, 46.0, 4, 0.35);
    let hexagon = regular(128.4, 185.3, 50.0, 6, 0.17);
    let mut polygons = poly(&tri);
    polygons.extend(poly(&square));
    polygons.extend(poly(&hexagon));
    let star: Vec<(f64, f64)> = (0..10)
        .map(|i| {
            let (a, r) = (-1.4 + std::f64::consts::PI * i as f64 / 5.0, if i % 2 == 0 { 100.0 } else { 40.0 });
            (128.3 + r * a.cos(), 128.6 + r * a.sin())
        })
        .collect();
    let a_outer = [(128.2, 40.4), (210.3, 216.5), (176.0, 216.5), (160.0, 176.4), (96.0, 176.4), (80.0, 216.5), (46.1, 216.5)];
    let a_counter = [(128.2, 96.0), (148.3, 150.4), (108.1, 150.4)];

    // Five lines 200 long, 0.5 to 5 wide, at small angles to the pixel grid.
    let mut lines = BezPath::new();
    for (i, (deg, width)) in [(0.0f64, 0.5), (6.0, 1.0), (-9.0, 2.0), (12.0, 3.0), (-4.0, 5.0)].into_iter().enumerate() {
        let (hw, hl) = (width / 2.0, 100.0);
        let mut strip = poly(&[(-hl, -hw), (hl, -hw), (hl, hw), (-hl, hw)]);
        strip.apply_affine(Affine::translate((128.3, 36.4 + 46.0 * i as f64)) * Affine::rotate(deg.to_radians()));
        lines.extend(strip);
    }

    // A rectangle 1.5° off level, a plus sign 1° off the other way, and two lines 1° and 2° off.
    let turned = |p: BezPath, deg: f64, about: (f64, f64)| Affine::rotate_about(deg.to_radians(), Point::new(about.0, about.1)) * p;
    let plus = [
        (150.0, 150.0),
        (170.0, 150.0),
        (170.0, 180.0),
        (200.0, 180.0),
        (200.0, 200.0),
        (170.0, 200.0),
        (170.0, 230.0),
        (150.0, 230.0),
        (150.0, 200.0),
        (120.0, 200.0),
        (120.0, 180.0),
        (150.0, 180.0),
    ];
    let mut askew = turned(poly(&[(40.3, 30.6), (200.7, 30.6), (200.7, 110.4), (40.3, 110.4)]), 1.5, (120.0, 70.0));
    askew.extend(turned(poly(&plus), -1.0, (160.0, 190.0)));
    askew.extend(turned(poly(&[(20.0, 150.3), (100.0, 150.3), (100.0, 153.3), (20.0, 153.3)]), 1.0, (60.0, 152.0)));
    askew.extend(turned(poly(&[(60.4, 170.0), (62.4, 170.0), (62.4, 240.0), (60.4, 240.0)]), 2.0, (61.0, 205.0)));

    let centre = Point::new(128.4, 128.3);
    let touching = vec![(half_disc(centre, 90.0, true), [200, 40, 50]), (half_disc(centre, 90.0, false), [30, 90, 200])];
    let bar = [(60.3, 180.4), (196.6, 180.4), (196.6, 206.7), (60.3, 206.7)];
    let multicolor = vec![
        (RoundedRect::new(30.4, 30.6, 226.3, 226.5, 30.0).to_path(1e-3), [20, 40, 90]),
        (Circle::new((128.3, 104.6), 50.0).to_path(1e-3), [250, 200, 40]),
        (Circle::new((128.3, 104.6), 20.0).to_path(1e-3), [240, 110, 30]),
        (poly(&bar), [40, 160, 90]),
    ];

    vec![
        art("circle", Class::Shape, ink(Circle::new((128.3, 127.6), 80.4).to_path(1e-3)), 4, vec![]),
        art("ellipses", Class::Shape, ink(ellipses), 8, vec![]),
        art("badge with a hole", Class::Shape, ink(badge), 12, vec![]),
        art("letter D", Class::Shape, ink(with_holes(d, &[counter])), 10, d_corners),
        art("polygons", Class::Poly, ink(polygons), 13, points(&[&tri[..], &square, &hexagon].concat())),
        art("star", Class::Poly, ink(poly(&star)), 10, points(&star)),
        art("letter A", Class::Poly, ink(with_holes(poly(&a_outer), &[poly(&a_counter)])), 10, points(&[&a_outer[..], &a_counter].concat())),
        art("lines", Class::Lines, ink(lines), 20, vec![]),
        art("askew shapes and lines", Class::Askew, ink(askew), 24, vec![]),
        art("two touching halves", Class::Flat, touching, 6, vec![(centre.x, centre.y - 90.0).into(), (centre.x, centre.y + 90.0).into()]),
        art("four-colour badge", Class::Flat, multicolor, 20, points(&bar)),
        tone("gradient", gradient),
        tone("photo-like", photo),
    ]
}

impl Input {
    /// The input drawn `size` pixels square as `variant`.
    pub fn draw(&self, size: u32, variant: Variant) -> Raster {
        // Art is drawn anti-aliased from 6 × 6 samples per pixel; the aliased variant takes one.
        let img = self.draw_sampled(size, if variant == Variant::Hard { 1 } else { 6 });
        match variant {
            Variant::Clean | Variant::Hard => img,
            Variant::Blur => raster::blur(&img),
            Variant::Noise => raster::noisy(&img),
            Variant::Scan => raster::scanned(&img),
        }
    }
    fn draw_sampled(&self, size: u32, samples: usize) -> Raster {
        let n = size as usize;
        match self.tone {
            Some(f) => Raster::from_fn(size, size, |x, y| {
                let c = f((f64::from(x) + 0.5) / f64::from(size), (f64::from(y) + 0.5) / f64::from(size), x, y);
                [c[0].round().clamp(0.0, 255.0) as u8, c[1].round().clamp(0.0, 255.0) as u8, c[2].round().clamp(0.0, 255.0) as u8, 255]
            }),
            None => raster::render_truth(&self.scaled(size), self.bg, n, n, samples),
        }
    }
    /// The art in the pixel coordinates of a `size`-pixel drawing.
    pub fn scaled(&self, size: u32) -> Vec<(BezPath, [u8; 3])> {
        let k = Affine::scale(f64::from(size) / 256.0);
        self.shapes.iter().map(|(p, c)| (k * p.clone(), *c)).collect()
    }
    pub fn scaled_corners(&self, size: u32) -> Vec<Point> {
        self.corners.iter().map(|p| (p.to_vec2() * (f64::from(size) / 256.0)).to_point()).collect()
    }
}
