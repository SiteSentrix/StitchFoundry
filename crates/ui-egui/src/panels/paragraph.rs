//! Paragraph panel: seven alignment buttons, indents, space before/after and Hyphenate.

use egui::{Ui, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::Justify;

use super::character::text_style;
use super::{pstate, set_pstate};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};

pub const ALIGNMENTS: [(Justify, &str, &str, &str); 7] = [
    (Justify::Left, "dc-para-left", "Align left", "left"),
    (Justify::Center, "dc-para-center", "Align center", "center"),
    (Justify::Right, "dc-para-right", "Align right", "right"),
    (Justify::JustifyLeft, "dc-para-justify-left", "Justify with last line aligned left", "justifyLeft"),
    (Justify::JustifyCenter, "dc-para-justify-center", "Justify with last line aligned center", "justifyCenter"),
    (Justify::JustifyRight, "dc-para-justify-right", "Justify with last line aligned right", "justifyRight"),
    (Justify::JustifyAll, "dc-para-justify-all", "Justify all lines", "justifyAll"),
];

/// Paragraph attributes apply to the whole text object (ending a Type tool typing session first).
fn format(app: &mut VectorcraftApp, p: Value) {
    para_cmd(app, "text.setFormat", p);
}

fn para_cmd(app: &mut VectorcraftApp, cmd: &str, mut p: Value) {
    if let Some((id, _, _)) = super::character::text_editing(app) {
        super::character::end_typing(app);
        p["ids"] = json!([id.0]);
    }
    app.run(cmd, p).ok();
}

