//! Marks and Bleeds: the boxes of each exported page.

use vectorcraft_doc::Document;
use vectorcraft_doc::marks::outset;
use vectorcraft_doc::setup::MAX_BLEED;
use vectorcraft_geom::Rect;

use crate::PdfSettings;

impl PdfSettings {
    /// The bleed of `doc`'s pages, `[top, bottom, left, right]` in points: the document's (Document
    /// Setup; one read from a file is kept to 0–[`MAX_BLEED`]) with Use Document Bleed, else the
    /// settings' own.
    pub fn bleed_of(&self, doc: &Document) -> [f64; 4] {
        if self.bleed.use_document { doc.setup.bleed.map(|b| if b.is_finite() { b.clamp(0.0, MAX_BLEED) } else { 0.0 }) } else { self.bleed.values() }
    }
}

/// The boxes of one page, in document coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PageBoxes {
    /// The artboard: the finished page.
    pub trim: Rect,
    /// The trim box grown by the bleed: the art it holds is kept.
    pub bleed: Rect,
    /// The sheet: the bleed box.
    pub media: Rect,
}

impl PageBoxes {
    pub fn new(trim: Rect, bleed: [f64; 4]) -> Self {
        let r = outset(trim, bleed);
        Self { trim, bleed: r, media: r }
    }
}
