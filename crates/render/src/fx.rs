//! Live effects in the renderer.
//!
//! Geometry effects (object-level, then per fill/stroke) rewrite the path before painting via
//! `vectorcraft-effects`. Raster effects use vello_cpu filter layers. Those only work in
//! single-threaded contexts, so on the multithreaded pipeline each filter layer is rendered
//! offscreen on the calling thread (cropped to its reach) and composited back with the same blend
//! mode and opacity ([`Renderer::with_filters`]):
//!
//! - Drop Shadow / Outer Glow: the object's silhouette in a `DropShadowOnly` filter layer, painted
//!   below the object with the effect's blend mode and opacity;
//! - Gaussian Blur: the object painted inside a Gaussian filter layer;
//! - Feather: like Gaussian Blur but clipped to the shape, so the edge fades inward;
//! - Inner Glow: a blurred inverse silhouette (Edge) or the blurred silhouette (Center), clipped
//!   to the shape and painted above it.

use vectorcraft_doc::{AppearanceItem, Effect, Node, NodeKind};
use vectorcraft_effects::{self as effects, GeomContext, RasterFx};
use vectorcraft_geom::{Affine, BezPath, FillRule, Rect, Shape};
use vello_common::filter_effects::{EdgeMode, Filter, FilterPrimitive};
use vello_cpu::RenderContext;
use vello_cpu::peniko;

use crate::{Frame, Renderer, blend_mode, fill_rule, paint};

fn visible(effects: &[Effect]) -> bool {
    effects.iter().any(|e| e.visible)
}

/// Does this node need the effects path (a path or compound path with visible effects)?
pub(crate) fn has_fx(n: &Node) -> bool {
    if !matches!(n.kind, NodeKind::Path { guide: false, .. } | NodeKind::Compound { .. }) {
        return false;
    }
    visible(&n.appearance.effects)
        || n.appearance.items.iter().any(|i| match i {
            AppearanceItem::Fill(f) => visible(&f.effects),
            AppearanceItem::Stroke(s) => visible(&s.effects),
        })
}

fn node_bezpath(n: &Node) -> Option<BezPath> {
    match &n.kind {
        NodeKind::Path { path, .. } => Some(path.to_bezpath()),
        NodeKind::Compound { children, .. } => {
            let mut bp = BezPath::new();
            for c in children {
                if let Some(p) = c.path_data() {
                    bp.extend(p.to_bezpath());
                }
            }
            Some(bp)
        }
        _ => None,
    }
}

fn apply(effects: &[Effect], bp: &BezPath, ctx: &GeomContext) -> BezPath {
    if !effects::has_geometry(effects) || bp.elements().is_empty() {
        return bp.clone();
    }
    effects::apply_geometry_bez(effects, bp, bp.bounding_box(), ctx)
}

/// The object-level effected geometry of `n` (base path `bp`).
pub(crate) fn effected_path(n: &Node, bp: &BezPath) -> BezPath {
    apply(&n.appearance.effects, bp, &GeomContext::of(n))
}

fn item_effects(item: &AppearanceItem) -> &[Effect] {
    match item {
        AppearanceItem::Fill(f) => &f.effects,
        AppearanceItem::Stroke(s) => &s.effects,
    }
}

/// Visual bounds including geometry effects, stroke outsets and shadows/glows.
pub(crate) fn visual_bounds(n: &Node) -> Option<Rect> {
    let bp = node_bezpath(n)?;
    let ctx = GeomContext::of(n);
    let g = effected_path(n, &bp);
    let mut r: Option<Rect> = (!g.elements().is_empty()).then(|| g.bounding_box());
    for item in &n.appearance.items {
        let fx = item_effects(item);
        if effects::has_geometry(fx) {
            let ig = apply(fx, &g, &ctx.item(item));
            if !ig.elements().is_empty() {
                let b = ig.bounding_box();
                r = Some(r.map_or(b, |r| r.union(b)));
            }
        }
    }
    let o = n.appearance.outset() + effects::outset(&n.appearance.effects);
    r.map(|r| r.inflate(o, o))
}

