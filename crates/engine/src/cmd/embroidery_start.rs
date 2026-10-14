//! Getting started: a new design the size of a hoop, and sample designs to open, look at and
//! sew (made here from the built-in clipart and shapes, so they need no fonts or files).

use serde_json::{Value, json};
use vectorcraft_embroidery::PT_PER_MM;

use super::*;

/// The sample designs: id, name, the hoop they are made for.
const SAMPLES: &[(&str, &str, &str)] = &[
    ("heart-patch", "Heart patch", "Brother 100 × 100 (4×4)"),
    ("sun-and-moon", "Sun and moon", "Brother 130 × 180 (5×7)"),
    ("applique-star", "Appliqué star", "Brother 100 × 100 (4×4)"),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "embroidery.newDesign",
            "New Design for Hoop",
            [],
            None,
            "{hoop?: a hoop's name (embroidery.hoops) | {width, height, shape?: rect|oval} (mm) (default: My Machine's everyday hoop)} a new document whose page is the hoop's sewing field, the hoop set, and My Machine's settings when one is chosen → {hoop, width, height} (mm)",
            always,
            new_design
        ),
        cmd!(query "embroidery.samples", "Sample Designs", [], None, "{} the sample designs → {samples: [{id, name, hoop}]}", always, samples),
        cmd!(
            "embroidery.sample",
            "Open Sample Design",
            [],
            None,
            "{id: heart-patch|sun-and-moon|applique-star} a sample design, digitized, in a new document → {id, name, objects}",
            always,
            sample
        ),
    ]
}

fn new_design(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.newDesign";
    // no hoop: My Machine's everyday one
    let mine = super::embroidery_my_machine::mine(s);
    let fallback = mine.and_then(|m| m.hoops.first()).map(|h| Value::String((*h).to_string()));
    let hoop_param = p.get("hoop").cloned().or(fallback);
    let (name, w, h) = match &hoop_param {
        Some(Value::String(name)) => {
            let hoop = super::embroidery_hoops::find_hoop(s, name).ok_or_else(|| bad(CMD, format!("unknown hoop `{name}`")))?;
            (hoop.name.clone(), hoop.width, hoop.height)
        }
        Some(h @ Value::Object(_)) => {
            let dim = |k: &str| h.get(k).and_then(Value::as_f64).filter(|v| v.is_finite() && (10.0..=2000.0).contains(v));
            let (Some(w), Some(ht)) = (dim("width"), dim("height")) else { return Err(bad(CMD, "a hoop's width and height are 10 to 2000 mm")) };
            (str_param(h, "name").unwrap_or("Custom").to_string(), w, ht)
        }
        _ => return Err(bad(CMD, "`hoop` is a hoop's name or {width, height, shape?} (or choose My Machine)")),
    };
    s.execute("file.new", &json!({ "width": w * PT_PER_MM, "height": h * PT_PER_MM }))?;
    s.execute("embroidery.settings", &json!({ "hoop": hoop_param }))?;
    if let Some(m) = mine {
        s.execute("embroidery.settings", &json!({ "machine": m.format }))?;
    }
    Ok(json!({ "hoop": name, "width": w, "height": h }))
}

fn samples(_: &mut Session, _: &Value) -> Result<Value> {
    Ok(json!({ "samples": SAMPLES.iter().map(|(id, name, hoop)| json!({ "id": id, "name": name, "hoop": hoop })).collect::<Vec<_>>() }))
}

