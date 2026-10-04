//! Document → PDF (krilla). Mirrors the tree walk of `vectorcraft-render`.

use std::collections::HashMap;

use krilla::color::{cmyk, luma, rgb};
use krilla::configure::{Archival, ConfigurationBuilder, PdfVersion};
use krilla::geom::{Path, PathBuilder, Size, Transform};
use krilla::image::Image;
use krilla::metadata::Metadata;
use krilla::num::NormalizedF32;
use krilla::page::PageSettings;
use krilla::paint::{Fill, LinearGradient, RadialGradient, SpreadMethod, Stop, Stroke, StrokeDash};
use krilla::surface::Surface;
use kurbo::{PathEl, Shape, Vec2};
use vectorcraft_color::{BlendMode, Color, GradientKind, Paint};
use vectorcraft_doc::appearance::stroke_paint_bounds;
use vectorcraft_doc::{AppearanceItem, Document, LineCap, LineJoin, Node, NodeKind, StrokeAlign, StrokeLayer, TextObject};
use vectorcraft_effects::stroke::{self, WrittenShape};
use vectorcraft_geom::{Affine, BezPath, FillRule, Rect};

use crate::{Compatibility, ExportReport, PdfError, PdfOptions};

/// Export `doc` as PDF bytes: one page per artboard (or the artboards chosen in `opts`).
pub fn export(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    export_with_report(doc, opts).map(|r| r.bytes)
}

/// Like [`export`], also returning warnings about approximated or dropped features.
pub fn export_with_report(doc: &Document, opts: &PdfOptions) -> Result<ExportReport, PdfError> {
    // Live geometry effects export as their result; raster effects are reported below.
    let baked = vectorcraft_effects::bake_document(doc);
    let doc = baked.as_ref().unwrap_or(doc);
    if doc.artboards.is_empty() {
        return Err(PdfError::NoArtboards);
    }
    let indices: Vec<usize> = match &opts.artboards {
        Some(v) => v.clone(),
        None => (0..doc.artboards.len()).collect(),
    };
    if indices.is_empty() {
        return Err(PdfError::NoArtboards);
    }
    if let Some(bad) = indices.iter().find(|i| **i >= doc.artboards.len()) {
        return Err(PdfError::BadArtboard(*bad));
    }

    let (version, archival) = match opts.compatibility {
        Compatibility::Pdf14 => (PdfVersion::Pdf14, None),
        Compatibility::Pdf15 => (PdfVersion::Pdf15, None),
        Compatibility::Pdf16 => (PdfVersion::Pdf16, None),
        Compatibility::Pdf17 => (PdfVersion::Pdf17, None),
        Compatibility::Pdf20 => (PdfVersion::Pdf20, None),
        Compatibility::PdfA2b => (PdfVersion::Pdf17, Some(Archival::A2_B)),
        Compatibility::PdfX4 => return Err(PdfError::Unsupported("PDF/X-4 output is not supported by the PDF writer yet".into())),
    };
    let mut cb = ConfigurationBuilder::new().with_version(version);
    if let Some(a) = archival {
        cb = cb.with_archival_validator(a);
    }
    let configuration = cb.finish().map_err(|e| PdfError::Write(format!("{e:?}")))?;
    let settings = krilla::SerializeSettings { compress_content_streams: opts.compress, configuration, ..Default::default() };

    let mut pdf = krilla::Document::new_with(settings);
    let title = opts.title.clone().unwrap_or_else(|| doc.title.clone());
    let mut meta = Metadata::new().creator("VectorCraft".into()).producer("VectorCraft".into());
    if !title.is_empty() {
        meta = meta.title(title);
    }
    if let Some(t) = opts.created.or_else(now_unix) {
        meta = meta.creation_date(date_time(t));
    }
    pdf.set_metadata(meta);

    let mut ex = Exporter { doc, warnings: vec![], images: HashMap::new(), brushes: None, knockout: doc.page_knockout };
    for i in indices {
        let ab = &doc.artboards[i];
        let r = ab.rect;
        let size = Size::from_wh(r.width().max(1.0) as f32, r.height().max(1.0) as f32).ok_or(PdfError::BadArtboard(i))?;
        let mut page = pdf.start_page_with(PageSettings::new(size));
        let mut s = page.surface();
        // krilla's page space is y-down with the origin at the top-left corner, like ours.
        s.push_transform(&xf(Affine::translate((-r.x0, -r.y0))));
        // Page Isolated Blending / Page Knockout Group: the page content is one group (the PDF
        // writer has no page group attributes).
        let page_group = doc.page_isolate || doc.page_knockout;
        if page_group {
            s.push_isolated();
        }
        ex.children(&mut s, &doc.layers, r);
        if page_group {
            s.pop();
        }
        s.pop();
        s.finish();
        page.finish();
    }
    let bytes = pdf.finish().map_err(|e| PdfError::Write(format!("{e:?}")))?;
    let mut warnings = ex.warnings;
    warnings.dedup();
    Ok(ExportReport { bytes, warnings })
}

