//! Document → PDF (krilla). Mirrors the tree walk of `vectorcraft-render`.

use std::collections::HashMap;

use krilla::color::separation::{Color as SepColor, SeparationColorant, SeparationSpace};
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
use vectorcraft_doc::{AppearanceItem, Document, LineCap, LineJoin, Node, NodeKind, StrokeAlign, StrokeLayer, TextObject};
use vectorcraft_effects::stroke::{self, WrittenShape};
use vectorcraft_geom::{Affine, BezPath, FillRule, Rect};

use crate::lab_spot::{find, rfind};
use crate::{Compatibility, ExportReport, PdfError, PdfOptions, Standard};

/// Export `doc` as PDF bytes: one page per artboard (or the artboards chosen in `opts`).
pub fn export(doc: &Document, opts: &PdfOptions) -> Result<Vec<u8>, PdfError> {
    export_with_report(doc, opts).map(|r| r.bytes)
}

/// Like [`export`], also returning warnings: options accepted but not applied yet (see
/// [`crate::PdfSettings::warnings`]) and features approximated or dropped.
pub fn export_with_report(doc: &Document, opts: &PdfOptions) -> Result<ExportReport, PdfError> {
    let set = &opts.settings;
    set.check()?;
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

    let version = match set.compatibility {
        Compatibility::Pdf14 => PdfVersion::Pdf14,
        Compatibility::Pdf15 => PdfVersion::Pdf15,
        Compatibility::Pdf16 => PdfVersion::Pdf16,
        Compatibility::Pdf17 => PdfVersion::Pdf17,
        Compatibility::Pdf20 => PdfVersion::Pdf20,
    };
    let mut cb = ConfigurationBuilder::new().with_version(version);
    // `check` has refused the standards (and standard/version pairs) the writer can't produce.
    if set.standard == Standard::PdfA2b {
        cb = cb.with_archival_validator(Archival::A2_B);
    }
    let configuration = cb.finish().map_err(|e| PdfError::Write(format!("{e:?}")))?;
    let settings = krilla::SerializeSettings { compress_content_streams: set.compression.compress_text, configuration, ..Default::default() };

    let mut pdf = krilla::Document::new_with(settings);
    let title = opts.title.clone().unwrap_or_else(|| doc.title.clone());
    let mut meta = Metadata::new().creator("VectorCraft".into()).producer("VectorCraft".into());
    if !title.is_empty() {
        meta = meta.title(title);
    }
    // File Info.
    let info = &doc.metadata;
    let given = |s: &str| Some(s.trim()).filter(|s| !s.is_empty()).map(str::to_string);
    if let Some(a) = given(&info.author) {
        meta = meta.authors(vec![a]);
    }
    if let Some(d) = given(&info.description) {
        meta = meta.description(d);
    }
    if !info.keywords.is_empty() {
        meta = meta.keywords(info.keywords.clone());
    }
    let created = opts.created.or_else(vectorcraft_doc::metadata::now_unix).map(date_time);
    if let Some(t) = created {
        meta = meta.creation_date(t);
    }
    pdf.set_metadata(meta);
    // Preserve Editing: the native document as an embedded file (`check` refused PDF/A with it).
    let mut warnings = set.warnings();
    let native = opts.native.as_deref().filter(|_| set.preserve_editing);
    if let Some(native) = native {
        pdf.embed_file(crate::editing::embedded_file(native, set.compression.compress_text, created));
    } else if set.preserve_editing {
        warnings.push("Preserve editing needs the native document, which wasn't given: the PDF reopens as plain artwork".into());
    }

    let mut ex = Exporter { doc, warnings: vec![], images: HashMap::new(), brushes: None, knockout: doc.page_knockout, lab_spots: vec![] };
    // CMYK documents blend in CMYK, as on screen: their groups' blending space is rewritten (see
    // `cmyk_blending`), and transparency at the top of a page is put in a non-isolated group of
    // its own (the PDF writer has no page group attributes).
    let cmyk = doc.color_mode == vectorcraft_doc::ColorMode::Cmyk;
    let page_group = doc.page_isolate || doc.page_knockout;
    let cmyk_page_group = cmyk && !page_group && doc.layers.iter().any(|l| l.shows_transparency());
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
        if page_group {
            s.push_isolated();
        } else if cmyk_page_group {
            let all = constant_mask(&mut s, r, 1.0);
            s.push_mask(all);
        }
        ex.children(&mut s, &doc.layers, r);
        if page_group || cmyk_page_group {
            s.pop();
        }
        s.pop();
        s.finish();
        page.finish();
    }
    let mut bytes = pdf.finish().map_err(|e| PdfError::Write(format!("{e:?}")))?;
    if cmyk {
        cmyk_blending(&mut bytes);
    }
    let bytes = if ex.lab_spots.is_empty() { bytes } else { crate::lab_spot::lab_alternates(bytes, &ex.lab_spots) };
    let bytes = if native.is_some() { crate::editing::seal(bytes)? } else { bytes };
    warnings.extend(ex.warnings);
    warnings.dedup();
    Ok(ExportReport { bytes, warnings })
}

