//! VectorCraft tools: pointer events in → commands and overlays out.
//!
//! Tools never mutate the document directly. They emit [`Action`]s that the engine executes:
//! `Begin` snapshots the document, each `Preview` re-applies one command on top of that snapshot
//! (replacing the previous preview), and `Commit` records the last preview as a single undo step
//! and journal entry. One-shot `Exec` actions run immediately. This makes every gesture replayable
//! by the control channel and MCP, and keeps tools testable without a UI.
#![forbid(unsafe_code)]

pub mod bbox;
pub mod builder;
pub mod catalog;
pub mod corners;
pub mod direct;
pub mod distort;
pub mod draw2;
pub mod extra;
pub mod guides;
pub mod meshblend;
pub mod params;
pub mod pen;
pub mod select;
pub mod shape;
pub mod symbolism;
pub mod text;
pub mod xform;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use vectorcraft_color::Paint;
use vectorcraft_doc::{Document, NodeId, Selection};
use vectorcraft_geom::{BezPath, Point, Rect};

pub use catalog::{TOOL_GROUPS, ToolInfo, tool_info};

/// Modifier keys.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mods {
    #[serde(default)]
    pub shift: bool,
    /// Option on macOS.
    #[serde(default)]
    pub alt: bool,
    /// Command on macOS, Ctrl elsewhere.
    #[serde(default)]
    pub cmd: bool,
    #[serde(default)]
    pub ctrl: bool,
    #[serde(default)]
    pub space: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PointerKind {
    /// Button pressed.
    Down,
    /// Moved with the button held.
    Drag,
    /// Button released.
    Up,
    /// Moved with no button (hover).
    Move,
    DoubleClick,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointerEvent {
    pub kind: PointerKind,
    /// Document coordinates.
    pub pos: Point,
    #[serde(default)]
    pub mods: Mods,
    #[serde(default = "one")]
    pub pressure: f32,
}

fn one() -> f32 {
    1.0
}

impl PointerEvent {
    pub fn new(kind: PointerKind, x: f64, y: f64) -> Self {
        Self { kind, pos: Point::new(x, y), mods: Mods::default(), pressure: 1.0 }
    }
    pub fn with_mods(mut self, m: Mods) -> Self {
        self.mods = m;
        self
    }
}

/// Keys tools care about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolKey {
    Enter,
    Escape,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    /// Illustrator: ↑/↓ while drawing a polygon/star change sides/points; `[`/`]` change sizes.
    BracketLeft,
    BracketRight,
    Tab,
    Home,
    End,
}

/// What a tool asks the engine to do.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// Start an interaction (snapshot the document). The label becomes the undo name.
    Begin(String),
    /// Replace the current preview with this command, applied to the snapshot.
    Preview(String, Value),
    /// Finish the interaction: keep the last preview as one undo step.
    Commit,
    /// Abort the interaction: restore the snapshot.
    Cancel,
    /// Execute a command immediately (its own undo step, if it edits).
    Exec(String, Value),
    /// Ask the UI to open a dialog (e.g. click with the Rectangle tool → size dialog).
    Dialog(String, Value),
    /// Switch to another tool (e.g. after placing a text frame).
    SwitchTool(String),
    /// Call the tool's `notify` after the preceding actions ran (e.g. to pick up a created object).
    Notify(String),
}

/// Paint defaults for new art (the fill/stroke proxy).
#[derive(Clone, Debug, PartialEq)]
pub struct PaintDefaults {
    pub fill: Paint,
    pub stroke: Paint,
    pub stroke_width: f64,
}

