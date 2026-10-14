//! Tile quilting (T-series): background quilting for a quilt sandwich hooped one block at a
//! time, as with clear marking tiles. A tile the size of the block is laid on the quilt, its
//! middle, crosshairs, up arrow, sides and corners marked; the block that size is hooped and
//! quilted. Every block is one running-stitch thread over the whole rectangle at the same scale,
//! its lines kept half a line-spacing in from the edge, so blocks of any size laid side by side
//! read as one continuous quilting with no lines to match. `embroidery.quiltBlock` makes one
//! block; `embroidery.quiltBlock.set` writes one machine file per size, named by its size.
//! Patterns and wording are our own.

use serde_json::{Value, json};
use vectorcraft_embroidery::PT_PER_MM;

use super::*;

/// mm in an inch.
const IN: f64 = 25.4;

/// The patterns: id, name.
pub const PATTERNS: &[(&str, &str)] = &[
    ("stipple", "Stipple"),
    ("swirls", "Swirls"),
    ("loops", "Loops"),
    ("feathers", "Feathers"),
    ("waves", "Waves"),
    ("spiral", "Squares spiral"),
    ("crosshatch", "Crosshatch"),
];

/// Block sizes (inches) to start from: common tile and hoop sizes from 2″ to 9 × 14″.
pub const SIZES: &[(f64, f64)] = &[
    (2.0, 2.0),
    (3.0, 3.0),
    (4.0, 4.0),
    (4.0, 6.0),
    (5.0, 5.0),
    (5.0, 7.0),
    (6.0, 6.0),
    (6.0, 8.0),
    (6.0, 10.0),
    (7.0, 7.0),
    (8.0, 8.0),
    (8.0, 12.0),
    (9.0, 9.0),
    (9.0, 14.0),
];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "embroidery.quiltBlock.patterns",
            "Quilt Block Patterns",
            [],
            None,
            "{} the background quilting patterns and the usual block sizes → {patterns: [{id, name}], sizes: [{width, height}] (inches)}",
            always,
            patterns
        ),
        cmd!(
            "embroidery.quiltBlock",
            "Quilt Block",
            [],
            None,
            "{width, height: the block (tile) size, unit?: in|mm (in), pattern?: stipple|swirls|loops|feathers|waves|spiral|crosshatch (stipple), spacing?: mm between lines (6; 2–25; keep it the same for every block of a quilt), stitch?: mm (3; 1.5–5), color?: \"#rrggbb\", hoop?: a hoop's name (else the smallest of My Machine's, or the usual ones, it fits)} a new design: the block's rectangle quilted edge to edge in one running-stitch thread, centred in the hoop, top up → {pattern, widthMm, heightMm, hoop, stitches}",
            always,
            quilt_block
        ),
        cmd!(
            "embroidery.quiltBlock.set",
            "Quilt Block Set",
            [],
            None,
            "{folder, pattern?, sizes?: [{width, height}] (inches; the usual sizes), spacing?, stitch?, color?, format?: dst|exp|jef|pes|vp3|xxx (My Machine's)} one machine file per block size in the folder, named \"<Pattern> <w>x<h> in.<ext>\"; sizes no hoop fits are left out and said → {files: [path], skipped: [{width, height, reason}]}",
            always,
            quilt_set
        ),
    ]
}

fn patterns(_s: &mut Session, _p: &Value) -> Result<Value> {
    Ok(json!({
        "patterns": PATTERNS.iter().map(|(id, name)| json!({ "id": id, "name": name })).collect::<Vec<_>>(),
        "sizes": SIZES.iter().map(|(w, h)| json!({ "width": w, "height": h })).collect::<Vec<_>>(),
    }))
}

/// A length as the file names write it: `5`, `5.5`.
fn num(v: f64) -> String {
    let r = (v * 100.0).round() / 100.0;
    if (r - r.round()).abs() < 1e-9 { format!("{}", r.round() as i64) } else { format!("{r}") }
}