/// Bounds used for culling: like `Node::visual_bounds`, but aware of live effects anywhere in
/// the subtree (a shadow can reach the view while its object is outside it).
pub(crate) fn cull_bounds(n: &Node) -> Option<Rect> {
    match &n.kind {
        NodeKind::Layer { children, .. } | NodeKind::Group { children, clip: false } => {
            children.iter().fold(None, |acc, c| vectorcraft_geom::union_opt(acc, cull_bounds(c)))
        }
        _ if has_fx(n) => visual_bounds(n),
        _ => n.visual_bounds(),
    }
}

fn shadow_filter(dx: f64, dy: f64, blur: f64, color: peniko::Color) -> Filter {
    Filter::from_primitive(FilterPrimitive::DropShadowOnly {
        dx: dx as f32,
        dy: dy as f32,
        std_deviation: (blur / 2.0).max(0.0) as f32,
        color,
        edge_mode: EdgeMode::None,
    })
}

fn blur_filter(sigma: f64) -> Filter {
    Filter::from_primitive(FilterPrimitive::GaussianBlur { std_deviation: sigma.max(0.0) as f32, edge_mode: EdgeMode::None })
}

fn pcolor(c: &vectorcraft_doc::color::Color) -> peniko::Color {
    let [r, g, b] = c.to_rgb();
    peniko::Color::new([r, g, b, 1.0])
}

/// A cached shadow / glow raster: premultiplied, already tinted and faded, positioned relative to
/// the view-space position of `anchor` (a document point).
pub(crate) struct ShadowEntry {
    node: Node,
    linear: [u64; 4],
    anchor: vectorcraft_geom::Point,
    /// Pixel offset of the raster's top-left from the anchor's view position.
    rel: (f64, f64),
    image: std::sync::Arc<vello_cpu::Pixmap>,
    pub(crate) stamp: u64,
}

/// Box sizes approximating a Gaussian of `sigma` with three box blurs.
fn gauss_boxes(sigma: f64) -> [usize; 3] {
    let n = 3.0;
    let w_ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut wl = w_ideal.floor() as i64;
    if wl % 2 == 0 {
        wl -= 1;
    }
    let wu = wl + 2;
    let m_ideal = (12.0 * sigma * sigma - n * (wl * wl) as f64 - 4.0 * n * wl as f64 - 3.0 * n) / (-4.0 * wl as f64 - 4.0);
    let m = m_ideal.round() as i64;
    let r = |i: i64| (((if i < m { wl } else { wu }) - 1) / 2).max(0) as usize;
    [r(0), r(1), r(2)]
}

/// One horizontal box-blur pass of radius `r` over rows of `w` (running sum, edges as zero).
fn box_blur_h(src: &[f32], dst: &mut [f32], w: usize, h: usize, r: usize) {
    if r == 0 {
        dst.copy_from_slice(src);
        return;
    }
    let norm = 1.0 / (2 * r + 1) as f32;
    for y in 0..h {
        let row = &src[y * w..(y + 1) * w];
        let out = &mut dst[y * w..(y + 1) * w];
        let mut acc: f32 = row.iter().take(r + 1).sum();
        for x in 0..w {
            out[x] = acc * norm;
            if x + r + 1 < w {
                acc += row[x + r + 1];
            }
            if x >= r {
                acc -= row[x - r];
            }
        }
    }
}

fn transpose(src: &[f32], dst: &mut [f32], w: usize, h: usize) {
    for y in 0..h {
        for x in 0..w {
            dst[x * h + y] = src[y * w + x];
        }
    }
}

/// Gaussian-blur an alpha plane in place (three box passes per axis).
fn blur_alpha(a: &mut Vec<f32>, w: usize, h: usize, sigma: f64) {
    if sigma < 0.2 {
        return;
    }
    let boxes = gauss_boxes(sigma);
    let mut tmp = vec![0.0; a.len()];
    for r in boxes {
        box_blur_h(a, &mut tmp, w, h, r);
        std::mem::swap(a, &mut tmp);
    }
    transpose(a, &mut tmp, w, h);
    std::mem::swap(a, &mut tmp);
    for r in boxes {
        box_blur_h(a, &mut tmp, h, w, r);
        std::mem::swap(a, &mut tmp);
    }
    transpose(a, &mut tmp, h, w);
    std::mem::swap(a, &mut tmp);
}

