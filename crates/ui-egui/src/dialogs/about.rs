//! Help → About VectorCraft.

use crate::VectorcraftApp;
use crate::theme;

/// The About window, while `UiState::about` is set.
pub(super) fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !app.ui.about {
        return;
    }
    let mut open = true;
    egui::Window::new(tl!("About VectorCraft"))
        .collapsible(false)
        .resizable(false)
        .open(&mut open)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .frame(egui::Frame::window(&ctx.global_style()).inner_margin(egui::Margin::same(18)))
        .show(ctx, |ui| {
            ui.set_width(380.0);
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                crate::brand::paint_mark(ui, r);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("VectorCraft").font(theme::semibold(22.0)));
                    ui.label(crate::i18n::fmt(
                        tl!("Version {version} — open-source vector illustration in pure Rust."),
                        &[("version", env!("CARGO_PKG_VERSION"))],
                    ));
                });
            });
            ui.add_space(12.0);
            crate::community::links(app, ui);
            ui.add_space(12.0);
            ui.label(
                egui::RichText::new(tl!(
                    "Part of ArtCraft. MIT OR Apache-2.0. Fonts: Source Sans 3, Inter, JetBrains Mono (OFL). Icons: Lucide (ISC) + VectorCraft."
                ))
                .size(11.0),
            );
            // The GPU the window renders with, for bug reports (Preferences › Performance picks it).
            if let Some(adapter) = &app.graphics_adapter {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(crate::i18n::fmt(tl!("Graphics: {adapter}"), &[("adapter", adapter.as_str())])).size(11.0));
            }
        });
    app.ui.about = open;
}
