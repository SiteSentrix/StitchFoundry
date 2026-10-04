//! Document nodes.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use vectorcraft_color::BlendMode;
use vectorcraft_geom::{Affine, BezPath, FillRule, PathData, Point, Rect, shapes};

use crate::appearance::Appearance;
use crate::live::{BlendSpec, EnvelopeKind, GradientMesh, Outliner};
use crate::pattern::RepeatSpec;
use crate::text::TextObject;

/// Stable per-document object id. Never reused.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeId(pub u64);

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Layer colours used for selection highlighting (index into [`LAYER_COLORS`]) or a custom RGB.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerColor {
    Preset(u8),
    Custom([u8; 3]),
}

/// Layer highlight colours, assigned in sequence to new layers. First is Illustrator-like light blue.
pub const LAYER_COLORS: [(&str, [u8; 3]); 27] = [
    ("Light Blue", [0x4f, 0x80, 0xff]),
    ("Red", [0xff, 0x4f, 0x4f]),
    ("Green", [0x4f, 0xff, 0x4f]),
    ("Blue", [0x4f, 0x4f, 0xff]),
    ("Yellow", [0xff, 0xff, 0x4f]),
    ("Magenta", [0xff, 0x4f, 0xff]),
    ("Cyan", [0x4f, 0xff, 0xff]),
    ("Gray", [0x80, 0x80, 0x80]),
    ("Black", [0x00, 0x00, 0x00]),
    ("Orange", [0xff, 0x66, 0x00]),
    ("Dark Green", [0x00, 0x80, 0x00]),
    ("Teal", [0x00, 0x80, 0x80]),
    ("Tan", [0xcc, 0x99, 0x66]),
    ("Brown", [0x99, 0x33, 0x00]),
    ("Violet", [0x99, 0x33, 0xff]),
    ("Gold", [0xff, 0x99, 0x00]),
    ("Dark Blue", [0x00, 0x00, 0x80]),
    ("Pink", [0xff, 0x99, 0xcc]),
    ("Lavender", [0x99, 0x99, 0xff]),
    ("Brick Red", [0x99, 0x00, 0x00]),
    ("Olive Green", [0x66, 0x66, 0x00]),
    ("Peach", [0xff, 0x99, 0x99]),
    ("Burgundy", [0x99, 0x00, 0x33]),
    ("Grass Green", [0x99, 0xcc, 0x00]),
    ("Ochre", [0x99, 0x66, 0x00]),
    ("Purple", [0x66, 0x00, 0x99]),
    ("Light Gray", [0xbb, 0xbb, 0xbb]),
];

impl LayerColor {
    pub fn rgb(self) -> [u8; 3] {
        match self {
            LayerColor::Preset(i) => LAYER_COLORS[i as usize % LAYER_COLORS.len()].1,
            LayerColor::Custom(c) => c,
        }
    }
}

/// Live shape parameters (Illustrator's "Live Shapes"). The node's path is regenerated from these
/// while the shape stays live; editing anchors directly converts it to a plain path.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "lowercase")]
pub enum LiveShape {
    Rectangle {
        /// Untransformed width/height.
        w: f64,
        h: f64,
        /// Corner radii: top-left, top-right, bottom-right, bottom-left.
        radii: [f64; 4],
        /// Maps the untransformed shape (origin at its top-left) into the document.
        xf: Affine,
    },
    Ellipse {
        w: f64,
        h: f64,
        /// Pie start/end angles in degrees (0/360 = full ellipse).
        pie: (f64, f64),
        xf: Affine,
    },
    Polygon {
        radius: f64,
        sides: u32,
        xf: Affine,
    },
    Line {
        a: Point,
        b: Point,
    },
}

