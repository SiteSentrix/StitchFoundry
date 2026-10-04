//! The VectorCraft engine façade.
//!
//! Every user-visible action is a command with a stable id (`object.group`, `select.same.fillColor`,
//! `shape.rectangle`…) and JSON parameters. The egui UI, the CLI, the control channel and the MCP
//! server all go through [`Session::execute`]. Tools (pointer gestures) are hosted here too and
//! reduce to commands, so every gesture is journaled and replayable.
#![forbid(unsafe_code)]

pub mod cmd;
pub mod guard;
pub mod inspect;
mod tooling;
pub mod units;

use std::sync::Arc;

use serde_json::Value;
use vectorcraft_color::{Color, GradientPaint, Paint};
use vectorcraft_doc::{Document, NodeId, NodeKind, Selection};
use vectorcraft_geom::Affine;
use vectorcraft_tools::{PaintDefaults, Tool};

pub use cmd::EyedropperOptions;
pub use cmd::clipboard::Clipboard;
pub use cmd::rasterfx::{export_pdf, flatten_raster_effects};
pub use cmd::{CommandInfo, CommandSpec, command_specs, find_command};
pub use tooling::{UiRequest, ViewInfo};
pub use vectorcraft_doc as doc;
pub use vectorcraft_tools as tools;

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("unknown command `{0}`")]
    UnknownCommand(String),
    #[error("command `{0}` is not available right now: {1}")]
    Disabled(String, String),
    #[error("invalid parameters for `{cmd}`: {msg}")]
    BadParams { cmd: String, msg: String },
    #[error("no active document")]
    NoDocument,
    #[error("no such object {0}")]
    NoNode(NodeId),
    #[error("{0}")]
    Other(String),
    /// A bug: the command panicked. The document was rolled back to its state before the command.
    #[error("internal error in `{cmd}`: {msg} (the document was left as it was before; please report this bug)")]
    Internal { cmd: String, msg: String },
}

impl From<vectorcraft_doc::DocError> for EngineError {
    fn from(e: vectorcraft_doc::DocError) -> Self {
        EngineError::Other(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, EngineError>;

/// One undo step.
#[derive(Clone, Debug)]
pub struct HistoryEntry {
    pub label: String,
    pub doc: Arc<Document>,
    pub selection: Selection,
}

#[derive(Clone, Debug, Default)]
pub struct History {
    pub undo: Vec<HistoryEntry>,
    pub redo: Vec<HistoryEntry>,
    /// Maximum undo depth (Illustrator has no hard limit; snapshots are cheap here).
    pub limit: usize,
}

/// An in-progress drag: the snapshot the previews are applied to.
#[derive(Clone, Debug)]
pub struct Interaction {
    pub label: String,
    pub doc: Arc<Document>,
    pub selection: Selection,
    pub preview: Option<(String, Value)>,
    /// Per-document state restored on cancel (current layer, isolation).
    pub active_layer: Option<NodeId>,
    pub isolation: Option<NodeId>,
}

/// Per-document editing state.
#[derive(Clone, Debug)]
pub struct DocState {
    pub doc: Arc<Document>,
    pub selection: Selection,
    pub history: History,
    pub path: Option<String>,
    /// Increments on every change (also selection and view-only changes); UIs re-render when it
    /// moves. Not a dirty flag: see [`DocState::is_dirty`].
    pub revision: u64,
    /// The document as last saved (or opened). Structural sharing makes "unchanged" a pointer
    /// comparison, and undoing back to the saved state counts as clean.
    saved_doc: Arc<Document>,
    /// The layer new art goes into (the "current layer" in the Layers panel).
    pub active_layer: Option<NodeId>,
    /// Isolation mode container.
    pub isolation: Option<NodeId>,
    pub interaction: Option<Interaction>,
    /// For Object → Transform → Transform Again (⌘D).
    pub last_transform: Option<(Affine, bool)>,
    /// Selection saved by Select → Reselect.
    pub last_selection_cmd: Option<(String, Value)>,
    /// Process-unique id of this open document (tab indices shift when tabs close).
    pub uid: u64,
    /// View Opacity Mask (Alt-click the mask thumbnail): the masked object whose mask the canvas
    /// shows alone, in greyscale, while that mask is edited (see [`DocState::shown_mask`]).
    pub mask_view: Option<NodeId>,
    /// View → Show Transparency Grid, per document (view state: not saved, not undoable).
    pub transparency_grid: bool,
    /// The format Save writes ([`cmd::fileio::SAVE_FORMATS`]): the one the document was opened
    /// from or last saved as.
    pub format: &'static str,
    /// That format's options as last saved (SVG options for SVG, the Save PDF settings for PDF;
    /// empty for native files): Save reuses them and `file.formatOptions` reads them back.
    pub save_options: serde_json::Map<String, Value>,
}

static NEXT_DOC_UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl DocState {
    pub fn new(doc: Document, path: Option<String>) -> Self {
        let active_layer = doc.default_layer();
        let doc = Arc::new(doc);
        Self {
            saved_doc: doc.clone(),
            doc,
            selection: Selection::default(),
            history: History { limit: 500, ..Default::default() },
            path,
            revision: 1,
            active_layer,
            isolation: None,
            interaction: None,
            last_transform: None,
            last_selection_cmd: None,
            uid: NEXT_DOC_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            mask_view: None,
            transparency_grid: false,
            format: "vectorcraft",
            save_options: Default::default(),
        }
    }
    /// Unsaved changes: the document differs from the saved one (selection changes don't count).
    pub fn is_dirty(&self) -> bool {
        !Arc::ptr_eq(&self.doc, &self.saved_doc)
    }
    /// Record the current document as saved.
    pub fn mark_saved(&mut self) {
        self.saved_doc = self.doc.clone();
    }
    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .and_then(|p| std::path::Path::new(p).file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.doc.title.clone())
    }
    /// Where new art is inserted: the isolation container, else the active layer.
    /// The current layer, if the remembered id still names a layer (ids are reused after undo).
    pub fn current_layer(&self) -> Option<NodeId> {
        self.active_layer.filter(|l| self.doc.node(*l).is_some_and(|n| n.is_layer()))
    }
    pub fn insertion_parent(&self) -> Option<NodeId> {
        // Ids are reused after undo (the id counter is part of the document), so a remembered
        // layer or isolated group must still be one: a pasted group must never land in a path.
        if let Some(i) = self.isolation
            && self.doc.node(i).is_some_and(|n| matches!(n.kind, NodeKind::Group { .. } | NodeKind::Layer { .. }))
        {
            return Some(i);
        }
        self.active_layer.filter(|l| self.doc.node(*l).is_some_and(|n| n.is_layer() && !n.locked)).or_else(|| self.doc.default_layer())
    }
    /// The object whose opacity mask View Opacity Mask shows ([`DocState::mask_view`]): only while
    /// its mask is being edited, so leaving editing by any route (undo, deleting the object) ends it.
    pub fn shown_mask(&self) -> Option<NodeId> {
        self.mask_view.filter(|id| self.doc.mask_edit.is_some_and(|m| m.object == *id))
    }
}

/// Coordinates beyond this (points) are rejected: ~1,400 m, far past Illustrator's large canvas.
pub const MAX_COORD: f64 = 4.0e6;

/// Cheap sanity check after an edit: artboards and the objects just touched (the selection) must
/// have finite, in-range geometry, so saved files always reload and renderers never see NaN/∞.
fn doc_sane(d: &Document, sel: &Selection) -> bool {
    let ok = |r: vectorcraft_geom::Rect| [r.x0, r.y0, r.x1, r.y1].iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD);
    d.artboards.iter().all(|a| ok(a.rect)) && sel.objects.iter().all(|id| d.node(*id).and_then(|n| n.geometric_bounds()).is_none_or(ok))
}

/// Where new art goes (Illustrator's drawing modes, Shift+D cycles).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DrawMode {
    #[default]
    Normal,
    Behind,
    Inside,
}

