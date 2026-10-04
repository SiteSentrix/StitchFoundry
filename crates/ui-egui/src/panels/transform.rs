//! Transform panel: reference point, X/Y/W/H with the constrain link, rotate and shear, flips,
//! live-shape properties and the Scale Corners / Scale Strokes & Effects options.

use egui::Ui;
use serde_json::json;
use vectorcraft_doc::NodeKind;

use super::{first_selected, pstate, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};
use crate::{VectorcraftApp, icons};

pub const ANGLE_PRESETS: [f64; 9] = [-180.0, -135.0, -90.0, -45.0, 0.0, 45.0, 90.0, 135.0, 180.0];

/// Proportional size: the other dimension when one changes with the link on.
pub fn constrained(w: f64, h: f64, new_w: Option<f64>, new_h: Option<f64>) -> (f64, f64) {
    match (new_w, new_h) {
        (Some(nw), _) if w.abs() > 1e-9 => (nw, h * nw / w),
        (_, Some(nh)) if h.abs() > 1e-9 => (w * nh / h, nh),
        (a, b) => (a.unwrap_or(w), b.unwrap_or(h)),
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(st) = app.session.active() else {
        widgets::dim_label(ui, "No document");
        return;
    };
    let units = st.doc.units;
    let bounds = st.doc.bounds_of(&st.selection.objects, false);
    let refi: usize = ui.data(|d| d.get_temp(egui::Id::new("refpt"))).unwrap_or(4);
    let link: bool = pstate(ui.ctx(), "xf-link");
    let has = bounds.is_some();
    let rp = bounds.map(|b| vectorcraft_geom::reference_point(b, refi));
    ui.horizontal(|ui| {
        if let Some(i) = widgets::reference_point(ui, refi) {
            ui.data_mut(|d| d.insert_temp(egui::Id::new("refpt"), i));
        }
        ui.add_space(4.0);
        ui.add_enabled_ui(has, |ui| {
            egui::Grid::new("xfp-grid").num_columns(4).spacing([4.0, 6.0]).min_col_width(0.0).show(ui, |ui| {
                widgets::dim_label(ui, "X:");
                if let Some(v) = widgets::num_field(ui, "xfp-x", rp.map(|p| p.x), units, 74.0) {
                    app.run("object.setBounds", json!({"x": v, "reference": refi})).ok();
                }
                widgets::dim_label(ui, "W:");
                if let Some(v) = widgets::num_field(ui, "xfp-w", bounds.map(|b| b.width()), units, 74.0) {
                    app.run("object.setBounds", json!({"width": v, "reference": refi, "proportional": link})).ok();
                }
                ui.end_row();
                widgets::dim_label(ui, "Y:");
                if let Some(v) = widgets::num_field(ui, "xfp-y", rp.map(|p| p.y), units, 74.0) {
                    app.run("object.setBounds", json!({"y": v, "reference": refi})).ok();
                }
                widgets::dim_label(ui, "H:");
                if let Some(v) = widgets::num_field(ui, "xfp-h", bounds.map(|b| b.height()), units, 74.0) {
                    app.run("object.setBounds", json!({"height": v, "reference": refi, "proportional": link})).ok();
                }
                ui.end_row();
            });
        });
        if widgets::icon_button(ui, if link { "link" } else { "link-2-off" }, "Constrain Width and Height Proportions", link, 22.0).clicked() {
            set_pstate(ui.ctx(), "xf-link", !link);
        }
    });
    ui.add_space(4.0);
    let origin = rp.map(|p| json!([p.x, p.y]));
    ui.horizontal(|ui| {
        ui.add_enabled_ui(has, |ui| {
            icons::icon(ui, "rotate-ccw", 16.0, t.icon).on_hover_text("Rotate");
            // Rotation is applied relative to the current orientation (0° shown after each apply).
            if let Some(a) = widgets::spin_plain(ui, "xfp-rot", 0.0, "°", 2, 96.0, 15.0, -360.0, &ANGLE_PRESETS)
                && a != 0.0
            {
                app.run("object.rotate", json!({"angle": a, "origin": origin})).ok();
            }
            ui.add_space(4.0);
            icons::icon(ui, "dc-shear", 16.0, t.icon).on_hover_text("Shear");
            if let Some(a) = widgets::plain_field(ui, "xfp-shear", 0.0, "°", 1, 56.0)
                && a != 0.0
            {
                app.run("object.shear", json!({"angle": a, "axis": "horizontal", "origin": origin})).ok();
            }
        });
    });
    ui.horizontal(|ui| {
        if widgets::icon_button_enabled(ui, "flip-horizontal-2", "Flip Horizontal", false, has, 24.0).clicked() {
            app.run("object.reflect", json!({"axis": "vertical", "origin": origin})).ok();
        }
        if widgets::icon_button_enabled(ui, "flip-vertical-2", "Flip Vertical", false, has, 24.0).clicked() {
            app.run("object.reflect", json!({"axis": "horizontal", "origin": origin})).ok();
        }
    });
    // Live shape properties.
    if let Some(n) = first_selected(app)
        && let NodeKind::Path { live: Some(live), .. } = &n.kind
    {
        widgets::divider(ui);
        match live {
            vectorcraft_doc::LiveShape::Rectangle { radii, .. } => {
                widgets::subheader(ui, "Rectangle Properties:");
                ui.horizontal(|ui| {
                    widgets::dim_label(ui, "Corner Radius:");
                    if let Some(r) = widgets::num_field(ui, "xfp-radius", Some(radii[0]), units, 80.0) {
                        app.run("object.setLiveShape", json!({"radius": r})).ok();
                    }
                });
            }
            vectorcraft_doc::LiveShape::Polygon { sides, .. } => {
                widgets::subheader(ui, "Polygon Properties:");
                ui.horizontal(|ui| {
                    widgets::dim_label(ui, "Sides:");
                    if let Some(s) = widgets::plain_field(ui, "xfp-sides", *sides as f64, "", 0, 60.0) {
                        app.run("object.setLiveShape", json!({"sides": s.clamp(3.0, 20.0) as u32})).ok();
                    }
                });
            }
            _ => {
                widgets::subheader(ui, "Shape Properties:");
                widgets::dim_label(ui, n.kind_label());
            }
        }
    }
    if pstate::<bool>(ui.ctx(), "xf-hide-options") {
        return;
    }
    widgets::divider(ui);
    let (sc, ss) = (app.session.prefs.scale_corners, app.session.prefs.scale_strokes);
    if widgets::check(ui, "Scale Corners", sc, true) {
        set_pref(app, "scaleCorners", !sc);
    }
    if widgets::check(ui, "Scale Strokes & Effects", ss, true) {
        set_pref(app, "scaleStrokes", !ss);
    }
}

/// Toggle a boolean preference (through `prefs.set`, as agents do).
pub fn set_pref(app: &mut VectorcraftApp, key: &str, on: bool) {
    if let Err(e) = app.run("prefs.set", json!({ "key": key, "value": on })) {
        app.status(e);
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let hidden: bool = pstate(ui.ctx(), "xf-hide-options");
    let has = super::selection_len(app) > 0;
    if menu_item(ui, if hidden { "Show Options" } else { "Hide Options" }, true, false) {
        set_pstate(ui.ctx(), "xf-hide-options", !hidden);
    }
    ui.separator();
    if menu_item(ui, "Flip Horizontal", has, false) {
        app.run("object.reflect", json!({"axis": "vertical"})).ok();
    }
    if menu_item(ui, "Flip Vertical", has, false) {
        app.run("object.reflect", json!({"axis": "horizontal"})).ok();
    }
    ui.separator();
    let ss = app.session.prefs.scale_strokes;
    if menu_item(ui, "Scale Strokes & Effects", true, ss) {
        set_pref(app, "scaleStrokes", !ss);
    }
    ui.separator();
    menu_item(ui, "Transform Object Only", false, true);
    menu_item(ui, "Transform Pattern Only", false, false);
    menu_item(ui, "Transform Both", false, false);
    ui.separator();
    menu_item(ui, "Use Registration Point for Symbol", false, false);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constrain_proportions() {
        assert_eq!(constrained(100.0, 50.0, Some(200.0), None), (200.0, 100.0));
        assert_eq!(constrained(100.0, 50.0, None, Some(25.0)), (50.0, 25.0));
        assert_eq!(constrained(0.0, 50.0, Some(10.0), None), (10.0, 50.0));
    }
}
