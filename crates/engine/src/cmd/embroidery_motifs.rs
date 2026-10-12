//! Single motifs (`embroidery.placeMotif`): one motif placed as a path sewn in a single run,
//! ready to move, scale and reshape like any art.

use serde_json::{Value, json};
use sf_core::attr::{Embroidery, StitchType};
use sf_core::model::{Motif, RunKind, RunParams};
use vectorcraft_color::{Color, Paint};
use vectorcraft_doc::{Appearance, Node};
use vectorcraft_embroidery::PT_PER_MM;
use vectorcraft_geom::path::{PathData, SubPath};

use super::*;

const CMD: &str = "embroidery.placeMotif";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "embroidery.placeMotif",
        "Place Motif",
        [],
        None,
        "{motif: wave|scallop|diamond|cross|feather|heart|star|leaf|loop, size?: mm (its length, 10), at?: [x, y] (its centre; default: the first artboard's centre), angle?: degrees} one motif as a path sewn in a single run, ready to move, scale and reshape → {id}",
        has_doc,
        place_motif
    )]
}

fn place_motif(s: &mut Session, p: &Value) -> Result<Value> {
    let name = str_param(p, "motif").ok_or_else(|| bad(CMD, "missing `motif`"))?;
    let motif = Motif::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(name)).ok_or_else(|| bad(CMD, format!("unknown motif `{name}`")))?;
    let size = match p.get("size") {
        None => 10.0,
        Some(v) => v.as_f64().filter(|v| (1.0..=500.0).contains(v)).ok_or_else(|| bad(CMD, "`size` is 1 to 500 mm"))?,
    };
    let angle = f64_or(p, "angle", 0.0);
    if !angle.is_finite() {
        return Err(bad(CMD, "`angle` must be a number of degrees"));
    }
    let at = match p.get("at") {
        None => s.doc()?.doc.artboards.first().map_or(Point::new(0.0, 0.0), |a| a.rect.center()),
        Some(_) => point_param(p, "at").filter(|q| q.x.is_finite() && q.y.is_finite()).ok_or_else(|| bad(CMD, "`at` is [x, y]"))?,
    };
    // The motif `size` long, centred on `at` and turned by `angle` (y down, so up is −y).
    let k = size * PT_PER_MM;
    let (sin, cos) = angle.to_radians().sin_cos();
    let pts: Vec<Point> = sf_core::engine::motif_line(motif)
        .iter()
        .map(|&(a, c)| {
            let (x, y) = ((a - 0.5) * k, -c * k);
            Point::new(at.x + x * cos - y * sin, at.y + x * sin + y * cos)
        })
        .collect();
    let path = PathData::single(SubPath::polyline(&pts, false));
    // Short stitches, so a small motif keeps its shape.
    let run = RunParams { kind: RunKind::Single, stitch_length: (size / 8.0).clamp(1.0, 2.5), ..RunParams::default() };
    let paint = [&s.paint.stroke, &s.paint.fill]
        .into_iter()
        .find(|p| matches!(p, Paint::Solid { .. }))
        .cloned()
        .unwrap_or_else(|| Paint::solid(Color::rgb8(0x1a, 0x3e, 0x8c)));
    let parent = s.doc()?.insertion_parent();
    let id = s.edit("Place Motif", |d, sel| {
        let id = d.alloc_id();
        let mut n = Node::path(id, path.clone(), Appearance::basic(Paint::None, paint.clone(), 0.75));
        n.name = Some(motif.label().to_string());
        n.embroidery = Some(Box::new(Embroidery::new(StitchType::Run(run.clone()))));
        d.insert(parent, usize::MAX, n)?;
        sel.set([id]);
        Ok(id)
    })?;
    Ok(json!({ "id": id.0 }))
}
