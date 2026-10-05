//! Modal dialogs. Fields live in `UiState::dialog` (string/number JSON) so agents can fill them
//! through `ui.dialog.set` and press OK with `ui.dialog.confirm`.
//!
//! Each dialog is a file here exporting a [`DialogSpec`] (heading, body, OK action, buttons) plus
//! a row in the `registry!` below that maps its `Dialog::kind` strings to it. Dialogs inside the
//! shared modal frame only supply a body and a confirm action; a few (Preferences, Keyboard
//! Shortcuts, Workspaces, Find Font) draw their own window.

mod about;
mod all_tools;
mod artboard_options;
pub mod color_balance;
pub mod color_guide_options;
mod color_picker;
mod command;
pub mod confirm;
mod document_setup;
pub mod dxf_import;
pub mod dxf_options;
mod effect;
pub mod eps_options;
pub mod expand;
mod export_as;
mod export_for_screens;
pub mod eyedropper;
pub mod file_info;
pub mod flatten;
pub mod flattener_presets;
mod form;
mod gradient_stop;
pub mod graphic_style_options;
pub mod import_pdf;
pub mod missing_links;
pub mod new_color_group;
mod new_document;
pub mod new_swatch;
pub mod office_export;
pub mod package;
mod path_ops;
pub mod pdf_presets;
pub mod place;
pub mod placement_options;
mod png_options;
pub mod raster_effects;
pub mod recolor;
mod recovery;
pub mod saturate;
mod save_changes;
pub(crate) mod save_options;
mod save_pdf;
pub mod save_style_library;
pub mod save_swatch_library;
mod shapes;
pub mod slices;
pub mod spot_colors;
pub(crate) mod svg_options;
pub mod swatch_conflict;
pub mod swatch_options;
mod text_export;
pub mod text_import;
mod tiff_bmp_tga;
pub mod tile_edge_color;
mod tools;
mod transform;
pub mod transform_each;
pub mod width_point;

use serde_json::{Value, json};

pub use color_picker::open as open_color_picker;
pub use document_setup::open as open_document_setup;
pub use effect::open as open_effect_dialog;
pub use export_as::open as open_export_as;
pub use export_for_screens::open as open_export_for_screens;
pub use new_document::{open as open_new_document, preset_card};
pub use png_options::open as open_raster_options;
pub use save_pdf::{open as open_save_pdf, open_preset as open_pdf_preset};
pub use tools::open_tool_dialog;

use crate::state::Dialog;
use crate::theme::{self, Tokens};
use crate::{VectorcraftApp, widgets};

type DialogResult = Result<Value, String>;

/// How a dialog draws and applies itself. Specs start from [`DialogSpec::FORM`] and override what
/// differs.
pub(crate) struct DialogSpec {
    /// Draws its own window instead of the shared frame (the frame fields below are then unused).
    pub window: Option<fn(&mut VectorcraftApp, &egui::Context)>,
    /// The heading (and window title).
    pub heading: fn(&Dialog) -> String,
    /// Draws the fields. Returns true to close the dialog as Cancel would.
    pub body: fn(&mut VectorcraftApp, &mut egui::Ui, &mut Dialog) -> bool,
    /// OK, Enter or `ui.dialog.confirm`: applies the dialog and closes it when done.
    pub confirm: fn(&mut VectorcraftApp, &Dialog) -> DialogResult,
    /// The OK button's label. None: no OK button, Enter does nothing and Cancel reads "Close".
    pub ok: Option<&'static str>,
    /// An extra button left of Cancel that sets `discard: true` and confirms ("Don't Save").
    pub discard: Option<&'static str>,
    pub min_width: f32,
    pub max_width: Option<f32>,
    /// The body runs a live preview interaction that Cancel rolls back.
    pub preview: bool,
    /// The OK button's label when it depends on the app (Export for Screens' "Download" on the
    /// web); `ok` when none. Only with an `ok`.
    pub ok_label: Option<fn(&VectorcraftApp) -> &'static str>,
}

impl DialogSpec {
    /// A text field per value; OK just closes. Also the fallback for unregistered kinds.
    pub const FORM: Self = Self {
        window: None,
        heading: |_| "Dialog".into(),
        body: |app, ui, d| {
            form::grid(ui, d, app.session.general_unit());
            false
        },
        confirm: |app, _| {
            app.ui.dialog = None;
            Ok(Value::Null)
        },
        ok: Some("OK"),
        discard: None,
        min_width: 320.0,
        max_width: None,
        preview: false,
        ok_label: None,
    };

    /// A dialog that draws its own window and confirms through its module.
    const fn window(show: fn(&mut VectorcraftApp, &egui::Context), confirm: fn(&mut VectorcraftApp, &Dialog) -> DialogResult) -> Self {
        Self { window: Some(show), confirm, ..Self::FORM }
    }
}