/// Rows across the block for a motif `amp` mm either side of its row line, about `pitch` apart:
/// the rows' middles, spread so the motifs reach `m` from the block's top and bottom.
fn rows(h: f64, pitch: f64, amp: f64, m: f64) -> Vec<f64> {
    let room = (h - 2.0 * m - 2.0 * amp).max(0.0);
    let n = ((room / pitch).round() as usize + 1).clamp(1, 2000);
    if n == 1 {
        return vec![h / 2.0];
    }
    // spread to reach from the top edge to the bottom one
    let step = room / (n as f64 - 1.0);
    (0..n).map(|i| m + amp + i as f64 * step).collect()
}

/// A row-by-row pattern: `row(x0, x1, y)` gives one row left to right; odd rows run right to
/// left (mirrored), and the line steps down the side to the next, all one line kept inside the
/// block `m` from its edges.
fn by_rows(w: f64, h: f64, m: f64, pitch: f64, amp: f64, row: impl Fn(f64, f64, f64) -> Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    let (x0, x1) = (m, w - m);
    let mut pts = Vec::new();
    for (i, y) in rows(h, pitch, amp, m).into_iter().enumerate() {
        let mut r = row(x0, x1, y);
        // odd rows come back right to left: the row mirrored, so it starts at the right edge
        if i % 2 == 1 {
            r = r.into_iter().map(|(x, y)| (x0 + x1 - x, y)).collect();
        }
        pts.extend(r);
    }
    pts.into_iter().map(|(x, y)| (x.clamp(m, w - m), y.clamp(m, h - m))).collect()
}

/// Loops: a row of round cursive loops (a looped cycloid), rows `gap` apart between loops.
fn loops(w: f64, h: f64, gap: f64) -> Vec<(f64, f64)> {
    let d = gap * 0.9;
    let r = d * 0.42;
    by_rows(w, h, gap / 2.0, 2.0 * d + gap * 0.6, d, |x0, x1, y| {
        // start and end on the row line (cos = 0), so rows join cleanly at the sides
        let turns = ((x1 - x0 - 2.0 * d) / r).max(0.0);
        let n = ((turns / 0.1).ceil() as usize).clamp(2, 200_000);
        let mut pts = vec![(x0, y)];
        pts.extend((0..=n).map(|i| {
            let t = std::f64::consts::FRAC_PI_2 + turns * i as f64 / n as f64;
            (x0 + d + r * (t - std::f64::consts::FRAC_PI_2) - d * t.sin() + d, y - d * t.cos())
        }));
        pts.push((x1, y));
        pts
    })
}

/// Swirls: along each row, a spiral in, its echo back out beside it, and half a turn round the
/// outside on to the next; each swirl turns the other way from the last.
fn swirls(w: f64, h: f64, gap: f64) -> Vec<(f64, f64)> {
    let big = gap * 1.6;
    let echo = gap * 0.35;
    let outer = big + echo;
    by_rows(w, h, gap / 2.0, 2.0 * outer + gap * 0.5, outer, |x0, x1, y| {
        let cell = 2.0 * outer;
        let n = (((x1 - x0) / cell).floor() as usize).clamp(1, 5000);
        let left = x0 + ((x1 - x0) - n as f64 * cell) / 2.0;
        let mut pts = vec![(x0, y)];
        let turn = 2.5 * std::f64::consts::PI;
        for k in 0..n {
            let cx = left + outer + k as f64 * cell;
            let s = if k % 2 == 0 { 1.0 } else { -1.0 };
            let at = |a: f64, rad: f64| (cx - rad * a.cos(), y - s * rad * a.sin());
            let steps = 60;
            // in: from the left, spiralling to a small middle
            for i in 0..=steps {
                let t = i as f64 / steps as f64;
                pts.push(at(t * turn, big * (1.0 - 0.8 * t)));
            }
            // out: back beside it, `echo` further out
            for i in (0..=steps).rev() {
                let t = i as f64 / steps as f64;
                pts.push(at(t * turn, big * (1.0 - 0.8 * t) + echo));
            }
            // round the outside to the right
            for i in 0..=steps / 2 {
                let a = -std::f64::consts::PI * i as f64 / (steps / 2) as f64;
                pts.push(at(a, outer));
            }
        }
        pts.push((x1, y));
        pts
    })
}

