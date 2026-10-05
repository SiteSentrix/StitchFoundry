//! usvg tree → document conversion.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;
use std::sync::Arc;

use usvg::roxmltree;
use vectorcraft_color::{BlendMode, Color, Gradient, GradientGeom, GradientKind, GradientPaint, GradientStop, Paint};
use vectorcraft_doc::{
    Appearance, AppearanceItem, Dash, Document, FillLayer, ImageBlob, ImageObject, LayerColor, LineCap, LineJoin, Node, NodeKind, PatternDef,
    StrokeLayer, Unit,
};
use vectorcraft_geom::{Affine, BezPath, FillRule, PathData, Point, Rect, Vec2, shapes};

use crate::{SvgError, fnv1a};
use css::XNode;
use text::TextSlots;

mod css;
mod text;

pub(crate) fn import(svg: &str) -> Result<(Document, Vec<String>), SvgError> {
    let (linked, links) = link_ids(svg);
    let (svg, hidden) = hidden_groups(&linked);
    let svg = svg.as_ref();
    let xml = roxmltree::Document::parse_with_options(svg, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() })
        .map_err(|e| SvgError::Parse(e.to_string()))?;
    let units = RootUnits::of(xml.root_element());
    let mut warnings = Vec::new();
    let (src, slots) = text::prepare(svg, &xml, units.dpi, &mut warnings);
    let opt = usvg::Options { dpi: units.dpi as f32, font_size: DEFAULT_FONT_SIZE as f32, ..usvg::Options::default() };
    let tree = usvg::Tree::from_str(&src, &opt).map_err(|e| SvgError::Parse(e.to_string()))?;
    let size = tree.size();
    let (kx, ky) = units.k;
    let mut doc = Document::new(size.width() as f64 * kx, size.height() as f64 * ky);
    doc.units = units.unit;
    let mut im = Importer {
        doc,
        warnings,
        mask_flags: mask_flags(&xml),
        links,
        slots,
        patterns: HashMap::new(),
        midpoints: midpoint_stops(&xml),
        labels: labels(&xml),
        hidden,
    };

    // usvg wraps everything in an id-less group carrying the viewBox transform when needed.
    let mut top = tree.root();
    let mut base = Affine::scale_non_uniform(kx, ky) * aff(top.transform());
    if let [usvg::Node::Group(g)] = top.children()
        && g.id().is_empty()
        && is_plain(g)
        && im.text_slot(g).is_none()
    {
        base *= aff(g.transform());
        top = g;
    }

    // Top-level `<g id>` elements become layers (as in the reference app); otherwise all art goes
    // into "Layer 1". Top-level text joins the layer below it (the first layer if none is).
    let is_layer = |im: &Importer, c: &usvg::Node| matches!(c, usvg::Node::Group(g) if !g.id().is_empty() && !im.links.contains_key(g.id()) && is_plain(g) && im.text_slot(g).is_none());
    let is_text = |im: &Importer, c: &usvg::Node| matches!(c, usvg::Node::Group(g) if im.text_slot(g).is_some());
    let layer_mode = top.children().iter().any(|c| is_layer(&im, c)) && top.children().iter().all(|c| is_layer(&im, c) || is_text(&im, c));
    if layer_mode {
        im.doc.layers.clear();
        let mut loose = vec![];
        for c in top.children() {
            let usvg::Node::Group(g) = c else { continue };
            if !is_layer(&im, c) {
                let n = im.node(c, base).map(Arc::new);
                match im.doc.layers.last_mut().and_then(|l| Arc::make_mut(l).children_mut()) {
                    Some(ch) => ch.extend(n),
                    None => loose.extend(n),
                }
                continue;
            }
            let mut children = std::mem::take(&mut loose);
            children.extend(im.children(g, base * aff(g.transform())));
            let id = im.doc.alloc_id();
            let preset = (im.doc.layers.len() % vectorcraft_doc::LAYER_COLORS.len()) as u8;
            let name = im.name_of(g.id()).to_string();
            let mut l = Node::layer(id, &name, LayerColor::Preset(preset));
            if let Some(ch) = l.children_mut() {
                *ch = children;
            }
            l.visible = !im.hidden.contains(g.id());
            im.doc.layers.push(Arc::new(l));
        }
    } else {
        let children = im.children(top, base);
        if let Some(l) = im.doc.layers.first_mut()
            && let Some(ch) = Arc::make_mut(l).children_mut()
        {
            *ch = children;
        }
    }
    Ok((im.doc, im.warnings))
}