/// The dialog registry: one row per dialog with its [`DialogKind`] variant, the `Dialog::kind`
/// strings it handles and its [`DialogSpec`]. Adding a dialog is a new file plus one row.
macro_rules! registry {
    ($($variant:ident: [$($kind:pat),+] => $spec:expr,)+) => {
        /// Every registered dialog.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum DialogKind {
            $($variant,)+
        }

        impl DialogKind {
            /// The dialog handling a `Dialog::kind` string.
            pub fn of(kind: &str) -> Option<Self> {
                match kind {
                    $($($kind)|+ => Some(Self::$variant),)+
                    _ => None,
                }
            }

            fn spec(self) -> &'static DialogSpec {
                match self {
                    $(Self::$variant => {
                        static SPEC: DialogSpec = $spec;
                        &SPEC
                    })+
                }
            }
        }
    };
}

registry! {
    NewDocument: [new_document::KIND] => new_document::SPEC,
    Shape: ["rectangle", "roundedRectangle", "ellipse", "polygon", "star", "lineSegment"] => shapes::SPEC,
    Transform: ["move", "rotate", "scale", "reflect", "shear"] => transform::SPEC,
    PathOp: ["average", "offsetPath", "simplify", "splitIntoGrid"] => path_ops::SPEC,
    DocumentSetup: ["documentSetup"] => document_setup::SPEC,
    ArtboardOptions: ["artboardOptions"] => artboard_options::SPEC,
    AllTools: ["allTools"] => all_tools::SPEC,
    ExportForScreens: ["exportForScreens"] => export_for_screens::SPEC,
    Recolor: [recolor::KIND] => recolor::SPEC,
    Command: ["command"] => command::SPEC,
    Effect: ["effect"] => effect::SPEC,
    SaveChanges: [crate::unsaved::KIND] => save_changes::SPEC,
    Preferences: ["preferences"] => DialogSpec::window(crate::prefs_dialog::show, |app, _| crate::prefs_dialog::confirm(app)),
    Shortcuts: ["shortcuts"] => DialogSpec::window(crate::shortcut_editor::show, |app, _| crate::shortcut_editor::confirm(app)),
    Workspaces: ["newWorkspace", "manageWorkspaces"] => DialogSpec::window(crate::workspaces::show, |app, _| crate::workspaces::confirm(app)),
    FindFont: ["findFont"] => DialogSpec::window(crate::find_font::show, |app, _| crate::find_font::confirm(app)),
    SwatchOptions: [swatch_options::KIND] => swatch_options::SPEC,
    Confirm: [confirm::KIND] => confirm::SPEC,
    NewSwatch: [new_swatch::KIND] => new_swatch::SPEC,
    NewColorGroup: [new_color_group::KIND] => new_color_group::SPEC,
    GradientStop: ["gradientStop"] => gradient_stop::SPEC,
    ColorPicker: [color_picker::KIND] => color_picker::SPEC,
    GraphicStyleOptions: [graphic_style_options::KIND] => graphic_style_options::SPEC,
    EffectExists: [effect::EXISTS] => effect::EXISTS_SPEC,
    ColorGuideOptions: [color_guide_options::KIND] => color_guide_options::SPEC,
    ColorBalance: [color_balance::KIND] => color_balance::SPEC,
    Saturate: [saturate::KIND] => saturate::SPEC,
    SaveSwatchLibrary: [save_swatch_library::KIND] => save_swatch_library::SPEC,
    TileEdgeColor: [tile_edge_color::KIND] => tile_edge_color::SPEC,
    EyedropperOptions: [eyedropper::KIND] => eyedropper::SPEC,
    FlattenTransparency: [flatten::KIND] => flatten::SPEC,
    FlattenerPresets: [flattener_presets::KIND] => flattener_presets::SPEC,
    SaveStyleLibrary: [save_style_library::KIND] => save_style_library::SPEC,
    Expand: [expand::KIND] => expand::SPEC,
    SpotColors: [spot_colors::KIND] => spot_colors::SPEC,
    TransformEach: [transform_each::KIND] => transform_each::SPEC,
    WidthPoint: [width_point::KIND] => width_point::SPEC,
    SavePdf: [save_pdf::KIND] => save_pdf::SPEC,
    SvgOptions: [svg_options::KIND] => svg_options::SPEC,
    NewDocumentMore: [new_document::MORE] => new_document::MORE_SPEC,
    Place: [place::KIND] => place::SPEC,
    RasterOptions: ["pngOptions", "jpgOptions", "webpOptions", "gifOptions", "png8Options"] => png_options::SPEC,
    ExportAs: ["exportAs"] => export_as::SPEC,
    ImportPdf: [import_pdf::KIND] => import_pdf::SPEC,
    SwatchConflict: [swatch_conflict::KIND] => swatch_conflict::SPEC,
    FileInfo: [file_info::KIND] => file_info::SPEC,
    RasterEffectsSettings: [raster_effects::KIND] => raster_effects::SPEC,
    MissingLinks: [missing_links::KIND] => missing_links::SPEC,
    TextImport: [text_import::KIND] => text_import::SPEC,
    PdfPresets: [pdf_presets::KIND] => pdf_presets::SPEC,
    PdfPreset: [save_pdf::PRESET_KIND] => save_pdf::PRESET_SPEC,
    SaveOptions: [save_options::KIND] => save_options::SPEC,
    TextExport: [text_export::KIND] => text_export::SPEC,
    OfficeExport: [office_export::KIND] => office_export::SPEC,
    DxfOptions: [dxf_options::KIND] => dxf_options::SPEC,
    PlacementOptions: [placement_options::KIND] => placement_options::SPEC,
    Package: [package::KIND] => package::SPEC,
    SliceOptions: [slices::OPTIONS] => slices::OPTIONS_SPEC,
    DivideSlices: [slices::DIVIDE] => slices::DIVIDE_SPEC,
    EpsOptions: [eps_options::KIND] => eps_options::SPEC,
    Recovery: [crate::recovery::KIND] => recovery::SPEC,
    DxfImport: [dxf_import::KIND] => dxf_import::SPEC,
    RasterFormatOptions: ["tiffOptions"] => png_options::SPEC,
}

