//! Live objects (blends, envelopes, gradient meshes, repeats) evaluated alike for the renderer and
//! every exporter, with what the pure evaluation in [`vectorcraft_doc::live`] can't do itself: type
//! inside an envelope distorts as its glyph outlines, run by run ([`outline_text`]).

use vectorcraft_doc::Node;
use vectorcraft_doc::live::{self, Outliner};

use crate::outline_text;

/// The text outliner live evaluation uses: type as its glyph outlines, one path per run in the
/// run's paint ([`outline_text`]).
pub fn text_outliner() -> Outliner<'static> {
    Some(&outline_text)
}

/// One level of evaluation of live object `n` ([`live::expand_live`]) with type outlined.
pub fn expand_live(n: &Node) -> Vec<Node> {
    live::expand_live_with(n, text_outliner())
}

/// `n` with every live object in it replaced by its evaluated art ([`live::expand_deep`]) and type
/// inside envelopes outlined: what the exporters write for live objects.
pub fn expand_live_deep(n: &Node) -> Node {
    live::expand_deep(n, text_outliner())
}
