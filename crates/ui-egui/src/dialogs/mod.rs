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
mod command;
mod document_setup;
mod effect;
mod export_for_screens;
mod form;
mod gradient_stop;
mod new_document;
mod path_ops;
mod recolor;
mod save_changes;
mod shapes;
mod tools;
mod transform;

use serde_json::{Value, json};

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
}

impl DialogSpec {
    /// A text field per value; OK just closes. Also the fallback for unregistered kinds.
    pub const FORM: Self = Self {
        window: None,
        heading: |_| "Dialog".into(),
        body: |_, ui, d| {
            form::grid(ui, d);
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
    NewDocument: ["newDocument"] => new_document::SPEC,
    Shape: ["rectangle", "roundedRectangle", "ellipse", "polygon", "star", "lineSegment"] => shapes::SPEC,
    Transform: ["move", "rotate", "scale", "reflect", "shear"] => transform::SPEC,
    PathOp: ["average", "offsetPath", "simplify", "splitIntoGrid"] => path_ops::SPEC,
    DocumentSetup: ["documentSetup"] => document_setup::SPEC,
    ArtboardOptions: ["artboardOptions"] => artboard_options::SPEC,
    AllTools: ["allTools"] => all_tools::SPEC,
    ExportForScreens: ["exportForScreens"] => export_for_screens::SPEC,
    Recolor: ["recolor"] => recolor::SPEC,
    Command: ["command"] => command::SPEC,
    Effect: ["effect"] => effect::SPEC,
    SaveChanges: [crate::unsaved::KIND] => save_changes::SPEC,
    Preferences: ["preferences"] => DialogSpec::window(crate::prefs_dialog::show, |app, _| crate::prefs_dialog::confirm(app)),
    Shortcuts: ["shortcuts"] => DialogSpec::window(crate::shortcut_editor::show, |app, _| crate::shortcut_editor::confirm(app)),
    Workspaces: ["newWorkspace", "manageWorkspaces"] => DialogSpec::window(crate::workspaces::show, |app, _| crate::workspaces::confirm(app)),
    FindFont: ["findFont"] => DialogSpec::window(crate::find_font::show, |app, _| crate::find_font::confirm(app)),
    GradientStop: ["gradientStop"] => gradient_stop::SPEC,
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

/// Apply the open dialog (OK).
pub fn confirm(app: &mut VectorcraftApp) -> DialogResult {
    let Some(d) = app.ui.dialog.clone() else { return Err("no dialog open".into()) };
    (spec(&d.kind).confirm)(app, &d)
}

pub fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    about::show(app, ctx);
    let Some(mut d) = app.ui.dialog.clone() else { return };
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
        .id(egui::Id::new("dialog"))
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
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(label) = spec.ok
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
        if spec.preview {
            let _ = app.session.cancel_interaction();
        }
        app.ui.dialog = None;
    } else if ok && let Err(e) = confirm(app) {
        app.status(e);
    }
}

#[cfg(test)]
mod tests;