/// The spec for a `Dialog::kind` ([`DialogSpec::FORM`] when unregistered).
fn spec(kind: &str) -> &'static DialogSpec {
    static FALLBACK: DialogSpec = DialogSpec::FORM;
    DialogKind::of(kind).map_or(&FALLBACK, DialogKind::spec)
}

/// Run a command and close the dialog (whether or not the command succeeded).
fn run_and_close(app: &mut VectorcraftApp, id: &str, params: Value) -> DialogResult {
    let r = app.run(id, params);
    app.ui.dialog = None;
    r
}

/// Close the open dialog as Cancel does, rolling back a live preview (`ui.dialog.cancel`).
pub fn cancel(app: &mut VectorcraftApp) {
    if app.ui.dialog.take().is_some_and(|d| spec(&d.kind).preview) {
        let _ = app.session.cancel_interaction();
    }
}

/// Apply the open dialog (OK).
pub fn confirm(app: &mut VectorcraftApp) -> DialogResult {
    let Some(d) = app.ui.dialog.clone() else { return Err("no dialog open".into()) };
    (spec(&d.kind).confirm)(app, &d)
}

pub fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    about::show(app, ctx);
    let Some(mut d) = app.ui.dialog.clone() else {
        app.ui.dialog_file = None;
        return;
    };
    let spec = spec(&d.kind);
    if let Some(window) = spec.window {
        return window(app, ctx);
    }
    let t = Tokens::get(ctx);
    let mut ok = false;
    let mut cancel = false;
    let mut discard = false;
    egui::Area::new(egui::Id::new("modal-dim")).order(egui::Order::Middle).fixed_pos(egui::pos2(0.0, 0.0)).show(ctx, |ui| {
        // Modal, but the canvas isn't dimmed so previews stay readable (as in the reference app).
        ui.allocate_rect(ctx.content_rect(), egui::Sense::click());
    });
    let heading = (spec.heading)(&d);
    egui::Window::new(heading.as_str())
        // One window per kind, so a dialog never inherits another dialog's size.
        .id(egui::Id::new(("dialog", d.kind.as_str())))
        .order(egui::Order::Foreground)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, -40.0])
        .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(22)))
        .show(ctx, |ui| {
            ui.set_min_width(spec.min_width);
            if let Some(w) = spec.max_width {
                ui.set_max_width(w);
            }
            ui.label(egui::RichText::new(heading.as_str()).font(theme::semibold(16.0)).color(t.text));
            ui.add_space(12.0);
            cancel = (spec.body)(app, ui, &mut d);
            ui.add_space(16.0);
            // The button row is as tall as the buttons: a right-to-left layout would otherwise take
            // all the height left in the window, so the window could never shrink to its content.
            let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
            ui.allocate_ui_with_layout(row, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(label) = spec.ok.map(|ok| spec.ok_label.map_or(ok, |f| f(app)))
                    && widgets::primary_button(ui, label).clicked()
                {
                    ok = true;
                }
                ui.add_space(8.0);
                if widgets::secondary_button(ui, if spec.ok.is_some() { "Cancel" } else { "Close" }).clicked() {
                    cancel = true;
                }
                if let Some(label) = spec.discard {
                    ui.add_space(28.0);
                    discard = widgets::secondary_button(ui, label).clicked();
                }
            });
        });
    if spec.ok.is_some() && ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
        ok = true;
    }
    if discard {
        d.fields.insert("discard".into(), json!(true));
        ok = true;
    }
    app.ui.dialog = Some(d);
    if cancel {
        self::cancel(app);
    } else if ok && let Err(e) = confirm(app) {
        app.status(e);
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_export;
#[cfg(test)]
mod tests_import_pdf;
#[cfg(test)]
mod tests_package;
#[cfg(test)]
mod tests_raster_formats;

#[cfg(test)]
mod tests_dxf;
#[cfg(test)]
mod tests_screens;

#[cfg(test)]
mod tests_eps;

#[cfg(test)]
mod tests_metafile;

#[cfg(test)]
mod tests_tiff_bmp_tga;