/// Application preferences (Edit → Preferences). Every field is reachable through `prefs.get` /
/// `prefs.set {key, value}` (camelCase keys, validated against [`cmd::prefscmds::PREF_SPECS`]).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Prefs {
    // General
    /// Arrow-key nudge distance (General → Keyboard Increment), points.
    pub keyboard_increment: f64,
    pub constrain_angle: f64,
    pub corner_radius: f64,
    pub disable_auto_add_delete: bool,
    pub use_precise_cursors: bool,
    pub show_tool_tips: bool,
    pub anti_aliased_artwork: bool,
    pub select_same_tint_percent: bool,
    pub show_home_screen: bool,
    pub use_preview_bounds: bool,
    pub display_print_size: bool,
    pub double_click_to_isolate: bool,
    pub transform_pattern_tiles: bool,
    pub scale_corners: bool,
    /// Scale Strokes & Effects.
    pub scale_strokes: bool,
    pub zoom_with_mouse_wheel: bool,
    /// Offset for Paste / duplicate (Illustrator pastes to the view centre; we offset by this).
    pub paste_offset: f64,
    // Selection & Anchor Display
    pub selection_tolerance: f64,
    pub object_selection_by_path_only: bool,
    pub snap_to_point_tolerance: f64,
    pub ctrl_click_selects_behind: bool,
    pub zoom_to_selection: bool,
    pub move_locked_with_artboard: bool,
    pub anchor_size: u32,
    pub handle_style: String,
    pub highlight_anchors_on_hover: bool,
    pub show_handles_multiple_anchors: bool,
    pub hide_corner_widget_above: f64,
    pub pen_rubber_band: bool,
    pub curvature_rubber_band: bool,
    // Type
    pub type_size_increment: f64,
    pub tracking_increment: f64,
    pub baseline_shift_increment: f64,
    pub show_east_asian_options: bool,
    pub show_indic_options: bool,
    pub type_selection_by_path_only: bool,
    pub font_names_in_english: bool,
    pub auto_size_area_type: bool,
    pub font_preview: bool,
    pub font_preview_size: String,
    pub recent_fonts_count: u32,
    pub missing_glyph_protection: bool,
    pub highlight_alternate_glyphs: bool,
    pub placeholder_text: bool,
    // Units
    pub units_general: String,
    pub units_stroke: String,
    pub units_type: String,
    pub units_asian_type: String,
    pub numbers_without_units_are_points: bool,
    pub identify_objects_by: String,
    // Guides & Grid
    pub guide_color: String,
    pub guide_style: String,
    pub grid_color: String,
    pub grid_style: String,
    pub gridline_every: f64,
    pub grid_subdivisions: u32,
    pub grids_in_back: bool,
    pub show_pixel_grid: bool,
    // Smart Guides
    pub smart_guide_color: String,
    pub alignment_guides: bool,
    pub object_highlighting: bool,
    pub transform_tools_guides: bool,
    pub construction_guides: bool,
    pub construction_angles: String,
    pub anchor_path_labels: bool,
    pub measurement_labels: bool,
    pub spacing_guides: bool,
    pub snapping_tolerance: f64,
    // Slices
    pub show_slice_numbers: bool,
    pub slice_line_color: String,
    // Hyphenation
    pub hyphenation_language: String,
    pub hyphenation_exceptions: String,
    // Performance & Storage (Plug-ins & Scratch Disks)
    pub plugins_folder: String,
    pub scratch_primary: String,
    pub scratch_secondary: String,
    // User Interface
    pub ui_brightness: String,
    pub canvas_color: String,
    pub auto_collapse_icon_panels: bool,
    pub open_documents_as_tabs: bool,
    pub large_tabs: bool,
    pub ui_scaling: f64,
    pub scale_cursor_with_ui: bool,
    // Performance
    pub gpu_performance: bool,
    pub animated_zoom: bool,
    pub history_states: u32,
    pub real_time_drawing: bool,
    /// Rasterizer worker threads; -1 = automatic.
    pub render_threads: i32,
    // File Handling
    pub background_save: bool,
    pub background_export: bool,
    pub autosave_recovery: bool,
    pub autosave_interval: u32,
    pub recovery_folder: String,
    pub recovery_off_for_complex: bool,
    pub recent_files_count: u32,
    pub low_res_proxy_eps: bool,
    pub anti_aliased_bitmaps: bool,
    pub update_links: String,
    // Clipboard Handling
    pub copy_as_svg: bool,
    pub copy_as_pdf: bool,
    pub copy_aicb: bool,
    pub aicb_mode: String,
    pub paste_text_formatting: String,
    // Appearance of Black
    pub black_on_screen: String,
    pub black_output: String,
    // Devices
    pub touch_workspace: bool,
    pub touch_gestures: bool,
    // Graphic Styles panel
    /// Override Character Color: a graphic style applied to type replaces its characters' fill
    /// and stroke with the style's fills and strokes.
    pub override_char_color: bool,
    // Pattern editing
    /// Object → Pattern → Tile Edge Color (`#rrggbb`): the tile edge and swatch bounds in pattern
    /// editing mode.
    pub pattern_tile_edge_color: String,
    /// Eyedropper Options (`eyedropper.setOptions`; a preference group, see
    /// [`cmd::prefscmds::PREF_GROUPS`]).
    pub eyedropper: EyedropperOptions,
    /// Edit → Transparency Flattener Presets: the user's presets (the built-in ones aren't stored).
    /// Not a `prefs.set` key or group (resetting the preferences keeps them): `flattener.presets.*`
    /// edit it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub flattener_presets: Vec<cmd::FlattenerPreset>,
    /// Width profiles saved to the Stroke panel's Profile list (`stroke.widthProfile.*`). Not a
    /// Preferences dialog field, so it has no [`cmd::prefscmds::PREF_SPECS`] row.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub width_profiles: Vec<vectorcraft_doc::SavedProfile>,
    /// General → Use Japanese Crop Marks: the style of Create Trim Marks and Effect → Crop Marks.
    pub japanese_crop_marks: bool,
    /// Appearance panel → New Art Has Basic Appearance (on): new art takes one fill and stroke;
    /// off, the whole appearance of the last selection (`appearance.setNewArtBasic`).
    pub new_art_basic: bool,
    /// The Color Themes panel's saved themes (`colorTheme.*`): a local library, not a Preferences
    /// dialog field, so it has no [`cmd::prefscmds::PREF_SPECS`] row and resetting keeps it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub color_themes: Vec<cmd::colortheme::ColorTheme>,
    /// New Document → Saved: the user's document presets (`file.newPresets.save`). A local
    /// library, not a Preferences dialog field: resetting the preferences keeps it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub new_doc_presets: Vec<cmd::newdoc::DocSettings>,
    /// New Document → Recent: the settings of the last documents made (newest first).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recent_new_docs: Vec<cmd::newdoc::DocSettings>,
    /// Edit → PDF Presets: the user's presets (the built-in ones aren't stored). A local library,
    /// not a Preferences dialog field: resetting the preferences keeps it; `pdf.preset.*` edit it.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub pdf_presets: Vec<vectorcraft_pdf::PdfPreset>,
    // File Handling (continued)
    /// Where Save as Template and New from Template start ("" = `Documents/VectorCraft Templates`).
    pub templates_folder: String,
}

