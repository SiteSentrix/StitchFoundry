//! In-the-hoop projects (K2, K3): small sewn pieces made in the hoop step by step, each step its
//! own thread named for what to do, so the machine stops and says it. Pillow fronts for the little
//! seasonal bench pillows (an 8″ square or 5½ × 9½″ form, the embroidered panel from a 5×7 hoop
//! with borders sewn on after), and pieces finished in the hoop: a mug rug, a coaster, an
//! ornament (placement, batting and top fabric tacked down, the design, the backing, a satin
//! edge). The supplies with their cut sizes and the steps are kept as the design's notes, so its
//! worksheet is the step sheet. Original work: shapes, sizes and wording are our own.

use serde_json::{Value, json};
use vectorcraft_embroidery::PT_PER_MM;

use super::*;

/// mm in an inch.
const IN: f64 = 25.4;
/// Seam allowance (mm): ¼″.
const SEAM: f64 = IN / 4.0;
/// Room kept between the project and the hoop's sewing field (mm).
const MARGIN: f64 = 6.0;

/// One project: id, name, finished width × height (mm), sewn round (an ornament), and whether the
/// hoop makes a panel that borders finish (else the piece is finished in the hoop).
struct Project {
    id: &'static str,
    name: &'static str,
    finished: (f64, f64),
    round: bool,
    panel: bool,
}

const PROJECTS: &[Project] = &[
    Project { id: "pillow-square", name: "Little pillow front, 8″ square", finished: (8.0 * IN, 8.0 * IN), round: false, panel: true },
    Project { id: "pillow-wide", name: "Little pillow front, 9½ × 5½″", finished: (9.5 * IN, 5.5 * IN), round: false, panel: true },
    Project { id: "mug-rug", name: "Mug rug, 6½ × 4½″", finished: (6.5 * IN, 4.5 * IN), round: false, panel: false },
    Project { id: "coaster", name: "Coaster, 3¾″ square", finished: (3.75 * IN, 3.75 * IN), round: false, panel: false },
    Project { id: "ornament", name: "Round ornament, 3½″", finished: (3.5 * IN, 3.5 * IN), round: true, panel: false },
];

/// Threads for the steps that aren't the design: a colour each, named for what to do.
const PLACEMENT: (&str, &str) = ("#9aa5b1", "Placement: lay batting, then top fabric over the line");
const TRIM_LINE: (&str, &str) = ("#9aa5b1", "Trim line for the panel");
const BACKING: (&str, &str) = ("#7b8794", "Backing: tape the backing fabric under the hoop");
const EDGE: (&str, &str) = ("#1f2933", "Edge: satin all round");
const FRAME: (&str, &str) = ("#1f2933", "Frame");

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.ith.projects",
            "In-the-Hoop Projects",
            [],
            None,
            "{} the in-the-hoop projects → {projects: [{id, name, widthMm, heightMm, panel (borders finish it) }]}",
            always,
            projects
        ),
        cmd!(
            "embroidery.ith.make",
            "Make In-the-Hoop Project",
            [],
            None,
            "{project: pillow-square|pillow-wide|mug-rug|coaster|ornament, hoop?: a hoop's name (else the smallest of My Machine's, or the usual ones, that it fits), text?: words, height?: mm (the words), shape?: Heart|Star|… (embroidery.addShape's), color?: \"#rrggbb\" (the design's thread)} a new design: the project's steps in sewing order, each its own thread named for what to do (the machine stops between them), the words and shape in the middle; the supplies with cut sizes and the steps become the design's notes (its worksheet is the step sheet) → {project, hoop, steps: [{step, thread, what}], supplies: [text], notes}",
            always,
            make
        ),
    ]
}

fn projects(_s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({
        "projects": PROJECTS.iter().map(|p| json!({
            "id": p.id, "name": p.name, "widthMm": p.finished.0, "heightMm": p.finished.1, "panel": p.panel,
        })).collect::<Vec<_>>()
    }))
}

