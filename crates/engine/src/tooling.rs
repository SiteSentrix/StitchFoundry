//! Hosting the active tool: pointer/key events → actions → commands.

use serde_json::Value;
use vectorcraft_geom::Point;
use vectorcraft_tools::{Action, Cursor, Mods, Overlay, PointerEvent, ToolContext, ToolKey};

use crate::{Result, Session};

/// View state the tools need from the frontend.
#[derive(Clone, Copy, Debug)]
pub struct ViewInfo {
    pub zoom: f64,
    pub outline: bool,
    pub smart_guides: bool,
    pub snap_to_grid: bool,
    pub show_bbox: bool,
    /// View → Snap to Pixel: drawing and moving land on whole pixels (points at 72 ppi).
    pub snap_to_pixel: bool,
    /// View → Snap to Point: picked points (a transform's reference point) land on anchors.
    pub snap_to_point: bool,
    /// View → Show Corner Widget.
    pub corner_widgets: bool,
}

impl Default for ViewInfo {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            outline: false,
            smart_guides: true,
            snap_to_grid: false,
            show_bbox: true,
            snap_to_pixel: false,
            snap_to_point: true,
            corner_widgets: true,
        }
    }
}

/// Requests from tools that only the frontend can fulfil.
#[derive(Clone, Debug, PartialEq)]
pub enum UiRequest {
    Dialog(String, Value),
    SwitchTool(String),
}

impl Session {
    pub fn tool_id(&self) -> &'static str {
        self.tool.id()
    }

    /// Switch tools (finishing any pending tool work first).
    pub fn select_tool(&mut self, id: &str, view: ViewInfo) -> Result<()> {
        if self.tool.id() == id {
            return Ok(());
        }
        let acts = self.with_tool_cx(view, |t, cx| t.deactivate(cx));
        self.apply_actions(acts)?;
        self.tool = vectorcraft_tools::create(id);
        Ok(())
    }

    pub(crate) fn with_tool_cx<R>(&mut self, view: ViewInfo, f: impl FnOnce(&mut dyn vectorcraft_tools::Tool, &ToolContext) -> R) -> R
    where
        R: Default,
    {
        let Some(st) = self.active.and_then(|i| self.docs.get(i)) else { return R::default() };
        let cx = ToolContext {
            doc: &st.doc,
            selection: &st.selection,
            zoom: view.zoom,
            isolation: st.isolation,
            paint: &self.paint,
            outline: view.outline,
            smart_guides: view.smart_guides,
            snap_to_grid: view.snap_to_grid,
            show_bbox: view.show_bbox,
            snap_to_pixel: view.snap_to_pixel,
            snap_to_point: view.snap_to_point,
            corner_widgets: view.corner_widgets,
        };
        f(self.tool.as_mut(), &cx)
    }

    /// Feed a pointer event to the active tool. Returns requests for the UI.
    pub fn pointer(&mut self, ev: &PointerEvent, view: ViewInfo) -> Result<Vec<UiRequest>> {
        self.last_view = view;
        let acts = self.with_tool_cx(view, |t, cx| t.pointer(cx, ev));
        self.apply_actions(acts)
    }

    pub fn tool_key(&mut self, key: ToolKey, mods: Mods, view: ViewInfo) -> Result<Vec<UiRequest>> {
        let acts = self.with_tool_cx(view, |t, cx| t.key(cx, key, mods));
        self.apply_actions(acts)
    }

    /// Typed text for the active tool (Type tool).
    pub fn tool_text(&mut self, text: &str, view: ViewInfo) -> Result<Vec<UiRequest>> {
        let acts = self.with_tool_cx(view, |t, cx| t.text_input(cx, text));
        self.apply_actions(acts)
    }

    pub fn tool_wants_text(&self) -> bool {
        self.tool.wants_text()
    }

    pub fn tool_busy(&self) -> bool {
        self.tool.busy()
    }

    pub fn overlays(&mut self, view: ViewInfo) -> Vec<Overlay> {
        let mut v = self.with_tool_cx(view, |t, cx| t.overlays(cx));
        if let Some(d) = self.active() {
            v.splice(0..0, vectorcraft_tools::distort::perspective::grid_overlays(&d.doc, 1.0 / view.zoom.max(1e-9), self.tool.id()));
        }
        v
    }

    pub fn cursor(&mut self, p: Point, mods: Mods, view: ViewInfo) -> Cursor {
        self.with_tool_cx(view, |t, cx| t.cursor(cx, p, mods))
    }

    pub fn tool_options(&self) -> Value {
        self.tool.options()
    }
    pub fn set_tool_option(&mut self, key: &str, v: &Value) {
        self.tool.set_option(key, v);
    }

    /// Apply tool actions. Errors from previews are reported but keep the interaction alive.
    pub fn apply_actions(&mut self, acts: Vec<Action>) -> Result<Vec<UiRequest>> {
        let mut ui = vec![];
        for a in acts {
            match a {
                Action::Begin(label) => self.begin_interaction(&label)?,
                Action::Preview(cmd, p) => {
                    if let Err(e) = self.preview(&cmd, &p) {
                        log::warn!("preview {cmd}: {e}");
                    }
                }
                Action::Commit => self.commit_interaction()?,
                Action::Cancel => self.cancel_interaction()?,
                Action::Exec(cmd, p) => {
                    self.execute(&cmd, &p)?;
                }
                Action::Dialog(k, p) => ui.push(UiRequest::Dialog(k, p)),
                Action::SwitchTool(t) => ui.push(UiRequest::SwitchTool(t)),
                Action::Notify(what) => {
                    let view = self.last_view;
                    self.with_tool_cx(view, |t, cx| t.notify(cx, &what));
                }
            }
        }
        Ok(ui)
    }
}