impl Default for Prefs {
    fn default() -> Self {
        let s = |v: &str| v.to_string();
        Self {
            keyboard_increment: 1.0,
            constrain_angle: 0.0,
            corner_radius: 12.0,
            disable_auto_add_delete: false,
            use_precise_cursors: false,
            show_tool_tips: true,
            anti_aliased_artwork: true,
            select_same_tint_percent: false,
            show_home_screen: true,
            use_preview_bounds: false,
            display_print_size: false,
            double_click_to_isolate: true,
            transform_pattern_tiles: false,
            scale_corners: false,
            scale_strokes: false,
            zoom_with_mouse_wheel: false,
            paste_offset: 10.0,
            selection_tolerance: 3.0,
            object_selection_by_path_only: false,
            snap_to_point_tolerance: 2.0,
            ctrl_click_selects_behind: true,
            zoom_to_selection: true,
            move_locked_with_artboard: false,
            anchor_size: 3,
            handle_style: s("solid"),
            highlight_anchors_on_hover: true,
            show_handles_multiple_anchors: true,
            hide_corner_widget_above: 177.0,
            pen_rubber_band: true,
            curvature_rubber_band: true,
            type_size_increment: 2.0,
            tracking_increment: 20.0,
            baseline_shift_increment: 2.0,
            show_east_asian_options: false,
            show_indic_options: false,
            type_selection_by_path_only: false,
            font_names_in_english: true,
            auto_size_area_type: false,
            font_preview: true,
            font_preview_size: s("medium"),
            recent_fonts_count: 10,
            missing_glyph_protection: true,
            highlight_alternate_glyphs: true,
            placeholder_text: true,
            units_general: s("points"),
            units_stroke: s("points"),
            units_type: s("points"),
            units_asian_type: s("points"),
            numbers_without_units_are_points: true,
            identify_objects_by: s("objectName"),
            guide_color: s("#4affff"),
            guide_style: s("lines"),
            grid_color: s("#c8c8c8"),
            grid_style: s("lines"),
            gridline_every: 72.0,
            grid_subdivisions: 8,
            grids_in_back: true,
            show_pixel_grid: true,
            smart_guide_color: s("#ff4af0"),
            alignment_guides: true,
            object_highlighting: true,
            transform_tools_guides: true,
            construction_guides: true,
            construction_angles: s("90° & 45° Angles"),
            anchor_path_labels: true,
            measurement_labels: true,
            spacing_guides: true,
            snapping_tolerance: 4.0,
            show_slice_numbers: true,
            slice_line_color: s("#ff3f3f"),
            hyphenation_language: s("English: USA"),
            hyphenation_exceptions: String::new(),
            plugins_folder: String::new(),
            scratch_primary: s("Startup"),
            scratch_secondary: s("None"),
            ui_brightness: s("mediumDark"),
            canvas_color: s("matchUi"),
            auto_collapse_icon_panels: false,
            open_documents_as_tabs: true,
            large_tabs: false,
            ui_scaling: 1.0,
            scale_cursor_with_ui: false,
            gpu_performance: true,
            animated_zoom: true,
            history_states: 500,
            real_time_drawing: true,
            render_threads: -1,
            background_save: true,
            background_export: true,
            autosave_recovery: true,
            autosave_interval: 2,
            recovery_folder: String::new(),
            recovery_off_for_complex: false,
            recent_files_count: 20,
            low_res_proxy_eps: false,
            anti_aliased_bitmaps: false,
            update_links: s("askWhenModified"),
            copy_as_svg: true,
            copy_as_pdf: false,
            copy_aicb: false,
            aicb_mode: s("preserveAppearance"),
            paste_text_formatting: s("keep"),
            black_on_screen: s("accurate"),
            black_output: s("accurate"),
            touch_workspace: true,
            touch_gestures: true,
            override_char_color: true,
            pattern_tile_edge_color: {
                let [r, g, b] = vectorcraft_doc::LAYER_COLORS[0].1;
                Color::rgb8(r, g, b).to_hex()
            },
            eyedropper: Default::default(),
            flattener_presets: vec![],
            width_profiles: vec![],
            japanese_crop_marks: false,
            new_art_basic: true,
            color_themes: vec![],
            new_doc_presets: vec![],
            recent_new_docs: vec![],
            pdf_presets: vec![],
            templates_folder: String::new(),
        }
    }
}