/// Make the transparency groups of `pdf` blend in DeviceCMYK. The PDF writer gives every group
/// DeviceRGB as its blending space, so the group dictionaries are rewritten in place, keeping their
/// length so the cross-reference offsets stay valid. Luminosity masks' groups keep RGB: mask
/// luminance is that of screen colours, as on screen.
fn cmyk_blending(pdf: &mut [u8]) {
    let masks = luminosity_mask_groups(pdf);
    let mut from = 0;
    while let Some(at) = find(pdf, b"/Group<<", from).map(|i| i + b"/Group".len()) {
        let end = find(pdf, b">>", at).map_or(pdf.len(), |i| i + 2);
        if !object_number(&pdf[..at]).is_some_and(|n| masks.contains(&n)) {
            cmyk_group(&mut pdf[at..end]);
        }
        from = end;
    }
}

/// Rewrite transparency group dictionary `dict` (`<<…>>`) to blend in DeviceCMYK, at the same
/// length: leaving out the optional `/Type/Group` makes room for the longer name, spaces pad the
/// rest.
fn cmyk_group(dict: &mut [u8]) {
    let Ok(text) = std::str::from_utf8(dict) else { return };
    if !(text.contains("/S/Transparency") && text.contains("/Type/Group") && text.contains("/CS/DeviceRGB")) {
        return;
    }
    let body = text.replacen("/Type/Group", "", 1).replacen("/CS/DeviceRGB", "/CS/DeviceCMYK", 1);
    let body = body.strip_suffix(">>").unwrap_or(&body);
    let new = format!("{body}{}>>", " ".repeat(dict.len() - body.len() - 2));
    dict.copy_from_slice(new.as_bytes());
}

/// Object numbers of the groups luminosity soft masks draw (`/G n 0 R` in their dictionaries).
fn luminosity_mask_groups(pdf: &[u8]) -> Vec<u32> {
    let mut out = vec![];
    let mut from = 0;
    while let Some(at) = find(pdf, b"/Type/Mask", from) {
        let end = find(pdf, b">>", at).unwrap_or(pdf.len());
        let dict = &pdf[at..end];
        if find(dict, b"/S/Luminosity", 0).is_some()
            && let Some(g) = find(dict, b"/G ", 0)
        {
            let rest = &dict[g + 3..];
            out.extend(number(&rest[..rest.iter().take_while(|b| b.is_ascii_digit()).count()]));
        }
        from = end;
    }
    out
}

/// The number of the object whose dictionary `before` ends in (its last `n 0 obj` header).
fn object_number(before: &[u8]) -> Option<u32> {
    let at = rfind(before, b" 0 obj")?;
    let digits = before[..at].iter().rev().take_while(|b| b.is_ascii_digit()).count();
    number(&before[at - digits..at])
}

fn number(digits: &[u8]) -> Option<u32> {
    std::str::from_utf8(digits).ok()?.parse().ok()
}

