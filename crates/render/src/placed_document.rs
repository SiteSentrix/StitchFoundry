//! Drawing placed documents ([`PlacedDocument`]): the art read from the file
//! ([`vectorcraft_doc::placed_document::read`]) drawn with its own resources, through a view that
//! maps its artboard into the object's box (nothing is copied into the document).
//!
//! A document opened with only its placed documents' previews
//! ([`vectorcraft_doc::ImageBlob::is_proxy`]) shows them until the file is at hand
//! ([`vectorcraft_doc::placed_document::full_bytes_cached`]), and for good when it can't be found. Placed documents inside a placed document are read from their
//! files, up to [`MAX_DEPTH`] deep; deeper ones show their previews.

use std::cell::Cell;
use std::sync::Arc;

use vectorcraft_doc::placed_document::MAX_DEPTH;
use vectorcraft_doc::{Node, PlacedDocument};
use vectorcraft_geom::Affine;
use vello_cpu::RenderContext;

use crate::{Frame, Renderer};

thread_local! {
    /// How deep inside placed documents this thread is drawing.
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

/// While alive: drawing inside a placed document, one level deeper.
struct Inside;

impl Inside {
    fn enter() -> Self {
        DEPTH.with(|d| d.set(d.get() + 1));
        Inside
    }
}

impl Drop for Inside {
    fn drop(&mut self) {
        DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

fn depth() -> usize {
    DEPTH.with(Cell::get)
}

/// Render `art` of a placed document's file (`doc`, its resources) into `w`×`h` transparent
/// pixels through `view`, the placed documents inside it one level deeper.
pub fn render_inside(r: &mut Renderer, doc: &vectorcraft_doc::Document, art: &Arc<Node>, w: u16, h: u16, view: Affine) -> Option<crate::Rendered> {
    let _inside = Inside::enter();
    r.render_node(doc, art, w, h, view)
}

/// The mean scale of `a`.
fn mean_scale(a: Affine) -> f64 {
    a.determinant().abs().sqrt()
}

impl Renderer {
    /// Draw placed document `p` in frame `f`.
    pub(crate) fn draw_placed(&mut self, ctx: &mut RenderContext, f: &Frame, p: &PlacedDocument) {
        let Some(blob) = f.doc.images.get(&p.key) else { return };
        // The file's bytes: stored, read again already, or (inside another placed document, while
        // not too deep) read again now.
        let full = match depth() {
            0 => vectorcraft_doc::placed_document::full_bytes_cached(p, blob),
            d if d < MAX_DEPTH => vectorcraft_doc::placed_document::full_bytes(p, blob),
            _ => None,
        };
        let read = full.and_then(|bytes| vectorcraft_doc::placed_document::read(p, &bytes));
        let Some(e) = read else {
            // Only the preview: drawn scaled into the box.
            if blob.is_proxy()
                && let Some(im) = p.preview_image(&blob.bytes)
            {
                self.draw_image(ctx, f, &im);
            }
            return;
        };
        let Some(m) = p.art_xf(e.frame) else { return };
        let k = mean_scale(m);
        if !k.is_finite() || k <= 1e-12 {
            return;
        }
        // The art in its own coordinates, with its own resources.
        let inner = Frame { doc: &e.doc, view: f.view * m, visible: m.inverse().transform_rect_bbox(f.visible), px: f.px / k, ..*f };
        let _inside = Inside::enter();
        self.draw_arc(ctx, &inner, &e.art);
    }
}