/// A length in inches to the nearest eighth, written with a fraction (`4⅝″`).
fn inches(mm: f64) -> String {
    let eighths = (mm / IN * 8.0).round().max(0.0) as u64;
    let (whole, part) = (eighths / 8, eighths % 8);
    let frac = ["", "⅛", "¼", "⅜", "½", "⅝", "¾", "⅞"].get(part as usize).copied().unwrap_or("");
    match (whole, frac) {
        (0, "") => "0″".into(),
        (0, f) => format!("{f}″"),
        (w, f) => format!("{w}{f}″"),
    }
}

/// `w × h` in inches and millimetres.
fn size(w: f64, h: f64) -> String {
    format!("{} × {} ({w:.0} × {h:.0} mm)", inches(w), inches(h))
}

/// The hoop to make the project in: `hoop` if given, else the smallest of My Machine's hoops (or
/// the usual ones) whose field holds `need` either way round. → (newDesign's `hoop`, field w, h).
fn pick_hoop(s: &mut Session, p: &Value, need: (f64, f64)) -> Result<(Value, f64, f64)> {
    const CMD: &str = "embroidery.ith.make";
    let fits = |w: f64, h: f64| (w >= need.0 && h >= need.1) || (w >= need.1 && h >= need.0);
    if let Some(name) = str_param(p, "hoop") {
        let hoop = super::embroidery_hoops::find_hoop(s, name).ok_or_else(|| bad(CMD, format!("unknown hoop `{name}`")))?;
        return Ok((json!(hoop.name), hoop.width, hoop.height));
    }
    let mine = s.execute("embroidery.myMachine", &json!({}))?;
    let mut hoops: Vec<(String, f64, f64)> = mine["hoops"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|h| Some((h["name"].as_str()?.to_string(), h["width"].as_f64()?, h["height"].as_f64()?)))
        .collect();
    if hoops.is_empty() {
        hoops = ["Brother 100 × 100 (4×4)", "Brother 130 × 180 (5×7)", "Brother 160 × 260 (6×10)", "Brother 200 × 200 (8×8)"]
            .iter()
            .filter_map(|n| super::embroidery_hoops::find_hoop(s, n).map(|h| (h.name.clone(), h.width, h.height)))
            .collect();
    }
    hoops.sort_by(|a, b| (a.1 * a.2).total_cmp(&(b.1 * b.2)));
    let (name, w, h) =
        hoops.into_iter().find(|(_, w, h)| fits(*w, *h)).ok_or_else(|| bad(CMD, "none of the hoops is big enough: name a larger `hoop`"))?;
    Ok((json!(name), w, h))
}

/// Selects `id` and runs `cmd` with `q` on it.
fn on(s: &mut Session, id: u64, cmd: &str, q: Value) -> Result<Value> {
    s.execute("select.set", &json!({ "ids": [id] }))?;
    s.execute(cmd, &q)
}

/// A rectangle (or circle) `w` × `h` mm round the page's middle → its id.
fn outline(s: &mut Session, c: kurbo::Point, w: f64, h: f64, round: bool) -> Result<u64> {
    let (wp, hp) = (w * PT_PER_MM, h * PT_PER_MM);
    let q = json!({ "x": c.x - wp / 2.0, "y": c.y - hp / 2.0, "width": wp, "height": hp });
    let r = s.execute(if round { "shape.ellipse" } else { "shape.rectangle" }, &q)?;
    r["id"].as_u64().ok_or_else(|| bad("embroidery.ith.make", "the outline wasn't made"))
}

/// A step: its object gets stitch type `ty`, `params`, and the step's thread (its own colour block).
fn step(s: &mut Session, id: u64, ty: &str, params: Value, thread: (&str, &str)) -> Result<()> {
    on(s, id, "embroidery.set", json!({ "type": ty }))?;
    let mut all = params;
    all["own_block"] = json!(true);
    s.execute("embroidery.setParams", &json!({ "params": all }))?;
    s.execute("embroidery.setThread", &json!({ "color": thread.0, "name": thread.1, "ids": [id], "paint": false }))?;
    Ok(())
}

