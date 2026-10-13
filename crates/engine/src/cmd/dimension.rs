//! Linked dimensions: generated groups share the existing text/path render and export pipeline.
use super::*;
use serde_json::{Value, json};
use std::sync::Arc;
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::dimension::{Dimension, DimensionOptions, DimensionSource, SegmentRef};
use vectorcraft_doc::{Appearance, Arrowhead, CharStyle, Dash, Document, Node, NodeId, NodeKind, TextObject};
use vectorcraft_geom::{Affine, BezPath, PathData, Point, Vec2};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "dimension.create",
            "Create Dimension",
            [],
            None,
            "{mode?: linear|angular|radius|diameter, a?: [x,y], b?: [x,y], vertex?: [x,y], source?: {kind: segment, edge: {node,subpath,segment}}|{kind: angle,first: edge,second: edge}|{kind: circle,node}, offset?: pt, options?: {units,precision,scale,hideUnits,lineWeight,lineType,lineColor,arrowStyle,arrowScale,extensionWeight,extensionType,extensionColor,extensionOffset,extensionLength,hideExtensions,fontFamily,fontStyle,fontSize,textColor,textPosition,textAlignment,textOffset}} → {id,label}; scale is drawing:real; linked measurements follow source edits",
            has_doc,
            create
        ),
        cmd!(
            "dimension.selection",
            "Dimension Selected Paths",
            ["Object", "Dimensions"],
            None,
            "{offset?: pt = 30, options?: dimension options} add a linked dimension to every segment of the selected paths (up to 1000), as one undo step → {ids}",
            has_selection,
            selection
        ),
        cmd!(
            "dimension.update",
            "Update Dimensions",
            [],
            None,
            "{ids?: [dimension ids] (default selection), options?: dimension options, offset?: pt} edit existing dimensions → {ids}",
            has_doc,
            update
        ),
    ]
}

fn error(s: impl ToString) -> EngineError {
    EngineError::Other(s.to_string())
}

fn options(s: &Session, p: &Value) -> Result<DimensionOptions> {
    let default = s.tool_options_of("dimension");
    let value = p.get("options").cloned().unwrap_or_else(|| default.get("style").cloned().unwrap_or(json!({})));
    let o: DimensionOptions = serde_json::from_value(value).map_err(error)?;
    o.validate().map_err(error)?;
    Ok(o)
}

fn point(p: &Value, key: &str) -> Result<Point> {
    let a = p.get(key).and_then(Value::as_array).filter(|a| a.len() == 2).ok_or_else(|| error(format!("{key} must be [x,y]")))?;
    let x = a.first().and_then(Value::as_f64).ok_or_else(|| error("Invalid point"))?;
    let y = a.get(1).and_then(Value::as_f64).ok_or_else(|| error("Invalid point"))?;
    Ok(Point::new(x, y))
}

fn distance(p: &Value, fallback: f64) -> Result<f64> {
    match p.get("offset") {
        Some(v) => v.as_f64().filter(|n| n.is_finite() && n.abs() <= 1e6).ok_or_else(|| error("Invalid dimension offset")),
        None => Ok(fallback),
    }
}

/// Return the signed winding area of a closed path subpath. Positive area means
/// the path interior is to the left of its directed segments, which lets the
/// automatic dimension command put its positive (left-normal) offset outside.
fn subpath_area(path: &PathData, subpath: usize) -> Option<f64> {
    use vectorcraft_geom::kurbo::ParamCurve;
    let sp = path.subpaths.get(subpath)?;
    if !sp.closed || sp.segment_count() == 0 {
        return None;
    }
    let mut area = 0.0;
    for index in 0..sp.segment_count() {
        let segment = sp.segment(index);
        let mut previous = segment.eval(0.0);
        for step in 1..=16 {
            let current = segment.eval(step as f64 / 16.0);
            area += previous.x * current.y - current.x * previous.y;
            previous = current;
        }
    }
    (area.is_finite() && area.abs() > 1e-9).then_some(area * 0.5)
}