impl Renderer {
    /// Drop shadow / outer glow `fx` (index `i` in the object's raster effects) from a cached raster:
    /// the silhouette is rendered once into a small crop, blurred and tinted on the CPU, and reused
    /// while the object and the zoom/rotation are unchanged (pans only move it). Returns false when
    /// the effect isn't a shadow/glow or the raster would be too large (caller falls back).
    #[allow(clippy::too_many_arguments)]
    fn draw_shadow_cached(
        &mut self,
        ctx: &mut RenderContext,
        f: &Frame,
        n: &Node,
        i: usize,
        fx: &RasterFx,
        g: &BezPath,
        rule: FillRule,
        gctx: &GeomContext,
        reach: Rect,
    ) -> bool {
        let (mode, opacity, dx, dy, blur, color) = match fx {
            RasterFx::DropShadow { mode, opacity, dx, dy, blur, color } => (*mode, *opacity, *dx, *dy, *blur, *color),
            RasterFx::OuterGlow { mode, opacity, blur, color } => (*mode, *opacity, 0.0, 0.0, *blur, *color),
            _ => return false,
        };
        let c = f.view.as_coeffs();
        let linear = [c[0].to_bits(), c[1].to_bits(), c[2].to_bits(), c[3].to_bits()];
        let key = (n as *const Node as usize, i);
        let stamp = self.stamp;
        let hit = self.shadows.get_mut(&key).filter(|e| e.linear == linear && e.node == *n);
        let (image, x, y) = match hit {
            Some(e) => {
                e.stamp = stamp;
                let p = f.view * e.anchor;
                (e.image.clone(), (p.x + e.rel.0).round(), (p.y + e.rel.1).round())
            }
            None => {
                let spread = blur.max(0.0) * 1.5 / f.px + 2.0;
                let reach = reach + vectorcraft_geom::Vec2::new(dx, dy);
                let r = f.view.transform_rect_bbox(reach).inflate(spread, spread);
                let (x0, y0) = (r.x0.floor(), r.y0.floor());
                let (w, h) = (r.x1.ceil() - x0, r.y1.ceil() - y0);
                if !(1.0..=8192.0).contains(&w) || !(1.0..=8192.0).contains(&h) || w * h > 4.0e6 {
                    return false;
                }
                let (w, h) = (w as u16, h as u16);
                // Silhouette (the object's painted alpha), offset by the shadow distance.
                let mut off = crate::single_threaded_context(w, h);
                let shifted = Frame { mt: false, view: Affine::translate((-x0, -y0)) * f.view * Affine::translate((dx, dy)), ..*f };
                self.paint_items(&mut off, &shifted, n, g, rule, gctx);
                off.flush();
                let mut pm = vello_cpu::Pixmap::new(w, h);
                off.render(&mut pm, &mut self.resources);
                let (wu, hu) = (w as usize, h as usize);
                let mut a: Vec<f32> = pm.data().iter().map(|p| p.a as f32).collect();
                blur_alpha(&mut a, wu, hu, blur / 2.0 / f.px);
                let [cr, cg, cb] = color.to_rgb();
                let tint = |v: f32, ch: f32| (v * ch).round().clamp(0.0, 255.0) as u8;
                for (px, av) in pm.data_mut().iter_mut().zip(&a) {
                    let al = (av * opacity).clamp(0.0, 255.0);
                    *px = vello_cpu::color::PremulRgba8 { r: tint(al, cr), g: tint(al, cg), b: tint(al, cb), a: al.round() as u8 };
                }
                let image = std::sync::Arc::new(pm);
                let anchor = reach.origin();
                let p = f.view * anchor;
                self.shadows.insert(key, ShadowEntry { node: n.clone(), linear, anchor, rel: (x0 - p.x, y0 - p.y), image: image.clone(), stamp });
                (image, x0, y0)
            }
        };
        // The blend mode applies per draw: a blend layer would be a full-viewport compositing pass
        // per shadow. The raster sits on whole pixels, so the cheapest sampler is exact.
        let (w, h) = (image.width() as f64, image.height() as f64);
        ctx.set_transform(Affine::translate((x, y)));
        ctx.set_blend_mode(blend_mode(mode));
        let sampler = peniko::ImageSampler { quality: peniko::ImageQuality::Low, ..Default::default() };
        ctx.set_paint(vello_cpu::Image { image: vello_cpu::ImageSource::Pixmap(image), sampler });
        ctx.fill_rect(&Rect::new(0.0, 0.0, w, h));
        ctx.set_blend_mode(blend_mode(vectorcraft_doc::color::BlendMode::Normal));
        ctx.set_transform(Affine::IDENTITY);
        true
    }

