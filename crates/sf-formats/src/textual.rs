//! Open text exports of a design's stitches, defined by StitchFoundry (not machine formats): a
//! CSV stitch list and a JSON stitch list to read into spreadsheets and scripts, and G-code for
//! machines driven by G-code (home-built embroidery machines on CNC controllers). Coordinates are
//! millimetres from the design's centre, x to the right; y down in the lists, up in G-code
//! (as CNC machines count) unless asked otherwise.

use std::fmt::Write as _;

use sf_core::{Cmd, Pattern};

/// The commands as the lists name them.
fn name(cmd: Cmd) -> &'static str {
    match cmd {
        Cmd::Stitch => "stitch",
        Cmd::Jump => "jump",
        Cmd::Trim => "trim",
        Cmd::ColorChange => "color",
        Cmd::Stop => "stop",
        Cmd::End => "end",
    }
}

/// The offset that puts the design's centre at 0, 0 (or none).
fn origin(pattern: &Pattern, center: bool) -> (f64, f64) {
    let b = pattern.bounds();
    if center && !b.is_empty() { ((b.min.x + b.max.x) / 2.0, (b.min.y + b.max.y) / 2.0) } else { (0.0, 0.0) }
}

/// A CSV field: quoted when it holds a comma, a quote or a line break.
fn field(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) { format!("\"{}\"", s.replace('"', "\"\"")) } else { s.to_string() }
}

/// The CSV stitch list: a header line, then `index,command,x,y,thread,color` a stitch (x, y in
/// mm to 0.01; thread from 1, as sewn).
pub fn write_csv(pattern: &Pattern, center: bool) -> String {
    let (ox, oy) = origin(pattern, center);
    let mut out = String::from("index,command,x,y,thread,color,name\n");
    let mut thread = 0usize;
    for (i, st) in pattern.stitches.iter().enumerate() {
        if st.cmd == Cmd::ColorChange {
            thread += 1;
        }
        let t = pattern.thread(thread);
        let _ = writeln!(out, "{i},{},{:.2},{:.2},{},{},{}", name(st.cmd), st.x - ox, st.y - oy, thread + 1, t.color.to_hex(), field(&t.label()));
    }
    out
}

/// The JSON stitch list: `{format: "stitchfoundry-stitches", version: 1, units: "mm", name,
/// threads: [{color, name, code, chart}], stitches: [[x, y, command]…]}`.
pub fn write_json(pattern: &Pattern, center: bool) -> String {
    let (ox, oy) = origin(pattern, center);
    let r = |v: f64| (v * 100.0).round() / 100.0;
    let blocks = pattern.stitches.iter().filter(|s| s.cmd == Cmd::ColorChange).count() + 1;
    let threads: Vec<serde_json::Value> = (0..blocks)
        .map(|i| {
            let t = pattern.thread(i);
            serde_json::json!({ "color": t.color.to_hex(), "name": t.name, "code": t.code, "chart": t.chart })
        })
        .collect();
    let stitches: Vec<serde_json::Value> = pattern.stitches.iter().map(|s| serde_json::json!([r(s.x - ox), r(s.y - oy), name(s.cmd)])).collect();
    serde_json::json!({
        "format": "stitchfoundry-stitches",
        "version": 1,
        "units": "mm",
        "name": pattern.name,
        "threads": threads,
        "stitches": stitches,
    })
    .to_string()
}

/// How G-code drives the needle and the machine.
#[derive(Clone, Debug, PartialEq)]
pub struct GcodeOptions {
    /// Feed rate for stitches (mm/min).
    pub feed: f64,
    /// Each stitch advances the Z axis by this much (one needle cycle); 0: no Z moves.
    pub z_step: f64,
    /// Written after each stitch's move (e.g. `M3` for a needle relay); empty: none.
    pub stitch_command: String,
    /// Written at a thread change, after a comment naming the next thread (`M0`: pause).
    pub color_command: String,
    /// Written at a trim; empty: a comment only.
    pub trim_command: String,
    /// Whether y counts up (as CNC machines do) rather than down.
    pub flip_y: bool,
    pub center: bool,
}

impl Default for GcodeOptions {
    fn default() -> Self {
        Self {
            feed: 1200.0,
            z_step: 1.0,
            stitch_command: String::new(),
            color_command: "M0".into(),
            trim_command: String::new(),
            flip_y: true,
            center: true,
        }
    }
}

