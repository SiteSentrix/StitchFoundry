//! Make a Design (E3): the guide's one step that starts a design — words, a shape, an appliqué
//! piece, a picture turned into stitches, or a sample — in the hoop chosen (else My Machine's).
//! The guide's other steps (colours, watching it sew, saving) are ordinary commands.

use serde_json::{Value, json};

use super::*;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "embroidery.makeDesign",
        "Make a Design",
        [],
        None,
        "{what: words|shape|applique|picture|sample|ith|quilt, project? (ith: embroidery.ith.projects), width?, height?, pattern?, spacing? (quilt: embroidery.quiltBlock), hoop?: a hoop's name (My Machine's first), text?, height?: mm (words), shape?: Heart|Star|… (shape, applique), size?: mm (shape, applique), path? | name? | dataBase64? (picture), colors?: 2-12 (picture, 6), sample?: heart-patch|sun-and-moon|applique-star} a new design made in one step: words or a shape in the middle of the hoop, an appliqué piece, a picture turned into stitches at most four fifths of the hoop, or a sample design, or an in-the-hoop project (embroidery.ith.make) → {what, ids, stitches, colors, widthMm, heightMm, fits}",
        always,
        make_design
    )]
}

fn make_design(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.makeDesign";
    let what = str_param(p, "what").ok_or_else(|| bad(CMD, "missing `what`: words, shape, applique, picture or sample"))?;
    let hoop = || match p.get("hoop") {
        Some(h) if !h.is_null() => json!({ "hoop": h }),
        _ => json!({}),
    };
    let ids: Vec<Value> = match what {
        "sample" => {
            let id = str_param(p, "sample").ok_or_else(|| bad(CMD, "missing `sample`: see embroidery.samples"))?;
            s.execute("embroidery.sample", &json!({ "id": id }))?;
            Vec::new()
        }
        "words" => {
            s.execute("embroidery.newDesign", &hoop())?;
            let mut q = json!({ "text": p.get("text").cloned().unwrap_or(Value::Null) });
            if let Some(h) = p.get("height") {
                q["height"] = h.clone();
            }
            vec![s.execute("embroidery.addWords", &q)?["id"].clone()]
        }
        "shape" | "applique" => {
            s.execute("embroidery.newDesign", &hoop())?;
            let mut q = json!({ "name": p.get("shape").cloned().unwrap_or(Value::Null), "applique": what == "applique" });
            if let Some(v) = p.get("size") {
                q["size"] = v.clone();
            }
            vec![s.execute("embroidery.addShape", &q)?["id"].clone()]
        }
        "picture" => {
            let mut place = json!({});
            match (p.get("path"), p.get("name"), p.get("dataBase64")) {
                (Some(path), _, _) => place["path"] = path.clone(),
                (None, Some(name), Some(data)) => {
                    place["name"] = name.clone();
                    place["dataBase64"] = data.clone();
                }
                _ => return Err(bad(CMD, "missing `path` (or `name` and `dataBase64`): the picture")),
            }
            let colors = match p.get("colors") {
                None | Some(Value::Null) => 6,
                Some(v) => v.as_u64().filter(|n| (2..=12).contains(n)).ok_or_else(|| bad(CMD, "`colors` is 2 to 12"))?,
            };
            let made = s.execute("embroidery.newDesign", &hoop())?;
            let (w, h) = (made["width"].as_f64().unwrap_or(100.0), made["height"].as_f64().unwrap_or(100.0));
            s.execute("file.place", &place)?;
            // four fifths of the hoop's shorter side, so it fits whichever way the picture is
            let size = (w.min(h) * 0.8).clamp(10.0, 400.0);
            let r = s.execute("embroidery.autoDigitize", &json!({ "colors": colors, "size": size, "keepImage": false }))?;
            vec![r["id"].clone()]
        }
        "ith" => {
            let mut q = json!({ "project": p.get("project").cloned().unwrap_or(Value::Null) });
            for k in ["hoop", "text", "shape", "height", "ribbon", "vinyl", "button"] {
                if let Some(v) = p.get(k).filter(|v| !v.is_null()) {
                    q[k] = v.clone();
                }
            }
            s.execute("embroidery.ith.make", &q)?;
            Vec::new()
        }
        "quilt" => {
            let mut q = json!({});
            for k in ["width", "height", "unit", "pattern", "spacing", "stitch", "color", "hoop", "corner", "text", "shape", "applique", "clearance"]
            {
                if let Some(v) = p.get(k).filter(|v| !v.is_null()) {
                    q[k] = v.clone();
                }
            }
            s.execute("embroidery.quiltBlock", &q)?;
            Vec::new()
        }
        other => return Err(bad(CMD, format!("`what` is words, shape, applique, picture, sample, ith or quilt, not `{other}`"))),
    };
    let info = s.execute("embroidery.info", &json!({}))?;
    Ok(json!({
        "what": what,
        "ids": ids,
        "stitches": info["stitches"],
        "colors": info["colors"],
        "widthMm": info["widthMm"],
        "heightMm": info["heightMm"],
        "fits": info["fits"],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_base64() -> String {
        // two colour blocks on white: something to turn into stitches
        let mut png = vec![];
        image::RgbaImage::from_fn(120, 80, |x, y| {
            if (20..60).contains(&x) && (20..60).contains(&y) {
                image::Rgba([200, 30, 40, 255])
            } else if (70..110).contains(&x) && (10..70).contains(&y) {
                image::Rgba([20, 60, 160, 255])
            } else {
                image::Rgba([255, 255, 255, 255])
            }
        })
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
        vectorcraft_format::base64_encode(&png)
    }

    /// E3: each kind of design the guide offers is made in one step, ready to sew in the hoop.
    #[test]
    fn the_guide_makes_each_kind_of_design_ready_to_sew() {
        let mut s = Session::new();
        s.execute("embroidery.myMachine.set", &json!({"machine": "Husqvarna Viking Designer Diamond deLuxe"})).unwrap();
        for q in [
            json!({"what": "words", "text": "Grace", "height": 20}),
            json!({"what": "shape", "shape": "Star"}),
            json!({"what": "applique", "shape": "Heart", "hoop": "Husqvarna 120 × 120"}),
            json!({"what": "picture", "name": "blocks.png", "dataBase64": png_base64(), "colors": 3}),
            json!({"what": "sample", "sample": "heart-patch"}),
            json!({"what": "ith", "project": "coaster", "text": "Tea"}),
            json!({"what": "quilt", "width": 4, "height": 4, "pattern": "stipple"}),
        ] {
            let r = s.execute("embroidery.makeDesign", &q).unwrap_or_else(|e| panic!("{q}: {e}"));
            assert!(r["stitches"].as_u64().unwrap() > 100, "{q}: {r}");
            assert_eq!(r["fits"], true, "{q}: {r}");
            assert!(!r["colors"].as_array().unwrap().is_empty(), "{q}: {r}");
        }
        // words in My Machine's first hoop, in its format
        let r = s.execute("embroidery.makeDesign", &json!({"what": "words", "text": "Al"})).unwrap();
        assert_eq!(r["what"], "words");
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert_eq!(info["machineFormat"], "vp3", "{info}");
        for bad in [json!({}), json!({"what": "quilt"}), json!({"what": "picture"}), json!({"what": "sample"}), json!({"what": "words", "text": " "})]
        {
            assert!(s.execute("embroidery.makeDesign", &bad).is_err(), "{bad}");
        }
    }
}