fn effective_alignment(app: &VectorcraftApp, alignment: Justify) -> Justify {
    if alignment != Justify::Auto {
        return alignment;
    }
    let editing = super::character::text_editing(app);
    let selected = super::first_selected(app);
    let node = editing.and_then(|(id, _, _)| app.session.active().and_then(|d| d.doc.node(id))).or(selected.as_ref());
    let Some(vectorcraft_doc::NodeKind::Text(t)) = node.map(|n| &n.kind) else { return Justify::Left };
    let plain = t.plain_text();
    let paragraph = vectorcraft_text::edit::paragraph_at(&plain, editing.map_or(0, |(_, a, _)| a));
    vectorcraft_text::automatic_alignment(plain.get(paragraph).unwrap_or_default())
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some((_, para)) = text_style(app) else {
        super::empty_state(ui, "pilcrow", tl!("No text selected"), tl!("Select a text object to edit its paragraph attributes."));
        return;
    };
    let resolved = effective_alignment(app, para.justify);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (j, icon, tip, id) in ALIGNMENTS {
            if widgets::icon_button(ui, icon, tip, resolved == j, 28.0).clicked() {
                para_cmd(app, "text.setStyle", json!({"justify": id}));
            }
        }
    });
    ui.add_space(4.0);
    if widgets::check(ui, tl!("Automatic alignment"), para.justify == Justify::Auto, true) {
        para_cmd(
            app,
            "text.setStyle",
            json!({"justify": if para.justify != Justify::Auto { "auto" } else if resolved == Justify::Right { "right" } else { "left" }}),
        );
    }
    let fw = ((ui.available_width() - 66.0) / 2.0).clamp(60.0, 100.0);
    // Indents and paragraph spacing are distances (General); type sizes follow Units ▸ Type.
    let unit = app.session.general_unit();
    let label = |ui: &mut Ui, s: &str, tip: &str| {
        ui.add_sized(vec2(22.0, 24.0), egui::Label::new(egui::RichText::new(s).size(11.5).strong().color(t.text))).on_hover_text(tip);
    };
    egui::Grid::new("para-grid").num_columns(4).spacing([4.0, 4.0]).show(ui, |ui| {
        label(ui, "→|", tl!("Left Indent"));
        if let Some(v) = widgets::spin_field(ui, "pa-li", Some(para.left_indent), unit, fw, 1.0, -1296.0, &[]) {
            format(app, json!({"leftIndent": v}));
        }
        label(ui, "|←", tl!("Right Indent"));
        if let Some(v) = widgets::spin_field(ui, "pa-ri", Some(para.right_indent), unit, fw, 1.0, -1296.0, &[]) {
            format(app, json!({"rightIndent": v}));
        }
        ui.end_row();
        label(ui, "1→", tl!("First-line Left Indent"));
        if let Some(v) = widgets::spin_field(ui, "pa-fi", Some(para.first_line_indent), unit, fw, 1.0, -1296.0, &[]) {
            format(app, json!({"firstLineIndent": v}));
        }
        ui.label("");
        ui.label("");
        ui.end_row();
        label(ui, "↑¶", tl!("Space Before Paragraph"));
        if let Some(v) = widgets::spin_field(ui, "pa-sb", Some(para.space_before), unit, fw, 1.0, 0.0, &[]) {
            format(app, json!({"spaceBefore": v}));
        }
        label(ui, "↓¶", tl!("Space After Paragraph"));
        if let Some(v) = widgets::spin_field(ui, "pa-sa", Some(para.space_after), unit, fw, 1.0, 0.0, &[]) {
            format(app, json!({"spaceAfter": v}));
        }
        ui.end_row();
    });
    ui.add_space(4.0);
    if pstate::<bool>(ui.ctx(), "pa-hide-options") {
        return;
    }
    if widgets::check(ui, tl!("Hyphenate"), para.hyphenate, true) {
        format(app, json!({"hyphenate": !para.hyphenate}));
    }
    // Japanese composition: how punctuation is spaced (JLREQ 3.1), with the East Asian options.
    if app.session.prefs.show_east_asian_options {
        ui.horizontal(|ui| {
            widgets::dim_label(ui, tl!("Mojikumi Set"));
            let sets = [tl!("None"), tl!("Line-end Punctuation Half Width")];
            let current = if para.mojikumi == vectorcraft_doc::Mojikumi::LineEndHalf { sets[1] } else { sets[0] };
            // Already translated: shown as they are.
            if let Some(i) = widgets::dropdown_names(ui, "pa-mojikumi", current, &sets, ui.available_width() - 4.0) {
                format(app, json!({"mojikumi": if i == 1 { "lineEndHalf" } else { "none" }}));
            }
        });
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let has = text_style(app).is_some();
    let hidden: bool = pstate(ui.ctx(), "pa-hide-options");
    if menu_item(ui, if hidden { tl!("Show Options") } else { tl!("Hide Options") }, true, false) {
        set_pstate(ui.ctx(), "pa-hide-options", !hidden);
    }
    ui.separator();
    for l in [tl!("Roman Hanging Punctuation"), tl!("Justification…"), tl!("Hyphenation…")] {
        menu_item(ui, l, false, false);
    }
    ui.separator();
    menu_item(ui, tl!("Single-line Composer"), false, false);
    menu_item(ui, tl!("Every-line Composer"), false, true);
    ui.separator();
    if menu_item(ui, tl!("Reset Panel"), has, false) {
        para_cmd(app, "text.setStyle", json!({"justify": "auto"}));
        format(app, json!({"leftIndent": 0, "rightIndent": 0, "firstLineIndent": 0, "spaceBefore": 0, "spaceAfter": 0, "hyphenate": false}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_engine::Session;

    #[test]
    fn panel_shows_automatic_alignment_and_resolves_the_selected_text() {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.session.execute("file.new", &json!({"width": 400, "height": 200})).unwrap();
        app.session.execute("text.create", &json!({"x": 200, "y": 50, "text": "שלום"})).unwrap();
        assert_eq!(effective_alignment(&app, Justify::Auto), Justify::Right);
        crate::i18n::set_current(crate::i18n::Lang::EN);
        let ctx = egui::Context::default();
        let mut frame = ctx.run_ui(egui::RawInput::default(), |ui| show(&mut app, ui));
        // This headless frame inspects shapes without uploading textures to a renderer.
        frame.textures_delta.clear();
        fn has_label(shape: &egui::Shape) -> bool {
            match shape {
                egui::Shape::Text(t) => t.galley.text() == "Automatic alignment",
                egui::Shape::Vec(shapes) => shapes.iter().any(has_label),
                _ => false,
            }
        }
        assert!(frame.shapes.iter().any(|shape| has_label(&shape.shape)));
        app.session.execute("text.setStyle", &json!({"justify": "left"})).unwrap();
        assert_eq!(effective_alignment(&app, text_style(&app).unwrap().1.justify), Justify::Left);
        app.session.execute("text.setStyle", &json!({"justify": "auto"})).unwrap();
        assert_eq!(effective_alignment(&app, text_style(&app).unwrap().1.justify), Justify::Right);
    }
}
