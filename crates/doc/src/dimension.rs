//! Persistent dimension recipes. Children are ordinary paths and type for all renderers/exporters.
use crate::{Document, NodeId, Unit};
use serde::{Deserialize, Serialize};
use vectorcraft_geom::{Affine, Point};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct DimensionOptions {
    pub units: String,
    pub precision: usize,
    /// Drawing : real size. 1:10 multiplies the drawn length by ten.
    pub scale: String,
    pub hide_units: bool,
    pub line_weight: f64,
    pub line_type: String,
    pub line_color: [u8; 3],
    pub arrow_style: String,
    pub arrow_scale: f64,
    pub extension_weight: f64,
    pub extension_type: String,
    pub extension_color: [u8; 3],
    pub extension_offset: f64,
    pub extension_length: f64,
    pub hide_extensions: bool,
    pub font_family: String,
    pub font_style: String,
    pub font_size: f64,
    pub text_color: [u8; 3],
    pub text_position: String,
    pub text_alignment: String,
    pub text_offset: f64,
}

impl Default for DimensionOptions {
    fn default() -> Self {
        Self {
            units: "cm".into(),
            precision: 1,
            scale: "1:1".into(),
            hide_units: false,
            line_weight: 1.0,
            line_type: "solid".into(),
            line_color: [224, 55, 65],
            arrow_style: "filled".into(),
            arrow_scale: 100.0,
            extension_weight: 0.5,
            extension_type: "solid".into(),
            extension_color: [224, 55, 65],
            extension_offset: 5.0,
            extension_length: 9.0,
            hide_extensions: false,
            font_family: "Arial".into(),
            font_style: "Regular".into(),
            font_size: 14.0,
            text_color: [224, 55, 65],
            text_position: "above".into(),
            text_alignment: "aligned".into(),
            text_offset: 9.0,
        }
    }
}