pub struct Session {
    docs: Vec<DocState>,
    active: Option<usize>,
    pub prefs: Prefs,
    /// Fill/stroke for new art (the toolbar proxy).
    pub paint: PaintDefaults,
    /// Which proxy is in front (true = Fill, false = Stroke) — the X key toggles.
    pub fill_active: bool,
    /// Internal clipboard: the copied objects and the document resources they use.
    pub clipboard: Clipboard,
    /// Executed commands (for actions and debugging).
    pub journal: Vec<(String, Value)>,
    pub(crate) tool: Box<dyn Tool>,
    pub(crate) last_view: ViewInfo,
    depth: u32,
    /// Set when the active tool panicked (see [`guard`]); reported by the next tool event.
    tool_panic: Option<EngineError>,
    /// Draw Normal / Behind / Inside (toolbar drawing modes).
    pub draw_mode: DrawMode,
    /// The path new art is drawn inside (Draw Inside).
    pub draw_inside: Option<NodeId>,
    untitled_counter: u32,
    /// Session-level state of the menu commands (saved selections, guide lock).
    pub(crate) menu: cmd::menucmds::MenuState,
    /// The selected gradient stop (`gradient.selectStop`) and whose gradient it was selected on,
    /// shared by the Gradient tool's annotator, the Gradient and Color panels and agents: read it
    /// with [`Session::selected_stop`].
    pub(crate) gradient_stop: Option<(usize, cmd::gradient::StopOwner)>,
    /// The Appearance panel's active fill/stroke row (`appearance.setActiveItem`); not saved. Read
    /// it through [`Session::appearance_item`], which drops it once the selection changes.
    pub(crate) active_appearance_item: Option<cmd::appearance::ActiveItem>,
    /// The last solid colour applied (the toolbar's Color button; `,` applies it again).
    pub last_solid: Color,
    /// The last gradient applied (the toolbar's Gradient button; `.` applies it again).
    pub last_gradient: GradientPaint,
    /// Recently applied solid colours, newest first (the Recent Colors rows), fed by every paint
    /// command whichever frontend runs it.
    pub recent_colors: Vec<Color>,
    /// A paint applied by a live preview: remembered when the interaction commits.
    pub(crate) pending_paint: Option<Paint>,
    /// User Defined and loaded swatch libraries (Window → Swatch Libraries); not saved.
    pub swatch_libraries: cmd::swatchlib::Libraries,
    /// The selected freeform gradient point (`paint.freeform.selectPoint`) and whose gradient it
    /// was selected on: read it with [`Session::selected_freeform_point`].
    pub(crate) freeform_point: Option<(usize, cmd::gradient::StopOwner)>,
    /// User Defined and loaded graphic style libraries (Window → Graphic Style Libraries); not saved.
    pub style_libraries: cmd::stylelib::Libraries,
    /// URLs recently given in the Attributes panel (`attributes.set {url}`), newest first; not saved.
    pub recent_urls: Vec<String>,
    /// Parameters the running top-level command resolved from the preferences, added to its
    /// journal entry so a replay does the same ([`Session::note_journal`]).
    journal_note: serde_json::Map<String, Value>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

impl Session {
    pub fn new() -> Self {
        Self {
            docs: vec![],
            active: None,
            prefs: Prefs::default(),
            paint: PaintDefaults::default(),
            fill_active: true,
            clipboard: Clipboard::default(),
            journal: vec![],
            tool: vectorcraft_tools::create("selection"),
            last_view: ViewInfo::default(),
            depth: 0,
            tool_panic: None,
            draw_mode: DrawMode::Normal,
            draw_inside: None,
            untitled_counter: 0,
            menu: Default::default(),
            gradient_stop: None,
            active_appearance_item: None,
            last_solid: Color::WHITE,
            last_gradient: GradientPaint::new(Default::default()),
            recent_colors: vec![],
            pending_paint: None,
            swatch_libraries: Default::default(),
            freeform_point: None,
            style_libraries: Default::default(),
            recent_urls: vec![],
            journal_note: Default::default(),
        }
    }