/// SVG's initial `font-size` (`medium`) in user units.
pub(super) const DEFAULT_FONT_SIZE: f64 = 12.0;

/// How lengths become points, from the root `<svg>`'s `width` / `height`.
///
/// A pixel (and a unitless user unit) is a point, as we export, and absolute lengths keep their
/// physical size at 72 pt per inch: `font-size="12pt"` is 12 pt, `1in` is 72 pt. A root size in
/// absolute units (`width="210mm"`) is that physical size, and its user units are CSS pixels of it
/// (96 per inch, 0.75 pt each), so a drawing without a `viewBox` keeps its proportions.
struct RootUnits {
    /// Pixels per inch usvg (and the text pass) convert absolute lengths at.
    dpi: f64,
    /// Points per user unit of the root along x and y.
    k: (f64, f64),
    /// Document units: those of the root `width` (pixels when it has none).
    unit: Unit,
}

impl RootUnits {
    fn of(root: XNode) -> Self {
        let unit = |name: &str| root.attribute(name).and_then(|v| svgtypes::Length::from_str(v.trim()).ok()).map(|l| l.unit);
        use svgtypes::LengthUnit as U;
        let physical = |u: Option<U>| matches!(u, Some(U::In | U::Cm | U::Mm | U::Pt | U::Pc));
        let (w, h) = (unit("width"), unit("height"));
        let k = |u| if physical(u) { PT_PER_IN / CSS_PX_PER_IN } else { 1.0 };
        let dpi = if physical(w) || physical(h) { CSS_PX_PER_IN } else { PT_PER_IN };
        let unit = match w {
            Some(U::Mm) => Unit::Millimeters,
            Some(U::Cm) => Unit::Centimeters,
            Some(U::In) => Unit::Inches,
            Some(U::Pt) => Unit::Points,
            Some(U::Pc) => Unit::Picas,
            _ => Unit::Pixels,
        };
        Self { dpi, k: (k(w), k(h)), unit }
    }
}

/// Points per inch.
pub(super) const PT_PER_IN: f64 = 72.0;
/// CSS pixels per inch.
const CSS_PX_PER_IN: f64 = 96.0;

struct Importer {
    doc: Document,
    warnings: Vec<String>,
    /// Opacity-mask options our export wrote, by mask id (see [`mask_flags`]).
    mask_flags: HashMap<String, (bool, bool)>,
    /// The URL of each `<a href>`'s group, by its id ([`link_ids`]).
    links: HashMap<String, String>,
    /// Texts read from the XML, by placeholder ([`text`]).
    slots: TextSlots,
    /// Pattern swatch made for each usvg pattern.
    patterns: HashMap<usize, String>,
    /// Midpoint stops by gradient id (see [`midpoint_stops`]).
    midpoints: HashMap<String, Vec<Option<f32>>>,
    /// Object names by element id (see [`labels`]).
    labels: HashMap<String, String>,
    /// Ids of the hidden groups shown for usvg ([`hidden_groups`]).
    hidden: HashSet<String>,
}

/// The stops our export added for midpoints (`data-vc-midpoint`), by gradient id: for each stop,
/// the midpoint it stands for.
fn midpoint_stops(xml: &roxmltree::Document) -> HashMap<String, Vec<Option<f32>>> {
    xml.descendants()
        .filter(|n| matches!(n.tag_name().name(), "linearGradient" | "radialGradient"))
        .filter_map(|g| {
            let id = g.attribute("id")?;
            let marks: Vec<Option<f32>> = g
                .children()
                .filter(|c| c.tag_name().name() == "stop")
                .map(|c| c.attribute("data-vc-midpoint").and_then(|v| v.parse().ok()))
                .collect();
            marks.iter().any(Option::is_some).then(|| (id.to_string(), marks))
        })
        .collect()
}