#[cfg(not(target_arch = "wasm32"))]
fn now_unix() -> Option<i64> {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).ok().map(|d| d.as_secs() as i64)
}
#[cfg(target_arch = "wasm32")]
fn now_unix() -> Option<i64> {
    None
}

/// Unix seconds (UTC) → krilla date (civil-from-days, proleptic Gregorian).
fn date_time(t: i64) -> krilla::metadata::DateTime {
    let days = t.div_euclid(86_400);
    let secs = t.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    krilla::metadata::DateTime::new(year.clamp(0, 9999) as u16)
        .month(month as u8)
        .day(day as u8)
        .hour((secs / 3600) as u8)
        .minute((secs / 60 % 60) as u8)
        .second((secs % 60) as u8)
        .utc_offset_hour(0)
        .utc_offset_minute(0)
}

struct Exporter<'a> {
    doc: &'a Document,
    warnings: Vec<String>,
    images: HashMap<String, Option<Image>>,
    /// The brush library, parsed when the first brushed stroke is written.
    brushes: Option<Vec<vectorcraft_brush::Brush>>,
    /// Whether the group being written is a knockout group (what its neutral children inherit).
    knockout: bool,
}

fn xf(a: Affine) -> Transform {
    let c = a.as_coeffs();
    Transform::from_row(c[0] as f32, c[1] as f32, c[2] as f32, c[3] as f32, c[4] as f32, c[5] as f32)
}

fn to_path(bp: &BezPath) -> Option<Path> {
    let mut pb = PathBuilder::new();
    for el in bp.elements() {
        match *el {
            PathEl::MoveTo(p) => pb.move_to(p.x as f32, p.y as f32),
            PathEl::LineTo(p) => pb.line_to(p.x as f32, p.y as f32),
            PathEl::QuadTo(a, p) => pb.quad_to(a.x as f32, a.y as f32, p.x as f32, p.y as f32),
            PathEl::CurveTo(a, b, p) => pb.cubic_to(a.x as f32, a.y as f32, b.x as f32, b.y as f32, p.x as f32, p.y as f32),
            PathEl::ClosePath => pb.close(),
        }
    }
    pb.finish()
}

fn norm(v: f32) -> NormalizedF32 {
    NormalizedF32::new(if v.is_finite() { v.clamp(0.0, 1.0) } else { 1.0 }).unwrap_or(NormalizedF32::ONE)
}

fn rule(r: FillRule) -> krilla::paint::FillRule {
    match r {
        FillRule::NonZero => krilla::paint::FillRule::NonZero,
        FillRule::EvenOdd => krilla::paint::FillRule::EvenOdd,
    }
}