    /// `draw_shape` for nodes with live effects.
    pub(crate) fn draw_shape_fx(&mut self, ctx: &mut RenderContext, f: &Frame, n: &Node, bp: &BezPath, rule: FillRule) {
        let g = effected_path(n, bp);
        if g.elements().is_empty() {
            return;
        }
        if f.opts.outline {
            self.hairline(ctx, f, &g, [0, 0, 0, 255]);
            return;
        }
        let gctx = GeomContext::of(n);
        let rfx = effects::raster_effects(&n.appearance.effects);
        // Document-space reach of the painted geometry (strokes, arrowheads…).
        let outset = n.appearance.outset();
        let reach = g.bounding_box().inflate(outset, outset);
        // Below the object: shadows and outer glows.
        for (i, fx) in rfx.iter().enumerate().filter(|(_, x)| x.is_below()) {
            if self.draw_shadow_cached(ctx, f, n, i, fx, &g, rule, &gctx, reach) {
                continue;
            }
            // The offset is applied to the geometry rather than in the filter: vello_cpu drops
            // layer content that lies entirely outside the viewport before filtering.
            let (mode, opacity, dx, dy, blur, filter) = match fx {
                RasterFx::DropShadow { mode, opacity, dx, dy, blur, color } => {
                    (*mode, *opacity, *dx, *dy, *blur, shadow_filter(0.0, 0.0, *blur, pcolor(color)))
                }
                RasterFx::OuterGlow { mode, opacity, blur, color } => {
                    (*mode, *opacity, 0.0, 0.0, *blur, shadow_filter(0.0, 0.0, *blur, pcolor(color)))
                }
                _ => continue,
            };
            let mut gs = g.clone();
            gs.apply_affine(Affine::translate((dx, dy)));
            let reach = reach + vectorcraft_geom::Vec2::new(dx, dy);
            self.with_filters(ctx, f, reach, blur, Some((mode, opacity)), |r, c, fr, comp| {
                c.set_transform(fr.view);
                c.push_layer(None, comp.map(|m| blend_mode(m.0)), comp.map(|m| m.1), None, Some(filter));
                r.paint_items(c, fr, n, &gs, rule, &gctx);
                c.pop_layer();
            });
        }
        // The object itself (blurred / feathered).
        let blur: f64 = rfx
            .iter()
            .map(|fx| match fx {
                RasterFx::Feather { radius } | RasterFx::GaussianBlur { radius } => radius.max(0.0),
                _ => 0.0,
            })
            .sum();
        if blur > 0.0 {
            self.with_filters(ctx, f, reach, blur, None, |r, c, fr, _| {
                let mut layers = 0;
                for fx in &rfx {
                    match fx {
                        RasterFx::Feather { radius } if *radius > 0.0 => {
                            c.set_transform(fr.view);
                            c.set_fill_rule(fill_rule(rule));
                            c.push_clip_layer(&g);
                            c.push_layer(None, None, None, None, Some(blur_filter(radius / 2.0)));
                            layers += 2;
                        }
                        RasterFx::GaussianBlur { radius } if *radius > 0.0 => {
                            c.set_transform(fr.view);
                            c.push_layer(None, None, None, None, Some(blur_filter(radius / 2.0)));
                            layers += 1;
                        }
                        _ => {}
                    }
                }
                r.paint_items(c, fr, n, &g, rule, &gctx);
                for _ in 0..layers {
                    c.pop_layer();
                }
            });
        } else {
            self.paint_items(ctx, f, n, &g, rule, &gctx);
        }
        // Above the object: inner glows, clipped to the shape.
        for fx in &rfx {
            let RasterFx::InnerGlow { mode, opacity, blur, color, center } = fx else { continue };
            let filter = shadow_filter(0.0, 0.0, *blur, pcolor(color));
            self.with_filters(ctx, f, g.bounding_box(), *blur, Some((*mode, *opacity)), |_, c, fr, comp| {
                // The blend layer goes outside the clip: a clip layer is isolated, so a blend
                // inside it would mix with nothing instead of the object below.
                if let Some((m, o)) = comp {
                    c.set_transform(Affine::IDENTITY);
                    c.push_layer(None, Some(blend_mode(m)), Some(o), None, None);
                }
                c.set_transform(fr.view);
                c.set_fill_rule(fill_rule(rule));
                c.push_clip_layer(&g);
                c.push_layer(None, None, None, None, Some(filter));
                c.set_paint(peniko::Color::BLACK);
                if *center {
                    c.set_fill_rule(fill_rule(rule));
                    c.fill_path(&g);
                } else {
                    // Everything outside the shape, so the glow bleeds in from the edges.
                    let pad = blur * 2.0 + 4.0 * fr.px;
                    let mut inv = g.bounding_box().inflate(pad, pad).to_path(0.1);
                    inv.extend(g.iter());
                    c.set_fill_rule(peniko::Fill::EvenOdd);
                    c.fill_path(&inv);
                }
                c.pop_layer();
                c.pop_layer();
                if comp.is_some() {
                    c.pop_layer();
                }
            });
        }
        ctx.set_transform(Affine::IDENTITY);
    }

