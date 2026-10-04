//! The SVG writer.

use std::collections::{HashMap, HashSet};

use vectorcraft_color::{BlendMode, GradientKind, GradientPaint, Paint};
use vectorcraft_doc::{
    AppearanceItem, Document, FillLayer, LineCap, LineJoin, Node, NodeId, NodeKind, StrokeAlign, StrokeLayer, TextKind, TextObject,
};
use vectorcraft_doc::{CharStyle, Justify};
use vectorcraft_effects::stroke::{self, Written, WrittenShape};
use vectorcraft_geom::{Affine, BezPath, FillRule, PathData, Point, Rect};

use crate::{EDITING_NS, ExportOptions, ImageMode, LinkedImage, ObjectIds, Output, Styling, base64_encode, fmt_num, fnv1a, xml_escape};

type Props = Vec<(&'static str, String)>;

pub(crate) fn export(doc: &Document, opts: &ExportOptions, native: Option<&[u8]>) -> Output {
    // Live geometry effects (Roughen, Warp, Offset Path, Effect → Pathfinder…) export as their result.
    let baked = vectorcraft_effects::bake_document(doc);
    let doc = baked.as_ref().unwrap_or(doc);
    if opts.object_ids != ObjectIds::Unique {
        return write(doc, opts, native, "");
    }
    // Unique ids: every id and class name gets a prefix hashed from the output written without
    // one, so the same document always gets the same ids and different ones never share any.
    let plain = write(doc, opts, native, "");
    let prefix = format!("u{:010x}-", fnv1a(plain.svg.as_bytes()) & 0xff_ffff_ffff);
    write(doc, opts, native, &prefix)
}

fn write(doc: &Document, opts: &ExportOptions, native: Option<&[u8]>, id_prefix: &str) -> Output {
    let rect = opts
        .artboard
        .and_then(|i| doc.artboards.get(i))
        .map(|a| a.rect)
        .or_else(|| doc.art_bounds())
        .or_else(|| doc.artboards.first().map(|a| a.rect))
        .unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0));
    let mut w = Writer {
        doc,
        opts,
        xf: Affine::translate((-rect.x0, -rect.y0)),
        body: String::new(),
        defs: String::new(),
        classes: Vec::new(),
        used_ids: HashSet::new(),
        names: HashMap::new(),
        depth: 1,
        patterns: HashMap::new(),
        pattern_nest: 0,
        brushes: None,
        knockout: doc.page_knockout,
        anonymous: false,
        knockout_filter: None,
        warnings: vec![],
        id_prefix,
        linked: Vec::new(),
    };
    w.assign_name_ids();
    // Page Isolated Blending / Page Knockout Group: the page content is one isolated group.
    let page_group = doc.page_isolate || doc.page_knockout;
    if page_group {
        let a = w.attrs(&vec![("isolation", "isolate".into())]);
        w.line(&format!("<g{a}>"));
        w.depth += 1;
    }
    w.children(&doc.layers);
    if page_group {
        w.depth -= 1;
        w.line("</g>");
    }

    let nl = if opts.minify { "" } else { "\n" };
    let mut out = String::new();
    if !opts.minify {
        out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    }
    if opts.styling == Styling::StyleEntities && !w.classes.is_empty() {
        out.push_str("<!DOCTYPE svg [");
        for (i, c) in w.classes.iter().enumerate() {
            out.push_str(&format!("{nl}{}<!ENTITY st{} \"{}\">", w.indent(1), i + 1, entity_value(c)));
        }
        out.push_str(&format!("{nl}]>{nl}"));
    }
    let (ww, hh) = (w.num(rect.width()), w.num(rect.height()));
    out.push_str("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink=\"http://www.w3.org/1999/xlink\"");
    if !opts.responsive {
        out.push_str(&format!(" width=\"{ww}\" height=\"{hh}\""));
    }
    out.push_str(&format!(" viewBox=\"0 0 {ww} {hh}\">{nl}"));
    let css = opts.styling == Styling::InternalCss && !w.classes.is_empty();
    if !w.defs.is_empty() || css {
        out.push_str(&w.indent(1));
        out.push_str(&format!("<defs>{nl}"));
        if css {
            out.push_str(&w.indent(2));
            out.push_str("<style>");
            for (i, c) in w.classes.iter().enumerate() {
                out.push_str(&format!("{nl}{}.{id_prefix}cls-{}{{{}}}", w.indent(3), i + 1, xml_escape(c)));
            }
            out.push_str(&format!("{nl}{}</style>{nl}", w.indent(2)));
        }
        out.push_str(&w.defs);
        out.push_str(&w.indent(1));
        out.push_str(&format!("</defs>{nl}"));
    }
    if !doc.title.is_empty() {
        out.push_str(&w.indent(1));
        out.push_str(&format!("<title>{}</title>{nl}", xml_escape(&doc.title)));
    }
    w.metadata(&mut out, native);
    out.push_str(&w.body);
    out.push_str("</svg>");
    out.push_str(nl);
    Output { svg: out, linked: w.linked, warnings: w.warnings }
}

/// A CSS declaration block as the literal value of an `<!ENTITY>`: quotes, `%` and markup
/// characters become references that still read as themselves inside the `style` attribute.
fn entity_value(decl: &str) -> String {
    let mut o = String::with_capacity(decl.len());
    for c in decl.chars() {
        match c {
            '"' => o.push_str("&#34;"),
            '%' => o.push_str("&#37;"),
            // Doubly escaped: the entity's replacement text is parsed again where it is used.
            '&' => o.push_str("&#38;#38;"),
            '<' => o.push_str("&#38;#60;"),
            _ => o.push(c),
        }
    }
    o
}

/// The usual file extension of an image MIME type.
fn image_ext(mime: &str) -> &str {
    match mime {
        "image/jpeg" => "jpg",
        "image/svg+xml" => "svg",
        m => m.strip_prefix("image/").filter(|e| !e.is_empty() && e.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or("bin"),
    }
}

struct Writer<'a> {
    doc: &'a Document,
    opts: &'a ExportOptions,
    /// Document → SVG user space (artboard origin translation, symbol instance transforms).
    xf: Affine,
    body: String,
    defs: String,
    /// Unique CSS declaration blocks; class name is `cls-{index + 1}`.
    classes: Vec<String>,
    used_ids: HashSet<String>,
    names: HashMap<NodeId, String>,
    depth: usize,
    /// `<pattern>` def ids by pattern name + placement.
    patterns: HashMap<String, String>,
    pattern_nest: u32,
    /// Whether the group being written is a knockout group (what its neutral children inherit).
    knockout: bool,
    /// Writing a copy of an object (a knockout mask): no object ids, so they stay unique.
    anonymous: bool,
    /// The filter that paints art black keeping its alpha (knockout masks), once defined.
    knockout_filter: Option<String>,
    /// The brush library, parsed when the first brushed stroke is written.
    brushes: Option<Vec<vectorcraft_brush::Brush>>,
    /// Features written approximately (once each).
    warnings: Vec<String>,
    /// Prepended to every id and class name ([`ObjectIds::Unique`]).
    id_prefix: &'a str,
    /// Image files the SVG links to ([`ImageMode::Link`]).
    linked: Vec<LinkedImage>,
}

/// How one stroke is written.
enum StrokePlan {
    /// Its brush art (document space).
    Brush(Vec<Node>),
    Written(Written),
}

/// A region covering any artwork (mask and filter extents).
const BIG: &str = "x=\"-100000\" y=\"-100000\" width=\"200000\" height=\"200000\"";
/// Attributes of every `<mask>`: user-space units, and luminance taken from the sRGB values (as the
/// canvas and PDF take it) rather than from linearised ones.
const MASK: &str = "maskUnits=\"userSpaceOnUse\" color-interpolation=\"sRGB\"";
/// Attribute of an exported opacity mask listing its options that differ from the defaults
/// (clipping, not inverted): `noclip`, `invert`.
pub(crate) const MASK_FLAGS: &str = "data-vectorcraft-mask";