    pub fn documents(&self) -> &[DocState] {
        &self.docs
    }
    pub fn active_index(&self) -> Option<usize> {
        self.active
    }
    pub fn active(&self) -> Option<&DocState> {
        self.active.and_then(|i| self.docs.get(i))
    }
    pub fn active_mut(&mut self) -> Option<&mut DocState> {
        self.active.and_then(|i| self.docs.get_mut(i))
    }
    pub fn doc(&self) -> Result<&DocState> {
        self.active().ok_or(EngineError::NoDocument)
    }
    pub fn doc_mut(&mut self) -> Result<&mut DocState> {
        self.active_mut().ok_or(EngineError::NoDocument)
    }
    /// Finish the tool's pending work in the current document and give it fresh state, so nothing
    /// (e.g. uncommitted typing) leaks into the document we are switching to.
    fn reset_tool_for_doc_switch(&mut self) {
        if self.active.is_none() {
            return;
        }
        let view = self.last_view;
        let acts = self.with_tool_cx(view, |t, cx| t.deactivate(cx));
        let _ = self.apply_actions(acts);
        let _ = self.cancel_interaction();
        self.tool = vectorcraft_tools::create(self.tool.id());
    }
    pub fn set_active(&mut self, index: usize) -> bool {
        if index < self.docs.len() {
            if self.active != Some(index) {
                self.reset_tool_for_doc_switch();
            }
            self.active = Some(index);
            true
        } else {
            false
        }
    }
    /// The name [`Session::next_untitled`] gives next (New Document's Name field).
    pub fn peek_untitled(&self) -> String {
        format!("Untitled-{}", self.untitled_counter + 1)
    }
    pub fn next_untitled(&mut self) -> String {
        self.untitled_counter += 1;
        format!("Untitled-{}", self.untitled_counter)
    }
    /// Text layout bounds are a cache (not saved): compute them for a document just read, or
    /// selection boxes and hit testing would use the rough estimate until each text is edited.
    fn refresh_text_bounds(doc: &mut Document) {
        let mut texts = vec![];
        doc.walk(|n| {
            if matches!(n.kind, NodeKind::Text(_)) {
                texts.push(n.id);
            }
        });
        for id in texts {
            if let Some(NodeKind::Text(t)) = doc.node_mut(id).map(|n| &mut n.kind)
                && t.cached_bounds.is_none()
            {
                cmd::typecmd::refresh_bounds(t);
            }
        }
    }
    /// Add a document and make it active.
    pub fn add_document(&mut self, mut doc: Document, path: Option<String>) -> usize {
        Self::refresh_text_bounds(&mut doc);
        self.reset_tool_for_doc_switch();
        let mut st = DocState::new(doc, path);
        st.history.limit = self.prefs.history_states as usize;
        self.docs.push(st);
        let i = self.docs.len() - 1;
        self.active = Some(i);
        i
    }
    /// Replace the document in tab `index` (File → Revert): new content, cleared history and
    /// selection, saved state. The tab keeps its place, path and format.
    pub fn replace_document(&mut self, index: usize, mut doc: Document) -> bool {
        if index >= self.docs.len() {
            return false;
        }
        Self::refresh_text_bounds(&mut doc);
        if self.active == Some(index) {
            // Pending tool work (typing, a drag) belongs to the content being thrown away.
            self.reset_tool_for_doc_switch();
        }
        let old = &self.docs[index];
        let mut st = DocState::new(doc, old.path.clone());
        st.history.limit = old.history.limit;
        // Same open document (caches keyed by uid stay valid); a new revision redraws it.
        st.uid = old.uid;
        st.revision = old.revision + 1;
        st.format = old.format;
        st.save_options = old.save_options.clone();
        self.docs[index] = st;
        true
    }
    pub fn close_document(&mut self, index: usize) -> bool {
        if index >= self.docs.len() {
            return false;
        }
        self.reset_tool_for_doc_switch();
        self.docs.remove(index);
        self.active = if self.docs.is_empty() { None } else { Some(index.min(self.docs.len() - 1)) };
        true
    }

