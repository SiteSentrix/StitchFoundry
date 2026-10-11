//! Appliqué: placement line → stop → tack-down → stop → cover stitch.

use super::out::Out;
use super::run::{CORNER_ANGLE, blanket};
use super::satin;
use crate::geom::{FLATTEN_TOLERANCE, Point, Shape, poly, polyline};
use crate::model::{AppliqueParams, CoverKind, TackdownKind};
use crate::pattern::Stitch;

fn rings_from(shape_polys: &[crate::geom::Polygon], here: Option<Point>) -> Vec<Vec<Point>> {
    let mut rings: Vec<Vec<Point>> = shape_polys.iter().flat_map(|p| p.rings().cloned().collect::<Vec<_>>()).collect();
    // Order rings nearest-first from `here`, each starting at its closest point.
    let mut ordered = Vec::new();
    let mut cur = here;
    while !rings.is_empty() {
        let i = match cur {
            Some(h) => (0..rings.len())
                .min_by(|&a, &b| polyline::distance_to_ring(&rings[a], h).total_cmp(&polyline::distance_to_ring(&rings[b], h)))
                .unwrap_or(0),
            None => 0,
        };
        let r = rings.remove(i);
        let r = match cur {
            Some(h) => polyline::ring_starting_near(&r, h),
            None => r,
        };
        cur = r.first().copied();
        ordered.push(polyline::closed(&r));
    }
    ordered
}

pub(crate) fn generate(shape: &Shape, p: &AppliqueParams, hint: Option<Point>, trim: f64) -> Vec<Stitch> {
    let base = poly::clean(&[shape.polygon(FLATTEN_TOLERANCE)]);
    if base.is_empty() {
        return Vec::new();
    }
    let mut out = Out::new();

    // 1. Placement line, then stop for the fabric to be laid down.
    let rings = rings_from(&base, hint);
    if p.placement {
        for r in &rings {
            out.connect(r[0], trim, 2.5);
            out.stitches(polyline::resample(r, 2.5, CORNER_ANGLE));
        }
        out.stop();
    }

    // 2. Tack-down slightly inside the outline.
    if p.tackdown != TackdownKind::None {
        let inset = poly::offset(&base, -p.tackdown_inset.max(0.0));
        let tack_polys = if inset.is_empty() { base.clone() } else { inset };
        for r in rings_from(&tack_polys, out.pos()) {
            out.connect(r[0], trim, 2.5);
            match p.tackdown {
                TackdownKind::Run => out.stitches(polyline::resample(&r, 2.0, CORNER_ANGLE)),
                TackdownKind::Zigzag => out.stitches(super::run::zigzag(&r, p.tackdown_width, 1.6)),
                TackdownKind::Blanket => out.stitches(blanket(&r, 2.5, p.tackdown_width, 2.0, false)),
                TackdownKind::None => {}
            }
        }
        if p.trim_stop {
            out.stop();
        }
    }

    // 3. Cover stitch centred on the outline.
    match p.cover {
        CoverKind::None => {}
        CoverKind::Satin => {
            for r in rings_from(&base, out.pos()) {
                // light centre-run underlay keeps the cover crisp
                out.connect(r[0], trim, 2.5);
                let pts = satin::border(&r, true, p.cover_width, p.cover_spacing, p.pull_comp);
                if let Some(&p0) = pts.first() {
                    out.run_to(p0, 2.5);
                }
                out.stitches(pts);
            }
        }
        CoverKind::Blanket => {
            for r in rings_from(&base, out.pos()) {
                out.connect(r[0], trim, 2.5);
                out.stitches(blanket(&r, 2.5, p.cover_width, 2.2, false));
            }
        }
    }
    out.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::Cmd;

    #[test]
    fn applique_sequence_has_two_stops() {
        let shape = Shape::ellipse(Point::new(0.0, 0.0), 20.0, 15.0);
        let st = generate(&shape, &AppliqueParams::default(), None, 2.0);
        assert_eq!(st.iter().filter(|s| s.cmd == Cmd::Stop).count(), 2);
        assert!(st.iter().filter(|s| s.cmd == Cmd::Stitch).count() > 300);
    }
}