pub(crate) fn blend_css(b: BlendMode) -> &'static str {
    match b {
        BlendMode::Normal => "normal",
        BlendMode::Darken => "darken",
        BlendMode::Multiply => "multiply",
        BlendMode::ColorBurn => "color-burn",
        BlendMode::Lighten => "lighten",
        BlendMode::Screen => "screen",
        BlendMode::ColorDodge => "color-dodge",
        BlendMode::Overlay => "overlay",
        BlendMode::SoftLight => "soft-light",
        BlendMode::HardLight => "hard-light",
        BlendMode::Difference => "difference",
        BlendMode::Exclusion => "exclusion",
        BlendMode::Hue => "hue",
        BlendMode::Saturation => "saturation",
        BlendMode::Color => "color",
        BlendMode::Luminosity => "luminosity",
    }
}

/// Turn an object name into a valid, readable XML id.
fn sanitize_id(name: &str) -> String {
    let mut s: String = name.trim().chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' }).collect();
    if s.is_empty() || !s.starts_with(|c: char| c.is_alphabetic() || c == '_') {
        s.insert(0, '_');
    }
    s
}

impl Writer<'_> {
    fn num(&self, v: f64) -> String {
        fmt_num(v, self.opts.decimals)
    }
    fn indent(&self, depth: usize) -> String {
        if self.opts.minify { String::new() } else { "  ".repeat(depth) }
    }
    fn line(&mut self, s: &str) {
        let ind = self.indent(self.depth);
        self.body.push_str(&ind);
        self.body.push_str(s);
        if !self.opts.minify {
            self.body.push('\n');
        }
    }
    fn def(&mut self, depth: usize, s: &str) {
        let ind = self.indent(depth + 1);
        self.defs.push_str(&ind);
        self.defs.push_str(s);
        if !self.opts.minify {
            self.defs.push('\n');
        }
    }
    fn fresh_id(&mut self, prefix: &str) -> String {
        let mut i = 1;
        loop {
            let id = format!("{}{prefix}-{i}", self.id_prefix);
            if self.used_ids.insert(id.clone()) {
                return id;
            }
            i += 1;
        }
    }
    /// Reserve ids for every named object up front so generated def ids never collide with them.
    fn assign_name_ids(&mut self) {
        if self.opts.object_ids == ObjectIds::Minimal {
            return;
        }
        let mut named: Vec<(NodeId, String)> = Vec::new();
        self.doc.walk(|n| {
            if let Some(name) = &n.name {
                named.push((n.id, format!("{}{}", self.id_prefix, sanitize_id(name))));
            }
        });
        for (id, base) in named {
            let mut cand = base.clone();
            let mut i = 2;
            while !self.used_ids.insert(cand.clone()) {
                cand = format!("{base}-{i}");
                i += 1;
            }
            self.names.insert(id, cand);
        }
    }
    fn id_attr(&self, n: &Node) -> String {
        if self.anonymous {
            return String::new();
        }
        self.names.get(&n.id).map(|id| format!(" id=\"{}\"", xml_escape(id))).unwrap_or_default()
    }
    fn matrix(&self, m: Affine) -> String {
        let c = m.as_coeffs();
        format!("matrix({} {} {} {} {} {})", self.num(c[0]), self.num(c[1]), self.num(c[2]), self.num(c[3]), self.num(c[4]), self.num(c[5]))
    }

    /// Style properties as attributes, an inline style, or a CSS class, per the styling option.
    fn attrs(&mut self, props: &Props) -> String {
        if props.is_empty() {
            return String::new();
        }
        let css = |ps: &[&(&str, String)]| ps.iter().map(|(k, v)| format!("{k}:{v}")).collect::<Vec<_>>().join(";");
        match self.opts.styling {
            Styling::PresentationAttributes => {
                // CSS-only properties go in a style attribute.
                let (style, attrs): (Vec<_>, Vec<_>) =
                    props.iter().partition(|(k, _)| matches!(*k, "mix-blend-mode" | "isolation" | "font-feature-settings" | "font-kerning"));
                let mut s: String = attrs.iter().map(|(k, v)| format!(" {k}=\"{}\"", xml_escape(v))).collect();
                if !style.is_empty() {
                    s.push_str(&format!(" style=\"{}\"", xml_escape(&css(&style))));
                }
                s
            }
            Styling::InlineStyle => format!(" style=\"{}\"", xml_escape(&css(&props.iter().collect::<Vec<_>>()))),
            Styling::StyleEntities => format!(" style=\"&st{};\"", self.class(css(&props.iter().collect::<Vec<_>>()))),
            Styling::InternalCss => {
                let i = self.class(css(&props.iter().collect::<Vec<_>>()));
                format!(" class=\"{}cls-{i}\"", self.id_prefix)
            }
        }
    }

    /// The 1-based number of a distinct declaration block (a CSS class or a style entity).
    fn class(&mut self, decl: String) -> usize {
        match self.classes.iter().position(|c| *c == decl) {
            Some(i) => i + 1,
            None => {
                self.classes.push(decl);
                self.classes.len()
            }
        }
    }

    /// `<metadata>`: the Dublin Core title and format ([`ExportOptions::metadata`]) and the native
    /// document ([`ExportOptions::preserve_editing`]).
    fn metadata(&self, out: &mut String, native: Option<&[u8]>) {
        let native = native.filter(|_| self.opts.preserve_editing);
        if !self.opts.metadata && native.is_none() {
            return;
        }
        let nl = if self.opts.minify { "" } else { "\n" };
        let mut line = |depth: usize, s: &str| {
            out.push_str(&self.indent(depth));
            out.push_str(s);
            out.push_str(nl);
        };
        line(1, "<metadata>");
        if self.opts.metadata {
            line(2, "<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\">");
            line(3, "<rdf:Description rdf:about=\"\">");
            line(4, "<dc:format>image/svg+xml</dc:format>");
            if !self.doc.title.is_empty() {
                line(4, &format!("<dc:title>{}</dc:title>", xml_escape(&self.doc.title)));
            }
            line(3, "</rdf:Description>");
            line(2, "</rdf:RDF>");
        }
        if let Some(bytes) = native {
            line(2, &format!("<vectorcraft:document xmlns:vectorcraft=\"{EDITING_NS}\">{}</vectorcraft:document>", base64_encode(bytes)));
        }
        line(1, "</metadata>");
    }

    fn node_props(n: &Node) -> Props {
        let mut p = Props::new();
        if n.opacity < 1.0 {
            p.push(("opacity", fmt_num(n.opacity as f64, 3)));
        }
        if n.blend != BlendMode::Normal {
            p.push(("mix-blend-mode", blend_css(n.blend).into()));
        }
        if n.isolate {
            p.push(("isolation", "isolate".into()));
        }
        p
    }

    fn path_d(&self, p: &PathData, m: Affine) -> String {
        let mut s = String::new();
        let pt = |p: Point| {
            let p = m * p;
            format!("{} {}", self.num(p.x), self.num(p.y))
        };
        for sp in &p.subpaths {
            let n = sp.anchors.len();
            if n == 0 {
                continue;
            }
            if !s.is_empty() {
                s.push(' ');
            }
            s.push('M');
            s.push_str(&pt(sp.anchors[0].p));
            for i in 0..sp.segment_count() {
                let (a, b) = (&sp.anchors[i], &sp.anchors[(i + 1) % n]);
                if sp.segment_is_line(i) {
                    if sp.closed && i == n - 1 {
                        break;
                    }
                    s.push_str(&format!(" L{}", pt(b.p)));
                } else {
                    s.push_str(&format!(" C{} {} {}", pt(a.h_out), pt(b.h_in), pt(b.p)));
                }
            }
            if sp.closed {
                s.push_str(" Z");
            }
        }
        s
    }

    fn paint(&mut self, p: &Paint, bounds: Option<Rect>) -> String {
        match p {
            Paint::None => "none".into(),
            Paint::Pattern { pattern, xf } => match self.pattern_def(pattern, *xf) {
                Some(id) => format!("url(#{id})"),
                None => "none".into(),
            },
            Paint::Solid { color, .. } => color.to_hex(),
            Paint::Gradient(g) => format!("url(#{})", self.gradient_def(g, bounds.unwrap_or(Rect::new(0.0, 0.0, 1.0, 1.0)))),
        }
    }

    /// A `<pattern>` def for pattern `name` placed by `xf`: one period (super-tile) of the tiling,
    /// with every instance that reaches into it.
    fn pattern_def(&mut self, name: &str, xf: Affine) -> Option<String> {
        let doc = self.doc;
        let def = doc.pattern(name)?;
        let m = self.xf * xf;
        let key = format!("{name}|{:?}", m.as_coeffs());
        if let Some(id) = self.patterns.get(&key) {
            return Some(id.clone());
        }
        if self.pattern_nest > 4 {
            return None;
        }
        let id = self.fresh_id("pattern");
        self.patterns.insert(key, id.clone());
        let (pw, ph) = def.period();
        let (saved_body, saved_xf, saved_depth) = (std::mem::take(&mut self.body), self.xf, self.depth);
        self.depth = 3;
        self.pattern_nest += 1;
        for o in def.offsets_covering(Rect::new(0.0, 0.0, pw, ph)) {
            self.xf = def.instance_xf(o);
            for a in &def.art {
                self.node(a);
            }
        }
        self.pattern_nest -= 1;
        let content = std::mem::replace(&mut self.body, saved_body);
        self.xf = saved_xf;
        self.depth = saved_depth;
        let head = format!(
            "<pattern id=\"{id}\" patternUnits=\"userSpaceOnUse\" width=\"{}\" height=\"{}\" patternTransform=\"{}\">",
            self.num(pw),
            self.num(ph),
            self.matrix(m)
        );
        self.def(1, &head);
        self.defs.push_str(&content);
        self.def(1, "</pattern>");
        Some(id)
    }

    fn gradient_def(&mut self, g: &GradientPaint, bounds: Rect) -> String {
        let mut geom = g.resolve(bounds);
        geom.transform(self.xf, g.gradient.kind);
        let (s, e) = (geom.start, geom.end);
        let head = match g.gradient.kind {
            GradientKind::Radial => {
                let id = self.fresh_id("radial-gradient");
                let r = (e - s).hypot();
                // An off-centre focal point: `fx`/`fy`, in the gradient's own (unsquashed) space.
                let focal = |f: Point| format!(" fx=\"{}\" fy=\"{}\"", self.num(f.x), self.num(f.y));
                if (geom.aspect - 1.0).abs() < 1e-9 {
                    (
                        id.clone(),
                        format!(
                            "<radialGradient id=\"{id}\" cx=\"{}\" cy=\"{}\" r=\"{}\"{} gradientUnits=\"userSpaceOnUse\">",
                            self.num(s.x),
                            self.num(s.y),
                            self.num(r),
                            geom.focal.map(focal).unwrap_or_default()
                        ),
                        "radialGradient",
                    )
                } else {
                    let v = e - s;
                    let m = Affine::translate(s.to_vec2()) * Affine::rotate(v.y.atan2(v.x)) * Affine::scale_non_uniform(1.0, geom.aspect);
                    (
                        id.clone(),
                        format!(
                            "<radialGradient id=\"{id}\" cx=\"0\" cy=\"0\" r=\"{}\"{} gradientTransform=\"{}\" gradientUnits=\"userSpaceOnUse\">",
                            self.num(r),
                            geom.focal.map(|f| focal(m.inverse() * f)).unwrap_or_default(),
                            self.matrix(m)
                        ),
                        "radialGradient",
                    )
                }
            }
            GradientKind::Linear | GradientKind::Freeform => {
                let id = self.fresh_id("linear-gradient");
                (
                    id.clone(),
                    format!(
                        "<linearGradient id=\"{id}\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" gradientUnits=\"userSpaceOnUse\">",
                        self.num(s.x),
                        self.num(s.y),
                        self.num(e.x),
                        self.num(e.y)
                    ),
                    "linearGradient",
                )
            }
        };
        let (id, open, tag) = head;
        self.def(1, &open);
        for (off, c, o) in g.gradient.expanded_stops() {
            let op = if o < 1.0 { format!(" stop-opacity=\"{}\"", fmt_num(o as f64, 3)) } else { String::new() };
            self.def(2, &format!("<stop offset=\"{}\" stop-color=\"{}\"{op}/>", fmt_num(off as f64, 4), c.to_hex()));
        }
        self.def(1, &format!("</{tag}>"));
        id
    }

    fn fill_props(&mut self, f: &FillLayer, rule: FillRule, bounds: Option<Rect>, p: &mut Props) {
        let paint = self.paint(&f.paint, bounds);
        p.push(("fill", paint));
        if f.opacity < 1.0 {
            p.push(("fill-opacity", fmt_num(f.opacity as f64, 3)));
        }
        if rule == FillRule::EvenOdd {
            p.push(("fill-rule", "evenodd".into()));
        }
    }

    fn stroke_props(&mut self, s: &StrokeLayer, width: f64, bounds: Option<Rect>, p: &mut Props) {
        let paint = self.paint(&s.paint, bounds);
        p.push(("stroke", paint));
        if s.opacity < 1.0 {
            p.push(("stroke-opacity", fmt_num(s.opacity as f64, 3)));
        }
        if (width - 1.0).abs() > 1e-9 {
            p.push(("stroke-width", self.num(width)));
        }
        match s.cap {
            LineCap::Butt => {}
            LineCap::Round => p.push(("stroke-linecap", "round".into())),
            LineCap::Square => p.push(("stroke-linecap", "square".into())),
        }
        match s.join {
            LineJoin::Miter => {
                if (s.miter_limit - 4.0).abs() > 1e-9 {
                    p.push(("stroke-miterlimit", self.num(s.miter_limit)));
                }
            }
            LineJoin::Round => p.push(("stroke-linejoin", "round".into())),
            LineJoin::Bevel => p.push(("stroke-linejoin", "bevel".into())),
        }
        if let Some(d) = &s.dash
            && d.is_dashed()
        {
            p.push(("stroke-dasharray", d.pattern.iter().map(|v| self.num(*v)).collect::<Vec<_>>().join(" ")));
            if d.offset != 0.0 {
                p.push(("stroke-dashoffset", self.num(d.offset)));
            }
        }
    }

    /// How stroke `st` is written (`bp`: the shape in document space, built on first use from
    /// `paths`): its brush art, or a plain stroke or filled outlines matching the canvas.
    fn stroke_plan(&mut self, st: &StrokeLayer, bp: &mut Option<BezPath>, paths: &[&PathData]) -> StrokePlan {
        if stroke::is_plain(st) {
            return StrokePlan::Written(stroke::for_writer(&BezPath::new(), st));
        }
        let bp = bp.get_or_insert_with(|| {
            let mut bp = BezPath::new();
            for p in paths {
                bp.extend(p.to_bezpath());
            }
            bp
        });
        let doc = self.doc;
        let brushes = self.brushes.get_or_insert_with(|| vectorcraft_brush::library(doc));
        match st.brush.as_deref().and_then(|name| brushes.iter().find(|b| b.name == name)) {
            Some(b) => StrokePlan::Brush(vectorcraft_brush::stroke_pieces(b, bp, st)),
            None => StrokePlan::Written(stroke::for_writer(bp, st)),
        }
    }

    /// The `clip-path` (inside) or `mask` (outside) attribute that keeps an aligned stroke on its
    /// side of the shape `d`; `reach` (document space) is what the stroke covers.
    fn side_attr(&mut self, side: Option<StrokeAlign>, d: &str, rule: FillRule, reach: Rect) -> String {
        match side {
            None | Some(StrokeAlign::Center) => String::new(),
            Some(StrokeAlign::Inside) => {
                let cid = self.fresh_id("clip-path");
                let r = if rule == FillRule::EvenOdd { " clip-rule=\"evenodd\"" } else { "" };
                self.def(1, &format!("<clipPath id=\"{cid}\">"));
                self.def(2, &format!("<path d=\"{d}\"{r}/>"));
                self.def(1, "</clipPath>");
                format!(" clip-path=\"url(#{cid})\"")
            }
            Some(StrokeAlign::Outside) => {
                let mid = self.fresh_id("mask");
                let b = self.xf.transform_rect_bbox(reach).inflate(1.0, 1.0);
                let (x, y, w, h) = (self.num(b.x0), self.num(b.y0), self.num(b.width()), self.num(b.height()));
                let fr = if rule == FillRule::EvenOdd { " fill-rule=\"evenodd\"" } else { "" };
                self.def(1, &format!("<mask id=\"{mid}\" {MASK} x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\">"));
                self.def(2, &format!("<rect x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\" fill=\"#fff\"/>"));
                self.def(2, &format!("<path d=\"{d}\" fill=\"#000\"{fr}/>"));
                self.def(1, "</mask>");
                format!(" mask=\"url(#{mid})\"")
            }
        }
    }

    /// Paint a shape (`d`) with a node's appearance stack. `paths` is the same shape in document
    /// space, for strokes that aren't plain (it may be empty when all are).
    fn shape(&mut self, n: &Node, d: &str, paths: &[&PathData], rule: FillRule, bounds: Option<Rect>) {
        let id = self.id_attr(n);
        let items: Vec<&AppearanceItem> = n
            .appearance
            .items
            .iter()
            .filter(|i| match i {
                AppearanceItem::Fill(f) => f.visible && !f.paint.is_none(),
                AppearanceItem::Stroke(s) => s.visible && !s.paint.is_none() && s.width > 0.0,
            })
            .collect();
        let mut bp = None;
        let plans: Vec<Option<StrokePlan>> = items
            .iter()
            .map(|i| match i {
                AppearanceItem::Stroke(s) => Some(self.stroke_plan(s, &mut bp, paths)),
                AppearanceItem::Fill(_) => None,
            })
            .collect();
        let fills = items.iter().filter(|i| matches!(i, AppearanceItem::Fill(_))).count();
        let strokes = items.len() - fills;
        // A stroke's paint box is the bounds grown by half its weight, as on the canvas.
        let paint_bounds = |s: &StrokeLayer| bounds.map(|b| s.paint_bounds(b));
        let simple = fills <= 1
            && strokes <= 1
            && !items.iter().any(|i| has_raster(i.effects()))
            && items.iter().all(|i| i.blend() == BlendMode::Normal)
            && plans.iter().flatten().all(|p| matches!(p, StrokePlan::Written(Written { shape: WrittenShape::Stroke { .. }, side: None })));
        if simple {
            let mut p = Props::new();
            match items.iter().find_map(|i| if let AppearanceItem::Fill(f) = i { Some(f) } else { None }) {
                Some(f) => self.fill_props(f, rule, bounds, &mut p),
                None => {
                    p.push(("fill", "none".into()));
                    if rule == FillRule::EvenOdd {
                        p.push(("fill-rule", "evenodd".into()));
                    }
                }
            }
            if let Some(s) = items.iter().find_map(|i| if let AppearanceItem::Stroke(s) = i { Some(s) } else { None }) {
                self.stroke_props(s, s.width, paint_bounds(s), &mut p);
                if matches!(items.first(), Some(AppearanceItem::Stroke(_))) && fills == 1 {
                    p.push(("paint-order", "stroke".into()));
                }
            }
            p.extend(Self::node_props(n));
            let a = self.attrs(&p);
            self.line(&format!("<path{id} d=\"{d}\"{a}/>"));
            return;
        }
        let a = self.attrs(&Self::node_props(n));
        self.line(&format!("<g{id}{a}>"));
        self.depth += 1;
        for (it, plan) in items.into_iter().zip(plans) {
            // The item's own raster effects filter that item alone.
            let filters = self.open_filters(n, it.effects());
            let mut p = Props::new();
            match (it, plan) {
                (AppearanceItem::Fill(f), _) => {
                    self.fill_props(f, rule, bounds, &mut p);
                    if f.blend != BlendMode::Normal {
                        p.push(("mix-blend-mode", blend_css(f.blend).into()));
                    }
                    let a = self.attrs(&p);
                    self.line(&format!("<path d=\"{d}\"{a}/>"));
                }
                (AppearanceItem::Stroke(s), Some(StrokePlan::Brush(art))) => {
                    // The brush art takes the stroke's opacity and blend mode as a group.
                    if s.opacity < 1.0 {
                        p.push(("opacity", fmt_num(s.opacity as f64, 3)));
                    }
                    if s.blend != BlendMode::Normal {
                        p.push(("mix-blend-mode", blend_css(s.blend).into()));
                    }
                    let a = self.attrs(&p);
                    self.line(&format!("<g{a}>"));
                    self.depth += 1;
                    for piece in &art {
                        self.node(piece);
                    }
                    self.depth -= 1;
                    self.line("</g>");
                }
                (AppearanceItem::Stroke(s), Some(StrokePlan::Written(w))) => {
                    let side = self.side_attr(w.side, d, rule, w.reach(s, bounds.unwrap_or_default()));
                    let blend = (s.blend != BlendMode::Normal).then(|| ("mix-blend-mode", blend_css(s.blend).to_string()));
                    match &w.shape {
                        WrittenShape::Stroke { width } => {
                            p.push(("fill", "none".into()));
                            self.stroke_props(s, *width, paint_bounds(s), &mut p);
                            p.extend(blend);
                            let a = self.attrs(&p);
                            self.line(&format!("<path d=\"{d}\"{a}{side}/>"));
                        }
                        WrittenShape::Fill(outlines) if s.path_gradient().is_some() => {
                            // A gradient along or across the stroke: slices clipped to its outlines.
                            if let Some(ws) = bp.as_ref().and_then(|bp| stroke::written_slices(bp, rule, s, outlines)) {
                                p.extend((s.opacity < 1.0).then(|| ("opacity", fmt_num(s.opacity as f64, 3))));
                                p.extend(blend);
                                self.sliced(&ws, &p, &side);
                            }
                        }
                        WrittenShape::Fill(outlines) => {
                            let paint = self.paint(&s.paint, paint_bounds(s));
                            let ds: Vec<String> = outlines.iter().map(|o| self.path_d(&PathData::from_bezpath(o), self.xf)).collect();
                            let opacity = (s.opacity < 1.0).then(|| fmt_num(s.opacity as f64, 3));
                            if let [one] = &ds[..] {
                                p.push(("fill", paint));
                                p.extend(opacity.map(|o| ("fill-opacity", o)));
                                p.extend(blend);
                                let a = self.attrs(&p);
                                self.line(&format!("<path d=\"{one}\"{a}{side}/>"));
                            } else if !ds.is_empty() {
                                // The line and its arrowheads overlap: one group takes the opacity.
                                p.extend(opacity.map(|o| ("opacity", o)));
                                p.extend(blend);
                                let a = self.attrs(&p);
                                self.line(&format!("<g{a}{side}>"));
                                self.depth += 1;
                                let fill = self.attrs(&vec![("fill", paint)]);
                                for one in &ds {
                                    self.line(&format!("<path d=\"{one}\"{fill}/>"));
                                }
                                self.depth -= 1;
                                self.line("</g>");
                            }
                        }
                    }
                }
                (AppearanceItem::Stroke(_), None) => {}
            }
            self.close_filters(filters);
        }
        self.depth -= 1;
        self.line("</g>");
    }

    /// A stroke whose gradient runs along or across it ([`stroke::written_slices`]): a group (with
    /// `props` and the `side` attribute) of its slices, clipped to its outlines.
    fn sliced(&mut self, ws: &stroke::WrittenSlices, props: &Props, side: &str) {
        self.warn("gradients along or across strokes are written as slices of linear gradients");
        let cid = self.fresh_id("clip-path");
        let clip = self.path_d(&PathData::from_bezpath(&ws.clip), self.xf);
        self.def(1, &format!("<clipPath id=\"{cid}\">"));
        self.def(2, &format!("<path d=\"{clip}\"/>"));
        self.def(1, "</clipPath>");
        let a = self.attrs(props);
        self.line(&format!("<g{a}{side}>"));
        self.depth += 1;
        self.line(&format!("<g clip-path=\"url(#{cid})\">"));
        self.depth += 1;
        for (shape, paint) in &ws.slices {
            let fill = self.paint(paint, None);
            let fill = self.attrs(&vec![("fill", fill)]);
            let d = self.path_d(&PathData::from_bezpath(shape), self.xf);
            self.line(&format!("<path d=\"{d}\"{fill}/>"));
        }
        self.depth -= 1;
        self.line("</g>");
        self.depth -= 1;
        self.line("</g>");
    }

    fn warn(&mut self, w: &str) {
        if !self.warnings.iter().any(|x| x == w) {
            self.warnings.push(w.to_string());
        }
    }

    /// An object with an opacity mask: `<g mask="url(#…)">` around the unmasked object. The mask
    /// art goes in `<defs>`; no-clip adds a white backdrop and invert a colour-inverting filter.
    fn masked(&mut self, n: &Node, m: &vectorcraft_doc::OpacityMask) {
        let mid = self.fresh_id("mask");
        // Mask art is a picture of its own: it takes no part in a knockout group around the object.
        let knockout = std::mem::take(&mut self.knockout);
        let art = self.detached(2, |w| w.node(&m.art));
        self.knockout = knockout;
        let inv = m.invert.then(|| {
            let fid = self.fresh_id("invert");
            self.def(1, &format!("<filter id=\"{fid}\" filterUnits=\"userSpaceOnUse\" color-interpolation-filters=\"sRGB\" {BIG}>"));
            self.def(2, "<feColorMatrix type=\"matrix\" values=\"-1 0 0 0 1 0 -1 0 0 1 0 0 -1 0 1 0 0 0 1 0\"/>");
            self.def(1, "</filter>");
            fid
        });
        // The mask's options, so import restores them (and its art without backdrop and filter).
        let flags: Vec<&str> = [(!m.clip).then_some("noclip"), m.invert.then_some("invert")].into_iter().flatten().collect();
        let data = if flags.is_empty() { String::new() } else { format!(" {MASK_FLAGS}=\"{}\"", flags.join(" ")) };
        self.def(1, &format!("<mask id=\"{mid}\" {MASK} {BIG}{data}>"));
        if let Some(fid) = &inv {
            self.def(1, &format!("<g filter=\"url(#{fid})\">"));
        }
        if !m.clip || m.invert {
            let c = if m.clip { "black" } else { "white" };
            self.def(1, &format!("<rect {BIG} fill=\"{c}\"/>"));
        }
        self.defs.push_str(&art);
        if inv.is_some() {
            self.def(1, "</g>");
        }
        self.def(1, "</mask>");
        self.line(&format!("<g mask=\"url(#{mid})\">"));
        self.depth += 1;
        let mut bare = n.clone();
        bare.mask = None;
        self.node_body(&bare);
        self.depth -= 1;
        self.line("</g>");
    }

    /// What `write` writes to the body, taken out of it (written at `depth`, for `<defs>`).
    fn detached(&mut self, depth: usize, write: impl FnOnce(&mut Self)) -> String {
        let (body, saved) = (std::mem::take(&mut self.body), self.depth);
        self.depth = depth;
        write(self);
        self.depth = saved;
        std::mem::replace(&mut self.body, body)
    }

    /// Props of a group: a knockout group is isolated.
    fn group_props(&self, n: &Node) -> Props {
        let mut p = Self::node_props(n);
        if !n.isolate && n.knocks_out(self.knockout) {
            p.push(("isolation", "isolate".into()));
        }
        p
    }

    /// The children of group `n`, as the elements of a knockout group when it is one.
    fn group_children(&mut self, n: &Node, children: &[std::sync::Arc<Node>]) {
        let knockout = n.knocks_out(self.knockout);
        let enclosing = std::mem::replace(&mut self.knockout, knockout);
        self.children(children);
        self.knockout = enclosing;
    }

    /// Children of the group being written. SVG has no knockout groups: in one, each element is
    /// drawn through a mask of where the elements above it don't paint (the same look for Normal
    /// blending). The masks nest, so element i sits inside the masks of elements i+1…n.
    fn children(&mut self, children: &[std::sync::Arc<Node>]) {
        if !self.knockout {
            for c in children {
                self.node(c);
            }
            return;
        }
        let elements = Node::knockout_elements(children);
        let Some((first, rest)) = elements.split_first() else { return };
        for c in rest.iter().rev() {
            let mid = self.knockout_mask(c);
            self.line(&format!("<g mask=\"url(#{mid})\">"));
            self.depth += 1;
        }
        self.node(first);
        for c in rest {
            self.depth -= 1;
            self.line("</g>");
            self.node(c);
        }
    }

    /// A mask that is 1 − the knockout shape of `c`: white, then `c` painted black (at full object
    /// opacity without its own mask, unless those define its shape). Returns the mask id.
    fn knockout_mask(&mut self, c: &Node) -> String {
        let filter = match &self.knockout_filter {
            Some(f) => f.clone(),
            None => {
                let f = self.fresh_id("knockout-shape");
                self.def(1, &format!("<filter id=\"{f}\" filterUnits=\"userSpaceOnUse\" {BIG}>"));
                self.def(2, "<feColorMatrix type=\"matrix\" values=\"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 1 0\"/>");
                self.def(1, "</filter>");
                self.knockout_filter = Some(f.clone());
                f
            }
        };
        let shape = if c.knockout_shape { c.clone() } else { Node { opacity: 1.0, mask: None, ..c.clone() } };
        let anonymous = std::mem::replace(&mut self.anonymous, true);
        let art = self.detached(3, |w| w.node(&shape));
        self.anonymous = anonymous;
        let mid = self.fresh_id("knockout");
        self.def(1, &format!("<mask id=\"{mid}\" {MASK} {BIG}>"));
        self.def(2, &format!("<rect {BIG} fill=\"white\"/>"));
        self.def(2, &format!("<g filter=\"url(#{filter})\">"));
        self.defs.push_str(&art);
        self.def(2, "</g>");
        self.def(1, "</mask>");
        mid
    }

    /// An object, inside `<a>` when it links to a URL (Attributes panel).
    fn node(&mut self, n: &Node) {
        match n.url().filter(|_| n.visible && !self.anonymous) {
            Some(url) => {
                self.line(&format!("<a xlink:href=\"{}\">", xml_escape(url)));
                self.depth += 1;
                self.node_body(n);
                self.depth -= 1;
                self.line("</a>");
            }
            None => self.node_body(n),
        }
    }

    fn node_body(&mut self, n: &Node) {
        if !n.visible {
            return;
        }
        if has_raster(&n.appearance.effects) {
            return self.filtered(n);
        }
        if let Some(m) = n.mask.as_deref()
            && !m.disabled
        {
            return self.masked(n, m);
        }
        match &n.kind {
            NodeKind::Layer { template: true, .. } => {}
            NodeKind::Layer { children, clip: false, .. } | NodeKind::Group { children, clip: false } => {
                let id = self.id_attr(n);
                let a = self.attrs(&self.group_props(n));
                self.line(&format!("<g{id}{a}>"));
                self.depth += 1;
                self.group_children(n, children);
                self.depth -= 1;
                self.line("</g>");
            }
            NodeKind::Group { children, clip: true } | NodeKind::Layer { children, clip: true, .. } => {
                let Some((clip, rest)) = children.split_first() else { return };
                let cid = self.fresh_id("clip-path");
                self.def(1, &format!("<clipPath id=\"{cid}\">"));
                // The region every output clips to; with nothing to clip by, an empty clip path
                // hides the clipped art.
                if let Some((bp, rule)) = vectorcraft_effects::clip_outline(clip) {
                    let d = self.path_d(&PathData::from_bezpath(&bp), self.xf);
                    let r = if rule == FillRule::EvenOdd { " clip-rule=\"evenodd\"" } else { "" };
                    self.def(2, &format!("<path d=\"{d}\"{r}/>"));
                }
                self.def(1, "</clipPath>");
                let id = self.id_attr(n);
                let a = self.attrs(&self.group_props(n));
                // The clipping path's fill paints behind the clipped art and its stroke over it,
                // outside the clip (the group then wraps both).
                let paint = clip.clip_paint();
                if paint.stroke.is_some() {
                    self.line(&format!("<g{id}{a}>"));
                    self.depth += 1;
                    self.line(&format!("<g clip-path=\"url(#{cid})\">"));
                } else {
                    self.line(&format!("<g{id} clip-path=\"url(#{cid})\"{a}>"));
                }
                self.depth += 1;
                if let Some(fill) = &paint.fill {
                    self.node(fill);
                }
                self.group_children(n, rest);
                self.depth -= 1;
                self.line("</g>");
                if let Some(stroke) = &paint.stroke {
                    // One element keeps the clipping path's id.
                    let anonymous = self.anonymous;
                    self.anonymous |= paint.fill.is_some();
                    self.node(stroke);
                    self.anonymous = anonymous;
                    self.depth -= 1;
                    self.line("</g>");
                }
            }
            NodeKind::Path { guide: true, .. } => {}
            NodeKind::Path { path, rule, .. } => {
                if path.is_empty() {
                    return;
                }
                let d = self.path_d(path, self.xf);
                self.shape(n, &d, &[path], *rule, n.geometric_bounds());
            }
            NodeKind::Compound { children, rule } => {
                let paths: Vec<&PathData> = children.iter().filter(|c| c.visible).filter_map(|c| c.path_data()).filter(|p| !p.is_empty()).collect();
                if paths.is_empty() {
                    return;
                }
                let d: Vec<String> = paths.iter().map(|p| self.path_d(p, self.xf)).collect();
                self.shape(n, &d.join(" "), &paths, *rule, n.geometric_bounds());
            }
            NodeKind::Text(t) => self.text_node(n, t),
            NodeKind::Image(im) => {
                let link = self.opts.images == ImageMode::Link;
                let href = match (im.link.as_ref().filter(|_| link), self.doc.images.get(&im.key)) {
                    (Some(l), _) => l.clone(),
                    (None, Some(b)) if !b.bytes.is_empty() && link => {
                        // Named after the bytes: image keys ("raster-1"…) repeat across documents.
                        let name = format!("{}.{}", b.content_key(), image_ext(&b.mime));
                        if !self.linked.iter().any(|l| l.name == name) {
                            self.linked.push(LinkedImage { name: name.clone(), bytes: b.bytes.clone() });
                        }
                        name
                    }
                    (None, Some(b)) if !b.bytes.is_empty() => format!("data:{};base64,{}", b.mime, base64_encode(&b.bytes)),
                    _ => match &im.link {
                        Some(l) => l.clone(),
                        None => return,
                    },
                };
                let id = self.id_attr(n);
                let m = self.matrix(self.xf * im.xf);
                let a = self.attrs(&Self::node_props(n));
                self.line(&format!(
                    "<image{id} width=\"{}\" height=\"{}\" transform=\"{m}\" preserveAspectRatio=\"none\"{a} xlink:href=\"{}\"/>",
                    im.width,
                    im.height,
                    xml_escape(&href)
                ));
            }
            NodeKind::SymbolInstance { symbol, xf } => {
                let Some(sym) = self.doc.symbols.iter().find(|s| s.name == *symbol) else { return };
                let saved = self.xf;
                self.xf = saved * *xf;
                let id = self.id_attr(n);
                let a = self.attrs(&Self::node_props(n));
                self.line(&format!("<g{id}{a}>"));
                self.depth += 1;
                // The symbol's art is the instance's own picture, outside any knockout around it.
                let knockout = std::mem::take(&mut self.knockout);
                self.node(&sym.art.clone());
                self.knockout = knockout;
                self.depth -= 1;
                self.line("</g>");
                self.xf = saved;
            }
            // Live blends/envelopes/meshes export their evaluated (expanded) form.
            NodeKind::Blend { .. } | NodeKind::Envelope { .. } | NodeKind::Mesh(_) | NodeKind::Repeat(_) => {
                let g = vectorcraft_doc::live::expand_deep(n, None);
                self.node_body(&g);
            }
        }
    }

    /// Raster effects (drop shadow, glows, Gaussian blur, feather) as SVG filters: one `<g filter>`
    /// per effect, the first effect innermost (the reference app's stacking order).
    fn filtered(&mut self, n: &Node) {
        let mut inner = n.clone();
        inner.appearance.effects.retain(|e| !vectorcraft_effects::is_raster(&e.id));
        let opened = self.open_filters(n, &n.appearance.effects);
        self.node_body(&inner);
        self.close_filters(opened);
    }

    /// Open one `<g filter>` per visible raster effect of `effects` (an object's, or one fill or
    /// stroke's) on `n`, outermost last effect; returns how many to close.
    fn open_filters(&mut self, n: &Node, effects: &[vectorcraft_doc::Effect]) -> usize {
        use vectorcraft_effects::RasterFx;
        let fx = vectorcraft_effects::raster_effects(effects);
        let reach: f64 = fx.iter().map(RasterFx::outset).sum::<f64>() + 2.0;
        let region = n.visual_bounds().map(|b| self.xf.transform_rect_bbox(b).inflate(reach, reach));
        for f in fx.iter().rev() {
            let fid = self.fresh_id("filter");
            let region_attr = match region {
                Some(r) => format!(
                    " filterUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
                    self.num(r.x0),
                    self.num(r.y0),
                    self.num(r.width()),
                    self.num(r.height())
                ),
                None => String::new(),
            };
            let sd = |blur: f64| fmt_num((blur / 2.0).max(0.0), 3);
            let flood = |c: &vectorcraft_color::Color, o: f32| {
                format!("<feFlood flood-color=\"{}\" flood-opacity=\"{}\"/>", c.to_hex(), fmt_num(o as f64, 3))
            };
            let body = match f {
                RasterFx::DropShadow { opacity, dx, dy, blur, color, .. } => format!(
                    "<feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"{}\"/><feOffset dx=\"{}\" dy=\"{}\" result=\"shadow\"/>{}<feComposite in2=\"shadow\" operator=\"in\" result=\"paint\"/><feMerge><feMergeNode in=\"paint\"/><feMergeNode in=\"SourceGraphic\"/></feMerge>",
                    sd(*blur),
                    self.num(*dx),
                    self.num(*dy),
                    flood(color, *opacity)
                ),
                RasterFx::OuterGlow { opacity, blur, color, .. } => format!(
                    "<feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"{}\" result=\"glow\"/>{}<feComposite in2=\"glow\" operator=\"in\" result=\"paint\"/><feMerge><feMergeNode in=\"paint\"/><feMergeNode in=\"SourceGraphic\"/></feMerge>",
                    sd(*blur),
                    flood(color, *opacity)
                ),
                RasterFx::InnerGlow { opacity, blur, color, center, .. } => {
                    // Edge: the blurred inverse silhouette; Center: the blurred silhouette; both clipped to the shape.
                    let src = if *center {
                        "<feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"SD\" result=\"glow\"/>".replace("SD", &sd(*blur))
                    } else {
                        format!(
                            "<feComponentTransfer in=\"SourceAlpha\"><feFuncA type=\"table\" tableValues=\"1 0\"/></feComponentTransfer><feGaussianBlur stdDeviation=\"{}\" result=\"glow\"/>",
                            sd(*blur)
                        )
                    };
                    format!(
                        "{src}{}<feComposite in2=\"glow\" operator=\"in\"/><feComposite in2=\"SourceAlpha\" operator=\"in\" result=\"paint\"/><feMerge><feMergeNode in=\"SourceGraphic\"/><feMergeNode in=\"paint\"/></feMerge>",
                        flood(color, *opacity)
                    )
                }
                RasterFx::Feather { radius } => format!(
                    "<feGaussianBlur in=\"SourceAlpha\" stdDeviation=\"{}\" result=\"soft\"/><feComposite in=\"SourceGraphic\" in2=\"soft\" operator=\"in\" result=\"f\"/><feComposite in=\"f\" in2=\"SourceAlpha\" operator=\"in\"/>",
                    sd(*radius)
                ),
                RasterFx::GaussianBlur { radius } => format!("<feGaussianBlur in=\"SourceGraphic\" stdDeviation=\"{}\"/>", sd(*radius)),
            };
            self.def(1, &format!("<filter id=\"{fid}\"{region_attr} color-interpolation-filters=\"sRGB\">{body}</filter>"));
            self.line(&format!("<g filter=\"url(#{fid})\">"));
            self.depth += 1;
        }
        fx.len()
    }

    fn close_filters(&mut self, opened: usize) {
        for _ in 0..opened {
            self.depth -= 1;
            self.line("</g>");
        }
    }

    fn char_props(&mut self, st: &CharStyle) -> Props {
        let mut p = Props::new();
        let fam =
            if st.font_family.contains(|c: char| c.is_whitespace() || c == ',') { format!("'{}'", st.font_family) } else { st.font_family.clone() };
        p.push(("font-family", fam));
        // Vertical scale is the font size; horizontal scale stretches it (`textLength`).
        p.push(("font-size", self.num(st.size * st.v_scale / 100.0)));
        let fs = st.font_style.to_ascii_lowercase();
        if fs.contains("bold") || fs.contains("black") || fs.contains("heavy") {
            p.push(("font-weight", "bold".into()));
        }
        if fs.contains("italic") || fs.contains("oblique") {
            p.push(("font-style", "italic".into()));
        }
        let fill = match &st.fill {
            Paint::Gradient(g) => g.gradient.stops.first().map(|s| s.color.to_hex()).unwrap_or_else(|| "none".into()),
            p => self.paint(p, None),
        };
        p.push(("fill", fill));
        if st.has_stroke() {
            // Weight, cap, join, miter limit and dashes as for object strokes (a gradient: its
            // first colour, as for the fill).
            let mut layer = st.stroke_layer();
            if let Paint::Gradient(g) = &layer.paint {
                layer.paint = g.gradient.stops.first().map_or(Paint::None, |s| Paint::solid(s.color));
            }
            self.stroke_props(&layer, layer.width, None, &mut p);
        }
        // Tracking and manual kerning both add space after every character.
        let spacing = st.tracking + st.kerning.unwrap_or(0.0);
        if spacing != 0.0 {
            p.push(("letter-spacing", self.num(spacing / 1000.0 * st.size)));
        }
        if st.kerning.is_some() {
            // Manual kerning replaces the font's pair kerning.
            p.push(("font-kerning", "none".into()));
        }
        if !st.features.is_empty() {
            let v: Vec<String> = st
                .features
                .iter()
                .map(|t| match t.strip_prefix('-') {
                    Some(off) => format!("\"{off}\" 0"),
                    None => format!("\"{t}\" 1"),
                })
                .collect();
            p.push(("font-feature-settings", v.join(", ")));
        }
        match (st.underline, st.strikethrough) {
            (true, true) => p.push(("text-decoration", "underline line-through".into())),
            (true, false) => p.push(("text-decoration", "underline".into())),
            (false, true) => p.push(("text-decoration", "line-through".into())),
            _ => {}
        }
        p
    }

    /// Type: its characters (live text, or glyph outlines when text is exported as outlines) and,
    /// as the canvas paints them, the object's own fills and strokes on the glyph outlines: those
    /// below the Characters row under the characters, the others over them.
    fn text_node(&mut self, n: &Node, t: &TextObject) {
        let chars = |w: &mut Self, n: &Node| if w.opts.outline_text { w.text_outlines(n, t) } else { w.text(n, t) };
        if !n.appearance.items.iter().any(|i| i.visible() && !i.paint().is_none()) {
            return chars(self, n);
        }
        let id = self.id_attr(n);
        let a = self.attrs(&Self::node_props(n));
        self.line(&format!("<g{id}{a}>"));
        self.depth += 1;
        let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t);
        let pd = PathData::from_bezpath(&lay.to_bezpath()).transformed(t.xf);
        let d = self.path_d(&pd, self.xf);
        let tb = Some(t.xf.transform_rect_bbox(lay.bounds));
        let (below, above) = n.appearance.split_contents();
        let glyphs = |w: &mut Self, items: &[AppearanceItem]| {
            if !items.is_empty() {
                let ap = vectorcraft_doc::Appearance { items: items.to_vec(), ..Default::default() };
                w.shape(&Node::path(NodeId(u64::MAX), pd.clone(), ap), &d, &[&pd], FillRule::NonZero, tb);
            }
        };
        glyphs(self, below);
        // The group carries the id and the object's transparency.
        let bare = Node { opacity: 1.0, blend: BlendMode::Normal, isolate: false, ..n.clone() };
        let anonymous = std::mem::replace(&mut self.anonymous, true);
        chars(self, &bare);
        glyphs(self, above);
        self.anonymous = anonymous;
        self.depth -= 1;
        self.line("</g>");
    }

    /// Text as glyph outlines: one compound path per run, painted like the run.
    fn text_outlines(&mut self, n: &Node, t: &TextObject) {
        let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t);
        let mut runs: Vec<(usize, kurbo::BezPath)> = vec![];
        for g in &lay.glyphs {
            match runs.last_mut() {
                Some((r, bp)) if *r == g.run => bp.extend(g.outline.iter()),
                _ => runs.push((g.run, g.outline.clone())),
            }
        }
        let id = self.id_attr(n);
        let a = self.attrs(&Self::node_props(n));
        self.line(&format!("<g{id}{a}>"));
        self.depth += 1;
        for (r, mut bp) in runs {
            let Some(run) = t.runs.get(r) else { continue };
            bp.apply_affine(t.xf);
            let pd = PathData::from_bezpath(&bp);
            // An unnamed id: the pieces carry no id attribute (the group has it).
            let glyphs = Node::path(NodeId(u64::MAX), pd.clone(), run.style.appearance());
            let d = self.path_d(&pd, self.xf);
            self.shape(&glyphs, &d, &[&pd], FillRule::NonZero, pd.bounds());
        }
        self.depth -= 1;
        self.line("</g>");
    }

    /// Point and area type, every laid-out line at its own position: wraps, indents, tabs and
    /// spacing come out as on the canvas. Centred and right-aligned lines are anchored
    /// (`text-anchor`) at their centre or right end, so they stay aligned in a viewer whose font
    /// differs; other lines are placed where each style run (and, justified, each word) starts.
    fn text(&mut self, n: &Node, t: &TextObject) {
        if let TextKind::OnPath { path, start } = &t.kind {
            return self.text_on_path(n, t, path, *start);
        }
        let lay = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t);
        let lines = text_lines(t, &lay, self.opts.fewer_tspans);
        // A tab starts a new chunk at its stop: only lines without tabs can be anchored.
        let anchor = match t.para.justify {
            Justify::Center => Some(("middle", 0.5)),
            Justify::Right => Some(("end", 1.0)),
            _ => None,
        }
        .filter(|_| !lines.iter().flatten().any(|s| s.brk));
        let base = self.char_props(&t.first_style());
        let mut props = base.clone();
        if let Some((a, _)) = anchor {
            props.push(("text-anchor", a.into()));
        }
        props.extend(Self::node_props(n));
        let id = self.id_attr(n);
        let a = self.attrs(&props);
        let m = self.xf * t.xf;
        let tr = if m == Affine::IDENTITY { String::new() } else { format!(" transform=\"{}\"", self.matrix(m)) };
        let starts: Vec<Option<(f64, f64)>> = lines.iter().map(|l| line_start(l, anchor.map(|a| a.1))).collect();
        // The `<text>` carries the first line's position too, so readers that place text by its
        // own x/y start where the first line does.
        let at = match starts.iter().flatten().next() {
            Some((x, y)) => format!(" x=\"{}\" y=\"{}\"", self.num(*x), self.num(*y)),
            None => String::new(),
        };
        let mut s = format!("<text{id}{tr}{at} xml:space=\"preserve\"{a}>");
        for (line, start) in lines.iter().zip(starts) {
            if let Some(start) = start {
                self.text_line(&mut s, t, line, start, anchor.is_some(), &base);
            }
        }
        s.push_str("</text>");
        self.line(&s);
    }

    /// One line of laid-out text starting at `start`. Default: a positioned `<tspan>` per segment
    /// (only the first of an `anchored` line). Fewer tspans: one positioned `<tspan>` per line with
    /// the style changes nested in it, positioned again only after tabs and justified word spaces.
    /// Baseline shifts are relative (`dy`), undone by the next segment that isn't shifted.
    fn text_line(&mut self, s: &mut String, t: &TextObject, line: &[Segment], start: (f64, f64), anchored: bool, base: &Props) {
        let fewer = self.opts.fewer_tspans;
        if fewer {
            s.push_str(&format!("<tspan x=\"{}\" y=\"{}\">", self.num(start.0), self.num(start.1)));
        }
        let mut place = !fewer;
        let mut first = true;
        // The baseline shift the current text position carries.
        let mut shift = 0.0;
        for seg in line {
            let Some(st) = t.runs.get(seg.run).map(|r| &r.style) else { continue };
            let mut attrs = String::new();
            if place {
                let (x, y) = if first { start } else { (seg.x, seg.y) };
                attrs.push_str(&format!(" x=\"{}\" y=\"{}\"", self.num(x), self.num(y)));
                shift = 0.0;
            }
            first = false;
            if st.baseline_shift != shift {
                attrs.push_str(&format!(" dy=\"{}\"", self.num(shift - st.baseline_shift)));
                shift = st.baseline_shift;
            }
            // After a tab or a justified word space the next segment is placed.
            place = seg.brk || (!fewer && !anchored);
            if (st.h_scale - st.v_scale).abs() > 1e-9 {
                attrs.push_str(&format!(" textLength=\"{}\" lengthAdjust=\"spacingAndGlyphs\"", self.num(seg.advance)));
            }
            if st.rotation != 0.0 {
                attrs.push_str(&format!(" rotate=\"{}\"", self.num(-st.rotation)));
            }
            let diff = run_diff(base, self.char_props(st));
            attrs.push_str(&self.attrs(&diff));
            let text = xml_escape(&seg.text);
            if attrs.is_empty() && fewer {
                s.push_str(&text);
            } else {
                s.push_str(&format!("<tspan{attrs}>{text}</tspan>"));
            }
        }
        if fewer {
            s.push_str("</tspan>");
        }
    }

    /// Type on a path: a `<textPath>` along the path (a def).
    fn text_on_path(&mut self, n: &Node, t: &TextObject, path: &PathData, start: f64) {
        let id = self.id_attr(n);
        let pid = self.fresh_id("text-path");
        let d = self.path_d(path, self.xf * t.xf);
        self.def(1, &format!("<path id=\"{pid}\" d=\"{d}\"/>"));
        let base = self.char_props(&t.first_style());
        let mut props = base.clone();
        match t.para.justify {
            Justify::Center | Justify::JustifyCenter => props.push(("text-anchor", "middle".into())),
            Justify::Right | Justify::JustifyRight => props.push(("text-anchor", "end".into())),
            _ => {}
        }
        props.extend(Self::node_props(n));
        let a = self.attrs(&props);
        let offset = fmt_num(start * 100.0, 3);
        let mut s = format!("<text{id} xml:space=\"preserve\"{a}><textPath xlink:href=\"#{pid}\" startOffset=\"{offset}%\">");
        for run in &t.runs {
            let diff = run_diff(&base, self.char_props(&run.style));
            let a = self.attrs(&diff);
            s.push_str(&format!("<tspan{a}>{}</tspan>", xml_escape(&run.text.replace('\n', " "))));
        }
        s.push_str("</textPath></text>");
        self.line(&s);
    }
}