    /// Execute a command by id. This is THE entry point for every frontend.
    pub fn execute(&mut self, id: &str, params: &Value) -> Result<Value> {
        let spec = find_command(id).ok_or_else(|| EngineError::UnknownCommand(id.to_string()))?;
        if let Err(why) = (spec.enabled)(self) {
            return Err(EngineError::Disabled(id.to_string(), why));
        }
        // Only top-level commands are journaled (commands that call other commands would otherwise
        // be recorded twice and replay differently).
        if self.depth == 0 {
            self.journal_note.clear();
        }
        let r = if self.depth == 0 { self.run_guarded(id, |s| (spec.run)(s, params)) } else { self.run_nested(|s| (spec.run)(s, params)) };
        let r = r?;
        if self.depth == 0 {
            self.inherit_new_art();
        }
        if spec.journal && self.depth == 0 && self.active().is_none_or(|d| d.interaction.is_none()) {
            let p = self.noted(params);
            self.journal.push((id.to_string(), p));
        }
        Ok(r)
    }

    /// Record `key: value` in the running top-level command's journal entry (or its interaction's
    /// preview) unless its params give `key`: a value it resolved from the preferences.
    pub fn note_journal(&mut self, key: &str, value: Value) {
        if self.depth == 1 {
            self.journal_note.insert(key.to_string(), value);
        }
    }

    /// `params` with the noted values added (see [`Session::note_journal`]).
    fn noted(&mut self, params: &Value) -> Value {
        let mut note = std::mem::take(&mut self.journal_note);
        match params {
            // The params' own values win.
            Value::Object(m) if !note.is_empty() => {
                note.extend(m.clone());
                Value::Object(note)
            }
            Value::Null if !note.is_empty() => Value::Object(note),
            _ => params.clone(),
        }
    }

    fn run_nested(&mut self, f: impl FnOnce(&mut Self) -> Result<Value>) -> Result<Value> {
        self.depth += 1;
        let r = f(self);
        self.depth -= 1;
        r
    }