fn recipe(s: &Session, p: &Value) -> Result<Dimension> {
    let mode = p.get("mode").map_or(Ok("linear"), |v| v.as_str().ok_or_else(|| error("Invalid dimension mode")))?;
    let source: Option<DimensionSource> = p.get("source").map(|v| serde_json::from_value(v.clone()).map_err(error)).transpose()?;
    if let Some(source) = &source {
        let doc = &s.doc()?.doc;
        for id in source.nodes() {
            if doc.ancestry(id).is_some_and(|path| path.iter().any(|id| doc.node(*id).is_some_and(|n| n.extras().dimension.is_some()))) {
                return Err(error("Choose original artwork, not a dimension's generated paths"));
            }
        }
    }
    if let Some(source) = &source {
        let compatible = matches!(
            (mode, source),
            ("linear", DimensionSource::Segment { .. })
                | ("angular", DimensionSource::Angle { .. })
                | ("radius" | "diameter", DimensionSource::Circle { .. })
        );
        if !compatible && !matches!((mode, source), ("linear" | "angular", DimensionSource::Points { .. })) {
            return Err(error("Dimension mode does not match its source"));
        }
    }
    let a = if source.is_none() || p.get("a").is_some() { point(p, "a")? } else { Point::ZERO };
    let b = if source.is_none() || p.get("b").is_some() { point(p, "b")? } else { Point::new(1.0, 0.0) };
    let vertex = if mode == "angular" && (source.is_none() || matches!(source, Some(DimensionSource::Points { .. }))) {
        point(p, "vertex")?
    } else {
        Point::ZERO
    };
    let mut d = Dimension { mode: mode.into(), a, b, vertex, offset: distance(p, 30.0)?, length: a.distance(b), options: options(s, p)?, source };
    d.resolve(&s.doc()?.doc).map_err(error)?;
    Ok(d)
}

fn insert(doc: &mut Document, parent: Option<NodeId>, dim: Dimension) -> Result<NodeId> {
    let id = doc.alloc_id();
    let mut group = Node::new(id, NodeKind::Group { children: vec![], clip: false });
    group.name = Some(format!("Dimension · {}", dim.label().map_err(error)?));
    group.kind = NodeKind::Group { children: artwork(doc, &dim, &[])?, clip: false };
    group.extras_mut().dimension = Some(Box::new(dim));
    doc.insert(parent, usize::MAX, group)?;
    Ok(id)
}

fn create(s: &mut Session, p: &Value) -> Result<Value> {
    let dim = recipe(s, p)?;
    let label = dim.label().map_err(error)?;
    let parent = s.doc()?.insertion_parent();
    s.note_journal("options", serde_json::to_value(&dim.options).map_err(error)?);
    let id = s.edit("Create Dimension", |doc, sel| {
        let id = insert(doc, parent, dim)?;
        sel.set([id]);
        Ok(id)
    })?;
    Ok(json!({"id":id.0,"label":label}))
}

fn selection(s: &mut Session, p: &Value) -> Result<Value> {
    let o = options(s, p)?;
    let offset = distance(p, 30.0)?;
    let st = s.doc()?;
    let mut refs = Vec::new();
    fn gather(n: &Node, refs: &mut Vec<SegmentRef>) -> Result<()> {
        if !n.visible || n.locked || n.extras().dimension.is_some() {
            return Ok(());
        }
        if let Some(path) = n.path_data() {
            for (si, sp) in path.subpaths.iter().enumerate() {
                for seg in 0..sp.segment_count() {
                    if refs.len() >= 1000 {
                        return Err(error("Select at most 1000 segments at a time"));
                    }
                    if sp.segment(seg).p0.distance(sp.segment(seg).p3) > 1e-6 {
                        refs.push(SegmentRef { node: n.id, subpath: si, segment: seg });
                    }
                }
            }
        }
        if let Some(children) = n.children() {
            for c in children {
                gather(c, refs)?;
            }
        }
        Ok(())
    }
    for id in &st.selection.objects {
        if !st.doc.is_editable(*id) {
            continue;
        }
        if let Some(n) = st.doc.node(*id) {
            gather(n, &mut refs)?;
        }
    }
    refs.sort_by_key(|r| (r.node, r.subpath, r.segment));
    refs.dedup();
    if refs.is_empty() {
        return Err(error("Select one or more paths first"));
    }
    let parent = st.insertion_parent();
    let offsets: Vec<f64> = refs
        .iter()
        .map(|edge| {
            st.doc
                .node(edge.node)
                .and_then(Node::path_data)
                .and_then(|path| subpath_area(path, edge.subpath))
                .map_or(offset, |area| -area.signum() * offset.abs())
        })
        .collect();
    s.note_journal("options", serde_json::to_value(&o).map_err(error)?);
    let ids = s.edit("Dimension Selected Paths", |doc, sel| {
        let mut ids = Vec::new();
        for (edge, edge_offset) in refs.into_iter().zip(offsets) {
            let mut dim = Dimension {
                mode: "linear".into(),
                a: Point::ZERO,
                b: Point::ZERO,
                vertex: Point::ZERO,
                offset: edge_offset,
                length: 0.0,
                options: o.clone(),
                source: Some(DimensionSource::Segment { edge }),
            };
            dim.resolve(doc).map_err(error)?;
            ids.push(insert(doc, parent, dim)?);
        }
        sel.set(ids.clone());
        Ok(ids)
    })?;
    Ok(json!({"ids":ids}))
}