/// What the `<tspan>` of a run whose properties are `run` sets over its `<text>`'s `base`: the
/// properties that differ, and the initial value of each one `base` sets and the run doesn't.
fn run_diff(base: &Props, run: Props) -> Props {
    let reset: Props =
        base.iter().filter(|(k, _)| !run.iter().any(|(r, _)| r == k)).filter_map(|(k, _)| initial_value(k).map(|v| (*k, v.to_string()))).collect();
    run.into_iter().filter(|kv| !base.contains(kv)).chain(reset).collect()
}

/// The initial value of an inherited property a run may leave unset (decorations can't be undone
/// on a `<tspan>`).
fn initial_value(k: &str) -> Option<&'static str> {
    Some(match k {
        "font-weight" | "font-style" | "font-feature-settings" => "normal",
        "stroke" | "stroke-dasharray" => "none",
        "stroke-width" | "stroke-opacity" => "1",
        "stroke-linecap" => "butt",
        "stroke-linejoin" => "miter",
        "stroke-miterlimit" => "4",
        "stroke-dashoffset" | "letter-spacing" => "0",
        "font-kerning" => "auto",
        _ => return None,
    })
}

/// Any visible raster effect (shadow, glow, blur, feather) in `effects`?
fn has_raster(effects: &[vectorcraft_doc::Effect]) -> bool {
    effects.iter().any(|e| e.visible && vectorcraft_effects::is_raster(&e.id))
}