fn sample(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.sample";
    let id = str_param(p, "id").ok_or_else(|| bad(CMD, "missing `id`: see embroidery.samples"))?;
    let &(id, name, hoop) = SAMPLES.iter().find(|(i, _, _)| *i == id).ok_or_else(|| bad(CMD, "no such sample: see embroidery.samples"))?;
    let r = s.execute("embroidery.newDesign", &json!({ "hoop": hoop }))?;
    let (w, h) = (r["width"].as_f64().unwrap_or(100.0), r["height"].as_f64().unwrap_or(100.0));
    let at = |x: f64, y: f64| json!([x * PT_PER_MM, y * PT_PER_MM]);
    match id {
        "heart-patch" => {
            // a heart, a satin border round it like a patch's
            s.execute("embroidery.clipart.insert", &json!({ "name": "Heart", "size": 64, "at": at(w / 2.0, h / 2.0) }))?;
            s.execute("embroidery.outlineAround", &json!({ "gap": 3, "width": 3, "color": "#7a1020" }))?;
        }
        "sun-and-moon" => {
            s.execute("embroidery.clipart.insert", &json!({ "name": "Sun", "size": 70, "at": at(w / 2.0, h * 0.3) }))?;
            s.execute("embroidery.clipart.insert", &json!({ "name": "Moon", "size": 60, "at": at(w / 2.0, h * 0.72) }))?;
        }
        _ => {
            // a star cut from gingham, a small heart sewn on it
            let (cx, cy) = (w / 2.0 * PT_PER_MM, h / 2.0 * PT_PER_MM);
            s.execute("shape.star", &json!({ "cx": cx, "cy": cy, "radius1": 42.0 * PT_PER_MM, "radius2": 20.0 * PT_PER_MM, "points": 5 }))?;
            s.execute("embroidery.set", &json!({ "type": "applique" }))?;
            s.execute(
                "embroidery.setParams",
                &json!({ "params": { "fabric_pattern": "Check", "fabric_color": "#2a6fdb", "fabricColor2": "#ffffff", "cover_width": 2.5 } }),
            )?;
            s.execute("embroidery.clipart.insert", &json!({ "name": "Heart", "size": 18, "at": at(w / 2.0, h / 2.0 + 3.0) }))?;
        }
    }
    s.execute("select.none", &json!({}))?;
    let objects = s.embroidery_plan().map_or(0, |plan| plan.design.objects.len());
    Ok(json!({ "id": id, "name": name, "objects": objects }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_designs_fit_their_hoop_and_samples_open_digitized() {
        let mut s = Session::new();
        let r = s.execute("embroidery.newDesign", &json!({"hoop": "Brother 130 × 180 (5×7)"})).unwrap();
        assert_eq!((r["width"].as_f64(), r["height"].as_f64()), (Some(130.0), Some(180.0)));
        let ab = s.doc().unwrap().doc.artboards[0].rect;
        assert!((ab.width() - 130.0 * PT_PER_MM).abs() < 0.01 && (ab.height() - 180.0 * PT_PER_MM).abs() < 0.01);
        assert_eq!(s.execute("embroidery.info", &json!({})).unwrap()["hoop"], "Brother 130 × 180 (5×7)");
        let r = s.execute("embroidery.newDesign", &json!({"hoop": {"width": 150, "height": 150, "name": "Mine"}})).unwrap();
        assert_eq!(r["hoop"], "Mine");
        assert!(s.execute("embroidery.newDesign", &json!({"hoop": "No such hoop"})).is_err());
        assert!(s.execute("embroidery.newDesign", &json!({"hoop": {"width": f64::NAN, "height": 5}})).is_err());
        let list = s.execute("embroidery.samples", &json!({})).unwrap();
        for sm in list["samples"].as_array().unwrap() {
            let docs = s.documents().len();
            let r = s.execute("embroidery.sample", &json!({"id": sm["id"]})).unwrap();
            assert_eq!(s.documents().len(), docs + 1);
            assert!(r["objects"].as_u64().unwrap() >= 2, "{r}");
            // every stitch inside the hoop's field
            let plan = s.embroidery_plan().unwrap();
            let ab = s.doc().unwrap().doc.artboards[0].rect;
            let (w, h) = (ab.width() / PT_PER_MM, ab.height() / PT_PER_MM);
            for st in plan.pattern().stitches.iter().filter(|st| st.cmd == sf_core::Cmd::Stitch) {
                assert!(st.x > -0.5 && st.x < w + 0.5 && st.y > -0.5 && st.y < h + 0.5, "{}: {} {}", sm["id"], st.x, st.y);
            }
        }
        assert!(s.execute("embroidery.sample", &json!({"id": "nope"})).is_err());
    }
}