fn update(s: &mut Session, p: &Value) -> Result<Value> {
    let ids: Vec<NodeId> = match p.get("ids") {
        Some(v) => serde_json::from_value(v.clone()).map_err(error)?,
        None => s.doc()?.selection.objects.clone(),
    };
    if ids.is_empty() || ids.len() > 1000 {
        return Err(error("Select dimensions to update (at most 1000)"));
    }
    let o = if p.get("options").is_some() { Some(options(s, p)?) } else { None };
    s.edit("Update Dimensions", |doc, _| {
        for id in &ids {
            if !doc.is_editable(*id) {
                return Err(error("Dimension is locked or hidden"));
            }
            let mut dim =
                doc.node(*id).and_then(|n| n.extras().dimension.as_deref()).cloned().ok_or_else(|| error("Select only dimension groups"))?;
            if let Some(o) = &o {
                dim.options = o.clone();
            }
            dim.offset = distance(p, dim.offset)?;
            dim.resolve(doc).map_err(error)?;
            replace(doc, *id, dim)?;
        }
        Ok(())
    })?;
    Ok(json!({"ids":ids}))
}

fn replace(doc: &mut Document, id: NodeId, dim: Dimension) -> Result<()> {
    let old: Vec<NodeId> = doc.node(id).and_then(Node::children).map(|ch| ch.iter().map(|n| n.id).collect()).unwrap_or_default();
    let children = artwork(doc, &dim, &old)?;
    if let Some(n) = doc.node_mut(id) {
        n.kind = NodeKind::Group { children, clip: false };
        n.name = Some(format!("Dimension · {}", dim.label().map_err(error)?));
        n.extras_mut().dimension = Some(Box::new(dim));
    }
    Ok(())
}

/// Called inside the source edit transaction, so its dimensions and undo history stay in sync.
pub(crate) fn refresh(before: &Document, doc: &mut Document) -> Result<()> {
    let mut dimensions = Vec::new();
    doc.walk(|n| {
        if let Some(d) = &n.extras().dimension {
            dimensions.push((n.id, (**d).clone()));
        }
    });
    for (id, old) in dimensions {
        let mut next = old.clone();
        if next.source.as_ref().is_some_and(|s| s.topology_changed(before, doc)) || next.resolve(doc).is_err() {
            // A deleted/degenerate source has no valid measurement. Remove its annotation in
            // this same undo step instead of keeping a silently wrong number.
            doc.remove(id)?;
        } else if next != old || before.node(id).and_then(|n| n.extras().dimension.as_deref()).is_some_and(|d| d != &next) {
            replace(doc, id, next)?;
        }
    }
    Ok(())
}

fn paint(c: [u8; 3]) -> Paint {
    Paint::solid(Color::rgb8(c[0], c[1], c[2]))
}
fn line(a: Point, b: Point) -> BezPath {
    let mut p = BezPath::new();
    p.move_to(a);
    p.line_to(b);
    p
}

fn appearance(o: &DimensionOptions, ext: bool, arrows: bool) -> Appearance {
    let (color, width, kind) =
        if ext { (o.extension_color, o.extension_weight, o.extension_type.as_str()) } else { (o.line_color, o.line_weight, o.line_type.as_str()) };
    let mut a = Appearance::basic(Paint::None, paint(color), width);
    if let Some(st) = a.stroke_mut() {
        st.dash = match kind {
            "dashed" => Some(Dash { pattern: vec![width * 5.0, width * 3.0], ..Default::default() }),
            "dotted" => {
                st.cap = vectorcraft_doc::LineCap::Round;
                Some(Dash { pattern: vec![0.0, width * 3.0], ..Default::default() })
            }
            _ => None,
        };
        if arrows {
            let head = match o.arrow_style.as_str() {
                "filled" => Some(Arrowhead::Triangle),
                "open" => Some(Arrowhead::TriangleOpen),
                "tick" => Some(Arrowhead::Slash),
                _ => None,
            };
            st.start_arrow = head;
            st.end_arrow = head;
            st.arrow_scale = (o.arrow_scale, o.arrow_scale);
            st.arrow_align = vectorcraft_doc::appearance::ArrowAlign::Tip;
        }
    }
    a
}