impl LiveShape {
    pub fn label(&self) -> &'static str {
        match self {
            LiveShape::Rectangle { radii, .. } if radii.iter().any(|r| *r > 0.0) => "Rounded Rectangle",
            LiveShape::Rectangle { .. } => "Rectangle",
            LiveShape::Ellipse { .. } => "Ellipse",
            LiveShape::Polygon { .. } => "Polygon",
            LiveShape::Line { .. } => "Line",
        }
    }
    /// Regenerate the path.
    pub fn to_path(&self) -> PathData {
        match self {
            LiveShape::Rectangle { w, h, radii, xf } => {
                let r = Rect::new(0.0, 0.0, *w, *h);
                let p = if radii.iter().all(|x| (x - radii[0]).abs() < 1e-9) {
                    shapes::rounded_rectangle(r, radii[0])
                } else {
                    // Per-corner radii: generate uniformly then adjust (approximation until M2.5).
                    shapes::rounded_rectangle(r, radii.iter().cloned().fold(0.0, f64::max))
                };
                p.transformed(*xf)
            }
            LiveShape::Ellipse { w, h, xf, .. } => shapes::ellipse(Rect::new(0.0, 0.0, *w, *h)).transformed(*xf),
            LiveShape::Polygon { radius, sides, xf } => shapes::polygon(Point::ZERO, *radius, *sides, 0.0).transformed(*xf),
            LiveShape::Line { a, b } => shapes::line(*a, *b),
        }
    }
    pub fn transform(&mut self, a: Affine) {
        match self {
            LiveShape::Rectangle { xf, .. } | LiveShape::Ellipse { xf, .. } | LiveShape::Polygon { xf, .. } => *xf = a * *xf,
            LiveShape::Line { a: p, b } => {
                *p = a * *p;
                *b = a * *b;
            }
        }
    }
    /// Rotation angle of the live shape in degrees (shown in the Properties panel).
    pub fn angle_deg(&self) -> f64 {
        match self {
            LiveShape::Rectangle { xf, .. } | LiveShape::Ellipse { xf, .. } | LiveShape::Polygon { xf, .. } => {
                let c = xf.as_coeffs();
                c[1].atan2(c[0]).to_degrees()
            }
            LiveShape::Line { a, b } => (b.y - a.y).atan2(b.x - a.x).to_degrees(),
        }
    }
}

/// Embedded or linked raster image.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImageObject {
    /// Key into [`crate::Document::images`].
    pub key: String,
    pub width: u32,
    pub height: u32,
    /// Maps pixel space (0..w, 0..h) into the document.
    pub xf: Affine,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum NodeKind {
    /// A top-level layer or sublayer.
    Layer {
        color: LayerColor,
        #[serde(default)]
        template: bool,
        #[serde(default = "yes")]
        printable: bool,
        children: Vec<Arc<Node>>,
    },
    /// A group. With `clip`, the first (bottom-most) child is the clipping path.
    Group {
        children: Vec<Arc<Node>>,
        #[serde(default)]
        clip: bool,
    },
    Path {
        path: PathData,
        #[serde(default, skip_serializing_if = "crate::skip::is_default")]
        rule: FillRule,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        live: Option<LiveShape>,
        /// This path acts as a clipping path inside a clip group.
        #[serde(default, skip_serializing_if = "crate::skip::is_default")]
        clipping: bool,
        /// Guide (non-printing) path.
        #[serde(default, skip_serializing_if = "crate::skip::is_default")]
        guide: bool,
    },
    /// Compound path: children are paths painted as one with the compound's appearance.
    Compound {
        children: Vec<Arc<Node>>,
        #[serde(default)]
        rule: FillRule,
    },
    Text(Box<TextObject>),
    Image(ImageObject),
    SymbolInstance {
        symbol: String,
        xf: Affine,
    },
    /// Live blend: the key objects (paint order) plus spacing/orientation/spine; the intermediate
    /// steps are evaluated on demand (`live::blend_expand`).
    Blend {
        children: Vec<Arc<Node>>,
        #[serde(default)]
        spec: BlendSpec,
    },
    /// Live envelope distortion of `content`.
    Envelope {
        content: Vec<Arc<Node>>,
        kind: EnvelopeKind,
        /// Envelope Options → Fidelity (0–100).
        #[serde(default = "crate::live::default_fidelity")]
        fidelity: f64,
        /// Edit Contents mode (the content, not the envelope, is edited).
        #[serde(default)]
        editing: bool,
    },
    /// Gradient mesh.
    Mesh(GradientMesh),
    /// Live Repeat (radial / grid / mirror) of source art.
    Repeat(RepeatSpec),
}

fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

/// One object in the tree. Children are `Arc`s so documents share structure between undo states.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Node {
    pub id: NodeId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default = "yes", skip_serializing_if = "crate::skip::is_true")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub locked: bool,
    #[serde(default = "one", skip_serializing_if = "crate::skip::is_one")]
    pub opacity: f32,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub blend: BlendMode,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub isolate: bool,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub knockout: bool,
    #[serde(default, skip_serializing_if = "crate::skip::is_default")]
    pub appearance: Appearance,
    /// Opacity mask (Transparency panel). Its art lives here, outside the layer tree.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<Box<OpacityMask>>,
    /// Image Trace object: `{preset, params}` it was traced with (the Image Trace panel shows them).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace: Option<Box<serde_json::Value>>,
    /// Object → Text Wrap: area type below this object (in the same layer) flows around it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wrap: Option<crate::text::TextWrap>,
    /// Graph object: the group's children are generated from this spec (Object → Graph).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph: Option<Box<crate::graph::GraphSpec>>,
    pub kind: NodeKind,
}

/// Opacity mask: the luminance of the mask art sets the object's opacity (white = opaque).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpacityMask {
    /// Mask art in document coordinates.
    pub art: Arc<Node>,
    /// Clip: outside the mask art is hidden. Off: outside the mask art stays visible.
    #[serde(default = "yes")]
    pub clip: bool,
    /// Invert the mask's luminance.
    #[serde(default)]
    pub invert: bool,
    /// Disabled masks are kept but not applied.
    #[serde(default)]
    pub disabled: bool,
    /// Linked masks move with the object.
    #[serde(default = "yes")]
    pub linked: bool,
}

impl OpacityMask {
    pub fn new(art: Node, clip: bool) -> Self {
        Self { art: Arc::new(art), clip, invert: false, disabled: false, linked: true }
    }
}