    /// A top-level command: a panic (a bug) becomes [`EngineError::Internal`] and the active
    /// document goes back to how it was before the command, instead of crashing the frontend.
    fn run_guarded(&mut self, id: &str, f: impl FnOnce(&mut Self) -> Result<Value>) -> Result<Value> {
        let snapshot = self.active().map(|d| (d.uid, d.doc.clone(), d.selection.clone(), d.interaction.clone()));
        let active = self.active;
        match guard::catch_panic(|| self.run_nested(f)) {
            Ok(r) => r,
            Err(msg) => {
                self.depth = 0;
                self.journal_note.clear();
                if let Some((uid, doc, selection, interaction)) = snapshot
                    && let Some(st) = self.docs.iter_mut().find(|d| d.uid == uid)
                {
                    st.doc = doc;
                    st.selection = selection;
                    st.interaction = interaction;
                    st.revision += 1;
                }
                if active.is_some_and(|i| i < self.docs.len()) {
                    self.active = active;
                }
                Err(EngineError::Internal { cmd: id.to_string(), msg })
            }
        }
    }

    /// Run `f` on a mutable copy of the active document. Outside an interaction this records one
    /// undo step labelled `label`; inside one it just mutates (the interaction commits later).
    pub fn edit<T>(&mut self, label: &str, f: impl FnOnce(&mut Document, &mut Selection) -> Result<T>) -> Result<T> {
        let st = self.doc_mut()?;
        let before = st.doc.clone();
        let before_sel = st.selection.clone();
        let doc = Arc::make_mut(&mut st.doc);
        let result = match f(doc, &mut st.selection) {
            Ok(v) => {
                // Text Wrap: area type follows its wrap objects; then threads re-flow.
                cmd::textwrap::refresh(Arc::make_mut(&mut st.doc));
                // Opacity-mask editing: the mask follows its art on the editing layer.
                if st.doc.mask_edit.is_some() {
                    cmd::maskedit::sync(Arc::make_mut(&mut st.doc));
                }
                // Threaded text re-flows when any of its frames changed.
                if !st.doc.text_threads.is_empty() {
                    cmd::threads::reflow(&before, Arc::make_mut(&mut st.doc));
                }
                if doc_sane(&st.doc, &st.selection) {
                    Ok(v)
                } else {
                    Err(EngineError::Other("result would exceed the canvas (coordinates out of range)".into()))
                }
            }
            Err(e) => Err(e),
        };
        match result {
            Ok(v) => {
                st.selection.prune(&st.doc);
                st.revision += 1;
                if st.interaction.is_none() {
                    st.history.undo.push(HistoryEntry { label: label.to_string(), doc: before, selection: before_sel });
                    if st.history.undo.len() > st.history.limit {
                        st.history.undo.remove(0);
                    }
                    st.history.redo.clear();
                }
                Ok(v)
            }
            Err(e) => {
                st.doc = before;
                st.selection = before_sel;
                Err(e)
            }
        }
    }

    /// Change only the selection (not an undo step).
    pub fn select(&mut self, f: impl FnOnce(&Document, &mut Selection)) -> Result<()> {
        self.active_appearance_item = None;
        let st = self.doc_mut()?;
        f(&st.doc, &mut st.selection);
        st.selection.prune(&st.doc);
        st.revision += 1;
        Ok(())
    }

    // ---------- interactions (live drags) ----------

    pub fn begin_interaction(&mut self, label: &str) -> Result<()> {
        let st = self.doc_mut()?;
        if st.interaction.is_some() {
            return Ok(());
        }
        st.interaction = Some(Interaction {
            label: label.to_string(),
            doc: st.doc.clone(),
            selection: st.selection.clone(),
            preview: None,
            active_layer: st.active_layer,
            isolation: st.isolation,
        });
        Ok(())
    }

    /// Re-apply `cmd` on top of the interaction snapshot (replacing the previous preview).
    pub fn preview(&mut self, cmd: &str, params: &Value) -> Result<Value> {
        {
            let st = self.doc_mut()?;
            let Some(it) = &st.interaction else { return Err(EngineError::Other("no interaction in progress".into())) };
            st.doc = it.doc.clone();
            st.selection = it.selection.clone();
        }
        let rw = cmd::distortcmds::perspective_rewrite(self, cmd, params);
        let (cmd, params) = rw.as_ref().map_or((cmd, params), |(c, p)| (c.as_str(), p));
        let r = self.execute(cmd, params);
        let params = self.noted(params);
        let st = self.doc_mut()?;
        if let Some(it) = &mut st.interaction {
            it.preview = Some((cmd.to_string(), params));
        }
        r
    }

    pub fn commit_interaction(&mut self) -> Result<()> {
        if let Some(p) = self.pending_paint.take() {
            self.remember_paint_now(&p);
        }
        let st = self.doc_mut()?;
        let Some(it) = st.interaction.take() else { return Ok(()) };
        let Some(preview) = it.preview else { return Ok(()) };
        if !Arc::ptr_eq(&it.doc, &st.doc) {
            st.history.undo.push(HistoryEntry { label: it.label, doc: it.doc, selection: it.selection });
            st.history.redo.clear();
            st.revision += 1;
        }
        if preview.0 == "object.transform" {
            let m = cmd::matrix_param(&preview.1, "matrix");
            let copy = preview.1.get("copy").and_then(Value::as_bool).unwrap_or(false);
            if let Some(m) = m {
                st.last_transform = Some((m, copy));
            }
        }
        self.journal.push(preview);
        Ok(())
    }