fn artwork(doc: &mut Document, d: &Dimension, old: &[NodeId]) -> Result<Vec<Arc<Node>>> {
    d.validate().map_err(error)?;
    let o = &d.options;
    let label = d.label().map_err(error)?;
    let mut paths: Vec<(BezPath, Appearance)> = Vec::new();
    let mut angle = 0.0;
    let text_at;
    let side = if d.offset < 0.0 { -1.0 } else { 1.0 };
    let text_shift = match o.text_position.as_str() {
        "below" => o.text_offset + o.font_size,
        "center" => o.font_size * 0.35,
        _ => -o.text_offset,
    };
    if d.mode == "angular" {
        let start = (d.a - d.vertex).atan2();
        let sweep = d.angle();
        let r = d.offset.abs().max(12.0);
        let on = |t: f64| d.vertex + Vec2::from_angle(start + sweep * t) * r;
        let mut arc = BezPath::new();
        arc.move_to(on(0.0));
        for i in 1..=64 {
            arc.line_to(on(i as f64 / 64.0));
        }
        paths.push((arc, appearance(o, false, true)));
        if !o.hide_extensions {
            for p in [d.a, d.b] {
                let dir = (p - d.vertex) / (p - d.vertex).hypot();
                paths.push((line(d.vertex + dir * o.extension_offset, d.vertex + dir * (r + o.extension_length)), appearance(o, true, false)));
            }
        }
        text_at = d.vertex + Vec2::from_angle(start + sweep * 0.5) * (r + o.text_offset + o.font_size * 0.5);
    } else {
        let delta = d.b - d.a;
        let u = delta / delta.hypot();
        let n = Vec2::new(-u.y, u.x);
        let (a, b) = match d.mode.as_str() {
            "radius" => (d.a, d.b),
            "diameter" => (d.a - delta, d.b),
            _ => (d.a + n * d.offset, d.b + n * d.offset),
        };
        if d.mode == "linear" && !o.hide_extensions {
            for (p, q) in [(d.a, a), (d.b, b)] {
                paths.push((line(p + n * (o.extension_offset * side), q + n * (o.extension_length * side)), appearance(o, true, false)));
            }
        }
        angle = u.atan2();
        if !(-std::f64::consts::FRAC_PI_2..=std::f64::consts::FRAC_PI_2).contains(&angle) {
            angle += std::f64::consts::PI;
        }
        let normal = Vec2::new(-angle.sin(), angle.cos());
        text_at = a.midpoint(b) + normal * text_shift;
        paths.push((line(a, b), appearance(o, false, true)));
    }
    if o.text_alignment == "horizontal" {
        angle = 0.0;
    }
    let style = CharStyle {
        font_family: o.font_family.clone(),
        font_style: o.font_style.clone(),
        size: o.font_size,
        fill: paint(o.text_color),
        ..Default::default()
    };
    let mut text = TextObject::point(Point::ZERO, &label, style);
    text.para.justify = vectorcraft_doc::Justify::Center;
    super::typecmd::refresh_bounds(&mut text);
    text.xf = Affine::translate(text_at.to_vec2()) * Affine::rotate(angle);
    // Split the centre line around the label rather than paint an opaque white patch over art.
    if o.text_position == "center"
        && d.mode != "angular"
        && let Some((path, app)) = paths.pop()
    {
        let elems = path.elements();
        if let (Some(vectorcraft_geom::PathEl::MoveTo(a)), Some(vectorcraft_geom::PathEl::LineTo(b))) = (elems.first(), elems.get(1)) {
            let delta = *b - *a;
            let len = delta.hypot();
            let u = delta / len;
            let bounds = text.cached_bounds.unwrap_or_else(|| text.estimate_bounds());
            let gap = (bounds.width() * 0.5 + 4.0).min(len * 0.45);
            let mid = a.midpoint(*b);
            let mut left = app.clone();
            if let Some(st) = left.stroke_mut() {
                st.end_arrow = None;
            }
            let mut right = app;
            if let Some(st) = right.stroke_mut() {
                st.start_arrow = None;
            }
            paths.push((line(*a, mid - u * gap), left));
            paths.push((line(mid + u * gap, *b), right));
        }
    }
    let mut children = Vec::new();
    for (path, a) in paths {
        let id = old.get(children.len()).copied().unwrap_or_else(|| doc.alloc_id());
        children.push(Arc::new(Node::path(id, PathData::from_bezpath(&path), a)));
    }
    let id = old.get(children.len()).copied().unwrap_or_else(|| doc.alloc_id());
    let mut node = Node::new(id, NodeKind::Text(Box::new(text)));
    node.appearance = Appearance::basic(paint(o.text_color), Paint::None, 0.0).into();
    children.push(Arc::new(node));
    Ok(children)
}