impl Node {
    pub fn new(id: NodeId, kind: NodeKind) -> Self {
        Self {
            id,
            name: None,
            visible: true,
            locked: false,
            opacity: 1.0,
            blend: BlendMode::Normal,
            isolate: false,
            knockout: false,
            appearance: Appearance::default(),
            mask: None,
            trace: None,
            wrap: None,
            graph: None,
            kind,
        }
    }
    pub fn path(id: NodeId, path: PathData, appearance: Appearance) -> Self {
        let mut n = Self::new(id, NodeKind::Path { path, rule: FillRule::NonZero, live: None, clipping: false, guide: false });
        n.appearance = appearance;
        n
    }
    pub fn group(id: NodeId, children: Vec<Arc<Node>>) -> Self {
        Self::new(id, NodeKind::Group { children, clip: false })
    }
    pub fn layer(id: NodeId, name: &str, color: LayerColor) -> Self {
        let mut n = Self::new(id, NodeKind::Layer { color, template: false, printable: true, children: vec![] });
        n.name = Some(name.to_string());
        n
    }
    pub fn children(&self) -> Option<&Vec<Arc<Node>>> {
        match &self.kind {
            NodeKind::Layer { children, .. }
            | NodeKind::Group { children, .. }
            | NodeKind::Compound { children, .. }
            | NodeKind::Blend { children, .. }
            | NodeKind::Envelope { content: children, .. }
            | NodeKind::Repeat(RepeatSpec { source: children, .. }) => Some(children),
            _ => None,
        }
    }
    pub fn children_mut(&mut self) -> Option<&mut Vec<Arc<Node>>> {
        match &mut self.kind {
            NodeKind::Layer { children, .. }
            | NodeKind::Group { children, .. }
            | NodeKind::Compound { children, .. }
            | NodeKind::Blend { children, .. }
            | NodeKind::Envelope { content: children, .. }
            | NodeKind::Repeat(RepeatSpec { source: children, .. }) => Some(children),
            _ => None,
        }
    }
    pub fn is_layer(&self) -> bool {
        matches!(self.kind, NodeKind::Layer { .. })
    }
    pub fn is_container(&self) -> bool {
        self.children().is_some()
    }
    /// Kind name as the Layers panel / Properties panel shows it.
    pub fn kind_label(&self) -> &'static str {
        match &self.kind {
            NodeKind::Layer { .. } => "Layer",
            NodeKind::Group { clip: true, .. } => "Clip Group",
            NodeKind::Group { .. } => "Group",
            NodeKind::Path { live: Some(l), .. } => l.label(),
            NodeKind::Path { guide: true, .. } => "Guide",
            NodeKind::Path { .. } => "Path",
            NodeKind::Compound { .. } => "Compound Path",
            NodeKind::Text(_) => "Text",
            NodeKind::Image(_) => "Image",
            NodeKind::SymbolInstance { .. } => "Symbol",
            NodeKind::Blend { .. } => "Blend",
            NodeKind::Envelope { .. } => "Envelope",
            NodeKind::Mesh(_) => "Mesh",
            NodeKind::Repeat(r) => r.kind.label(),
        }
    }
    /// Name shown in the Layers panel: explicit name or `<Kind>`.
    pub fn display_name(&self) -> String {
        if let Some(n) = &self.name {
            return n.clone();
        }
        match &self.kind {
            NodeKind::Text(t) => {
                let s: String = t.plain_text().chars().take(32).collect();
                if s.is_empty() { "<Text>".into() } else { s }
            }
            _ => format!("<{}>", self.kind_label()),
        }
    }
    /// Path data for path nodes.
    pub fn path_data(&self) -> Option<&PathData> {
        match &self.kind {
            NodeKind::Path { path, .. } => Some(path),
            _ => None,
        }
    }
    pub fn path_data_mut(&mut self) -> Option<&mut PathData> {
        match &mut self.kind {
            NodeKind::Path { path, .. } => Some(path),
            _ => None,
        }
    }
    /// Geometric bounds (no stroke), recursively. Clip groups are bounded by their clip path.
    pub fn geometric_bounds(&self) -> Option<Rect> {
        match &self.kind {
            NodeKind::Path { path, .. } => path.bounds(),
            NodeKind::Group { children, clip: true } => children.first().and_then(|c| c.geometric_bounds()),
            NodeKind::Layer { children, .. } | NodeKind::Group { children, .. } | NodeKind::Compound { children, .. } => children
                .iter()
                .filter(|c| c.visible || !matches!(self.kind, NodeKind::Layer { .. }))
                .fold(None, |acc, c| vectorcraft_geom::union_opt(acc, c.geometric_bounds())),
            NodeKind::Text(t) => t.bounds(),
            NodeKind::Image(im) => Some(im.xf.transform_rect_bbox(Rect::new(0.0, 0.0, im.width as f64, im.height as f64))),
            NodeKind::SymbolInstance { xf, .. } => Some(xf.transform_rect_bbox(Rect::new(-10.0, -10.0, 10.0, 10.0))),
            NodeKind::Blend { children, spec } => {
                let b = crate::live::nodes_bounds(children);
                vectorcraft_geom::union_opt(b, spec.spine.as_ref().and_then(|s| s.bounds()))
            }
            NodeKind::Envelope { content, kind, .. } => crate::live::envelope_bounds(content, kind),
            NodeKind::Mesh(m) => m.bounds(),
            NodeKind::Repeat(r) => r.bounds(),
        }
    }
    /// Visual bounds (includes stroke outset).
    pub fn visual_bounds(&self) -> Option<Rect> {
        match &self.kind {
            NodeKind::Group { children, clip: true } => children.first().and_then(|c| c.geometric_bounds()),
            NodeKind::Layer { children, .. } | NodeKind::Group { children, .. } => {
                children.iter().fold(None, |acc, c| vectorcraft_geom::union_opt(acc, c.visual_bounds()))
            }
            NodeKind::Blend { children, spec } => {
                let b = children.iter().fold(None, |acc, c| vectorcraft_geom::union_opt(acc, c.visual_bounds()));
                let o = crate::live::max_outset(children);
                vectorcraft_geom::union_opt(b, spec.spine.as_ref().and_then(|s| s.bounds()).map(|r| r.inflate(o, o)))
            }
            NodeKind::Envelope { content, .. } | NodeKind::Repeat(RepeatSpec { source: content, .. }) => {
                let o = crate::live::max_outset(content);
                self.geometric_bounds().map(|b| b.inflate(o, o))
            }
            _ => {
                let o = self.appearance.outset();
                self.geometric_bounds().map(|b| b.inflate(o, o))
            }
        }
    }
    /// Apply an affine transform to the geometry (and gradients) of this node and its descendants.
    /// `scale_strokes` also scales stroke weights by the transform's mean scale.
    pub fn transform(&mut self, a: Affine, scale_strokes: bool) {
        if scale_strokes {
            let det = a.determinant().abs().sqrt();
            if (det - 1.0).abs() > 1e-9 {
                self.appearance.scale_strokes(det);
            }
        }
        transform_paints(&mut self.appearance, a);
        match &mut self.kind {
            NodeKind::Path { path, live, .. } => {
                path.transform(a);
                if let Some(l) = live {
                    l.transform(a);
                }
            }
            NodeKind::Layer { children, .. } | NodeKind::Group { children, .. } | NodeKind::Compound { children, .. } => {
                for c in children.iter_mut() {
                    Arc::make_mut(c).transform(a, scale_strokes);
                }
            }
            NodeKind::Text(t) => t.transform(a),
            NodeKind::Image(im) => im.xf = a * im.xf,
            NodeKind::SymbolInstance { xf, .. } => *xf = a * *xf,
            NodeKind::Blend { children, spec } => {
                for c in children.iter_mut() {
                    Arc::make_mut(c).transform(a, scale_strokes);
                }
                if let Some(s) = &mut spec.spine {
                    s.transform(a);
                }
            }
            NodeKind::Envelope { content, kind, .. } => {
                for c in content.iter_mut() {
                    Arc::make_mut(c).transform(a, scale_strokes);
                }
                match kind {
                    EnvelopeKind::Mesh { points, .. } => {
                        for p in points.iter_mut() {
                            *p = a * *p;
                        }
                    }
                    EnvelopeKind::TopObject { path } => path.transform(a),
                    EnvelopeKind::Warp { .. } => {}
                }
            }
            NodeKind::Mesh(m) => m.transform(a),
            NodeKind::Repeat(r) => r.transform(a, scale_strokes),
        }
        if let Some(m) = &mut self.mask
            && m.linked
        {
            Arc::make_mut(&mut m.art).transform(a, scale_strokes);
        }
    }
    /// Visit this node and all descendants depth first (paint order).
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Node)) {
        f(self);
        if let Some(ch) = self.children() {
            for c in ch {
                c.walk(f);
            }
        }
    }
    /// Count of nodes in this subtree.
    pub fn count(&self) -> usize {
        let mut n = 0;
        self.walk(&mut |_| n += 1);
        n
    }
}

