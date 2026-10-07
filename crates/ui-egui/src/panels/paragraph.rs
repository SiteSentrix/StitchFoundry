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

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some((_, para)) = text_style(app) else {
        super::empty_state(ui, "pilcrow", tl!("No text selected"), tl!("Select a text object to edit its paragraph attributes."));
        return;
    };
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        for (j, icon, tip, id) in ALIGNMENTS {
            if widgets::icon_button(ui, icon, tip, para.justify == j, 28.0).clicked() {
                para_cmd(app, "text.setStyle", json!({"justify": id}));
            }
        }
    });
    ui.add_space(4.0);
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
    if !pstate::<bool>(ui.ctx(), "pa-hide-options") && widgets::check(ui, tl!("Hyphenate"), para.hyphenate, true) {
        format(app, json!({"hyphenate": !para.hyphenate}));
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
        para_cmd(app, "text.setStyle", json!({"justify": "left"}));
        format(app, json!({"leftIndent": 0, "rightIndent": 0, "firstLineIndent": 0, "spaceBefore": 0, "spaceAfter": 0, "hyphenate": false}));
    }
}