fn blend(b: BlendMode) -> krilla::blend::BlendMode {
    use krilla::blend::BlendMode as K;
    match b {
        BlendMode::Normal => K::Normal,
        BlendMode::Darken => K::Darken,
        BlendMode::Multiply => K::Multiply,
        BlendMode::ColorBurn => K::ColorBurn,
        BlendMode::Lighten => K::Lighten,
        BlendMode::Screen => K::Screen,
        BlendMode::ColorDodge => K::ColorDodge,
        BlendMode::Overlay => K::Overlay,
        BlendMode::SoftLight => K::SoftLight,
        BlendMode::HardLight => K::HardLight,
        BlendMode::Difference => K::Difference,
        BlendMode::Exclusion => K::Exclusion,
        BlendMode::Hue => K::Hue,
        BlendMode::Saturation => K::Saturation,
        BlendMode::Color => K::Color,
        BlendMode::Luminosity => K::Luminosity,
    }
}

fn q(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn color(c: &Color) -> krilla::color::Color {
    match *c {
        Color::Rgb { r, g, b } => rgb::Color::new(q(r), q(g), q(b)).into(),
        Color::Cmyk { c, m, y, k } => cmyk::Color::new(q(c), q(m), q(y), q(k)).into(),
        // VectorCraft grey is ink coverage (0 = white); PDF DeviceGray is lightness.
        Color::Gray { k } => luma::Color::new(q(1.0 - k)).into(),
    }
}

/// Fill the page (with a margin) with a grey level (0 = black, 255 = white) at `opacity`: mask
/// backdrops.
fn cover(s: &mut Surface, page: Rect, level: u8, opacity: f32) {
    let Some(p) = to_path(&page.inflate(1.0, 1.0).to_path(0.1)) else { return };
    s.set_stroke(None);
    s.set_fill(Some(Fill { paint: rgb::Color::new(level, level, level).into(), opacity: norm(opacity), rule: krilla::paint::FillRule::NonZero }));
    s.draw_path(&p);
}

/// An alpha mask of constant `alpha` over the page. A group drawn through it is a transparency
/// group of that opacity that, unlike the writer's opacity groups, is not isolated.
fn constant_mask(s: &mut Surface, page: Rect, alpha: f32) -> krilla::mask::Mask {
    let mut sb = s.stream_builder();
    let mut ms = sb.surface();
    cover(&mut ms, page, 0, alpha);
    ms.finish();
    krilla::mask::Mask::new(sb.finish(), krilla::mask::MaskType::Alpha)
}

fn rects_overlap(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
}

impl Exporter<'_> {
    /// A colour for the page: in CMYK documents RGB colours are separated into DeviceCMYK through
    /// the active colour settings, so the file carries press values.
    fn col(&mut self, c: &Color) -> krilla::color::Color {
        if self.doc.color_mode == vectorcraft_doc::ColorMode::Cmyk && matches!(c, Color::Rgb { .. }) {
            let cms = vectorcraft_color::cms::active();
            let [cc, m, y, k] = cms.to_cmyk(c, cms.settings().intent);
            return cmyk::Color::new(q(cc), q(m), q(y), q(k)).into();
        }
        color(c)
    }

    /// A solid paint, as a Separation colour space when it's linked to a spot swatch (the
    /// swatch's CMYK equivalent is the alternate space).
    fn solid(&mut self, c: &Color, swatch: Option<&str>) -> krilla::color::Color {
        use krilla::color::separation::{Color as SepColor, SeparationColorant, SeparationSpace};
        let spot = swatch.and_then(|n| self.doc.swatch(n).filter(|s| s.spot)).and_then(|s| s.paint.color().map(|sc| (s.name.clone(), sc)));
        let Some((name, sc)) = spot else { return self.col(c) };
        let cms = vectorcraft_color::cms::active();
        let intent = cms.settings().intent;
        let full = cms.to_cmyk(&sc, intent);
        let total: f32 = full.iter().sum();
        let tint = if total <= 1e-4 { 1.0 } else { (cms.to_cmyk(c, intent).iter().sum::<f32>() / total).clamp(0.0, 1.0) };
        let alt = krilla::color::RegularColor::Cmyk(cmyk::Color::new(q(full[0]), q(full[1]), q(full[2]), q(full[3])));
        SepColor::new(q(tint), SeparationSpace::new(SeparationColorant::Custom(name), alt)).into()
    }

    /// Warn when `effects` (an object's, a fill's or a stroke's) has a visible raster effect.
    fn warn_raster(&mut self, effects: &[vectorcraft_doc::Effect]) {
        if effects.iter().any(|e| e.visible && vectorcraft_effects::is_raster(&e.id)) {
            self.warn("raster effects (shadows, glows, blur, feather) are not exported to PDF yet");
        }
    }

    fn warn(&mut self, w: impl Into<String>) {
        let w = w.into();
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    /// Convert a paint. `bounds` resolves unset gradient geometry (like the renderer).
    fn paint(&mut self, p: &Paint, bounds: Rect) -> Option<krilla::paint::Paint> {
        match p {
            Paint::None => None,
            Paint::Solid { color: c, swatch } => Some(self.solid(c, swatch.as_deref()).into()),
            Paint::Gradient(g) => {
                let geom = g.resolve(bounds);
                let mut stops: Vec<Stop> = Vec::new();
                let mut last = 0.0f32;
                for (o, c, a) in g.gradient.expanded_stops() {
                    let o = o.clamp(last, 1.0);
                    last = o;
                    stops.push(Stop { offset: norm(o), color: self.col(&c), opacity: norm(a) });
                }
                if stops.is_empty() {
                    return None;
                }
                match g.gradient.kind {
                    GradientKind::Linear => {
                        let (s, mut e) = (geom.start, geom.end);
                        if s.distance(e) < 1e-9 {
                            e = s + Vec2::new(1.0, 0.0);
                        }
                        Some(
                            LinearGradient {
                                x1: s.x as f32,
                                y1: s.y as f32,
                                x2: e.x as f32,
                                y2: e.y as f32,
                                transform: Transform::identity(),
                                spread_method: SpreadMethod::Pad,
                                stops,
                                anti_alias: false,
                            }
                            .into(),
                        )
                    }
                    GradientKind::Radial => {
                        let r = geom.length().max(1e-6) as f32;
                        let angle = (geom.end - geom.start).atan2();
                        let t = Affine::translate(geom.start.to_vec2())
                            * Affine::rotate(angle)
                            * Affine::scale_non_uniform(1.0, geom.aspect.max(1e-3))
                            * Affine::rotate(-angle)
                            * Affine::translate(-geom.start.to_vec2());
                        let (cx, cy) = (geom.start.x as f32, geom.start.y as f32);
                        Some(
                            RadialGradient {
                                fx: cx,
                                fy: cy,
                                fr: 0.0,
                                cx,
                                cy,
                                cr: r,
                                transform: xf(t),
                                spread_method: SpreadMethod::Pad,
                                stops,
                                anti_alias: false,
                            }
                            .into(),
                        )
                    }
                    GradientKind::Freeform => {
                        self.warn("freeform gradients are exported as their average colour");
                        let n = g.gradient.stops.len().max(1) as f32;
                        let mut acc = [0.0f32; 3];
                        for s in &g.gradient.stops {
                            let c = s.color.to_rgb();
                            for i in 0..3 {
                                acc[i] += c[i] / n;
                            }
                        }
                        Some(color(&Color::rgb(acc[0], acc[1], acc[2])).into())
                    }
                }
            }
            Paint::Pattern { .. } => {
                self.warn("pattern strokes (and missing patterns) are exported as mid-grey");
                Some(rgb::Color::new(128, 128, 128).into())
            }
        }
    }

    /// Opacity mask → luminosity soft mask. Outside the art the backdrop is black (clip) or white;
    /// invert is a white Difference rect on top (luminance is linear, so luma(1 − c) = 1 − luma(c)).
    fn soft_mask(&mut self, s: &mut Surface, m: &vectorcraft_doc::OpacityMask, page: Rect) -> krilla::mask::Mask {
        let mut sb = s.stream_builder();
        let mut ms = sb.surface();
        // An opaque backdrop when it matters: white outside the art (no clip), or black for the
        // inverting Difference pass below to turn white.
        if !m.clip || m.invert {
            cover(&mut ms, page, if m.clip { 0 } else { 255 }, 1.0);
        }
        // Mask art is a picture of its own: it takes no part in a knockout group around the object.
        let knockout = std::mem::take(&mut self.knockout);
        self.node(&mut ms, &m.art, page, true);
        self.knockout = knockout;
        if m.invert {
            ms.push_blend_mode(krilla::blend::BlendMode::Difference);
            cover(&mut ms, page, 255, 1.0);
            ms.pop();
        }
        ms.finish();
        krilla::mask::Mask::new(sb.finish(), krilla::mask::MaskType::Luminosity)
    }

    /// The children of a group, as the elements of a knockout group when it is one.
    fn children(&mut self, s: &mut Surface, children: &[std::sync::Arc<Node>], page: Rect) {
        if !self.knockout {
            for c in children {
                self.node(s, c, page, false);
            }
            return;
        }
        // The PDF writer has no knockout groups: each element is drawn through a soft mask of
        // where the elements above it don't paint (the same look for Normal blending). The masks
        // nest, so element i sits inside the masks of elements i+1…n: open them outermost first.
        self.warn("knockout groups are written as soft-masked groups (same look, but not editable as knockout groups)");
        let elements: Vec<_> = Node::knockout_elements(children)
            .into_iter()
            .filter(|c| c.visual_bounds().is_some_and(|b| rects_overlap(b.inflate(1.0, 1.0), page)))
            .collect();
        let Some((first, rest)) = elements.split_first() else { return };
        for c in rest.iter().rev() {
            let mask = self.knockout_mask(s, c, page);
            s.push_mask(mask);
        }
        self.node(s, first, page, false);
        for c in rest {
            s.pop();
            self.node(s, c, page, false);
        }
    }

    /// A luminosity mask that is 1 − the knockout shape of `c`: white, then black through an alpha
    /// mask of `c` (at full object opacity without its own mask, unless those define its shape).
    fn knockout_mask(&mut self, s: &mut Surface, c: &Node, page: Rect) -> krilla::mask::Mask {
        let shape = if c.knockout_shape { c.clone() } else { Node { opacity: 1.0, mask: None, ..c.clone() } };
        let mut sb = s.stream_builder();
        let mut ms = sb.surface();
        cover(&mut ms, page, 255, 1.0);
        let alpha = {
            let mut ab = ms.stream_builder();
            let mut als = ab.surface();
            self.node(&mut als, &shape, page, false);
            als.finish();
            krilla::mask::Mask::new(ab.finish(), krilla::mask::MaskType::Alpha)
        };
        ms.push_mask(alpha);
        cover(&mut ms, page, 0, 1.0);
        ms.pop();
        ms.finish();
        krilla::mask::Mask::new(sb.finish(), krilla::mask::MaskType::Luminosity)
    }

    fn node(&mut self, s: &mut Surface, n: &Node, page: Rect, force: bool) {
        if !force && !n.visible {
            return;
        }
        if let NodeKind::Layer { template: true, .. } = n.kind {
            return;
        }
        match n.visual_bounds() {
            Some(b) if !force && !rects_overlap(b.inflate(1.0, 1.0), page) => return,
            None if !n.is_container() => return,
            _ => {}
        }
        self.warn_raster(&n.appearance.effects);
        let container = n.is_container() && !matches!(n.kind, NodeKind::Compound { .. });
        // Whether this container's children knock each other out (a knockout group is written as a group).
        let knockout = n.knocks_out(self.knockout);
        let enclosing = std::mem::replace(&mut self.knockout, knockout);
        let mut pushes = 0;
        if n.blend != BlendMode::Normal {
            s.push_blend_mode(blend(n.blend));
            pushes += 1;
        }
        if n.opacity < 1.0 || (container && (n.isolate || n.blend != BlendMode::Normal || knockout)) {
            if !n.isolate && n.blends_through() {
                // Not isolated: blending inside reaches the art below the group.
                let alpha = constant_mask(s, page, n.opacity);
                s.push_mask(alpha);
            } else if n.opacity < 1.0 {
                s.push_opacity(norm(n.opacity));
            } else {
                s.push_isolated();
            }
            pushes += 1;
        }
        if let Some(m) = n.mask.as_deref()
            && !m.disabled
        {
            let mask = self.soft_mask(s, m, page);
            s.push_mask(mask);
            pushes += 1;
        }
        match &n.kind {
            NodeKind::Layer { children, .. } | NodeKind::Group { children, clip: false } => self.children(s, children, page),
            NodeKind::Group { children, clip: true } => {
                // The region every output clips to; nothing to clip by hides the clipped art.
                if let Some((clip, rest)) = children.split_first()
                    && let Some((p, r)) = vectorcraft_effects::clip_outline(clip).and_then(|(bp, r)| to_path(&bp).map(|p| (p, r)))
                {
                    s.push_clip_path(&p, &rule(r));
                    self.children(s, rest, page);
                    s.pop();
                }
            }
            NodeKind::Path { path, rule, guide, .. } => {
                if !*guide {
                    self.shape(s, n, &path.to_bezpath(), *rule, page);
                }
            }
            NodeKind::Compound { children, rule } => {
                let mut bp = BezPath::new();
                for c in children {
                    if let Some(p) = c.path_data() {
                        bp.extend(p.to_bezpath());
                    }
                }
                self.shape(s, n, &bp, *rule, page);
            }
            NodeKind::Text(t) => self.text(s, n, t),
            NodeKind::Image(im) => self.image(s, im),
            NodeKind::SymbolInstance { symbol, xf } => {
                if let Some(sym) = self.doc.symbols.iter().find(|x| &x.name == symbol) {
                    let mut art = (*sym.art).clone();
                    art.transform(*xf, false);
                    self.node(s, &art, page, true);
                }
            }
            // Live blends/envelopes/meshes export their evaluated (expanded) form.
            NodeKind::Blend { .. } | NodeKind::Envelope { .. } | NodeKind::Mesh(_) | NodeKind::Repeat(_) => {
                let g = vectorcraft_doc::live::expand_deep(n, None);
                for c in g.children().into_iter().flatten() {
                    self.node(s, c, page, false);
                }
            }
        }
        self.knockout = enclosing;
        for _ in 0..pushes {
            s.pop();
        }
    }

    fn shape(&mut self, s: &mut Surface, n: &Node, bp: &BezPath, r: FillRule, page: Rect) {
        let Some(path) = to_path(bp) else { return };
        let bounds = bp.bounding_box();
        for item in &n.appearance.items {
            match item {
                AppearanceItem::Fill(fl) => {
                    if !fl.visible || fl.paint.is_none() {
                        continue;
                    }
                    self.warn_raster(&fl.effects);
                    // Pattern fills: the tile instances covering the shape, clipped to it.
                    let doc = self.doc;
                    if let Paint::Pattern { pattern, xf } = &fl.paint
                        && let Some(def) = doc.pattern(pattern)
                    {
                        s.push_clip_path(&path, &rule(r));
                        if fl.opacity < 1.0 {
                            s.push_opacity(norm(fl.opacity));
                        }
                        for inst in def.instances_in(*xf, bounds) {
                            self.node(s, &inst, bounds, true);
                        }
                        if fl.opacity < 1.0 {
                            s.pop();
                        }
                        s.pop();
                        continue;
                    }
                    let Some(paint) = self.paint(&fl.paint, bounds) else { continue };
                    let bl = fl.blend != BlendMode::Normal;
                    if bl {
                        s.push_blend_mode(blend(fl.blend));
                    }
                    s.set_stroke(None);
                    s.set_fill(Some(Fill { paint, opacity: norm(fl.opacity), rule: rule(r) }));
                    s.draw_path(&path);
                    if bl {
                        s.pop();
                    }
                }
                AppearanceItem::Stroke(st) => {
                    if !st.visible || st.paint.is_none() || st.width <= 0.0 {
                        continue;
                    }
                    self.warn_raster(&st.effects);
                    self.stroke(s, bp, &path, r, st, page);
                }
            }
        }
        s.set_fill(None);
        s.set_stroke(None);
    }

    fn stroke(&mut self, s: &mut Surface, bp: &BezPath, path: &Path, r: FillRule, st: &StrokeLayer, page: Rect) {
        if !stroke::is_plain(st) && self.brush_art(s, bp, st, page) {
            return;
        }
        let bounds = bp.bounding_box();
        let Some(paint) = self.paint(&st.paint, st.paint_bounds(bounds)) else { return };
        let w = stroke::for_writer(bp, st);
        let mut pushes = 0;
        if st.blend != BlendMode::Normal {
            s.push_blend_mode(blend(st.blend));
            pushes += 1;
        }
        match w.side {
            Some(StrokeAlign::Inside) => {
                s.push_clip_path(path, &rule(r));
                pushes += 1;
            }
            Some(StrokeAlign::Outside) => {
                // Clip to everything outside the path: a frame around all the stroke reaches
                // (miter spikes included) plus the path, even-odd.
                let mut outside = w.reach(st, bounds).inflate(1.0, 1.0).to_path(0.1);
                outside.extend(bp.iter());
                if let Some(p) = to_path(&outside) {
                    s.push_clip_path(&p, &krilla::paint::FillRule::EvenOdd);
                    pushes += 1;
                }
            }
            _ => {}
        }
        match &w.shape {
            WrittenShape::Stroke { width } => {
                let dash = st.dash.as_ref().filter(|d| d.is_dashed()).map(|d| {
                    let mut pat: Vec<f32> = d.pattern.iter().map(|v| *v as f32).collect();
                    if pat.len() % 2 == 1 {
                        pat.extend(pat.clone());
                    }
                    StrokeDash { array: pat, offset: d.offset as f32 }
                });
                s.set_fill(None);
                s.set_stroke(Some(Stroke {
                    paint,
                    width: *width as f32,
                    miter_limit: st.miter_limit.max(1.0) as f32,
                    line_cap: match st.cap {
                        LineCap::Butt => krilla::paint::LineCap::Butt,
                        LineCap::Round => krilla::paint::LineCap::Round,
                        LineCap::Square => krilla::paint::LineCap::Square,
                    },
                    line_join: match st.join {
                        LineJoin::Miter => krilla::paint::LineJoin::Miter,
                        LineJoin::Round => krilla::paint::LineJoin::Round,
                        LineJoin::Bevel => krilla::paint::LineJoin::Bevel,
                    },
                    opacity: norm(st.opacity),
                    dash,
                }));
                s.draw_path(path);
                s.set_stroke(None);
            }
            WrittenShape::Fill(outlines) => {
                // The line and its arrowheads overlap: they take the opacity once, as a group.
                let grouped = outlines.len() > 1 && st.opacity < 1.0;
                if grouped {
                    s.push_opacity(norm(st.opacity));
                    pushes += 1;
                }
                let opacity = if grouped { NormalizedF32::ONE } else { norm(st.opacity) };
                s.set_stroke(None);
                s.set_fill(Some(Fill { paint, opacity, rule: krilla::paint::FillRule::NonZero }));
                for p in outlines.iter().filter_map(to_path) {
                    s.draw_path(&p);
                }
                s.set_fill(None);
            }
        }
        for _ in 0..pushes {
            s.pop();
        }
    }

    /// Paint stroke `st` of the shape `bp` with its brush art (the stroke's opacity and blend mode
    /// over all of it). False when it has no known brush.
    fn brush_art(&mut self, s: &mut Surface, bp: &BezPath, st: &StrokeLayer, page: Rect) -> bool {
        let doc = self.doc;
        let brushes = self.brushes.get_or_insert_with(|| vectorcraft_brush::library(doc));
        let Some(b) = st.brush.as_deref().and_then(|name| brushes.iter().find(|b| b.name == name)) else { return false };
        let art = vectorcraft_brush::stroke_pieces(b, bp, st);
        let mut pushes = 0;
        if st.blend != BlendMode::Normal {
            s.push_blend_mode(blend(st.blend));
            pushes += 1;
        }
        if st.opacity < 1.0 {
            s.push_opacity(norm(st.opacity));
            pushes += 1;
        }
        for piece in &art {
            self.node(s, piece, page, false);
        }
        for _ in 0..pushes {
            s.pop();
        }
        true
    }

    fn text(&mut self, s: &mut Surface, n: &Node, t: &TextObject) {
        let layout = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t);
        let tb = t.xf.transform_rect_bbox(layout.bounds);
        s.push_transform(&xf(t.xf));
        for (i, run) in t.runs.iter().enumerate() {
            let mut bp = BezPath::new();
            for g in layout.glyphs.iter().filter(|g| g.run == i) {
                bp.extend(g.outline.iter());
            }
            let Some(path) = to_path(&bp) else { continue };
            if let Some(paint) = self.paint(&run.style.fill, layout.bounds) {
                s.set_stroke(None);
                s.set_fill(Some(Fill { paint, opacity: NormalizedF32::ONE, rule: krilla::paint::FillRule::NonZero }));
                s.draw_path(&path);
            }
            if run.style.stroke_width > 0.0
                && let Some(paint) = self.paint(&run.style.stroke, stroke_paint_bounds(layout.bounds, run.style.stroke_width))
            {
                s.set_fill(None);
                s.set_stroke(Some(Stroke { paint, width: run.style.stroke_width as f32, ..Default::default() }));
                s.draw_path(&path);
            }
        }
        s.set_fill(None);
        s.set_stroke(None);
        s.pop();
        if !n.appearance.items.is_empty() {
            let mut all = layout.to_bezpath();
            all.apply_affine(t.xf);
            let Some(path) = to_path(&all) else { return };
            for item in &n.appearance.items {
                match item {
                    AppearanceItem::Fill(fl) if fl.visible => {
                        if let Some(paint) = self.paint(&fl.paint, tb) {
                            s.set_stroke(None);
                            s.set_fill(Some(Fill { paint, opacity: norm(fl.opacity), rule: krilla::paint::FillRule::NonZero }));
                            s.draw_path(&path);
                        }
                    }
                    AppearanceItem::Stroke(st) if st.visible && st.width > 0.0 => {
                        if let Some(paint) = self.paint(&st.paint, st.paint_bounds(tb)) {
                            s.set_fill(None);
                            s.set_stroke(Some(Stroke { paint, width: st.width as f32, opacity: norm(st.opacity), ..Default::default() }));
                            s.draw_path(&path);
                        }
                    }
                    _ => {}
                }
            }
            s.set_fill(None);
            s.set_stroke(None);
        }
    }

    fn load_image(&mut self, key: &str) -> Option<Image> {
        if let Some(i) = self.images.get(key) {
            return i.clone();
        }
        let img = self.doc.images.get(key).and_then(|blob| {
            let bytes = blob.bytes.as_ref().clone();
            let direct = match blob.mime.as_str() {
                "image/png" => Image::from_png(bytes.clone().into(), true).ok(),
                "image/jpeg" | "image/jpg" => Image::from_jpeg(bytes.clone().into(), true).ok(),
                "image/gif" => Image::from_gif(bytes.clone().into(), true).ok(),
                "image/webp" => Image::from_webp(bytes.clone().into(), true).ok(),
                _ => None,
            };
            direct.or_else(|| {
                let rgba = image::load_from_memory(&bytes).ok()?.to_rgba8();
                let (w, h) = rgba.dimensions();
                Some(Image::from_rgba8(rgba.into_raw(), w, h))
            })
        });
        if img.is_none() {
            self.warn(format!("image '{key}' could not be decoded and was skipped"));
        }
        self.images.insert(key.to_string(), img.clone());
        img
    }

    fn image(&mut self, s: &mut Surface, im: &vectorcraft_doc::ImageObject) {
        let Some(img) = self.load_image(&im.key) else { return };
        let Some(size) = Size::from_wh(im.width.max(1) as f32, im.height.max(1) as f32) else { return };
        s.push_transform(&xf(im.xf));
        s.draw_image(img, size);
        s.pop();
    }
}