/// A gradient's stops, with the midpoint stops `marks` names folded back into the midpoints of
/// the stops before them.
fn gradient_stops(g: &usvg::BaseGradient, marks: Option<&[Option<f32>]>) -> Vec<GradientStop> {
    let marks = marks.filter(|m| m.len() == g.stops().len());
    let mut out: Vec<GradientStop> = Vec::with_capacity(g.stops().len());
    for (i, s) in g.stops().iter().enumerate() {
        if let (Some(mid), Some(prev)) = (marks.and_then(|m| m.get(i).copied().flatten()), out.last_mut()) {
            prev.midpoint = mid.clamp(0.0, 1.0);
            continue;
        }
        let c = s.color();
        out.push(GradientStop { opacity: s.opacity().get(), ..GradientStop::new(s.offset().get(), Color::rgb8(c.red, c.green, c.blue)) });
    }
    out
}

/// The options [`crate::export::MASK_FLAGS`] records on exported `<mask>` elements, by mask id:
/// (clip, invert).
fn mask_flags(xml: &roxmltree::Document) -> HashMap<String, (bool, bool)> {
    let attr = crate::export::MASK_FLAGS;
    if !xml.input_text().contains(attr) {
        return HashMap::new();
    }
    let flags = |n: roxmltree::Node| {
        let has = |f: &str| n.attribute(attr).is_some_and(|v| v.split_whitespace().any(|x| x == f));
        Some((n.attribute("id")?.to_string(), (!has("noclip"), has("invert"))))
    };
    xml.descendants().filter(|n| n.tag_name().name() == "mask" && n.has_attribute(attr)).filter_map(flags).collect()
}

/// Prefix of the ids [`link_ids`] gives the `<a>` elements that have none.
const LINK_ID: &str = "vectorcraft-link-";

/// The URL an `<a>` element links to.
pub(super) fn href<'a>(a: XNode<'a, '_>) -> Option<&'a str> {
    a.attribute("href").or_else(|| a.attribute(("http://www.w3.org/1999/xlink", "href"))).filter(|h| !h.is_empty())
}

/// usvg reads `<a href>` as a plain group and drops the URL. So that the group can be found, every
/// link without an id gets one ([`LINK_ID`]…); returns the SVG with those ids and the URLs by id.
fn link_ids(svg: &str) -> (std::borrow::Cow<'_, str>, HashMap<String, String>) {
    let mut links = HashMap::new();
    if !svg.contains("<a") {
        return (svg.into(), links);
    }
    let Ok(xml) = roxmltree::Document::parse_with_options(svg, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }) else {
        return (svg.into(), links);
    };
    // Where to insert each new id: right after the tag name.
    let mut inserts = vec![];
    for a in xml.descendants().filter(|n| n.is_element() && n.tag_name().name() == "a") {
        let Some(url) = href(a) else { continue };
        let id = match a.attribute("id") {
            Some(id) => id.to_string(),
            None => {
                let id = format!("{LINK_ID}{}", inserts.len());
                inserts.push((tag_name_end(svg, a), format!(" id=\"{id}\"")));
                id
            }
        };
        links.insert(id, url.to_string());
    }
    (insert(svg, inserts), links)
}

/// Where the start tag of element `n` ends its name (where an attribute can go).
fn tag_name_end(svg: &str, n: XNode) -> usize {
    let start = n.range().start;
    svg.get(start + 1..).and_then(|s| s.find(|c: char| c.is_whitespace() || c == '/' || c == '>')).map_or(start + 2, |i| start + 1 + i)
}

/// `svg` with each `(offset, text)` of `inserts` (in offset order) inserted; offsets that are out
/// of order or not on a character boundary are skipped.
fn insert(svg: &str, inserts: Vec<(usize, String)>) -> Cow<'_, str> {
    if inserts.is_empty() {
        return svg.into();
    }
    let mut out = String::with_capacity(svg.len() + inserts.iter().map(|(_, t)| t.len()).sum::<usize>());
    let mut last = 0;
    for (at, text) in inserts {
        let Some(part) = svg.get(last..at) else { continue };
        out.push_str(part);
        out.push_str(&text);
        last = at;
    }
    out.push_str(svg.get(last..).unwrap_or(""));
    out.into()
}