/// G-code for a machine driven by a CNC controller: absolute millimetres; a jump is a rapid move
/// (`G0`), a stitch a feed move (`G1`) then a needle cycle (the Z step and/or the stitch
/// command); thread changes and stops pause with the colour command, named in a comment.
pub fn write_gcode(pattern: &Pattern, opts: &GcodeOptions) -> String {
    let (ox, oy) = origin(pattern, opts.center);
    let fy = if opts.flip_y { -1.0 } else { 1.0 };
    let one_line = |s: &str| s.replace(['\n', '\r', '(', ')', ';'], " ").trim().to_string();
    let mut out = String::new();
    let _ = writeln!(out, "(StitchFoundry G-code: {})", one_line(&pattern.name));
    let _ =
        writeln!(out, "(stitches {}, threads {})", pattern.stitch_count(), pattern.stitches.iter().filter(|s| s.cmd == Cmd::ColorChange).count() + 1);
    out.push_str("G21 (millimetres)\nG90 (absolute)\n");
    let _ = writeln!(out, "(thread 1: {} {})", pattern.thread(0).color.to_hex(), one_line(&pattern.thread(0).label()));
    let (mut z, mut thread) = (0.0f64, 0usize);
    let mut feed_set = false;
    for st in &pattern.stitches {
        let (x, y) = (st.x - ox, (st.y - oy) * fy);
        match st.cmd {
            Cmd::Stitch => {
                if feed_set {
                    let _ = writeln!(out, "G1 X{x:.2} Y{y:.2}");
                } else {
                    let _ = writeln!(out, "G1 X{x:.2} Y{y:.2} F{:.0}", opts.feed);
                    feed_set = true;
                }
                if opts.z_step > 0.0 {
                    z += opts.z_step;
                    let _ = writeln!(out, "G1 Z{z:.2}");
                }
                if !opts.stitch_command.trim().is_empty() {
                    let _ = writeln!(out, "{}", one_line(&opts.stitch_command));
                }
            }
            Cmd::Jump => {
                let _ = writeln!(out, "G0 X{x:.2} Y{y:.2}");
            }
            Cmd::Trim => {
                out.push_str("(trim)\n");
                if !opts.trim_command.trim().is_empty() {
                    let _ = writeln!(out, "{}", one_line(&opts.trim_command));
                }
            }
            Cmd::ColorChange => {
                thread += 1;
                let t = pattern.thread(thread);
                let _ = writeln!(out, "(thread {}: {} {})", thread + 1, t.color.to_hex(), one_line(&t.label()));
                let _ = writeln!(out, "{}", one_line(&opts.color_command));
            }
            Cmd::Stop => {
                out.push_str("(stop)\n");
                let _ = writeln!(out, "{}", one_line(&opts.color_command));
            }
            Cmd::End => {}
        }
    }
    out.push_str("M2 (end)\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use sf_core::{Rgb, Stitch, Thread};

    fn pattern() -> Pattern {
        let mut p = Pattern::new();
        p.name = "Logo, \"one\"".into();
        p.threads = vec![Thread::new(Rgb::new(0xd6, 0x28, 0x28), "Red, bright"), Thread::new(Rgb::new(0, 0, 0xff), "Blue")];
        for (x, y, cmd) in [
            (0.0, 0.0, Cmd::Jump),
            (0.0, 0.0, Cmd::Stitch),
            (10.0, 0.0, Cmd::Stitch),
            (10.0, 0.0, Cmd::Trim),
            (10.0, 0.0, Cmd::ColorChange),
            (10.0, 20.0, Cmd::Jump),
            (20.0, 20.0, Cmd::Stitch),
            (20.0, 20.0, Cmd::End),
        ] {
            p.stitches.push(Stitch::at(sf_core::Point::new(x, y), cmd));
        }
        p
    }

    #[test]
    fn the_csv_list_has_a_line_a_stitch_with_its_thread() {
        let csv = write_csv(&pattern(), true);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 9);
        assert_eq!(lines[0], "index,command,x,y,thread,color,name");
        assert_eq!(lines[2], "1,stitch,-10.00,-10.00,1,#d62828,\"Red, bright\"");
        assert_eq!(lines[7], "6,stitch,10.00,10.00,2,#0000ff,Blue");
        assert!(write_csv(&pattern(), false).lines().nth(3).unwrap().starts_with("2,stitch,10.00,0.00"));
    }

    #[test]
    fn the_json_list_reads_back_as_json() {
        let v: serde_json::Value = serde_json::from_str(&write_json(&pattern(), true)).unwrap();
        assert_eq!(v["format"], "stitchfoundry-stitches");
        assert_eq!(v["threads"].as_array().unwrap().len(), 2);
        assert_eq!(v["stitches"][2], serde_json::json!([0.0, -10.0, "stitch"]));
        assert_eq!(v["stitches"][4][2], "color");
    }

    #[test]
    fn gcode_moves_feeds_and_pauses_for_threads() {
        let g = write_gcode(&pattern(), &GcodeOptions::default());
        let lines: Vec<&str> = g.lines().collect();
        assert!(lines.contains(&"G0 X-10.00 Y10.00"), "y counts up: {g}");
        assert!(lines.contains(&"G1 X-10.00 Y10.00 F1200") && lines.contains(&"G1 X0.00 Y10.00"));
        assert_eq!(lines.iter().filter(|l| l.starts_with("G1 Z")).count(), 3);
        assert!(lines.contains(&"G1 Z3.00") && lines.contains(&"M0") && lines.contains(&"(trim)"));
        assert!(g.starts_with("(StitchFoundry G-code: Logo, \"one\")"), "{g}");
        assert_eq!(lines.last(), Some(&"M2 (end)"));
        let o = GcodeOptions { z_step: 0.0, stitch_command: "M3\nM5".into(), trim_command: "M8".into(), flip_y: false, ..GcodeOptions::default() };
        let g = write_gcode(&pattern(), &o);
        assert!(!g.contains("G1 Z") && g.contains("M3 M5\n") && g.contains("M8\n") && g.contains("G0 X-10.00 Y-10.00"), "{g}");
    }
}
