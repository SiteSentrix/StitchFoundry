//! Writes a small hand-made design (a filled square and a satin-like zigzag in
//! two colours) in every machine format.
//!
//! ```text
//! cargo run -p sf-formats --example write_all -- OUT_DIR
//! ```

use sf_core::{Pattern, Point, Rgb, Thread};
use sf_formats::{Format, WriteOptions, read, write};
use std::path::PathBuf;

fn sample() -> Pattern {
    let mut p = Pattern::new();
    p.name = "Sample".into();
    p.threads = vec![Thread::new(Rgb::new(0x0A, 0x55, 0xA3), "Blue"), Thread::new(Rgb::new(0xED, 0x17, 0x1F), "Red")];

    // A 20 mm square filled with rows 0.4 mm apart, 2.5 mm stitches, every other row offset.
    let (x0, y0, size, spacing, stitch) = (-22.0, -10.0, 20.0, 0.4, 2.5);
    p.jump(Point::new(x0, y0));
    let rows = (size / spacing) as usize;
    for row in 0..=rows {
        let y = y0 + row as f64 * spacing;
        let (from, to) = if row % 2 == 0 { (x0, x0 + size) } else { (x0 + size, x0) };
        let offset = if row % 4 < 2 { 0.0 } else { stitch / 2.0 };
        let mut x = 0.0;
        p.stitch(Point::new(from, y));
        while x + stitch < size {
            x += if x == 0.0 && offset > 0.0 { offset } else { stitch };
            p.stitch(Point::new(from + (to - from) * x / size, y));
        }
        p.stitch(Point::new(to, y));
    }
    p.trim();
    p.color_change();

    // A satin-like column 4 mm wide along a diagonal, swinging side to side every 0.5 mm.
    p.jump(Point::new(6.0, -10.0));
    let steps = 60;
    for i in 0..=steps {
        let t = i as f64 / steps as f64;
        let (cx, cy) = (8.0 + 12.0 * t, -10.0 + 20.0 * t);
        let side = if i % 2 == 0 { -1.0 } else { 1.0 };
        p.stitch(Point::new(cx - 2.0 * side, cy + 0.6 * side));
    }
    p.trim();
    p.ensure_end();
    p
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = PathBuf::from(std::env::args().nth(1).ok_or("usage: write_all OUT_DIR")?);
    std::fs::create_dir_all(&dir)?;
    let pattern = sample();
    for format in Format::ALL {
        let bytes = write(format, &pattern, &WriteOptions::named("Sample"))?;
        let path = dir.join(format!("sample.{}", format.extension()));
        std::fs::write(&path, &bytes)?;
        // Read it back as a check.
        let back = read(format, &bytes)?;
        println!(
            "{:<12} {} ({} bytes, {} stitches, {} colours)",
            format.name(),
            path.display(),
            bytes.len(),
            back.stitch_count(),
            back.threads.len()
        );
    }
    Ok(())
}