/// usvg drops what isn't displayed, but hidden layers (as Save writes them: `display: none` on a
/// top-level `<g id>`, or on such a group inside groups) come back hidden. Those groups are shown
/// for usvg; returns the SVG so changed and their ids. A group something links to (a `<use>`
/// template) stays as it is.
fn hidden_groups(svg: &str) -> (Cow<'_, str>, HashSet<String>) {
    let mut ids = HashSet::new();
    if !svg.contains("display") {
        return (svg.into(), ids);
    }
    let Ok(xml) = roxmltree::Document::parse_with_options(svg, roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() }) else {
        return (svg.into(), ids);
    };
    let css = css::Styles::new(&xml);
    let linked: HashSet<&str> = xml.descendants().filter_map(href).filter_map(|h| h.strip_prefix('#')).collect();
    let group = |n: &XNode| n.is_element() && n.tag_name().name() == "g" && n.attribute("id").is_none_or(|id| !linked.contains(id));
    let mut inserts = vec![];
    let mut groups: Vec<XNode> = xml.root_element().children().filter(group).collect();
    while let Some(g) = groups.pop() {
        groups.extend(g.children().filter(group));
        let Some(id) = g.attribute("id").filter(|id| !id.is_empty()) else { continue };
        if css.own(g, "display").as_deref() != Some("none") {
            continue;
        }
        ids.insert(id.to_string());
        // A `style` declaration wins over the attribute and style sheet rules.
        inserts.push(match g.attributes().find(|a| a.name() == "style" && a.namespace().is_none()) {
            Some(a) => (a.range_value().end, ";display:inline".to_string()),
            None => (tag_name_end(svg, g), " style=\"display:inline\"".to_string()),
        });
    }
    inserts.sort_by_key(|(at, _)| *at);
    (insert(svg, inserts), ids)
}

/// Make `n` link to `url`, unless it links somewhere already (an inner link wins).
fn link(n: &mut Node, url: &str) {
    if n.url().is_none() {
        n.edit_attrs(|a| a.url = url.to_string());
    }
}

/// The names apps keep beside element ids, by id: `data-name`, `inkscape:label`, `serif:id` or
/// `aria-label` (the first one present).
fn labels(xml: &roxmltree::Document) -> HashMap<String, String> {
    const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
    const SERIF: &str = "http://www.serif.com/";
    xml.descendants()
        .filter_map(|n| {
            let id = n.attribute("id").filter(|id| !id.is_empty())?;
            let name = n
                .attribute("data-name")
                .or_else(|| n.attribute((INKSCAPE, "label")))
                .or_else(|| n.attribute((SERIF, "id")))
                .or_else(|| n.attribute("aria-label"))?;
            Some((id.to_string(), name.to_string()))
        })
        .collect()
}

fn aff(t: usvg::Transform) -> Affine {
    Affine::new([t.sx as f64, t.ky as f64, t.kx as f64, t.sy as f64, t.tx as f64, t.ty as f64])
}

fn is_plain(g: &usvg::Group) -> bool {
    g.clip_path().is_none() && g.mask().is_none() && g.filters().is_empty() && g.opacity().get() >= 1.0 && g.blend_mode() == usvg::BlendMode::Normal
}

fn blend(b: usvg::BlendMode) -> BlendMode {
    use usvg::BlendMode as B;
    match b {
        B::Normal => BlendMode::Normal,
        B::Multiply => BlendMode::Multiply,
        B::Screen => BlendMode::Screen,
        B::Overlay => BlendMode::Overlay,
        B::Darken => BlendMode::Darken,
        B::Lighten => BlendMode::Lighten,
        B::ColorDodge => BlendMode::ColorDodge,
        B::ColorBurn => BlendMode::ColorBurn,
        B::HardLight => BlendMode::HardLight,
        B::SoftLight => BlendMode::SoftLight,
        B::Difference => BlendMode::Difference,
        B::Exclusion => BlendMode::Exclusion,
        B::Hue => BlendMode::Hue,
        B::Saturation => BlendMode::Saturation,
        B::Color => BlendMode::Color,
        B::Luminosity => BlendMode::Luminosity,
    }
}

fn bezpath(p: &usvg::tiny_skia_path::Path, m: Affine) -> BezPath {
    use usvg::tiny_skia_path::PathSegment as S;
    let pt = |p: usvg::tiny_skia_path::Point| m * Point::new(p.x as f64, p.y as f64);
    let mut bp = BezPath::new();
    for s in p.segments() {
        match s {
            S::MoveTo(p) => bp.move_to(pt(p)),
            S::LineTo(p) => bp.line_to(pt(p)),
            S::QuadTo(a, p) => bp.quad_to(pt(a), pt(p)),
            S::CubicTo(a, b, p) => bp.curve_to(pt(a), pt(b), pt(p)),
            S::Close => bp.close_path(),
        }
    }
    bp
}