/// Unites filled regions (each under its own fill rule) into one path filled non-zero. Booleans
/// live above this crate (`vectorcraft-pathops`), so callers supply it.
pub type Uniter<'a> = &'a dyn Fn(&[(BezPath, FillRule)]) -> BezPath;

impl Node {
    /// The filled regions this object clips to as the clipping path of a clip group, in document
    /// space, each with its fill rule: a path, a compound path (with its holes), an image's frame,
    /// text outlined by `text` (glyph outlines need the font engine, above this crate), the visible
    /// members of a group (their union) and the clipping path of a nested clip group. Live objects
    /// are evaluated first. Guides and symbol instances add nothing.
    pub fn clip_shapes(&self, text: Outliner) -> Vec<(BezPath, FillRule)> {
        let mut out = vec![];
        self.push_clip_shapes(text, &mut out);
        out
    }

    fn push_clip_shapes(&self, text: Outliner, out: &mut Vec<(BezPath, FillRule)>) {
        match &self.kind {
            NodeKind::Path { guide: true, .. } | NodeKind::SymbolInstance { .. } => {}
            NodeKind::Path { path, rule, .. } => out.push((path.to_bezpath(), *rule)),
            NodeKind::Compound { children, rule } => {
                let mut bp = BezPath::new();
                for p in children.iter().filter_map(|c| c.path_data()) {
                    bp.extend(p.to_bezpath());
                }
                out.push((bp, *rule));
            }
            NodeKind::Group { children, clip: true } => {
                if let Some(c) = children.first() {
                    c.push_clip_shapes(text, out);
                }
            }
            NodeKind::Group { children, .. } | NodeKind::Layer { children, .. } => {
                for c in children.iter().filter(|c| c.visible) {
                    c.push_clip_shapes(text, out);
                }
            }
            NodeKind::Text(_) => {
                if let Some(o) = text.and_then(|f| f(self)) {
                    o.push_clip_shapes(None, out);
                }
            }
            NodeKind::Image(im) => {
                let frame = shapes::rectangle(Rect::new(0.0, 0.0, im.width as f64, im.height as f64)).transformed(im.xf);
                out.push((frame.to_bezpath(), FillRule::NonZero));
            }
            NodeKind::Blend { .. } | NodeKind::Envelope { .. } | NodeKind::Mesh(_) | NodeKind::Repeat(_) => {
                crate::live::expand_deep(self, text).push_clip_shapes(text, out);
            }
        }
    }