/// Read-only context a tool sees.
pub struct ToolContext<'a> {
    pub doc: &'a Document,
    pub selection: &'a Selection,
    /// Screen pixels per document point.
    pub zoom: f64,
    pub isolation: Option<NodeId>,
    pub paint: &'a PaintDefaults,
    pub outline: bool,
    pub smart_guides: bool,
    pub snap_to_grid: bool,
    /// Bounding box shown (View → Show/Hide Bounding Box).
    pub show_bbox: bool,
    /// View → Snap to Pixel.
    pub snap_to_pixel: bool,
    /// View → Snap to Point: picked points (a transform's reference point) land on anchors.
    pub snap_to_point: bool,
    /// View → Show Corner Widget: live rectangles show draggable Live Corners widgets.
    pub corner_widgets: bool,
    /// Which paint proxy is in front (true = Fill): the one the Gradient tool edits.
    pub fill_active: bool,
    /// The selected gradient stop (`gradient.selectStop`), marked on the gradient annotator.
    pub gradient_stop: Option<usize>,
    /// The Appearance panel's active fill/stroke (paint-order index into the first selected
    /// object's stack); paint tools show and edit that item.
    pub appearance_item: Option<usize>,
    /// General → Constrain Angle (degrees): Shift constrains drags to 45° steps from it.
    pub constrain_angle: f64,
}

impl ToolContext<'_> {
    /// Tolerance in document units for `px` screen pixels.
    pub fn tol(&self, px: f64) -> f64 {
        px / self.zoom.max(1e-9)
    }
    pub fn hit_options(&self) -> vectorcraft_doc::hit::HitOptions {
        vectorcraft_doc::hit::HitOptions { tol: self.tol(3.0), outline: self.outline, path_only: false }
    }
}