fn rule(r: usvg::FillRule) -> FillRule {
    match r {
        usvg::FillRule::NonZero => FillRule::NonZero,
        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
    }
}

impl Importer {
    fn warn(&mut self, s: String) {
        if !self.warnings.contains(&s) {
            self.warnings.push(s);
        }
    }

    fn children(&mut self, g: &usvg::Group, acc: Affine) -> Vec<Arc<Node>> {
        g.children().iter().filter_map(|c| self.node(c, acc)).map(Arc::new).collect()
    }

    fn node(&mut self, n: &usvg::Node, acc: Affine) -> Option<Node> {
        match n {
            usvg::Node::Group(g) => self.group(g, acc),
            usvg::Node::Path(p) => self.path(p, acc),
            usvg::Node::Image(i) => self.image(i, acc),
            // Only present when usvg had fonts; text is read from the XML instead.
            usvg::Node::Text(_) => None,
        }
    }

    /// A gradient's stops, midpoints restored.
    fn stops(&self, g: &usvg::BaseGradient) -> Vec<GradientStop> {
        gradient_stops(g, self.midpoints.get(g.id()).map(Vec::as_slice))
    }

    /// The object name for an element id: its label (see [`labels`]), else the id itself.
    fn name_of<'a>(&'a self, id: &'a str) -> &'a str {
        self.labels.get(id).map_or(id, String::as_str)
    }

    fn named(&mut self, id: &str, kind: NodeKind) -> Node {
        let nid = self.doc.alloc_id();
        let mut n = Node::new(nid, kind);
        if !id.is_empty() {
            n.name = Some(self.name_of(id).to_string());
        }
        n
    }

    fn group(&mut self, g: &usvg::Group, acc: Affine) -> Option<Node> {
        let url = self.links.get(g.id()).cloned();
        let mut n = self.group_node(g, acc)?;
        if let Some(url) = url {
            link(&mut n, &url);
        }
        if self.hidden.contains(g.id()) {
            n.visible = false;
        }
        Some(n)
    }

    /// The text a placeholder group stands for (see [`text`]) and the index of its main path.
    fn text_slot(&self, g: &usvg::Group) -> Option<(usize, usize)> {
        g.children().iter().enumerate().find_map(|(k, c)| match c {
            usvg::Node::Path(p) => self.slots.find(p).map(|i| (i, k)),
            _ => None,
        })
    }

    /// A text placeholder group → its text object (nothing when the text is hidden).
    fn text(&mut self, g: &usvg::Group, ts: Affine) -> Option<Vec<Arc<Node>>> {
        let (i, main) = self.text_slot(g)?;
        let kids = g.children();
        if !matches!(kids.get(main), Some(usvg::Node::Path(p)) if p.is_visible()) {
            return Some(vec![]);
        }
        let text::PendingText { name, mut obj, servers, paints, .. } = self.slots.texts.get(i)?.clone();
        // usvg resolved the `url(#…)` paints (the paths after the main one) in the element's user
        // space; runs paint in text space.
        let to_text = obj.xf.inverse();
        let resolved: Vec<Paint> = (0..paints)
            .map(|k| match kids.get(main + 1 + k) {
                Some(usvg::Node::Path(p)) => p.fill().map_or(Paint::None, |f| self.paint(f.paint(), to_text)),
                _ => Paint::None,
            })
            .collect();
        let server = |k: usize| resolved.get(k).cloned().unwrap_or(Paint::None);
        for (run, (fill, stroke)) in obj.runs.iter_mut().zip(servers) {
            if let Some(k) = fill {
                run.style.fill = server(k);
            }
            if let Some(k) = stroke {
                run.style.stroke = server(k);
            }
        }
        obj.xf = ts * obj.xf;
        Some(vec![Arc::new(self.named(&name, NodeKind::Text(Box::new(obj))))])
    }