/// Unix seconds (UTC) → krilla date.
fn date_time(t: i64) -> krilla::metadata::DateTime {
    let [year, month, day, hour, minute, second] = vectorcraft_doc::metadata::civil(t);
    krilla::metadata::DateTime::new(year as u16)
        .month(month as u8)
        .day(day as u8)
        .hour(hour as u8)
        .minute(minute as u8)
        .second(second as u8)
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
    /// Spot colours written with a Lab alternate ([`crate::lab_spot`]): colorant name, Lab values.
    lab_spots: Vec<(String, vectorcraft_color::cms::Lab)>,
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
        Color::Lab { .. } => {
            let [r, g, b] = c.to_rgb();
            rgb::Color::new(q(r), q(g), q(b)).into()
        }
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
    /// A colour for the page: in CMYK documents RGB and Lab colours are separated into DeviceCMYK
    /// through the active colour settings, so the file carries press values.
    fn col(&mut self, c: &Color) -> krilla::color::Color {
        if self.doc.color_mode == vectorcraft_doc::ColorMode::Cmyk && matches!(c, Color::Rgb { .. } | Color::Lab { .. }) {
            let cms = vectorcraft_color::cms::active();
            let [cc, m, y, k] = cms.to_cmyk(c, cms.settings().intent);
            return cmyk::Color::new(q(cc), q(m), q(y), q(k)).into();
        }
        color(c)
    }

    /// The Separation colour space of spot swatch `name` (its CMYK equivalent is the alternate
    /// space; a Lab spot colour also gets a Lab one, [`crate::lab_spot`], unless the Spot Colors
    /// options use CMYK values); `None` when `name` isn't a spot colour.
    fn separation(&mut self, name: &str) -> Option<SeparationSpace> {
        let doc = self.doc;
        let sw = doc.swatch(name).filter(|s| s.spot)?;
        let color = doc.linked_color(sw.paint.color()?, true);
        if let Color::Lab { l, a, b } = color
            && !self.lab_spots.iter().any(|(n, _)| n == name)
        {
            self.lab_spots.push((name.to_string(), vectorcraft_color::cms::Lab::new(l, a, b)));
        }
        let cms = vectorcraft_color::cms::active();
        let full = cms.to_cmyk(&color, cms.settings().intent);
        let alt = krilla::color::RegularColor::Cmyk(cmyk::Color::new(q(full[0]), q(full[1]), q(full[2]), q(full[3])));
        Some(SeparationSpace::new(SeparationColorant::Custom(sw.name.clone()), alt))
    }

    /// A solid paint, in the Separation colour space at its tint when it's linked to a spot swatch
    /// or to Registration (`/All`: every plate).
    fn solid(&mut self, c: &Color, link: Option<&str>, tint: f32) -> krilla::color::Color {
        if link == Some(vectorcraft_color::swatch::REGISTRATION) {
            let alt = krilla::color::RegularColor::Cmyk(cmyk::Color::new(255, 255, 255, 255));
            return SepColor::new(q(tint), SeparationSpace::new(SeparationColorant::AllColorants, alt)).into();
        }
        match link.and_then(|n| self.separation(n)) {
            Some(space) => SepColor::new(q(tint), space).into(),
            None => self.col(c),
        }
    }

    /// The stops of a gradient whose every stop is a tint of one spot ink (or paper white, 0% of
    /// it) as tints of that ink — a Separation shading — with midpoints as explicit stops (the
    /// tint and opacity there are halfway). `None` for other gradients; one that mixes a spot ink
    /// with other colours is written in process colours (the PDF writer has no DeviceN).
    fn spot_stops(&mut self, g: &vectorcraft_color::Gradient) -> Option<Vec<(f32, krilla::color::Color, f32)>> {
        let ink = g.stops.iter().find_map(|s| s.swatch.as_deref().filter(|n| self.doc.swatch(n).is_some_and(|w| w.spot)))?;
        let paper = |c: &Color| c.to_rgba8(1.0)[..3] == [255; 3];
        let tints: Option<Vec<f32>> = g
            .stops
            .iter()
            .map(|s| match s.swatch.as_deref() {
                Some(n) if n == ink => Some(s.tint),
                None if paper(&s.color) => Some(0.0),
                _ => None,
            })
            .collect();
        let Some(tints) = tints else {
            self.warn("gradients mixing a spot color with other colors are exported in process colors");
            return None;
        };
        let space = self.separation(ink)?;
        let tint = |t: f32| -> krilla::color::Color { SepColor::new(q(t), space.clone()).into() };
        let mut out = vec![];
        for (i, s) in g.stops.iter().enumerate() {
            out.push((s.offset, tint(tints[i]), s.opacity));
            if let Some(n) = g.stops.get(i + 1)
                && (s.midpoint - 0.5).abs() > 1e-3
            {
                let at = s.offset + (n.offset - s.offset) * s.midpoint;
                out.push((at, tint((tints[i] + tints[i + 1]) / 2.0), (s.opacity + n.opacity) / 2.0));
            }
        }
        Some(out)
    }

    /// Warn when `effects` (an object's, a fill's or a stroke's) has a visible raster effect: the
    /// app renders them to images before export (`vectorcraft_engine::export_pdf`), so the ones
    /// still here (on `what`) can't be written.
    fn warn_raster(&mut self, effects: &[vectorcraft_doc::Effect], what: impl FnOnce() -> String) {
        if effects.iter().any(|e| e.visible && vectorcraft_effects::is_raster(&e.id)) {
            self.warn(format!("raster effects (shadows, glows, blur, feather) on {} are left out of the PDF", what()));
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
            Paint::Solid { color: c, swatch, tint } => Some(self.solid(c, swatch.as_deref(), *tint).into()),
            Paint::Gradient(g) => {
                let geom = g.resolve(bounds);
                let spot = (g.gradient.kind != GradientKind::Freeform).then(|| self.spot_stops(&g.gradient)).flatten();
                let colored = match spot {
                    Some(v) => v,
                    None => {
                        let stops = g.gradient.expanded_stops();
                        // The PDF writer needs every stop in one colour space: stops of mixed
                        // models (midpoints sample in RGB) go through RGB, or stay CMYK in a CMYK
                        // document, where `col` separates the rest.
                        let mixed = stops.windows(2).any(|w| w[0].1.model() != w[1].1.model());
                        let cmyk_doc = self.doc.color_mode == vectorcraft_doc::ColorMode::Cmyk;
                        let one = |c: Color| match c {
                            Color::Cmyk { .. } if cmyk_doc => c,
                            _ if mixed => c.in_model(vectorcraft_color::cms::Model::Rgb),
                            _ => c,
                        };
                        stops.into_iter().map(|(o, c, a)| (o, self.col(&one(c)), a)).collect()
                    }
                };
                let mut stops: Vec<Stop> = Vec::new();
                let mut last = 0.0f32;
                for (o, color, a) in colored {
                    let o = o.clamp(last, 1.0);
                    last = o;
                    stops.push(Stop { offset: norm(o), color, opacity: norm(a) });
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
                        let t = geom.radial_squash();
                        let (cx, cy) = (geom.start.x as f32, geom.start.y as f32);
                        // An off-centre focal point: a two-point radial shading from it.
                        let f = t.inverse() * geom.focal_point();
                        Some(
                            RadialGradient {
                                fx: f.x as f32,
                                fy: f.y as f32,
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
        self.warn_raster(&n.appearance.effects, || format!("{} objects", n.kind_label().to_lowercase()));
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
            NodeKind::Layer { children, clip: false, .. } | NodeKind::Group { children, clip: false } => self.children(s, children, page),
            NodeKind::Group { children, clip: true } | NodeKind::Layer { children, clip: true, .. } => {
                // The region every output clips to; nothing to clip by hides the clipped art. The
                // clipping path's fill paints behind the clipped art and its stroke over it, unclipped.
                if let Some((clip, rest)) = children.split_first()
                    && let Some((p, r)) = vectorcraft_effects::clip_outline(clip).and_then(|(bp, r)| to_path(&bp).map(|p| (p, r)))
                {
                    let paint = clip.clip_paint();
                    s.push_clip_path(&p, &rule(r));
                    if let Some(fill) = &paint.fill {
                        self.node(s, fill, page, false);
                    }
                    self.children(s, rest, page);
                    s.pop();
                    if let Some(stroke) = &paint.stroke {
                        self.node(s, stroke, page, false);
                    }
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
            NodeKind::Text(t) => self.text(s, n, t, page),
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
                    self.warn_raster(&fl.effects, || "fills".into());
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
                    self.warn_raster(&st.effects, || "strokes".into());
                    self.stroke(s, bp, &path, r, st, page, bounds);
                }
            }
        }
        s.set_fill(None);
        s.set_stroke(None);
    }

    /// Paint stroke `st` of the shape `bp` (`path`), whose geometric bounds `bounds` place its
    /// unplaced gradients.
    #[allow(clippy::too_many_arguments)]
    fn stroke(&mut self, s: &mut Surface, bp: &BezPath, path: &Path, r: FillRule, st: &StrokeLayer, page: Rect, bounds: Rect) {
        if !stroke::is_plain(st) && self.brush_art(s, bp, st, page) {
            return;
        }
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
            WrittenShape::Fill(outlines) if st.path_gradient().is_some() => {
                // A gradient along or across the stroke: slices clipped to its outlines, under
                // the stroke's opacity as a group.
                if let Some(ws) = stroke::written_slices(bp, r, st, outlines)
                    && let Some(clip) = to_path(&ws.clip)
                {
                    self.warn("gradients along or across strokes are exported as slices of linear gradients");
                    if st.opacity < 1.0 {
                        s.push_opacity(norm(st.opacity));
                        pushes += 1;
                    }
                    s.push_clip_path(&clip, &krilla::paint::FillRule::NonZero);
                    s.set_stroke(None);
                    for (shape, paint) in &ws.slices {
                        let b = shape.bounding_box();
                        if let (Some(paint), Some(p)) = (self.paint(paint, b), to_path(shape)) {
                            s.set_fill(Some(Fill { paint, opacity: NormalizedF32::ONE, rule: krilla::paint::FillRule::NonZero }));
                            s.draw_path(&p);
                        }
                    }
                    s.set_fill(None);
                    s.pop();
                }
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

    fn text(&mut self, s: &mut Surface, n: &Node, t: &TextObject, page: Rect) {
        let layout = vectorcraft_text::layout(vectorcraft_text::FontDb::global(), t);
        let tb = t.xf.transform_rect_bbox(layout.bounds);
        // The object's own fills and strokes paint the whole outline: those below the Characters
        // row under the characters, the others over them.
        let (below, above) = n.appearance.split_contents();
        let all = (!n.appearance.items.is_empty()).then(|| {
            let mut all = layout.to_bezpath();
            all.apply_affine(t.xf);
            to_path(&all).map(|p| (all, p))
        });
        let all = all.flatten();
        if let Some((bp, path)) = &all {
            self.text_items(s, below, bp, path, page, tb);
        }
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
            if run.style.has_stroke() {
                // Character strokes are drawn in text space, with their cap, join and dashes.
                self.stroke(s, &bp, &path, FillRule::NonZero, &run.style.stroke_layer(), page, layout.bounds);
            }
        }
        s.set_fill(None);
        s.set_stroke(None);
        s.pop();
        if let Some((bp, path)) = &all {
            self.text_items(s, above, bp, path, page, tb);
        }
    }

    /// Some of a type object's own fills and strokes (`items`) on its glyph outlines `bp` (`path`;
    /// `tb`: their bounds). Strokes take every stroke option, as on paths.
    #[allow(clippy::too_many_arguments)]
    fn text_items(&mut self, s: &mut Surface, items: &[AppearanceItem], bp: &BezPath, path: &Path, page: Rect, tb: Rect) {
        for item in items {
            match item {
                AppearanceItem::Fill(fl) if fl.visible => {
                    if let Some(paint) = self.paint(&fl.paint, tb) {
                        s.set_stroke(None);
                        s.set_fill(Some(Fill { paint, opacity: norm(fl.opacity), rule: krilla::paint::FillRule::NonZero }));
                        s.draw_path(path);
                    }
                }
                AppearanceItem::Stroke(st) if st.visible && st.width > 0.0 => self.stroke(s, bp, path, FillRule::NonZero, st, page, tb),
                _ => {}
            }
        }
        s.set_fill(None);
        s.set_stroke(None);
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
