//! Icon lookup and drawing (SVGs rasterized by egui_extras, tinted with theme colours).

use std::sync::OnceLock;

use craft_ui::icons::{SvgIconSet, SvgStyle};
use egui::{Color32, ImageSource, Rect, Ui, Vec2};

fn table() -> &'static SvgIconSet {
    static T: OnceLock<SvgIconSet> = OnceLock::new();
    T.get_or_init(|| {
        SvgIconSet::new(crate::icon_data::ICONS, SvgStyle { current_color: "white", stroke_width: Some("1.6") }, "bytes://icon/", "square-dashed")
    })
}

pub fn exists(name: &str) -> bool {
    table().contains(name)
}

/// Map tool-catalogue icon names to SVG files.
pub fn tool_icon(name: &str) -> &'static str {
    match name {
        "tool-selection" => "dc-selection",
        "tool-direct" => "dc-direct",
        "tool-group-select" => "dc-group-select",
        "tool-magic-wand" => "wand-sparkles",
        "tool-lasso" => "lasso",
        "tool-pen" => "pen-tool",
        "tool-pen-add" => "pen-tool-add",
        "tool-pen-delete" => "pen-tool-delete",
        "tool-anchor" => "dc-anchor",
        "tool-curvature" => "spline",
        "tool-type" => "type",
        "tool-type-area" => "dc-type-area",
        "tool-type-path" => "dc-type-path",
        "tool-type-vertical" => "dc-type-vertical",
        "tool-touch-type" => "dc-touch-type",
        "tool-line" => "dc-line",
        "tool-arc" => "dc-arc",
        "tool-spiral" => "tornado",
        "tool-rect-grid" => "dc-rect-grid",
        "tool-polar-grid" => "dc-polar-grid",
        "tool-rect" => "square",
        "tool-rounded-rect" => "dc-rounded-rect",
        "tool-ellipse" => "dc-ellipse",
        "tool-polygon" => "dc-polygon",
        "tool-star" => "star",
        "tool-flare" => "dc-flare",
        "tool-brush" => "paintbrush",
        "tool-blob-brush" => "brush",
        "tool-shaper" => "shapes",
        "tool-pencil" => "pencil",
        "tool-smooth" => "dc-smooth",
        "tool-path-eraser" => "dc-path-eraser",
        "tool-join" => "dc-join",
        "tool-eraser" => "eraser",
        "tool-scissors" => "scissors",
        "tool-knife" => "dc-knife",
        "tool-mirror-cut" => "dc-mirror-cut",
        "tool-line-cut" => "dc-line-cut",
        "tool-rect-cut" => "dc-rect-cut",
        "tool-rotate" => "rotate-ccw",
        "tool-reflect" => "flip-horizontal-2",
        "tool-scale" => "scaling",
        "tool-shear" => "dc-shear",
        "tool-reshape" => "dc-reshape",
        "tool-width" => "dc-width",
        "tool-warp" => "waves",
        "tool-twirl" => "dc-twirl",
        "tool-pucker" => "dc-pucker",
        "tool-bloat" => "dc-bloat",
        "tool-scallop" => "dc-scallop",
        "tool-crystallize" => "dc-crystallize",
        "tool-wrinkle" => "dc-wrinkle",
        "tool-free-transform" => "dc-free-transform",
        "tool-puppet" => "dc-puppet",
        "tool-shape-builder" => "dc-shape-builder",
        "tool-bucket" => "dc-live-bucket",
        "tool-live-select" => "dc-live-select",
        "tool-perspective" => "dc-perspective",
        "tool-perspective-select" => "dc-perspective",
        "tool-mesh" => "dc-mesh",
        "tool-gradient" => "dc-gradient",
        "tool-eyedropper" => "pipette",
        "tool-measure" => "dc-measure",
        "tool-blend" => "dc-blend",
        "tool-symbol" => "dc-symbol-sprayer",
        "tool-graph" => "chart-column",
        "tool-artboard" => "frame",
        "tool-slice" => "slice",
        "tool-hand" => "hand",
        "tool-rotate-view" => "dc-rotate-view",
        "tool-print-tiling" => "printer",
        "tool-zoom" => "zoom-in",
        other => {
            if exists(other) {
                table().canonical_name(other).unwrap_or("square-dashed")
            } else {
                "square-dashed"
            }
        }
    }
}

pub fn source(name: &str) -> ImageSource<'static> {
    table().source(name)
}

/// Paint icon `name` into `rect` tinted with `tint`.
pub fn paint(ui: &Ui, name: &str, rect: Rect, tint: Color32) {
    egui::Image::new(source(name)).tint(tint).fit_to_exact_size(rect.size()).paint_at(ui, rect);
}

/// An icon widget of `size` points.
pub fn icon(ui: &mut Ui, name: &str, size: f32, tint: Color32) -> egui::Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    paint(ui, name, rect, tint);
    resp
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Characterize the pre-extraction loader against every embedded asset, including custom
    /// icons. A shared-crate update must not silently change our stroke weight or cache keys.
    #[test]
    fn shared_catalog_preserves_existing_icons() {
        for (name, original) in crate::icon_data::ICONS {
            let expected = String::from_utf8_lossy(original).replace("currentColor", "white").replace("stroke-width=\"2\"", "stroke-width=\"1.6\"");
            let ImageSource::Bytes { uri, bytes } = source(name) else { panic!("expected SVG bytes") };
            assert_eq!(uri.as_ref(), format!("bytes://icon/{name}.svg"));
            assert_eq!(bytes.as_ref(), expected.as_bytes(), "{name}");
            assert!(exists(name));
            assert_eq!(tool_icon(name), *name);
        }
    }

    #[test]
    fn shared_catalog_preserves_fallback_and_tool_aliases() {
        let ImageSource::Bytes { uri, bytes } = source("missing-icon") else { panic!("expected SVG bytes") };
        let ImageSource::Bytes { bytes: fallback, .. } = source("square-dashed") else { panic!("expected SVG bytes") };
        assert_eq!(uri.as_ref(), "bytes://icon/missing-icon.svg");
        assert_eq!(bytes.as_ref(), fallback.as_ref());
        assert!(!exists("missing-icon"));
        assert_eq!(tool_icon("missing-icon"), "square-dashed");
        assert_eq!(tool_icon("tool-selection"), "dc-selection");
    }
}