    fn group_node(&mut self, g: &usvg::Group, acc: Affine) -> Option<Node> {
        let ts = acc * aff(g.transform());
        // A text placeholder becomes its text, named after the element (the group stays unnamed).
        let text = self.text(g, ts);
        if text.as_ref().is_some_and(Vec::is_empty) {
            return None;
        }
        let is_text = text.is_some();
        // The made-up ids of links aren't names.
        let id = if is_text || g.id().starts_with(LINK_ID) { "" } else { g.id() };
        let label = match (&text, id) {
            (Some(t), _) => t.first().and_then(|n| n.name.as_deref()).map_or_else(|| "a text".to_string(), |n| format!("text '{n}'")),
            (None, "") => "a group".to_string(),
            (None, id) => format!("'{id}'"),
        };
        let mask = g.mask().and_then(|m| self.opacity_mask(m, ts, &label));
        if !g.filters().is_empty() {
            self.warn(format!("filter on {label} ignored"));
        }
        let mut children = match text {
            Some(t) => t,
            None => self.children(g, ts),
        };
        let mut n = if let Some(cp) = g.clip_path() {
            if cp.clip_path().is_some() {
                self.warn(format!("nested clip path on {label} approximated by its outer clip"));
            }
            let clip = self.clip_node(cp, ts)?;
            let mut ch = vec![Arc::new(clip)];
            ch.extend(children);
            self.named(id, NodeKind::Group { children: ch, clip: true })
        } else {
            if children.is_empty() {
                return None;
            }
            // An id-less wrapper around a single object (usvg adds these for opacity/transform on
            // shapes): fold its opacity and blend into the object. A text takes its own mask too.
            if (is_text || id.is_empty() && mask.is_none())
                && children.len() == 1
                && (g.blend_mode() == usvg::BlendMode::Normal || children[0].blend == BlendMode::Normal)
                && let Some(only) = children.pop()
            {
                let mut c = Arc::unwrap_or_clone(only);
                c.opacity *= g.opacity().get();
                if g.blend_mode() != usvg::BlendMode::Normal {
                    c.blend = blend(g.blend_mode());
                }
                c.isolate |= g.isolate();
                if mask.is_some() {
                    c.mask = mask;
                }
                return Some(c);
            }
            self.named(id, NodeKind::Group { children, clip: false })
        };
        n.opacity = g.opacity().get();
        n.blend = blend(g.blend_mode());
        n.isolate = g.isolate();
        n.mask = mask;
        Some(n)
    }

    /// `<mask>` → opacity mask (luminance; alpha masks are approximated by their luminance). A
    /// mask our export wrote gets its options back ([`mask_flags`]) and its art without what
    /// stands for them: the inverting filter group and the backdrop rectangle.
    fn opacity_mask(&mut self, m: &usvg::Mask, ts: Affine, label: &str) -> Option<Box<vectorcraft_doc::OpacityMask>> {
        if m.kind() == usvg::MaskType::Alpha {
            self.warn(format!("alpha mask on {label} imported as a luminance opacity mask"));
        }
        if m.mask().is_some() {
            self.warn(format!("nested mask on {label} ignored"));
        }
        let (clip, invert) = self.mask_flags.get(m.id()).copied().unwrap_or((true, false));
        let (mut nodes, mut ts) = (m.root().children(), ts);
        if invert
            && let [usvg::Node::Group(g)] = nodes
            && !g.filters().is_empty()
        {
            (nodes, ts) = (g.children(), ts * aff(g.transform()));
        }
        if (!clip || invert)
            && let [usvg::Node::Path(_), rest @ ..] = nodes
        {
            nodes = rest;
        }
        let mut children: Vec<Arc<Node>> = nodes.iter().filter_map(|c| self.node(c, ts)).map(Arc::new).collect();
        let art = match children.len() {
            0 => return None,
            1 => Arc::unwrap_or_clone(children.pop()?),
            _ => self.named("", NodeKind::Group { children, clip: false }),
        };
        let mut mask = vectorcraft_doc::OpacityMask::new(art, clip);
        mask.invert = invert;
        Some(Box::new(mask))
    }

    fn clip_node(&mut self, cp: &usvg::ClipPath, ts: Affine) -> Option<Node> {
        fn collect(g: &usvg::Group, m: Affine, out: &mut Vec<(BezPath, FillRule)>) {
            for c in g.children() {
                match c {
                    usvg::Node::Group(g) => collect(g, m * aff(g.transform()), out),
                    usvg::Node::Path(p) => out.push((bezpath(p.data(), m), p.fill().map(|f| rule(f.rule())).unwrap_or_default())),
                    _ => {}
                }
            }
        }
        let mut shapes = Vec::new();
        collect(cp.root(), ts * aff(cp.transform()), &mut shapes);
        if shapes.is_empty() {
            return None;
        }
        let r = shapes[0].1;
        let mut subs = Vec::new();
        for (bp, _) in &shapes {
            subs.extend(PathData::from_bezpath(bp).subpaths);
        }
        let id = cp.id().to_string();
        if subs.len() == 1 {
            let path = PathData::new(subs);
            return Some(self.named(&id, NodeKind::Path { path, rule: r, live: None, clipping: true, guide: false }));
        }
        let children = subs
            .into_iter()
            .map(|sp| {
                let nid = self.doc.alloc_id();
                Arc::new(Node::new(nid, NodeKind::Path { path: PathData::single(sp), rule: r, live: None, clipping: false, guide: false }))
            })
            .collect();
        Some(self.named(&id, NodeKind::Compound { children, rule: r }))
    }

