//! Embroidery hoops (sewing fields). Hoops are centred on the design origin.

use crate::geom::{BBox, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HoopShape {
    #[default]
    Rect,
    Oval,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hoop {
    pub name: String,
    /// Sewing field width (mm).
    pub width: f64,
    /// Sewing field height (mm).
    pub height: f64,
    #[serde(default)]
    pub shape: HoopShape,
}

impl Hoop {
    pub fn new(name: &str, width: f64, height: f64, shape: HoopShape) -> Self {
        Hoop { name: name.into(), width, height, shape }
    }

    /// Sewing field bounds (centred on the origin).
    pub fn bounds(&self) -> BBox {
        BBox::new(Point::new(-self.width / 2.0, -self.height / 2.0), Point::new(self.width / 2.0, self.height / 2.0))
    }

    /// Whether a point lies inside the sewing field.
    pub fn contains(&self, p: Point) -> bool {
        let (hw, hh) = (self.width / 2.0, self.height / 2.0);
        match self.shape {
            HoopShape::Rect => p.x.abs() <= hw && p.y.abs() <= hh,
            HoopShape::Oval => (p.x / hw).powi(2) + (p.y / hh).powi(2) <= 1.0,
        }
    }

    /// Whether a bounding box fits entirely in the sewing field.
    pub fn fits(&self, b: &BBox) -> bool {
        b.is_empty() || b.corners().iter().all(|&c| self.contains(c))
    }

    /// Outline polygon of the sewing field for display.
    pub fn outline(&self) -> Vec<Point> {
        let (hw, hh) = (self.width / 2.0, self.height / 2.0);
        match self.shape {
            HoopShape::Rect => {
                vec![Point::new(-hw, -hh), Point::new(hw, -hh), Point::new(hw, hh), Point::new(-hw, hh)]
            }
            HoopShape::Oval => (0..96)
                .map(|i| {
                    let a = i as f64 / 96.0 * std::f64::consts::TAU;
                    Point::new(hw * a.cos(), hh * a.sin())
                })
                .collect(),
        }
    }
}

/// Built-in hoop library (common domestic and commercial sewing fields).
pub fn standard_hoops() -> Vec<Hoop> {
    use HoopShape::*;
    vec![
        Hoop::new("Brother 100 × 100 (4×4)", 100.0, 100.0, Rect),
        Hoop::new("Brother 130 × 180 (5×7)", 130.0, 180.0, Rect),
        Hoop::new("Brother 160 × 260 (6×10)", 160.0, 260.0, Rect),
        Hoop::new("Brother 200 × 200 (8×8)", 200.0, 200.0, Rect),
        Hoop::new("Brother 200 × 300 (8×12)", 200.0, 300.0, Rect),
        Hoop::new("Brother 240 × 360 (9.5×14)", 240.0, 360.0, Rect),
        Hoop::new("Janome SQ14 140 × 140", 140.0, 140.0, Rect),
        Hoop::new("Janome RE20b 140 × 200", 140.0, 200.0, Rect),
        Hoop::new("Janome SQ20b 200 × 200", 200.0, 200.0, Rect),
        Hoop::new("Janome RE28b 200 × 280", 200.0, 280.0, Rect),
        Hoop::new("Bernina Medium 130 × 100", 130.0, 100.0, Oval),
        Hoop::new("Bernina Large Oval 145 × 255", 145.0, 255.0, Oval),
        Hoop::new("Husqvarna 120 × 120", 120.0, 120.0, Rect),
        Hoop::new("Husqvarna 240 × 150", 240.0, 150.0, Rect),
        Hoop::new("Husqvarna 260 × 200", 260.0, 200.0, Rect),
        Hoop::new("Husqvarna 360 × 200", 360.0, 200.0, Rect),
        Hoop::new("Husqvarna 360 × 350", 360.0, 350.0, Rect),
        Hoop::new("Tajima Round 12 cm", 120.0, 120.0, Oval),
        Hoop::new("Tajima Round 15 cm", 150.0, 150.0, Oval),
        Hoop::new("Tajima Round 18 cm", 180.0, 180.0, Oval),
        Hoop::new("Tajima Round 21 cm", 210.0, 210.0, Oval),
        Hoop::new("Tajima Round 24 cm", 240.0, 240.0, Oval),
        Hoop::new("Tajima Square 30 × 30 cm", 300.0, 300.0, Rect),
        Hoop::new("Cap frame 130 × 60", 130.0, 60.0, Rect),
        Hoop::new("Wide cap frame 160 × 60", 160.0, 60.0, Rect),
        Hoop::new("Jacket back 360 × 360", 360.0, 360.0, Rect),
        Hoop::new("Sleeve 80 × 300", 80.0, 300.0, Rect),
        // magnetic frames: common sewing fields, by size
        Hoop::new("Magnetic frame 100 × 100", 100.0, 100.0, Rect),
        Hoop::new("Magnetic frame 130 × 130", 130.0, 130.0, Rect),
        Hoop::new("Magnetic frame 130 × 180", 130.0, 180.0, Rect),
        Hoop::new("Magnetic frame 150 × 240", 150.0, 240.0, Rect),
        Hoop::new("Magnetic frame 200 × 200", 200.0, 200.0, Rect),
        Hoop::new("Magnetic frame 200 × 300", 200.0, 300.0, Rect),
        Hoop::new("Magnetic frame 250 × 250", 250.0, 250.0, Rect),
        Hoop::new("Magnetic frame 350 × 350", 350.0, 350.0, Rect),
        Hoop::new("Magnetic pocket frame 100 × 70", 100.0, 70.0, Rect),
    ]
}

/// Smallest standard hoop that fits the bounds (by area).
pub fn smallest_fitting(b: &BBox) -> Option<Hoop> {
    let centered = BBox::new(Point::new(-b.width() / 2.0, -b.height() / 2.0), Point::new(b.width() / 2.0, b.height() / 2.0));
    standard_hoops().into_iter().filter(|h| h.fits(&centered)).min_by(|a, b| (a.width * a.height).total_cmp(&(b.width * b.height)))
}
