//! Printer's marks drawn as art: the trim marks of Object → Create Trim Marks and Effect → Crop
//! Marks, stroked in the Registration colour so that they print on every plate.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vectorcraft_color::Paint;
use vectorcraft_geom::{Point, Rect, shapes};

use crate::{Appearance, Node, NodeId};

/// Trim mark style (the preference `japaneseCropMarks` picks Japanese).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkStyle {
    /// One line per edge at each corner, set off from the trim box.
    #[default]
    Roman,
    /// Double lines at each corner (the trim and the bleed edges) and centre marks on each side.
    Japanese,
}

impl MarkStyle {
    pub fn id(self) -> &'static str {
        match self {
            MarkStyle::Roman => "roman",
            MarkStyle::Japanese => "japanese",
        }
    }

    /// Parse an id, ignoring case.
    pub fn parse(s: &str) -> Option<Self> {
        [MarkStyle::Roman, MarkStyle::Japanese].into_iter().find(|m| m.id().eq_ignore_ascii_case(s))
    }
}

/// Trim marks around a rectangle (the trim box).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrimMarks {
    pub style: MarkStyle,
    /// Roman: the gap between the trim box and the marks.
    pub offset: f64,
    /// How long each mark is.
    pub length: f64,
    /// Japanese: the bleed, between the trim lines and the outer lines (3 mm).
    pub bleed: f64,
    /// Stroke weight.
    pub weight: f64,
}

impl Default for TrimMarks {
    fn default() -> Self {
        Self { style: MarkStyle::Roman, offset: 9.0, length: 18.0, bleed: 3.0 * 72.0 / 25.4, weight: 0.3 }
    }
}

impl TrimMarks {
    /// The default marks of `style`.
    pub fn of(style: MarkStyle) -> Self {
        Self { style, ..Default::default() }
    }

    /// How far the marks reach outside the trim box.
    pub fn reach(&self) -> f64 {
        let gap = match self.style {
            MarkStyle::Roman => self.offset,
            MarkStyle::Japanese => self.bleed,
        };
        gap + self.length + self.weight
    }

    /// The marks' lines around `r`.
    pub fn lines(&self, r: Rect) -> Vec<(Point, Point)> {
        let (l, mut out) = (self.length, vec![]);
        // Each corner with its outward directions.
        let corners = [(r.x0, r.y0, -1.0, -1.0), (r.x1, r.y0, 1.0, -1.0), (r.x1, r.y1, 1.0, 1.0), (r.x0, r.y1, -1.0, 1.0)];
        match self.style {
            MarkStyle::Roman => {
                let o = self.offset;
                for (x, y, sx, sy) in corners {
                    out.push((Point::new(x + sx * o, y), Point::new(x + sx * (o + l), y)));
                    out.push((Point::new(x, y + sy * o), Point::new(x, y + sy * (o + l))));
                }
            }
            MarkStyle::Japanese => {
                let b = self.bleed;
                for (x, y, sx, sy) in corners {
                    // The trim lines, outside the bleed, and the bleed lines meeting at its corner.
                    out.push((Point::new(x + sx * b, y), Point::new(x + sx * (b + l), y)));
                    out.push((Point::new(x, y + sy * b), Point::new(x, y + sy * (b + l))));
                    out.push((Point::new(x, y + sy * b), Point::new(x + sx * (b + l), y + sy * b)));
                    out.push((Point::new(x + sx * b, y), Point::new(x + sx * b, y + sy * (b + l))));
                }
                // A cross outside the bleed at the middle of each side.
                let c = r.center();
                for (p, dx, dy) in [
                    (Point::new(c.x, r.y0), 0.0, -1.0),
                    (Point::new(r.x1, c.y), 1.0, 0.0),
                    (Point::new(c.x, r.y1), 0.0, 1.0),
                    (Point::new(r.x0, c.y), -1.0, 0.0),
                ] {
                    let at = |d: f64| Point::new(p.x + dx * d, p.y + dy * d);
                    out.push((at(b), at(b + l)));
                    let m = at(b + l / 2.0);
                    out.push((Point::new(m.x - dy * l / 2.0, m.y - dx * l / 2.0), Point::new(m.x + dy * l / 2.0, m.y + dx * l / 2.0)));
                }
            }
        }
        out
    }

    /// The marks around `r` as a group named "Trim Marks" of lines stroked in Registration, with
    /// ids from `alloc` (the group's last).
    pub fn group(&self, r: Rect, alloc: &mut dyn FnMut() -> NodeId) -> Node {
        let ap = Appearance::basic(Paint::None, Paint::registration(), self.weight);
        let children = self.lines(r).into_iter().map(|(a, b)| Arc::new(Node::path(alloc(), shapes::line(a, b), ap.clone()))).collect();
        let mut g = Node::group(alloc(), children);
        g.name = Some("Trim Marks".into());
        g
    }
}

/// `r` grown by `[top, bottom, left, right]` (a bleed, or how far marks reach).
pub fn outset(r: Rect, [top, bottom, left, right]: [f64; 4]) -> Rect {
    Rect::new(r.x0 - left, r.y0 - top, r.x1 + right, r.y1 + bottom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roman_marks_sit_off_the_corners_and_japanese_ones_are_doubled() {
        let r = Rect::new(0.0, 0.0, 100.0, 50.0);
        let roman = TrimMarks::default().lines(r);
        assert_eq!(roman.len(), 8);
        assert!(roman.iter().all(|(a, b)| !r.contains(*a) && !r.contains(*b)));
        assert_eq!(roman[0], (Point::new(-9.0, 0.0), Point::new(-27.0, 0.0)));
        let jp = TrimMarks::of(MarkStyle::Japanese);
        let lines = jp.lines(r);
        assert_eq!(lines.len(), 24, "four lines per corner and a cross per side");
        // Two horizontal lines at the top-left corner: the trim line and the bleed line.
        let tl: Vec<f64> = lines[..4].iter().filter(|(a, b)| a.y == b.y).map(|(a, _)| a.y).collect();
        assert_eq!(tl.len(), 2);
        assert!((tl[0] - 0.0).abs() < 1e-9 && (tl[1] + jp.bleed).abs() < 1e-9, "{tl:?}");
        let mut n = 0;
        let g = jp.group(r, &mut || {
            n += 1;
            NodeId(n)
        });
        assert_eq!((g.children().unwrap().len(), g.id), (24, NodeId(25)));
        assert!(g.children().unwrap()[0].appearance.stroke().unwrap().paint.is_registration());
        assert_eq!(MarkStyle::parse("Japanese"), Some(MarkStyle::Japanese));
    }
}