fn make(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.ith.make";
    let id = str_param(p, "project").ok_or_else(|| bad(CMD, "missing `project`: see embroidery.ith.projects"))?;
    let pr = PROJECTS.iter().find(|x| x.id == id).ok_or_else(|| bad(CMD, format!("no project `{id}`: see embroidery.ith.projects")))?;
    let text = str_param(p, "text").map(str::trim).filter(|t| !t.is_empty()).map(str::to_string);
    let shape = str_param(p, "shape").map(str::to_string);
    let color = str_param(p, "color").unwrap_or("#8a2432").to_string();
    sf_core::Rgb::from_hex(&color).ok_or_else(|| bad(CMD, "`color` is \"#rrggbb\""))?;
    // a panel needs a 5×7-class hoop; a finished piece its own size
    let need = if pr.panel { (118.0 + 2.0 * MARGIN, 160.0 + 2.0 * MARGIN) } else { (pr.finished.0 + 2.0 * MARGIN, pr.finished.1 + 2.0 * MARGIN) };
    let (hoop, fw, fh) = pick_hoop(s, p, need)?;
    let (long, short) = (fw.max(fh) - 2.0 * MARGIN, fw.min(fh) - 2.0 * MARGIN);
    // the piece sewn in the hoop: a panel as big as the field allows (no bigger than the front),
    // or the finished piece itself
    let landscape = pr.finished.0 >= pr.finished.1;
    let (pw, ph) = if pr.panel {
        if (pr.finished.0 - pr.finished.1).abs() < 1.0 {
            let side = short.min(pr.finished.0);
            (side, side)
        } else {
            let (a, b) = (long.min(pr.finished.0.max(pr.finished.1)), short.min(pr.finished.0.min(pr.finished.1)));
            if landscape { (a, b) } else { (b, a) }
        }
    } else {
        pr.finished
    };
    // the hoop turned to the piece's way round
    let hoop = if (pw > ph) != (fw > fh) && (pw - ph).abs() > 1.0 { json!({ "width": fh, "height": fw }) } else { hoop };
    let made = s.execute("embroidery.newDesign", &json!({ "hoop": hoop }))?;
    let c = {
        let d = &s.doc()?.doc;
        d.artboards.first().map(|a| a.rect.center()).ok_or_else(|| EngineError::Other("the design has no page".into()))?
    };
    let mut steps: Vec<Value> = Vec::new();
    let mut add_step = |thread: &str, what: &str| {
        steps.push(json!({ "step": steps.len() + 1, "thread": thread, "what": what }));
    };
    // 1: where the piece goes
    if pr.panel {
        let o = outline(s, c, pw, ph, pr.round)?;
        step(s, o, "run", json!({}), TRIM_LINE)?;
        add_step(TRIM_LINE.1, "Hoop the top fabric with tear-away stabilizer. This line marks the panel; trim on it after.");
    } else {
        let o = outline(s, c, pw, ph, pr.round)?;
        step(s, o, "applique", json!({ "placement": true, "tackdown": "Run", "trim_stop": false, "cover": "None" }), PLACEMENT)?;
        add_step(
            PLACEMENT.1,
            "Hoop cut-away stabilizer and sew the line. Lay the batting, then the top fabric right side up over it; the machine tacks them down.",
        );
    }
    // 2: the design in the middle (above the words when both)
    let inner = (pw.min(ph) - 24.0).max(10.0);
    let both = text.is_some() && shape.is_some();
    if let Some(name) = &shape {
        let size = if both { inner * 0.5 } else { inner * 0.7 };
        let r = s.execute("embroidery.addShape", &json!({ "name": name, "size": size }))?;
        if both && let Some(sid) = r["id"].as_u64() {
            on(s, sid, "object.move", json!({ "dx": 0.0, "dy": -inner * 0.18 * PT_PER_MM }))?;
        }
        add_step("The design", "Sew the shape in its colours.");
    }
    if let Some(t) = &text {
        let height = match p.get("height").and_then(Value::as_f64) {
            Some(h) => h.clamp(5.0, 80.0),
            None => (inner * if both { 0.18 } else { 0.28 }).clamp(6.0, 40.0),
        };
        let r = s.execute("embroidery.addWords", &json!({ "text": t, "height": height, "color": color }))?;
        if both && let Some(wid) = r["id"].as_u64() {
            on(s, wid, "object.move", json!({ "dx": 0.0, "dy": inner * 0.3 * PT_PER_MM }))?;
        }
        add_step("The words", "Sew the words.");
    }
    // 3: finishing in the hoop
    if pr.panel {
        let o = outline(s, c, pw - 16.0, ph - 16.0, pr.round)?;
        step(s, o, "run", json!({ "passes": 3 }), FRAME)?;
        add_step(FRAME.1, "A triple-stitch frame round the design. Take the panel out of the hoop and tear the stabilizer away.");
    } else {
        let back = outline(s, c, pw, ph, pr.round)?;
        step(s, back, "applique", json!({ "placement": false, "tackdown": "Run", "trim_stop": true, "cover": "None" }), BACKING)?;
        add_step(
            BACKING.1,
            "Take the hoop off the machine without unhooping. Tape the backing fabric right side out under the hoop over the piece, put the hoop back; the machine tacks it on, then stops: trim all the layers close to the stitching, front and back.",
        );
        let edge = outline(s, c, pw, ph, pr.round)?;
        step(s, edge, "applique", json!({ "placement": false, "tackdown": "None", "trim_stop": false, "cover": "Satin", "cover_width": 4.0 }), EDGE)?;
        add_step(EDGE.1, "A satin edge all round covers the cut edges. Take it out of the hoop and cut the stabilizer away close to the satin.");
    }
    // the supplies and cut sizes
    let mut supplies = vec![format!("Hoop: {} (field {})", made["hoop"].as_str().unwrap_or("custom"), size(fw, fh))];
    if pr.panel {
        let (w, h) = pr.finished;
        let (side, topb) = (((w - pw) / 2.0).max(0.0), ((h - ph) / 2.0).max(0.0));
        supplies.push(format!("Top fabric for hooping: {}", size(fw + 2.0 * IN, fh + 2.0 * IN)));
        supplies.push("Tear-away stabilizer, the size of the hoop".into());
        supplies.push(format!("Trim the embroidered panel to {} (¼″ seams)", size(pw + 2.0 * SEAM, ph + 2.0 * SEAM)));
        if side > 1.0 {
            supplies.push(format!("Side borders: 2 strips {}", size(side + 2.0 * SEAM, ph + 2.0 * SEAM)));
        }
        if topb > 1.0 {
            supplies.push(format!("Top and bottom borders: 2 strips {}", size(w + 2.0 * SEAM, topb + 2.0 * SEAM)));
        }
        let back_h = (h * 0.65 + IN).round();
        supplies.push(format!("Envelope back: 2 pieces {} (hem one long edge of each, overlap them)", size(w + 2.0 * SEAM, back_h)));
        supplies.push(format!("Pillow form {} finished", size(w, h)));
        add_step(
            "Sew the front",
            "Sew the side borders on, then the top and bottom ones, ¼″ seams; press. The front measures the pillow's size plus ¼″ all round.",
        );
        add_step(
            "Sew the cover",
            "Lay the two hemmed back pieces right side down on the front, overlapping; sew all round with a ¼″ seam, clip the corners, turn out and put the form in.",
        );
    } else {
        supplies.push(format!("Cut-away stabilizer, the size of the hoop; batting {}", size(pw + IN, ph + IN)));
        supplies.push(format!("Top fabric {}; backing fabric {}", size(pw + IN, ph + IN), size(pw + IN, ph + IN)));
        supplies.push(format!("Finished size {}", size(pw, ph)));
    }
    let notes: String = std::iter::once(pr.name.to_string())
        .chain(std::iter::once(String::new()))
        .chain(std::iter::once("You need:".to_string()))
        .chain(supplies.iter().map(|x| format!("- {x}")))
        .chain(std::iter::once(String::new()))
        .chain(std::iter::once("Steps:".to_string()))
        .chain(
            steps
                .iter()
                .map(|st| format!("{}. {}: {}", st["step"], st["thread"].as_str().unwrap_or_default(), st["what"].as_str().unwrap_or_default())),
        )
        .collect::<Vec<_>>()
        .join("\n")
        .chars()
        .take(2000)
        .collect();
    s.execute("embroidery.settings", &json!({ "notes": notes }))?;
    s.execute("select.none", &json!({}))?;
    Ok(json!({ "project": pr.id, "hoop": made["hoop"], "steps": steps, "supplies": supplies, "notes": notes }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// K2: every project is made in a hoop it fits, its steps each a thread of their own in sewing
    /// order, its supplies and steps kept as the design's notes.
    #[test]
    fn each_project_sews_step_by_step_in_a_hoop_it_fits() {
        let mut s = Session::new();
        let list = s.execute("embroidery.ith.projects", &json!({})).unwrap();
        assert_eq!(list["projects"].as_array().unwrap().len(), PROJECTS.len());
        for pr in PROJECTS {
            let r = s
                .execute("embroidery.ith.make", &json!({"project": pr.id, "text": "Joy", "shape": "Heart"}))
                .unwrap_or_else(|e| panic!("{}: {e}", pr.id));
            let info = s.execute("embroidery.info", &json!({})).unwrap();
            assert_eq!(info["fits"], true, "{}: {info}", pr.id);
            let threads: Vec<String> = info["colors"].as_array().unwrap().iter().map(|c| c["name"].as_str().unwrap_or("").to_string()).collect();
            if pr.panel {
                assert_eq!(threads.first().map(String::as_str), Some(TRIM_LINE.1), "{}: {threads:?}", pr.id);
                assert_eq!(threads.last().map(String::as_str), Some(FRAME.1), "{}: {threads:?}", pr.id);
            } else {
                assert_eq!(threads.first().map(String::as_str), Some(PLACEMENT.1), "{}: {threads:?}", pr.id);
                assert_eq!(threads.last().map(String::as_str), Some(EDGE.1), "{}: {threads:?}", pr.id);
                assert!(threads.iter().any(|t| t == BACKING.1), "{}: {threads:?}", pr.id);
                // placement, tack-down and trim stops: the machine stops for each layer
                assert!(info["stops"].as_u64().unwrap_or(0) + info["colorChanges"].as_u64().unwrap_or(0) >= 4, "{}: {info}", pr.id);
            }
            assert!(r["steps"].as_array().unwrap().len() >= 4, "{r}");
            assert!(info["notes"].as_str().unwrap().contains("You need:"), "{info}");
        }
        // the 8″ square front: a square panel from a 5×7 hoop, borders to make up the rest
        let r = s.execute("embroidery.ith.make", &json!({"project": "pillow-square", "hoop": "Brother 130 × 180 (5×7)"})).unwrap();
        let sup = r["supplies"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect::<Vec<_>>().join("\n");
        assert!(sup.contains("Side borders: 2 strips") && sup.contains("Envelope back"), "{sup}");
        assert!(sup.contains("Pillow form 8″ × 8″"), "{sup}");
        // the wide front turns the hoop landscape
        s.execute("embroidery.ith.make", &json!({"project": "pillow-wide", "hoop": "Brother 130 × 180 (5×7)", "text": "Hello fall"})).unwrap();
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert!(info["widthMm"].as_f64().unwrap() > info["heightMm"].as_f64().unwrap(), "{info}");
        for bad in
            [json!({}), json!({"project": "quilt"}), json!({"project": "coaster", "hoop": "nope"}), json!({"project": "coaster", "color": "red"})]
        {
            assert!(s.execute("embroidery.ith.make", &bad).is_err(), "{bad}");
        }
        assert_eq!(inches(8.0 * IN), "8″");
        assert_eq!(inches(4.625 * IN), "4⅝″");
        assert_eq!(inches(0.25 * IN), "¼″");
    }
}