/// A cubic Bézier's points from `a` to `d`.
fn cubic(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64), n: usize) -> impl Iterator<Item = (f64, f64)> {
    (0..=n).map(move |i| {
        let t = i as f64 / n as f64;
        let u = 1.0 - t;
        let (k0, k1, k2, k3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        (k0 * a.0 + k1 * b.0 + k2 * c.0 + k3 * d.0, k0 * a.1 + k1 * b.1 + k2 * c.1 + k3 * d.1)
    })
}

/// Feathers: rows of teardrop plumes all growing one way, each swelling back over the one before
/// and coming down to the stem a plume further on, the next row's plumes filling in below.
fn feathers(w: f64, h: f64, gap: f64) -> Vec<(f64, f64)> {
    let ph = gap * 1.8;
    let pw = gap * 1.1;
    by_rows(w, h, gap / 2.0, ph + gap * 0.6, ph / 2.0, |x0, x1, y| {
        let stem = y + ph / 2.0;
        let n = (((x1 - x0) / pw).floor() as usize).clamp(1, 20_000);
        let left = x0 + ((x1 - x0) - n as f64 * pw) / 2.0;
        let mut pts = vec![(x0, stem)];
        for k in 0..n {
            let b = left + k as f64 * pw;
            let tip = (b + pw * 0.3, stem - ph);
            // up and back over the plume before, round the tip, down to the next base
            pts.extend(cubic((b, stem), (b - pw * 0.9, stem - ph * 0.4), (b - pw * 0.5, stem - ph * 1.05), tip, 16));
            pts.extend(cubic(tip, (b + pw * 1.0, stem - ph * 0.95), (b + pw * 1.2, stem - ph * 0.35), (b + pw, stem), 16).skip(1));
        }
        pts.push((x1, stem));
        pts
    })
}

/// Waves: rows `gap` apart across the block, each a gentle wave, joined end to end at the sides
/// into one line (points in mm, the block's top left at 0, 0).
fn waves(w: f64, h: f64, gap: f64) -> Vec<(f64, f64)> {
    let (amp, period) = (gap * 0.3, gap * 4.0);
    let rows = ((h / gap).floor() as usize).clamp(1, 2000);
    let top = (h - (rows as f64 - 1.0) * gap) / 2.0;
    let steps = ((w / 1.0).ceil() as usize).clamp(2, 20_000);
    let mut pts = Vec::new();
    for r in 0..rows {
        let y = top + r as f64 * gap;
        for i in 0..=steps {
            let k = if r % 2 == 0 { i } else { steps - i };
            let x = gap * 0.25 + (w - gap * 0.5) * k as f64 / steps as f64;
            let wave = (x / period * std::f64::consts::TAU).sin() * amp;
            pts.push((x, (y + wave).clamp(0.0, h)));
        }
    }
    pts
}

/// Crosshatch: lines at 45° both ways, `gap` apart, sewn as one line that turns at the edges.
fn crosshatch(w: f64, h: f64, gap: f64) -> Vec<(f64, f64)> {
    let m = gap * 0.5;
    let (x0, y0, x1, y1) = (m, m, w - m, h - m);
    // a line x - y = c (one way) or x + y = c (the other) clipped to the inner box
    let clip = |c: f64, up: bool| -> Option<((f64, f64), (f64, f64))> {
        let mut hits: Vec<(f64, f64)> = Vec::new();
        let on = |x: f64, y: f64| x >= x0 - 1e-9 && x <= x1 + 1e-9 && y >= y0 - 1e-9 && y <= y1 + 1e-9;
        for &x in &[x0, x1] {
            let y = if up { c - x } else { x - c };
            if on(x, y) {
                hits.push((x, y));
            }
        }
        for &y in &[y0, y1] {
            let x = if up { c - y } else { c + y };
            if on(x, y) {
                hits.push((x, y));
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        Some((*hits.first()?, *hits.last()?))
    };
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let step = gap * std::f64::consts::SQRT_2;
    for up in [false, true] {
        let (lo, hi) = if up { (x0 + y0, x1 + y1) } else { (x0 - y1, x1 - y0) };
        let n = (((hi - lo) / step).floor() as usize).min(5000);
        for i in 1..=n {
            let c = lo + i as f64 * step;
            if let Some((a, b)) = clip(c, up) {
                // alternate the direction so each line starts where the last one ended
                let (a, b) = if pts.len().is_multiple_of(4) { (a, b) } else { (b, a) };
                pts.push(a);
                pts.push(b);
            }
        }
    }
    pts
}

/// SVG path data for a polyline at the block's place (mm → points, `left`, `top` its corner).
fn path_d(pts: &[(f64, f64)], left: f64, top: f64) -> String {
    let mut d = String::new();
    for (i, (x, y)) in pts.iter().enumerate() {
        d.push_str(&format!("{}{:.3} {:.3} ", if i == 0 { "M" } else { "L" }, left + x * PT_PER_MM, top + y * PT_PER_MM));
    }
    d
}

fn quilt_block(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.quiltBlock";
    let unit = match str_param(p, "unit").unwrap_or("in") {
        "in" => IN,
        "mm" => 1.0,
        _ => return Err(bad(CMD, "`unit` is in or mm")),
    };
    let dim = |k: &str| p.get(k).and_then(Value::as_f64).filter(|v| v.is_finite()).map(|v| v * unit).filter(|v| (10.0..=600.0).contains(v));
    let (Some(w), Some(h)) = (dim("width"), dim("height")) else {
        return Err(bad(CMD, "`width` and `height` are the block's size: 10 to 600 mm (0.4 to 23 in)"));
    };
    let pattern = str_param(p, "pattern").unwrap_or("stipple");
    if !PATTERNS.iter().any(|(id, _)| *id == pattern) {
        return Err(bad(CMD, "`pattern` is stipple, swirls, loops, feathers, waves, spiral or crosshatch"));
    }
    let gap = match p.get("spacing") {
        None | Some(Value::Null) => 6.0,
        Some(v) => v.as_f64().filter(|v| (2.0..=25.0).contains(v)).ok_or_else(|| bad(CMD, "`spacing` is 2 to 25 mm"))?,
    };
    let stitch = match p.get("stitch") {
        None | Some(Value::Null) => 3.0,
        Some(v) => v.as_f64().filter(|v| (1.5..=5.0).contains(v)).ok_or_else(|| bad(CMD, "`stitch` is 1.5 to 5 mm"))?,
    };
    let color = str_param(p, "color").unwrap_or("#5b6b7a").to_string();
    sf_core::Rgb::from_hex(&color).ok_or_else(|| bad(CMD, "`color` is \"#rrggbb\""))?;
    // the hoop: the block plus a little room, turned the block's way round
    let (hoop, fw, fh) = super::embroidery_ith::pick_hoop(s, p, (w + 4.0, h + 4.0), CMD)?;
    let fits = (fw >= w && fh >= h) || (fw >= h && fh >= w);
    if !fits {
        return Err(bad(CMD, "the block is bigger than the hoop's sewing field"));
    }
    let hoop = if (w > h) != (fw > fh) && (w - h).abs() > 1.0 { json!({ "width": fh, "height": fw }) } else { hoop };
    let made = s.execute("embroidery.newDesign", &json!({ "hoop": hoop }))?;
    let c = {
        let d = &s.doc()?.doc;
        d.artboards.first().map(|a| a.rect.center()).ok_or_else(|| EngineError::Other("the design has no page".into()))?
    };
    let (left, top) = (c.x - w * PT_PER_MM / 2.0, c.y - h * PT_PER_MM / 2.0);
    let id = match pattern {
        "stipple" | "spiral" => {
            let r = s.execute("shape.rectangle", &json!({ "x": left, "y": top, "width": w * PT_PER_MM, "height": h * PT_PER_MM }))?;
            let id = r["id"].as_u64().ok_or_else(|| bad(CMD, "the block wasn't made"))?;
            s.execute("select.set", &json!({ "ids": [id] }))?;
            s.execute("embroidery.set", &json!({ "type": "fill" }))?;
            let kind = if pattern == "stipple" { "Meander" } else { "Spiral" };
            let mut params = json!({ "kind": kind, "spacing": if pattern == "stipple" { gap / 2.0 } else { gap }, "stitch_length": stitch, "underlay": { "kinds": [] } });
            if pattern == "stipple" {
                params["random"] = json!(0.6);
            }
            s.execute("embroidery.setParams", &json!({ "params": params }))?;
            id
        }
        _ => {
            let pts = match pattern {
                "swirls" => swirls(w, h, gap),
                "loops" => loops(w, h, gap),
                "feathers" => feathers(w, h, gap),
                "waves" => waves(w, h, gap),
                _ => crosshatch(w, h, gap),
            };
            let r = s.execute("path.create", &json!({ "d": path_d(&pts, left, top) }))?;
            let id = r["id"].as_u64().ok_or_else(|| bad(CMD, "the block wasn't made"))?;
            s.execute("select.set", &json!({ "ids": [id] }))?;
            s.execute("embroidery.set", &json!({ "type": "run" }))?;
            s.execute("embroidery.setParams", &json!({ "params": { "stitch_length": stitch } }))?;
            id
        }
    };
    let name = PATTERNS.iter().find(|(i, _)| *i == pattern).map_or("Quilting", |(_, n)| n);
    s.execute("embroidery.setThread", &json!({ "color": color, "name": format!("Quilting: {name}"), "ids": [id], "paint": false }))?;
    let notes = format!(
        "Quilt block {} × {} in, {name}, lines {gap:.0} mm apart.\nLay the tile on the quilt and mark its middle, crosshairs, up arrow, sides and corners; hoop the quilt sandwich with the marks centred and the arrow up; quilt; spritz the marks away.",
        num(w / IN),
        num(h / IN)
    );
    s.execute("embroidery.settings", &json!({ "notes": notes }))?;
    s.execute("select.none", &json!({}))?;
    let info = s.execute("embroidery.info", &json!({}))?;
    Ok(json!({ "pattern": pattern, "widthMm": w, "heightMm": h, "hoop": made["hoop"], "stitches": info["stitches"] }))
}

fn quilt_set(s: &mut Session, p: &Value) -> Result<Value> {
    const CMD: &str = "embroidery.quiltBlock.set";
    let folder = str_param(p, "folder").ok_or_else(|| bad(CMD, "missing `folder`: where the files go"))?.to_string();
    let pattern = str_param(p, "pattern").unwrap_or("stipple").to_string();
    let name = PATTERNS
        .iter()
        .find(|(i, _)| *i == pattern)
        .map(|(_, n)| *n)
        .ok_or_else(|| bad(CMD, "`pattern` is stipple, swirls, loops, feathers, waves, spiral or crosshatch"))?;
    let sizes: Vec<(f64, f64)> = match p.get("sizes").and_then(Value::as_array) {
        Some(a) => a.iter().take(60).filter_map(|v| Some((v["width"].as_f64()?, v["height"].as_f64()?))).collect(),
        None => SIZES.to_vec(),
    };
    if sizes.is_empty() {
        return Err(bad(CMD, "`sizes` is a list of {width, height} in inches"));
    }
    let mut files = Vec::new();
    let mut skipped = Vec::new();
    for (w, h) in sizes {
        let mut q = json!({ "width": w, "height": h, "pattern": pattern });
        for k in ["spacing", "stitch", "color"] {
            if let Some(v) = p.get(k).filter(|v| !v.is_null()) {
                q[k] = v.clone();
            }
        }
        match s.execute("embroidery.quiltBlock", &q) {
            Ok(_) => {
                let mut send = json!({ "folder": folder, "name": format!("{name} {}x{} in", num(w), num(h)) });
                if let Some(f) = p.get("format").filter(|v| !v.is_null()) {
                    send["format"] = f.clone();
                }
                let r = s.execute("embroidery.send", &send)?;
                files.push(r["path"].clone());
                // the block's document is done with
                s.execute("file.close", &json!({ "save": false })).ok();
            }
            Err(e) => skipped.push(json!({ "width": w, "height": h, "reason": e.to_string() })),
        }
    }
    Ok(json!({ "files": files, "skipped": skipped }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T1: a block of each pattern fills its rectangle edge to edge with one thread, in a hoop it
    /// fits, its lines staying inside the block.
    #[test]
    fn each_pattern_quilts_the_whole_block_in_one_thread() {
        let mut s = Session::new();
        s.execute("embroidery.myMachine.set", &json!({"machine": "Husqvarna Viking Designer Diamond deLuxe"})).unwrap();
        for (pattern, _) in PATTERNS {
            let r = s
                .execute("embroidery.quiltBlock", &json!({"width": 5, "height": 7, "pattern": pattern}))
                .unwrap_or_else(|e| panic!("{pattern}: {e}"));
            let info = s.execute("embroidery.info", &json!({})).unwrap();
            assert_eq!(info["fits"], true, "{pattern}: {info}");
            assert_eq!(info["colors"].as_array().unwrap().len(), 1, "{pattern}: one thread");
            // covers the block: within a line spacing of its size, never beyond it
            let (w, h) = (info["widthMm"].as_f64().unwrap(), info["heightMm"].as_f64().unwrap());
            assert!(w <= 5.0 * IN + 1.0 && h <= 7.0 * IN + 1.0, "{pattern}: {w} × {h}");
            assert!(w >= 5.0 * IN - 13.0 && h >= 7.0 * IN - 13.0, "{pattern}: {w} × {h}");
            assert!(r["stitches"].as_u64().unwrap() > 200, "{pattern}: {r}");
            // one continuous line: at most the trim that ends the design
            assert!(info["trims"].as_u64().unwrap_or(0) <= 1, "{pattern}: one continuous line, {info}");
        }
        // a wide block turns the hoop
        s.execute("embroidery.quiltBlock", &json!({"width": 8, "height": 4})).unwrap();
        let info = s.execute("embroidery.info", &json!({})).unwrap();
        assert!(info["widthMm"].as_f64().unwrap() > info["heightMm"].as_f64().unwrap());
        for bad in [
            json!({}),
            json!({"width": 5}),
            json!({"width": 5, "height": 5, "pattern": "plaid"}),
            json!({"width": 50, "height": 50}),
            json!({"width": 5, "height": 5, "spacing": 0.1}),
        ] {
            assert!(s.execute("embroidery.quiltBlock", &bad).is_err(), "{bad}");
        }
    }

    /// T2: a set writes one file per size, named by its size; sizes no hoop takes are skipped.
    #[test]
    fn a_set_writes_a_file_per_block_size() {
        let dir = std::env::temp_dir().join(format!("sf-quilt-set-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).unwrap();
        let mut s = Session::new();
        s.execute("embroidery.myMachine.set", &json!({"machine": "Husqvarna Viking Designer Diamond deLuxe"})).unwrap();
        let r = s
            .execute(
                "embroidery.quiltBlock.set",
                &json!({"folder": dir.to_string_lossy(), "pattern": "waves", "sizes": [{"width": 4, "height": 4}, {"width": 5, "height": 7}, {"width": 30, "height": 30}]}),
            )
            .unwrap();
        let files: Vec<String> = r["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap().to_string()).collect();
        assert_eq!(files.len(), 2, "{r}");
        assert!(files.iter().any(|f| f.ends_with("Waves 4x4 in.vp3")) && files.iter().any(|f| f.ends_with("Waves 5x7 in.vp3")), "{files:?}");
        assert!(files.iter().all(|f| std::fs::metadata(f).unwrap().len() > 100));
        assert_eq!(r["skipped"].as_array().unwrap().len(), 1, "{r}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