/// Characters of one style on one line, written as one `<tspan>`.
struct Segment {
    /// Index into `TextObject::runs`.
    run: usize,
    /// Where the segment starts (text space; `y` is the baseline).
    x: f64,
    y: f64,
    text: String,
    /// The laid-out width (tracking and justification included).
    advance: f64,
    /// The next segment needs its own position (after a tab, or a word space of a justified line).
    brk: bool,
}

/// The laid-out lines of `t` as segments: a new segment at every style change, after tabs and,
/// on justified lines, after each word space (unless `fewer`). The text comes from the clusters
/// the layout placed: soft hyphens draw nothing, a line broken by hyphenation ends in `-`, all
/// caps are upper case.
fn text_lines(t: &TextObject, lay: &vectorcraft_text::TextLayout, fewer: bool) -> Vec<Vec<Segment>> {
    let plain = t.plain_text();
    let split_words = !fewer && !matches!(t.para.justify, Justify::Left | Justify::Center | Justify::Right);
    let mut lines = Vec::with_capacity(lay.lines.len());
    for line in &lay.lines {
        let mut segs: Vec<Segment> = Vec::new();
        let mut cluster = None;
        let source = |g: &vectorcraft_text::PositionedGlyph| if g.len == 0 { "-" } else { plain.get(g.byte..g.byte + g.len).unwrap_or("") };
        let mut glyphs = lay.glyphs.get(line.glyph_start..line.glyph_end).unwrap_or_default();
        // Spaces where the line wrapped (it doesn't end its paragraph) don't belong to the line.
        if plain.get(line.end..).is_some_and(|rest| !rest.is_empty() && !rest.starts_with('\n')) {
            while let Some((_, rest)) = glyphs.split_last().filter(|(g, _)| source(g).chars().all(|c| c.is_whitespace() && c != '\t')) {
                glyphs = rest;
            }
        }
        for g in glyphs {
            let Some(run) = t.runs.get(g.run) else { continue };
            if g.len > 0 && cluster == Some(g.byte) {
                // Another glyph of the same cluster.
                if let Some(last) = segs.last_mut() {
                    last.advance += g.advance;
                }
                continue;
            }
            cluster = Some(g.byte);
            let src: String = source(g).chars().filter(|c| *c != '\u{ad}').collect();
            let piece = if run.style.all_caps { src.to_uppercase() } else { src };
            let brk = piece == "\t" || (split_words && !piece.is_empty() && piece.chars().all(char::is_whitespace));
            match segs.last_mut() {
                Some(last) if last.run == g.run && !last.brk => {
                    last.text.push_str(&piece);
                    last.advance += g.advance;
                    last.brk = brk;
                }
                _ => segs.push(Segment { run: g.run, x: g.origin.x, y: g.origin.y, text: piece, advance: g.advance, brk }),
            }
        }
        segs.retain(|s| !s.text.is_empty());
        if !segs.is_empty() {
            lines.push(segs);
        }
    }
    lines
}

/// Where a line of segments starts: its first segment's origin or, `anchor`ed (0.5: centre, 1:
/// right end), that point of its width. `None` for an empty line.
fn line_start(line: &[Segment], anchor: Option<f64>) -> Option<(f64, f64)> {
    let (first, last) = (line.first()?, line.last()?);
    let x = match anchor {
        Some(f) => first.x + f * (last.x + last.advance - first.x),
        None => first.x,
    };
    Some((x, first.y))
}