    /// Run `draw`, which pushes filter layers. Single-threaded contexts draw directly and `draw`
    /// applies `composite` (blend mode, opacity) on its filter layer. On a multithreaded context
    /// the layer is drawn into an offscreen single-threaded context covering `reach` (document
    /// space) plus the blur's spread, then composited back with `composite`.
    fn with_filters(
        &mut self,
        ctx: &mut RenderContext,
        f: &Frame,
        reach: Rect,
        blur: f64,
        composite: Option<(vectorcraft_doc::color::BlendMode, f32)>,
        draw: impl FnOnce(&mut Self, &mut RenderContext, &Frame, Option<(vectorcraft_doc::color::BlendMode, f32)>),
    ) {
        if !f.mt {
            return draw(self, ctx, f, composite);
        }
        // 3σ of the Gaussian (σ = blur / 2) in pixels, plus a pixel of antialiasing.
        let spread = blur.max(0.0) * 1.5 / f.px + 2.0;
        let screen = Rect::new(0.0, 0.0, ctx.width() as f64, ctx.height() as f64).inflate(spread, spread);
        let r = f.view.transform_rect_bbox(reach).inflate(spread, spread).intersect(screen);
        let (x0, y0) = (r.x0.floor(), r.y0.floor());
        let (w, h) = ((r.x1.ceil() - x0).min(u16::MAX as f64), (r.y1.ceil() - y0).min(u16::MAX as f64));
        if w < 1.0 || h < 1.0 {
            return;
        }
        let (w, h) = (w as u16, h as u16);
        let mut off = crate::single_threaded_context(w, h);
        let shifted = Frame { mt: false, view: Affine::translate((-x0, -y0)) * f.view, ..*f };
        draw(self, &mut off, &shifted, None);
        off.flush();
        let mut pm = vello_cpu::Pixmap::new(w, h);
        off.render(&mut pm, &mut self.resources);
        let layered = composite.is_some_and(|(m, o)| m != vectorcraft_doc::color::BlendMode::Normal || o < 1.0);
        if let Some((m, o)) = composite.filter(|_| layered) {
            ctx.set_transform(Affine::IDENTITY);
            ctx.push_layer(None, Some(blend_mode(m)), Some(o), None, None);
        }
        ctx.set_transform(Affine::translate((x0, y0)));
        ctx.set_paint(vello_cpu::Image { image: vello_cpu::ImageSource::Pixmap(std::sync::Arc::new(pm)), sampler: peniko::ImageSampler::default() });
        ctx.fill_rect(&Rect::new(0.0, 0.0, w as f64, h as f64));
        if layered {
            ctx.pop_layer();
        }
        ctx.set_transform(Affine::IDENTITY);
    }