impl DimensionOptions {
    pub fn unit(&self) -> Option<Unit> {
        Some(match self.units.as_str() {
            "pt" => Unit::Points,
            "px" => Unit::Pixels,
            "mm" => Unit::Millimeters,
            "cm" => Unit::Centimeters,
            "m" => Unit::Meters,
            "in" => Unit::Inches,
            "ft" => Unit::Feet,
            "yd" => Unit::Yards,
            _ => return None,
        })
    }
    pub fn factor(&self) -> Result<f64, String> {
        let (a, b) = self.scale.split_once(':').ok_or("Scale must be drawing:real, for example 1:10")?;
        let parse = |s: &str| s.trim().parse::<f64>().ok().filter(|n| n.is_finite() && *n > 0.0 && *n <= 1e9);
        let a = parse(a).ok_or("Invalid drawing scale")?;
        let b = parse(b).ok_or("Invalid real scale")?;
        let f = b / a;
        if !(1e-6..=1e6).contains(&f) {
            return Err("Scale ratio must be between 1:1000000 and 1000000:1".into());
        }
        Ok(f)
    }
    pub fn validate(&self) -> Result<(), String> {
        self.factor()?;
        if self.unit().is_none() || self.precision > 6 {
            return Err("Choose a length unit and precision from 0 to 6".into());
        }
        for (v, min, max) in [
            (self.line_weight, 0.1, 100.0),
            (self.extension_weight, 0.1, 100.0),
            (self.arrow_scale, 1.0, 1000.0),
            (self.font_size, 1.0, 1000.0),
            (self.extension_offset, 0.0, 10000.0),
            (self.extension_length, 0.0, 10000.0),
            (self.text_offset, 0.0, 10000.0),
        ] {
            if !v.is_finite() || v < min || v > max {
                return Err("Dimension size is outside its supported range".into());
            }
        }
        if !["solid", "dashed", "dotted"].contains(&self.line_type.as_str())
            || !["solid", "dashed", "dotted"].contains(&self.extension_type.as_str())
            || !["filled", "open", "tick", "none"].contains(&self.arrow_style.as_str())
            || !["above", "center", "below"].contains(&self.text_position.as_str())
            || !["aligned", "horizontal"].contains(&self.text_alignment.as_str())
        {
            return Err("Unknown dimension style".into());
        }
        if self.font_family.is_empty() || self.font_family.len() > 256 || self.font_style.len() > 128 {
            return Err("Invalid dimension font name".into());
        }
        Ok(())
    }
    pub fn label(&self, points: f64, angle: bool) -> Result<String, String> {
        self.validate()?;
        let value = if angle { points } else { points * self.factor()? / self.unit().ok_or("Unknown unit")?.points() };
        if !value.is_finite() {
            return Err("Non-finite dimension".into());
        }
        let suffix = if self.hide_units {
            ""
        } else if angle {
            "°"
        } else {
            self.units.as_str()
        };
        let sep = if suffix.is_empty() || angle { "" } else { " " };
        Ok(format!("{:.*}{sep}{suffix}", self.precision, value))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SegmentRef {
    pub node: NodeId,
    pub subpath: usize,
    pub segment: usize,
}

impl SegmentRef {
    pub fn endpoints(&self, doc: &Document) -> Option<(Point, Point, f64)> {
        let sp = doc.node(self.node)?.path_data()?.subpaths.get(self.subpath)?;
        if self.segment >= sp.segment_count() {
            return None;
        }
        let seg = sp.segment(self.segment);
        use vectorcraft_geom::kurbo::ParamCurveArclen;
        Some((seg.p0, seg.p3, if sp.segment_is_line(self.segment) { seg.p0.distance(seg.p3) } else { seg.arclen(1e-5) }))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum DimensionSource {
    Segment { edge: SegmentRef },
    Angle { first: SegmentRef, second: SegmentRef },
    Circle { node: NodeId },
    Points { a: Option<PointRef>, b: Option<PointRef>, vertex: Option<PointRef> },
}

impl DimensionSource {
    pub fn nodes(&self) -> Vec<NodeId> {
        match self {
            Self::Segment { edge } => vec![edge.node],
            Self::Angle { first, second } => vec![first.node, second.node],
            Self::Circle { node } => vec![*node],
            Self::Points { a, b, vertex } => [a, b, vertex].into_iter().filter_map(|p| p.as_ref().map(|r| r.node)).collect(),
        }
    }
    /// Anchor indexes cannot safely follow a topology replacement. Ordinary moves and handle
    /// edits keep their topology and remain linked.
    pub fn topology_changed(&self, before: &Document, after: &Document) -> bool {
        if matches!(self, Self::Circle { .. }) {
            return false;
        }
        self.nodes().iter().any(|id| {
            let counts = |doc: &Document| {
                doc.node(*id).and_then(|n| n.path_data()).map(|p| p.subpaths.iter().map(|sp| (sp.anchors.len(), sp.closed)).collect::<Vec<_>>())
            };
            let old = counts(before);
            old.is_some() && old != counts(after)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PointRef {
    pub node: NodeId,
    pub subpath: usize,
    pub anchor: usize,
}

impl PointRef {
    pub fn position(&self, doc: &Document) -> Option<Point> {
        Some(doc.node(self.node)?.path_data()?.subpaths.get(self.subpath)?.anchors.get(self.anchor)?.p)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dimension {
    /// linear, angular, radius or diameter.
    pub mode: String,
    pub a: Point,
    pub b: Point,
    /// Angular vertex. For radius/diameter a is the centre and b lies on the circle.
    pub vertex: Point,
    pub offset: f64,
    pub length: f64,
    pub options: DimensionOptions,
    pub source: Option<DimensionSource>,
}

impl Dimension {
    pub fn transform(&mut self, xf: Affine) {
        // Linked recipes are resolved again from the source by the engine.
        if self.source.is_none() {
            self.a = xf * self.a;
            self.b = xf * self.b;
            self.vertex = xf * self.vertex;
            self.length = self.a.distance(self.b);
        }
    }
    pub fn resolve(&mut self, doc: &Document) -> Result<(), String> {
        match &self.source {
            Some(DimensionSource::Points { a, b, vertex }) => {
                for (reference, point) in [(a, &mut self.a), (b, &mut self.b), (vertex, &mut self.vertex)] {
                    if let Some(r) = reference {
                        *point = r.position(doc).ok_or("Measured anchor no longer exists")?;
                    }
                }
                self.length = self.a.distance(self.b);
            }
            Some(DimensionSource::Segment { edge }) => {
                let (a, b, length) = edge.endpoints(doc).ok_or("Measured segment no longer exists")?;
                self.a = a;
                self.b = b;
                self.length = length;
            }
            Some(DimensionSource::Angle { first, second }) => {
                for edge in [first, second] {
                    let sp = doc
                        .node(edge.node)
                        .and_then(|n| n.path_data())
                        .and_then(|p| p.subpaths.get(edge.subpath))
                        .ok_or("Measured path no longer exists")?;
                    if edge.segment >= sp.segment_count() || !sp.segment_is_line(edge.segment) {
                        return Err("Angles require two straight segments".into());
                    }
                }
                let (a, b, _) = first.endpoints(doc).ok_or("First measured segment no longer exists")?;
                let (c, d, _) = second.endpoints(doc).ok_or("Second measured segment no longer exists")?;
                let u = b - a;
                let v = d - c;
                let den = u.cross(v);
                if den.abs() < 1e-9 {
                    return Err("Choose two non-parallel straight segments".into());
                }
                let p = a + u * ((c - a).cross(v) / den);
                self.vertex = p;
                self.a = if a.distance(p) > b.distance(p) { a } else { b };
                self.b = if c.distance(p) > d.distance(p) { c } else { d };
            }
            Some(DimensionSource::Circle { node }) => {
                let (center, radius) = circle(doc, *node).ok_or("Choose a circular path")?;
                let direction = self.b - self.a;
                let dir = if direction.hypot() > 1e-9 { direction / direction.hypot() } else { vectorcraft_geom::Vec2::new(1.0, 0.0) };
                self.a = center;
                self.b = center + dir * radius;
                self.length = radius;
            }
            None => {}
        }
        self.validate()
    }
    pub fn validate(&self) -> Result<(), String> {
        self.options.validate()?;
        for p in [self.a, self.b, self.vertex] {
            if !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1e8 || p.y.abs() > 1e8 {
                return Err("Dimension point is outside the canvas".into());
            }
        }
        if !self.offset.is_finite() || self.offset.abs() > 1e6 || !self.length.is_finite() || self.length < 0.0 || self.length > 1e9 {
            return Err("Invalid dimension distance".into());
        }
        if !["linear", "angular", "radius", "diameter"].contains(&self.mode.as_str()) {
            return Err("Unknown dimension mode".into());
        }
        if self.a.distance(self.b) < 1e-6 {
            return Err("Choose two different points".into());
        }
        if self.mode == "angular" && (self.a.distance(self.vertex) < 1e-6 || self.b.distance(self.vertex) < 1e-6) {
            return Err("Choose two nonzero angle arms".into());
        }
        Ok(())
    }
    pub fn angle(&self) -> f64 {
        let u = self.a - self.vertex;
        let v = self.b - self.vertex;
        u.cross(v).atan2(u.dot(v))
    }
    pub fn label(&self) -> Result<String, String> {
        let (value, prefix) = match self.mode.as_str() {
            "angular" => (self.angle().abs().to_degrees(), ""),
            "radius" => (self.a.distance(self.b), "R "),
            "diameter" => (self.a.distance(self.b) * 2.0, "Ø "),
            _ => (self.length, if self.length > self.a.distance(self.b) + 0.01 { "Arc " } else { "" }),
        };
        Ok(format!("{prefix}{}", self.options.label(value, self.mode == "angular")?))
    }
}

/// Accept circular paths (including imported circles), reject ellipses and arbitrary closed shapes.
pub fn circle(doc: &Document, id: NodeId) -> Option<(Point, f64)> {
    use vectorcraft_geom::kurbo::ParamCurve;
    let path = doc.node(id)?.path_data()?;
    let sp = path.subpaths.first()?;
    if path.subpaths.len() != 1 || !sp.closed || !(2..=64).contains(&sp.segment_count()) {
        return None;
    }
    let bounds = path.bounds()?;
    let center = bounds.center();
    let radius = bounds.width() * 0.5;
    if radius < 1e-6 || (bounds.height() * 0.5 - radius).abs() > radius * 0.002 {
        return None;
    }
    for i in 0..sp.segment_count() {
        for t in [0.0, 0.25, 0.5, 0.75] {
            if (sp.segment(i).eval(t).distance(center) - radius).abs() > radius * 0.002 {
                return None;
            }
        }
    }
    Some((center, radius))
}