    fn paint(&mut self, p: &usvg::Paint, m: Affine) -> Paint {
        let gp = |kind, stops, geom: GradientGeom| {
            Paint::Gradient(Box::new(GradientPaint {
                gradient: Gradient { kind, stops },
                geom: Some(geom),
                angle: geom.angle_deg(),
                swatch: None,
                freeform: None,
            }))
        };
        match p {
            usvg::Paint::Color(c) => Paint::solid(Color::rgb8(c.red, c.green, c.blue)),
            usvg::Paint::LinearGradient(lg) => {
                if lg.spread_method() != usvg::SpreadMethod::Pad {
                    self.warn(format!("gradient '{}': spreadMethod approximated as pad", lg.id()));
                }
                let mut geom = GradientGeom {
                    start: Point::new(lg.x1() as f64, lg.y1() as f64),
                    end: Point::new(lg.x2() as f64, lg.y2() as f64),
                    aspect: 1.0,
                    focal: None,
                };
                geom.transform(m * aff(lg.transform()), GradientKind::Linear);
                gp(GradientKind::Linear, self.stops(lg), geom)
            }
            usvg::Paint::RadialGradient(rg) => {
                if rg.spread_method() != usvg::SpreadMethod::Pad {
                    self.warn(format!("gradient '{}': spreadMethod approximated as pad", rg.id()));
                }
                if rg.fr().get() > 1e-4 {
                    self.warn(format!("gradient '{}': focal radius ignored", rg.id()));
                }
                let c = Point::new(rg.cx() as f64, rg.cy() as f64);
                let mut geom = GradientGeom { start: c, end: c + Vec2::new(rg.r().get() as f64, 0.0), aspect: 1.0, focal: None };
                // A focal point outside the circle is pulled inside it.
                geom.set_focal(Some(Point::new(rg.fx() as f64, rg.fy() as f64)));
                geom.transform(m * aff(rg.transform()), GradientKind::Radial);
                gp(GradientKind::Radial, self.stops(rg), geom)
            }
            usvg::Paint::Pattern(pt) => self.pattern(pt, m),
        }
    }

    /// `<pattern>` → a pattern swatch painted with the pattern's tile placement. The swatch art is
    /// the pattern content clipped to the tile, as SVG draws it.
    fn pattern(&mut self, pt: &Arc<usvg::Pattern>, m: Affine) -> Paint {
        let r = pt.rect();
        let xf = m * aff(pt.transform()) * Affine::translate((r.x() as f64, r.y() as f64));
        let key = Arc::as_ptr(pt) as usize;
        if let Some(name) = self.patterns.get(&key) {
            return Paint::Pattern { pattern: name.clone(), xf };
        }
        let tile = Rect::new(0.0, 0.0, r.width() as f64, r.height() as f64);
        let content = self.children(pt.root(), aff(pt.root().transform()));
        if content.is_empty() {
            return Paint::None;
        }
        let clip =
            self.named("", NodeKind::Path { path: shapes::rectangle(tile), rule: FillRule::NonZero, live: None, clipping: true, guide: false });
        let mut children = vec![Arc::new(clip)];
        children.extend(content);
        let art = self.named("", NodeKind::Group { children, clip: true });
        let base = if pt.id().is_empty() { "Pattern" } else { pt.id() };
        let mut name = base.to_string();
        let mut k = 2;
        while self.doc.pattern(&name).is_some() {
            name = format!("{base} {k}");
            k += 1;
        }
        let mut def = PatternDef::new(&name, vec![Arc::new(art)]);
        def.tile = tile;
        self.doc.patterns.push(def);
        self.patterns.insert(key, name.clone());
        Paint::Pattern { pattern: name, xf }
    }