    pub fn cancel_interaction(&mut self) -> Result<()> {
        self.pending_paint = None;
        let st = self.doc_mut()?;
        if let Some(it) = st.interaction.take() {
            st.doc = it.doc;
            st.selection = it.selection;
            st.active_layer = it.active_layer;
            st.isolation = it.isolation;
            st.revision += 1;
        }
        Ok(())
    }

    pub fn in_interaction(&self) -> bool {
        self.active().is_some_and(|d| d.interaction.is_some())
    }

    /// Commands with enablement (for menus, palette, MCP `list_commands`).
    pub fn commands(&self) -> Vec<CommandInfo> {
        command_specs().iter().map(|c| c.info(self)).collect()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_appearance;
#[cfg(test)]
mod tests_attributes;
#[cfg(test)]
mod tests_brushsym;
#[cfg(test)]
mod tests_build;
#[cfg(test)]
mod tests_charstroke;
#[cfg(test)]
mod tests_clip;
#[cfg(test)]
mod tests_clipboard;
#[cfg(test)]
mod tests_clipflavours;
#[cfg(test)]
mod tests_clippaint;
#[cfg(test)]
mod tests_cmdsplit;
#[cfg(test)]
mod tests_cmykflatten;
#[cfg(test)]
mod tests_colorguide;
#[cfg(test)]
mod tests_colorguidelib;
#[cfg(test)]
mod tests_colormgmt;
#[cfg(test)]
mod tests_colorthemes;
#[cfg(test)]
mod tests_containers;
#[cfg(test)]
mod tests_dashalign;
#[cfg(test)]
mod tests_distort;
#[cfg(test)]
mod tests_docsetup;
#[cfg(test)]
mod tests_draw2;
#[cfg(test)]
mod tests_editcolors;
#[cfg(test)]
mod tests_effectedit;
#[cfg(test)]
mod tests_expand;
#[cfg(test)]
mod tests_eyedropper;
#[cfg(test)]
mod tests_file;
#[cfg(test)]
mod tests_fileinfo;
#[cfg(test)]
mod tests_flatpresets;
#[cfg(test)]
mod tests_flatpreview;
#[cfg(test)]
mod tests_flatten;
#[cfg(test)]
mod tests_focal;
#[cfg(test)]
mod tests_freeform;
#[cfg(test)]
mod tests_gradient;
#[cfg(test)]
mod tests_gradpanel;
#[cfg(test)]
mod tests_knockout;
#[cfg(test)]
mod tests_labspots;
#[cfg(test)]
mod tests_layerclip;
#[cfg(test)]
mod tests_linked_stops;
#[cfg(test)]
mod tests_links;
#[cfg(test)]
mod tests_live;
#[cfg(test)]
mod tests_maskview;
#[cfg(test)]
mod tests_menucmds;
#[cfg(test)]
mod tests_newart;
#[cfg(test)]
mod tests_newdoc;
#[cfg(test)]
mod tests_objexpand;
#[cfg(test)]
mod tests_opacitymask;
#[cfg(test)]
mod tests_outlinestroke;
#[cfg(test)]
mod tests_overprint;
#[cfg(test)]
mod tests_paintproxy;
#[cfg(test)]
mod tests_panelcmds;
#[cfg(test)]
mod tests_pathops;
#[cfg(test)]
mod tests_pattern;
#[cfg(test)]
mod tests_pdfpresets;
#[cfg(test)]
mod tests_place;
#[cfg(test)]
mod tests_prefs;
#[cfg(test)]
mod tests_previewbounds;
#[cfg(test)]
mod tests_proxyitems;
#[cfg(test)]
mod tests_rastersettings;
#[cfg(test)]
mod tests_recolor;
#[cfg(test)]
mod tests_registration;
#[cfg(test)]
mod tests_save;
#[cfg(test)]
mod tests_scalestrokes;
#[cfg(test)]
mod tests_strokegeom;
#[cfg(test)]
mod tests_strokegradient;
#[cfg(test)]
mod tests_strokereach;
#[cfg(test)]
mod tests_strokeux;
#[cfg(test)]
mod tests_stylelib;
#[cfg(test)]
mod tests_stylepanel;
#[cfg(test)]
mod tests_styles;
#[cfg(test)]
mod tests_swatchcmds;
#[cfg(test)]
mod tests_swatches;
#[cfg(test)]
mod tests_swatchlib;
#[cfg(test)]
mod tests_targeting;
#[cfg(test)]
mod tests_textedit;
#[cfg(test)]
mod tests_textimport;
#[cfg(test)]
mod tests_tileedge;
#[cfg(test)]
mod tests_tints;
#[cfg(test)]
mod tests_transparencygrid;
#[cfg(test)]
mod tests_units;
#[cfg(test)]
mod tests_widthpoints;
#[cfg(test)]
mod tests_widthprofiles;
#[cfg(test)]
mod tests_xform;