/// Visual feedback drawn by the UI in screen space (coordinates here are document space).
#[derive(Clone, Debug, PartialEq)]
pub enum Overlay {
    /// Marquee / rubber band rectangle.
    Marquee(Rect),
    /// A path preview (e.g. pen rubber band) in a colour (RGB).
    Path { path: BezPath, color: [u8; 3], width: f32, dashed: bool },
    /// A line segment (smart guide, handle line).
    Line { a: Point, b: Point, color: [u8; 3], dashed: bool },
    /// An anchor square: filled = selected.
    Anchor { p: Point, color: [u8; 3], filled: bool, size: f32 },
    /// A handle end circle.
    Handle { p: Point, color: [u8; 3] },
    /// Smart-guide style label (e.g. "anchor", "W: 100 pt").
    Label { p: Point, text: String, color: [u8; 3] },
    /// Measurement pill near the cursor (grey box with white text).
    Measure { p: Point, text: String },
    /// Translucent filled quad (text selection highlight), RGBA.
    Highlight { quad: [Point; 4], color: [u8; 4] },
    /// A colour chip of fixed screen size (a gradient stop), RGBA; ringed when selected.
    Swatch { p: Point, color: [u8; 4], selected: bool },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cursor {
    #[default]
    Arrow,
    ArrowHollow,
    Move,
    Crosshair,
    ResizeH,
    ResizeV,
    ResizeNwSe,
    ResizeNeSw,
    Rotate,
    /// Over a Live Corners widget (drag to round the corners).
    CornerRadius,
    Pen,
    PenAdd,
    PenDelete,
    PenClose,
    PenContinue,
    Text,
    Hand,
    HandGrab,
    ZoomIn,
    ZoomOut,
    Eyedropper,
    NotAllowed,
    /// Over the gradient annotator's bar: a click adds a stop.
    AddStop,
    /// A gradient stop dragged off the bar: releasing deletes it.
    RemoveStop,
}

/// A tool state machine.
pub trait Tool: Send {
    fn id(&self) -> &'static str;
    fn pointer(&mut self, cx: &ToolContext, ev: &PointerEvent) -> Vec<Action>;
    fn key(&mut self, _cx: &ToolContext, _key: ToolKey, _mods: Mods) -> Vec<Action> {
        vec![]
    }
    fn overlays(&self, _cx: &ToolContext) -> Vec<Overlay> {
        vec![]
    }
    fn cursor(&self, _cx: &ToolContext, _p: Point, _mods: Mods) -> Cursor {
        Cursor::Arrow
    }
    /// Tool options as JSON (shown by the Control bar / tool options dialog).
    fn options(&self) -> Value {
        Value::Null
    }
    fn set_option(&mut self, _key: &str, _value: &Value) {}
    /// Is an interaction in progress (drag, open pen path)?
    fn busy(&self) -> bool {
        false
    }
    /// Does the tool take `key` now, ahead of the command shortcuts bound to it (the Gradient tool
    /// with a stop selected takes Delete and the arrows)?
    fn claims_key(&self, _cx: &ToolContext, _key: ToolKey) -> bool {
        false
    }
    /// Does the tool want typed text (Type tool editing)? Single-key shortcuts are suppressed.
    fn wants_text(&self) -> bool {
        false
    }
    fn text_input(&mut self, _cx: &ToolContext, _s: &str) -> Vec<Action> {
        vec![]
    }
    fn notify(&mut self, _cx: &ToolContext, _what: &str) {}
    /// Called when the user switches away (finish pending work).
    fn deactivate(&mut self, _cx: &ToolContext) -> Vec<Action> {
        vec![]
    }
}

/// Create a tool by id. Unknown or not-yet-implemented tools fall back to a no-op tool that keeps the id.
pub fn create(id: &str) -> Box<dyn Tool> {
    match id {
        "selection" => Box::new(select::SelectionTool::default()),
        "directSelection" => Box::new(direct::DirectSelectionTool::new(false)),
        "groupSelection" => Box::new(direct::DirectSelectionTool::new(true)),
        "pen" => Box::new(pen::PenTool::default()),
        "type" | "areaType" | "typeOnPath" => Box::new(text::TypeTool::new(id)),
        "rectangle" | "roundedRectangle" | "ellipse" | "polygon" | "star" | "lineSegment" => Box::new(shape::ShapeTool::new(id)),
        other => symbolism::create(other)
            .or_else(|| builder::create(other))
            .or_else(|| draw2::create(other))
            .or_else(|| xform::create(other))
            .or_else(|| meshblend::create(other))
            .or_else(|| distort::create(other))
            .or_else(|| extra::create(other))
            .unwrap_or_else(|| Box::new(NoopTool(tool_info(other).map(|t| t.id).unwrap_or("selection")))),
    }
}

/// Placeholder for tools whose behaviour hasn't landed yet (selecting them still works).
pub struct NoopTool(&'static str);

impl Tool for NoopTool {
    fn id(&self) -> &'static str {
        self.0
    }
    fn pointer(&mut self, _cx: &ToolContext, _ev: &PointerEvent) -> Vec<Action> {
        vec![]
    }
}

pub(crate) fn json_ids(ids: &[NodeId]) -> Value {
    Value::Array(ids.iter().map(|i| Value::from(i.0)).collect())
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use vectorcraft_doc::{Appearance, Node};
    use vectorcraft_geom::shapes;

    pub fn doc_with_rect() -> (Document, NodeId) {
        let mut d = Document::new(500.0, 500.0);
        let l = d.layers[0].id;
        let id = d.alloc_id();
        d.insert(Some(l), 0, Node::path(id, shapes::rectangle(Rect::new(100.0, 100.0, 200.0, 200.0)), Appearance::default_art())).unwrap();
        (d, id)
    }

    pub fn paint() -> PaintDefaults {
        PaintDefaults {
            fill: Paint::solid(vectorcraft_color::Color::WHITE),
            stroke: Paint::solid(vectorcraft_color::Color::BLACK),
            stroke_width: 1.0,
        }
    }

    pub fn cx<'a>(d: &'a Document, s: &'a Selection, p: &'a PaintDefaults) -> ToolContext<'a> {
        ToolContext {
            doc: d,
            selection: s,
            zoom: 1.0,
            isolation: None,
            paint: p,
            outline: false,
            smart_guides: true,
            snap_to_grid: false,
            show_bbox: true,
            snap_to_pixel: false,
            snap_to_point: true,
            corner_widgets: true,
            fill_active: true,
            gradient_stop: None,
            appearance_item: None,
            constrain_angle: 0.0,
        }
    }
}