    fn appearance(&mut self, p: &usvg::Path, m: Affine) -> (Appearance, FillRule) {
        let mut fill = None;
        let mut r = FillRule::NonZero;
        if let Some(f) = p.fill() {
            let paint = self.paint(f.paint(), m);
            let mut fl = FillLayer::new(paint);
            fl.opacity = f.opacity().get();
            r = rule(f.rule());
            fill = Some(AppearanceItem::Fill(fl));
        }
        let mut stroke = None;
        if let Some(s) = p.stroke() {
            let scale = m.determinant().abs().sqrt();
            let paint = self.paint(s.paint(), m);
            let mut sl = StrokeLayer::new(paint, s.width().get() as f64 * scale);
            sl.opacity = s.opacity().get();
            sl.cap = match s.linecap() {
                usvg::LineCap::Butt => LineCap::Butt,
                usvg::LineCap::Round => LineCap::Round,
                usvg::LineCap::Square => LineCap::Square,
            };
            sl.join = match s.linejoin() {
                usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => LineJoin::Miter,
                usvg::LineJoin::Round => LineJoin::Round,
                usvg::LineJoin::Bevel => LineJoin::Bevel,
            };
            sl.miter_limit = s.miterlimit().get() as f64;
            if let Some(d) = s.dasharray() {
                sl.dash = Some(Dash {
                    pattern: d.iter().map(|v| *v as f64 * scale).collect(),
                    offset: s.dashoffset() as f64 * scale,
                    align_corners: false,
                });
            }
            stroke = Some(AppearanceItem::Stroke(sl));
        }
        let items = match p.paint_order() {
            usvg::PaintOrder::FillAndStroke => [fill, stroke],
            usvg::PaintOrder::StrokeAndFill => [stroke, fill],
        };
        (Appearance { items: items.into_iter().flatten().collect(), ..Default::default() }, r)
    }

    fn path(&mut self, p: &usvg::Path, acc: Affine) -> Option<Node> {
        if !p.is_visible() {
            return None;
        }
        let path = PathData::from_bezpath(&bezpath(p.data(), acc));
        if path.is_empty() {
            return None;
        }
        let (appearance, r) = self.appearance(p, acc);
        let mut n = if path.subpaths.len() > 1 {
            // Multi-subpath SVG paths are compound paths (as in Illustrator).
            let children = path
                .subpaths
                .into_iter()
                .map(|sp| {
                    let id = self.doc.alloc_id();
                    let mut c = Node::new(id, NodeKind::Path { path: PathData::single(sp), rule: r, live: None, clipping: false, guide: false });
                    c.appearance = appearance.clone();
                    Arc::new(c)
                })
                .collect();
            self.named(p.id(), NodeKind::Compound { children, rule: r })
        } else {
            self.named(p.id(), NodeKind::Path { path, rule: r, live: None, clipping: false, guide: false })
        };
        n.appearance = appearance;
        Some(n)
    }

    fn image(&mut self, i: &usvg::Image, acc: Affine) -> Option<Node> {
        if !i.is_visible() {
            return None;
        }
        let (bytes, mime) = match i.kind() {
            usvg::ImageKind::JPEG(d) => (d, "image/jpeg"),
            usvg::ImageKind::PNG(d) => (d, "image/png"),
            usvg::ImageKind::GIF(d) => (d, "image/gif"),
            usvg::ImageKind::WEBP(d) => (d, "image/webp"),
            usvg::ImageKind::SVG(_) => {
                self.warn("embedded SVG image skipped".into());
                return None;
            }
        };
        let size = i.size();
        let (pw, ph) = image::ImageReader::new(std::io::Cursor::new(&bytes[..]))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.into_dimensions().ok())
            .unwrap_or((size.width().round().max(1.0) as u32, size.height().round().max(1.0) as u32));
        let key = format!("img-{:016x}", fnv1a(bytes));
        self.doc.images.entry(key.clone()).or_insert_with(|| ImageBlob { mime: mime.into(), bytes: Arc::new(bytes.to_vec()) });
        let xf = acc * Affine::scale_non_uniform(size.width() as f64 / pw as f64, size.height() as f64 / ph as f64);
        Some(self.named(i.id(), NodeKind::Image(ImageObject { key, width: pw, height: ph, xf, link: None })))
    }
}