    /// Paint the fills and strokes of `n` on geometry `g`, applying per-item geometry effects.
    fn paint_items(&mut self, ctx: &mut RenderContext, f: &Frame, n: &Node, g: &BezPath, rule: FillRule, gctx: &GeomContext) {
        let bounds = g.bounding_box();
        for item in &n.appearance.items {
            let ig = apply(item_effects(item), g, &gctx.item(item));
            let ib = if effects::has_geometry(item_effects(item)) && !ig.elements().is_empty() { ig.bounding_box() } else { bounds };
            match item {
                AppearanceItem::Fill(fl) => {
                    if !fl.visible || fl.paint.is_none() {
                        continue;
                    }
                    let layered = fl.opacity < 1.0 || fl.blend != vectorcraft_doc::color::BlendMode::Normal;
                    if layered {
                        ctx.set_transform(Affine::IDENTITY);
                        ctx.push_layer(None, Some(blend_mode(fl.blend)), Some(fl.opacity), None, None);
                    }
                    ctx.set_transform(f.view);
                    if paint::set_paint(ctx, &fl.paint, ib, f.doc) {
                        ctx.set_fill_rule(fill_rule(rule));
                        ctx.fill_path(&ig);
                    }
                    if layered {
                        ctx.pop_layer();
                    }
                }
                AppearanceItem::Stroke(st) => {
                    if !st.visible || st.paint.is_none() || st.width <= 0.0 {
                        continue;
                    }
                    // Brushed strokes keep their brush art under effects (e.g. a glowing scatter brush).
                    if st.brush.is_some() && self.draw_brush(ctx, f, n, &ig, st) {
                        continue;
                    }
                    self.draw_stroke(ctx, f, &ig, rule, st, ib);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use vectorcraft_doc::color::{Color, Paint};
    use vectorcraft_doc::{Appearance, Document, Effect, Node};
    use vectorcraft_geom::{Affine, Rect, shapes};

    use crate::{RenderOptions, Rendered, Renderer};

    fn doc_with(r: Rect, fill: Color, effects: Vec<(&str, serde_json::Value)>) -> Document {
        let mut d = Document::new(100.0, 100.0);
        let id = d.alloc_id();
        let mut n = Node::path(id, shapes::rectangle(r), Appearance::basic(Paint::solid(fill), Paint::None, 0.0));
        n.appearance.effects = effects.into_iter().map(|(id, p)| Effect { id: id.into(), params: p, visible: true }).collect();
        let l = d.layers[0].id;
        d.insert(Some(l), 0, n).unwrap();
        d
    }

    fn render(d: &Document) -> Rendered {
        Renderer::new().render(d, 100, 100, Affine::IDENTITY, &RenderOptions { background: Some([255, 255, 255, 255]), ..Default::default() })
    }

    fn lum(p: [u8; 4]) -> u32 {
        p[0] as u32 + p[1] as u32 + p[2] as u32
    }

    const WHITE: u32 = 765;

    #[test]
    fn thumbnails_of_glowing_objects_render_without_panicking() {
        // Regression: thumbnails used vello's default (multithreaded) context, which panics on filters.
        let d = doc_with(
            Rect::new(20.0, 20.0, 60.0, 60.0),
            Color::BLACK,
            vec![("stylize.outerGlow", json!({"color": "#ff0000", "opacity": 100, "blur": 6}))],
        );
        let id = d.layers[0].children().unwrap()[0].id;
        let t = Renderer::new().render_thumbnail(&d, id, 48).unwrap();
        assert_eq!((t.width, t.height), (48, 48));
    }

    #[test]
    fn per_stroke_raster_effects_use_the_single_threaded_pipeline() {
        let mut d = doc_with(Rect::new(20.0, 20.0, 60.0, 60.0), Color::BLACK, vec![]);
        let l = d.layers[0].id;
        let id = d.layers[0].children().unwrap()[0].id;
        let mut n = (*d.node(id).unwrap()).clone();
        n.appearance = Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 4.0);
        if let Some(vectorcraft_doc::AppearanceItem::Stroke(s)) =
            n.appearance.items.iter_mut().find(|i| matches!(i, vectorcraft_doc::AppearanceItem::Stroke(_)))
        {
            s.effects =
                vec![Effect { id: "stylize.outerGlow".into(), params: json!({"color": "#ff0000", "opacity": 100, "blur": 6}), visible: true }];
        }
        d.remove(id).unwrap();
        d.insert(Some(l), 0, n).unwrap();
        let mut r = Renderer::new();
        r.threads = 4;
        let _ = r.render(&d, 100, 100, Affine::IDENTITY, &RenderOptions::default());
    }

    #[test]
    fn drop_shadow_darkens_outside_the_shape() {
        let r = Rect::new(20.0, 20.0, 60.0, 60.0);
        let plain = render(&doc_with(r, Color::rgb(1.0, 0.0, 0.0), vec![]));
        assert_eq!(lum(plain.pixel(66, 66)), WHITE);
        let d = doc_with(r, Color::rgb(1.0, 0.0, 0.0), vec![("stylize.dropShadow", json!({"x": 10, "y": 10, "blur": 2, "opacity": 100}))]);
        let img = render(&d);
        assert!(lum(img.pixel(66, 66)) < 200, "shadow below-right: {:?}", img.pixel(66, 66));
        // The object stays on top and unshadowed; the far side has no shadow.
        assert_eq!(img.pixel(40, 40), [255, 0, 0, 255]);
        assert_eq!(lum(img.pixel(15, 15)), WHITE);
    }

    #[test]
    fn shadow_of_offscreen_object_is_not_culled() {
        let d = doc_with(
            Rect::new(-60.0, 20.0, -5.0, 60.0),
            Color::WHITE,
            vec![("stylize.dropShadow", json!({"x": 30, "y": 0, "blur": 0, "opacity": 100}))],
        );
        let img = render(&d);
        let row: Vec<u32> = (0..10).map(|i| lum(img.pixel(i * 10, 40))).collect();
        assert!(lum(img.pixel(10, 40)) < 100, "{row:?}");
    }

    #[test]
    fn geometry_effect_is_rendered() {
        let r = Rect::new(30.0, 30.0, 70.0, 70.0);
        let d = doc_with(r, Color::BLACK, vec![("path.offsetPath", json!({"offset": 10}))]);
        let img = render(&d);
        assert_eq!(lum(img.pixel(25, 50)), 0, "offset grows the fill");
        let d = doc_with(r, Color::BLACK, vec![("distort.transform", json!({"moveH": -25}))]);
        let img = render(&d);
        assert_eq!(lum(img.pixel(10, 50)), 0);
        assert_eq!(lum(img.pixel(60, 50)), WHITE);
    }

    #[test]
    fn per_fill_effects_apply_to_that_fill_only() {
        let mut d = doc_with(Rect::new(30.0, 30.0, 70.0, 70.0), Color::BLACK, vec![]);
        let id = d.layers[0].children().unwrap()[0].id;
        let n = d.node_mut(id).unwrap();
        n.appearance.fill_mut().unwrap().effects.push(Effect { id: "distort.transform".into(), params: json!({"moveH": 20}), visible: true });
        let img = render(&d);
        assert_eq!(lum(img.pixel(85, 50)), 0);
        assert_eq!(lum(img.pixel(40, 50)), WHITE);
    }

    #[test]
    fn blur_glow_and_feather_soften() {
        let r = Rect::new(30.0, 30.0, 70.0, 70.0);
        let blur = render(&doc_with(r, Color::BLACK, vec![("blur.gaussian", json!({"radius": 6}))]));
        let edge = lum(blur.pixel(29, 50));
        assert!(edge > 0 && edge < WHITE, "blur spreads over the edge: {edge}");
        let glow =
            render(&doc_with(r, Color::BLACK, vec![("stylize.outerGlow", json!({"color": "#ff0000", "mode": "normal", "opacity": 100, "blur": 4}))]));
        let g = glow.pixel(27, 50);
        assert!(g[0] > g[1] + 20, "outer glow is red: {g:?}");
        let inner =
            render(&doc_with(r, Color::BLACK, vec![("stylize.innerGlow", json!({"color": "#ffffff", "mode": "normal", "opacity": 100, "blur": 4}))]));
        assert!(lum(inner.pixel(31, 50)) > 60, "inner glow lightens the edge: {:?}", inner.pixel(31, 50));
        assert!(lum(inner.pixel(50, 50)) < 30, "centre stays dark");
        let feather = render(&doc_with(r, Color::BLACK, vec![("stylize.feather", json!({"radius": 8}))]));
        assert!(lum(feather.pixel(31, 50)) > 20, "feather fades the edge inward");
        assert_eq!(lum(feather.pixel(26, 50)), WHITE, "feather stays inside");
    }

    #[test]
    fn hidden_effects_are_ignored_and_outline_mode_uses_effected_geometry() {
        let r = Rect::new(30.0, 30.0, 70.0, 70.0);
        let mut d = doc_with(r, Color::BLACK, vec![("distort.transform", json!({"moveH": -25}))]);
        let id = d.layers[0].children().unwrap()[0].id;
        let opts = RenderOptions { background: Some([255, 255, 255, 255]), outline: true, ..Default::default() };
        let img = Renderer::new().render(&d, 100, 100, Affine::IDENTITY, &opts);
        assert!(lum(img.pixel(5, 50)) < 400, "outline follows the moved path");
        d.node_mut(id).unwrap().appearance.effects[0].visible = false;
        let img = render(&d);
        assert_eq!(lum(img.pixel(50, 50)), 0);
        assert_eq!(lum(img.pixel(10, 50)), WHITE);
    }

    #[test]
    fn visual_bounds_include_shadow() {
        let d = doc_with(Rect::new(0.0, 0.0, 10.0, 10.0), Color::BLACK, vec![("stylize.dropShadow", json!({"x": 7, "y": 7, "blur": 5}))]);
        let n = &d.layers[0].children().unwrap()[0];
        let b = super::visual_bounds(n).unwrap();
        assert!(b.x1 >= 10.0 + 7.0 + 7.5 - 1e-9);
    }

    /// Multithreaded rendering filters each effect offscreen; it must match the single-threaded
    /// reference (blend modes against the real backdrop, objects partly off-screen, all effects).
    #[test]
    fn multithreaded_filters_match_single_threaded() {
        let cases: Vec<(Rect, Vec<(&str, serde_json::Value)>)> = vec![
            (Rect::new(20.0, 20.0, 60.0, 60.0), vec![("stylize.dropShadow", json!({"x": 6, "y": 4, "blur": 5, "opacity": 75}))]),
            (Rect::new(20.0, 20.0, 60.0, 60.0), vec![("stylize.outerGlow", json!({"color": "#ff00aa", "blur": 8, "opacity": 90}))]),
            (
                Rect::new(20.0, 20.0, 60.0, 60.0),
                vec![
                    ("stylize.innerGlow", json!({"blur": 6})),
                    ("stylize.innerGlow", json!({"blur": 4, "source": "center", "mode": "multiply", "color": "#0044ff"})),
                ],
            ),
            (Rect::new(20.0, 20.0, 60.0, 60.0), vec![("stylize.feather", json!({"radius": 6}))]),
            (Rect::new(20.0, 20.0, 60.0, 60.0), vec![("blur.gaussian", json!({"radius": 4})), ("stylize.dropShadow", json!({"blur": 3}))]),
            // Partly outside the frame: the blur still spreads in from off-screen content.
            (
                Rect::new(-30.0, 70.0, 30.0, 130.0),
                vec![("stylize.outerGlow", json!({"blur": 12, "mode": "normal"})), ("blur.gaussian", json!({"radius": 3}))],
            ),
        ];
        for (r, fx) in cases {
            let name = format!("{fx:?}");
            let mut d = doc_with(r, Color::rgb(0.2, 0.6, 0.3), fx);
            // A coloured backdrop so blend modes have something to blend with.
            let l = d.layers[0].id;
            let bid = d.alloc_id();
            let bg = Node::path(
                bid,
                shapes::rectangle(Rect::new(0.0, 0.0, 100.0, 100.0)),
                Appearance::basic(Paint::solid(Color::rgb(0.9, 0.5, 0.1)), Paint::None, 0.0),
            );
            d.insert(Some(l), 0, bg).unwrap();
            let opts = RenderOptions::default();
            let mut st = Renderer::new();
            st.threads = 0;
            let mut mt = Renderer::new();
            mt.threads = 3;
            let a = st.render(&d, 100, 100, Affine::IDENTITY, &opts);
            let b = mt.render(&d, 100, 100, Affine::IDENTITY, &opts);
            let worst = a.pixels.iter().zip(&b.pixels).map(|(x, y)| x.abs_diff(*y)).max().unwrap();
            // The offscreen pass adds one 8-bit premultiplied round trip: a few levels of rounding.
            assert!(worst <= 4, "{name}: max channel difference {worst}");
        }
    }
}