    /// The region this object clips to as one path and fill rule ([`Self::clip_shapes`]), shared
    /// by the renderer and the SVG and PDF writers so every output clips alike. One shape keeps its
    /// own rule; several are united by `unite` (filled non-zero). `None` when there is nothing to
    /// clip by: the clipped art is then hidden.
    pub fn clip_outline(&self, text: Outliner, unite: Uniter) -> Option<(BezPath, FillRule)> {
        let mut shapes = self.clip_shapes(text);
        match shapes.len() {
            0 => None,
            1 => shapes.pop(),
            _ => Some((unite(&shapes), FillRule::NonZero)),
        }
    }
}

fn transform_paints(ap: &mut Appearance, a: Affine) {
    use crate::appearance::AppearanceItem;
    use vectorcraft_color::Paint;
    for it in &mut ap.items {
        let p = match it {
            AppearanceItem::Fill(f) => &mut f.paint,
            AppearanceItem::Stroke(s) => &mut s.paint,
        };
        if let Paint::Gradient(g) = p
            && let Some(geom) = &mut g.geom
        {
            geom.transform(a);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_live_shape_roundtrip() {
        let l = LiveShape::Rectangle { w: 10.0, h: 20.0, radii: [0.0; 4], xf: Affine::translate((5.0, 5.0)) };
        assert_eq!(l.to_path().bounds(), Some(Rect::new(5.0, 5.0, 15.0, 25.0)));
        assert_eq!(l.label(), "Rectangle");
    }

    #[test]
    fn transform_group_recurses() {
        let p = Node::path(NodeId(2), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), Appearance::default_art());
        let mut g = Node::group(NodeId(1), vec![Arc::new(p)]);
        g.transform(Affine::scale(2.0), true);
        assert_eq!(g.geometric_bounds(), Some(Rect::new(0.0, 0.0, 20.0, 20.0)));
        let child = &g.children().unwrap()[0];
        assert_eq!(child.appearance.stroke_width(), 2.0);
    }

    #[test]
    fn display_names() {
        let p = Node::path(NodeId(2), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), Appearance::default_art());
        assert_eq!(p.display_name(), "<Path>");
        let l = Node::layer(NodeId(1), "Layer 1", LayerColor::Preset(0));
        assert_eq!(l.display_name(), "Layer 1");
    }

    #[test]
    fn visual_bounds_include_stroke() {
        let mut p = Node::path(NodeId(2), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), Appearance::default_art());
        p.appearance.stroke_mut().unwrap().join = crate::appearance::LineJoin::Round;
        assert_eq!(p.visual_bounds(), Some(Rect::new(-0.5, -0.5, 10.5, 10.5)));
    }
}
