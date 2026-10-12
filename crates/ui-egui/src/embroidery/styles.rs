//! Embroidery Styles panel: named sets of stitch properties.
//!
//! Click a style to apply it to the selected objects. The style the selection follows is
//! highlighted, with a "+" when the object's properties differ from it.

use egui::{Sense, Ui, vec2};
use serde_json::json;
use vectorcraft_embroidery::sf_core::attr::Embroidery;

use crate::VectorcraftApp;
use crate::panels::{pstate, selection_len, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};

pub const ID: &str = "embroideryStyles";

const SEL_KEY: &str = "embroidery-style-sel";

/// (name, stitch type, uses) for every style of the document.
fn styles(app: &mut VectorcraftApp) -> Vec<(String, String, u64)> {
    let v = app.session.execute("embroidery.style.list", &json!({})).unwrap_or_default();
    v["styles"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| {
                    (s["name"].as_str().unwrap_or("").to_string(), s["type"].as_str().unwrap_or("").to_string(), s["uses"].as_u64().unwrap_or(0))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The first selected stitched object's embroidery and whether its properties differ from
/// its style's.
fn selected(app: &VectorcraftApp) -> Option<(Embroidery, bool)> {
    let st = app.session.active()?;
    let d = &st.doc;
    let e = st.selection.objects.iter().find_map(|id| first_stitched(d, *id))?;
    let changed = e.style.as_deref().is_some_and(|name| {
        let cfg = d.embroidery.as_deref();
        cfg.and_then(|c| c.style(name)).is_some_and(|s| s.properties != e.properties())
    });
    Some((e, changed))
}

fn first_stitched(d: &vectorcraft_doc::Document, id: vectorcraft_doc::NodeId) -> Option<Embroidery> {
    let n = d.node(id)?;
    if vectorcraft_embroidery::stitchable(n) {
        return n.embroidery.as_deref().cloned();
    }
    n.children()?.iter().find_map(|c| first_stitched(d, c.id))
}

/// A stitch type id as its menu name.
fn type_label(ty: &str) -> &'static str {
    match ty {
        "run" => tl!("Run"),
        "bean" => tl!("Bean Run"),
        "backstitch" => tl!("Backstitch"),
        "stemstitch" => tl!("Stem Stitch"),
        "blanket" => tl!("Blanket"),
        "satin" => tl!("Satin"),
        "fill" => tl!("Tatami Fill"),
        "satinfill" => tl!("Satin Fill"),
        "contour" => tl!("Contour Fill"),
        "spiral" => tl!("Spiral Fill"),
        "radial" => tl!("Radial Fill"),
        "cross" => tl!("Cross Fill"),
        "meander" => tl!("Meander Fill"),
        "motiffill" => tl!("Motif Fill"),
        "applique" => tl!("Appliqué"),
        _ => "",
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if app.session.active().is_none() {
        crate::panels::empty_state(ui, "waves", tl!("No document"), tl!("Open or create a document to digitize embroidery."));
        return;
    }
    let all = styles(app);
    let cur = selected(app);
    let cur_name = cur.as_ref().and_then(|(e, _)| e.style.clone());
    let mut sel: Option<String> = pstate(ui.ctx(), SEL_KEY);
    let mut clicked: Option<String> = None;
    if all.is_empty() {
        ui.label(
            egui::RichText::new(tl!("No styles yet. Select a stitched object and create one to reuse its stitch properties.")).color(t.text_dim),
        );
    }
    widgets::list_box(ui, |ui| {
        for (name, ty, uses) in &all {
            let is_cur = cur_name.as_deref() == Some(name.as_str());
            let changed = is_cur && cur.as_ref().is_some_and(|(_, c)| *c);
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::click());
            if is_cur || sel.as_deref() == Some(name.as_str()) {
                ui.painter().rect_filled(r, 0.0, if is_cur { t.row_selected } else { t.hover });
            } else if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, t.hover);
            }
            let label = if changed { format!("{name}+") } else { name.clone() };
            ui.painter().text(r.left_center() + vec2(8.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), t.text);
            let detail = format!("{} · {uses}", type_label(ty));
            ui.painter().text(r.right_center() - vec2(8.0, 0.0), egui::Align2::RIGHT_CENTER, detail, egui::FontId::proportional(11.0), t.text_dim);
            if resp.clicked() {
                clicked = Some(name.clone());
            }
        }
    });
    if let Some(name) = clicked {
        sel = Some(name.clone());
        if cur.is_some() {
            app.run("embroidery.style.apply", json!({ "name": name })).ok();
        }
    }
    set_pstate(ui.ctx(), SEL_KEY, sel.clone());
    widgets::bottom_bar(ui, |ui| {
        ui.add_space((ui.available_width() - 2.0 * 28.0).max(0.0));
        if widgets::icon_button_enabled(ui, "dc-new-item", tl!("New Embroidery Style"), false, cur.is_some(), 24.0).clicked()
            && let Ok(r) = app.run("embroidery.style.new", json!({}))
        {
            set_pstate(ui.ctx(), SEL_KEY, r["name"].as_str().map(str::to_string));
        }
        if widgets::icon_button_enabled(ui, "trash-2", tl!("Delete Embroidery Style"), false, sel.is_some(), 24.0).clicked()
            && let Some(n) = &sel
            && app.run("embroidery.style.delete", json!({ "name": n })).is_ok()
        {
            set_pstate::<Option<String>>(ui.ctx(), SEL_KEY, None);
        }
    });
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let sel: Option<String> = pstate(ui.ctx(), SEL_KEY);
    let stitched = selected(app).is_some();
    if menu_item(ui, tl!("New Embroidery Style"), stitched, false) {
        app.run("embroidery.style.new", json!({})).ok();
    }
    if menu_item(ui, tl!("Duplicate Embroidery Style"), sel.is_some(), false)
        && let Some(n) = &sel
    {
        app.run("embroidery.style.duplicate", json!({ "name": n })).ok();
    }
    if menu_item(ui, tl!("Delete Embroidery Style"), sel.is_some(), false)
        && let Some(n) = &sel
    {
        app.run("embroidery.style.delete", json!({ "name": n })).ok();
    }
    ui.separator();
    if menu_item(ui, tl!("Redefine Embroidery Style"), sel.is_some() && stitched, false)
        && let Some(n) = &sel
    {
        app.run("embroidery.style.redefine", json!({ "name": n })).ok();
    }
    if menu_item(ui, tl!("Break Link to Embroidery Style"), stitched && selection_len(app) > 0, false) {
        app.run("embroidery.style.unlink", json!({})).ok();
    }
    if menu_item(ui, tl!("Rename Embroidery Style…"), sel.is_some(), false)
        && let Some(n) = &sel
    {
        app.run(
            "ui.paramDialog",
            json!({ "command": "embroidery.style.rename", "label": "Rename Embroidery Style", "params": { "name": n, "to": n } }),
        )
        .ok();
    }
}

/// The style the selection follows (shown in the Embroidery panel).
pub fn style_of_selection(app: &VectorcraftApp) -> Option<(String, bool)> {
    let (e, changed) = selected(app)?;
    Some((e.style?, changed))
}
