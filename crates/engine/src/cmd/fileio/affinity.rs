//! Explicit preview-only Affinity import; native vectors and document layers are not decoded.

use std::io::Cursor;

use vectorcraft_doc::Document;

use super::load::{err, raster_doc};
use crate::Result;

pub(super) fn import(name: &str, bytes: &[u8]) -> Result<(Document, Vec<String>)> {
    let p = vectorcraft_affinity::preview(bytes).map_err(err)?;
    // `raster_doc` normally probes PNG dimensions and keeps its encoded bytes. Verify this
    // untrusted preview's complete pixels first, so corrupt data cannot create a blank image.
    let mut reader = image::ImageReader::with_format(Cursor::new(p.png), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(128 << 20);
    reader.limits(limits);
    let _pixels = reader.decode().map_err(err)?;
    let d = raster_doc(name, p.png)?;
    let warnings = vec![format!(
        "Opened only Affinity's embedded {}×{} PNG preview, which may be smaller than the document. Native vectors, layers, text, masks, effects, pages and the document's colour settings are not imported. Save a new copy; the Affinity source cannot be saved back. Export SVG or PDF from Affinity for editable vectors.",
        p.width, p.height
    )];
    Ok((d, warnings))
}
