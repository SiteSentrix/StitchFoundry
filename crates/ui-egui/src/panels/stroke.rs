//! Stroke panel: weight spinner + presets, cap / corner / align toggles, miter limit, dashed line
//! with three dash/gap pairs, arrowheads with drawn previews, arrow scale, width profiles.

use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};
use vectorcraft_doc::{Arrowhead, LineCap, LineJoin, StrokeAlign, StrokeLayer, Unit, WidthProfile};

use super::{first_selected, pstate, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, menu_item};
use crate::{VectorcraftApp, icons};

pub const WEIGHT_PRESETS: [f64; 22] =
    [0.25, 0.5, 0.75, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 70.0, 80.0, 90.0, 100.0];

/// Width profile presets: (id for `stroke.set`, label).
pub const PROFILES: [(&str, &str); 4] = [("uniform", "Uniform"), ("lens", "Lens"), ("taperStart", "Taper Start"), ("taperEnd", "Taper End")];

/// Which preset a stroke's profile is (None profile = uniform).
pub fn profile_id(p: Option<&WidthProfile>) -> &'static str {
    let Some(p) = p else { return "uniform" };
    for (id, prof) in [("lens", WidthProfile::lens()), ("taperStart", WidthProfile::taper_start()), ("taperEnd", WidthProfile::taper_end())] {
        if prof.points == p.points {
            return id;
        }
    }
    "custom"
}

fn profile_of(id: &str) -> Option<WidthProfile> {
    match id {
        "lens" => Some(WidthProfile::lens()),
        "taperStart" => Some(WidthProfile::taper_start()),
        "taperEnd" => Some(WidthProfile::taper_end()),
        _ => None,
    }
}

/// Dash pattern → the panel's six dash/gap fields (None = empty field).
pub fn dash_fields(pattern: &[f64]) -> [Option<f64>; 6] {
    let mut out = [None; 6];
    for (i, v) in pattern.iter().take(6).enumerate() {
        out[i] = Some(*v);
    }
    out
}

/// Six dash/gap fields → a dash pattern: stops at the first empty dash; a dash without a gap
/// repeats its length as the gap (Illustrator's behaviour).
pub fn dash_pattern(fields: &[Option<f64>; 6]) -> Vec<f64> {
    let mut out = vec![];
    for pair in 0..3 {
        let Some(d) = fields[pair * 2] else { break };
        let g = fields[pair * 2 + 1].unwrap_or(d);
        out.push(d.max(0.0));
        out.push(g.max(0.0));
    }
    if out.iter().all(|v| *v == 0.0) {
        out.clear();
    }
    out
}

fn arrow_label(a: Option<Arrowhead>) -> String {
    match a {
        None => "None".into(),
        Some(a) => {
            let s = format!("{a:?}");
            s.replace("Open", " (open)")
        }
    }
}

fn stroke_now(app: &VectorcraftApp) -> Option<StrokeLayer> {
    first_selected(app).and_then(|n| n.appearance.stroke().cloned())
}

fn set(app: &mut VectorcraftApp, p: Value) {
    app.run("stroke.set", p).ok();
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let st = stroke_now(app);
    let weight = st.as_ref().map(|s| s.width).unwrap_or(app.session.paint.stroke_width);
    let hidden: bool = pstate(ui.ctx(), "stroke-hide-options");
    let label_w = 64.0;
    let row_label = |ui: &mut Ui, s: &str| {
        ui.add_sized(vec2(label_w, 24.0), egui::Label::new(egui::RichText::new(s).size(12.5).color(t.text)).halign(egui::Align::RIGHT));
    };
    ui.horizontal(|ui| {
        row_label(ui, "Weight:");
        if let Some(w) = widgets::spin_field(ui, "stroke-weight", Some(weight), Unit::Points, 120.0, 1.0, 0.0, &WEIGHT_PRESETS) {
            set(app, json!({"weight": w}));
        }
    });
    if hidden {
        return;
    }
    let cap = st.as_ref().map(|s| s.cap).unwrap_or_default();
    let join = st.as_ref().map(|s| s.join).unwrap_or_default();
    let align = st.as_ref().map(|s| s.align).unwrap_or_default();
    ui.horizontal(|ui| {
        row_label(ui, "Cap:");
        for (v, icon, tip, name) in [
            (LineCap::Butt, "dc-cap-butt", "Butt Cap", "butt"),
            (LineCap::Round, "dc-cap-round", "Round Cap", "round"),
            (LineCap::Square, "dc-cap-square", "Projecting Cap", "square"),
        ] {
            if widgets::icon_button(ui, icon, tip, cap == v, 24.0).clicked() {
                set(app, json!({"cap": name}));
            }
        }
    });
    ui.horizontal(|ui| {
        row_label(ui, "Corner:");
        for (v, icon, tip, name) in [
            (LineJoin::Miter, "dc-join-miter", "Miter Join", "miter"),
            (LineJoin::Round, "dc-join-round", "Round Join", "round"),
            (LineJoin::Bevel, "dc-join-bevel", "Bevel Join", "bevel"),
        ] {
            if widgets::icon_button(ui, icon, tip, join == v, 24.0).clicked() {
                set(app, json!({"join": name}));
            }
        }
        widgets::dim_label(ui, "Limit:");
        let lim = st.as_ref().map(|s| s.miter_limit).unwrap_or(10.0);
        if join == LineJoin::Miter {
            if let Some(v) = widgets::plain_field(ui, "stroke-limit", lim, " x", 0, 50.0) {
                set(app, json!({"miterLimit": v.clamp(1.0, 500.0)}));
            }
        } else {
            ui.label(egui::RichText::new(format!("{lim:.0} x")).color(t.text_disabled));
        }
    });
    ui.horizontal(|ui| {
        row_label(ui, "Align Stroke:");
        for (v, icon, tip, name) in [
            (StrokeAlign::Center, "dc-stroke-center", "Align Stroke to Center", "center"),
            (StrokeAlign::Inside, "dc-stroke-inside", "Align Stroke to Inside", "inside"),
            (StrokeAlign::Outside, "dc-stroke-outside", "Align Stroke to Outside", "outside"),
        ] {
            if widgets::icon_button(ui, icon, tip, align == v, 24.0).clicked() {
                set(app, json!({"align": name}));
            }
        }
    });
    widgets::divider(ui);
    // Dashed line.
    let dash = st.as_ref().and_then(|s| s.dash.clone());
    let fields: [Option<f64>; 6] = match &dash {
        Some(d) => dash_fields(&d.pattern),
        None => pstate::<Option<[Option<f64>; 6]>>(ui.ctx(), "stroke-dash-last").unwrap_or([Some(12.0), None, None, None, None, None]),
    };
    let align_corners = dash.as_ref().is_some_and(|d| d.align_corners);
    ui.horizontal(|ui| {
        if widgets::check(ui, "Dashed Line", dash.is_some(), st.is_some()) {
            if dash.is_some() {
                set_pstate(ui.ctx(), "stroke-dash-last", Some(fields));
                set(app, json!({"dash": null}));
            } else {
                set(app, json!({"dash": dash_pattern(&fields), "alignDashes": align_corners}));
            }
        }
        ui.add_space((ui.available_width() - 56.0).max(0.0));
        let on = dash.is_some();
        if widgets::icon_button_enabled(ui, "dc-dash-exact", "Exact dash lengths", on && !align_corners, on, 24.0).clicked() {
            set(app, json!({"dash": dash_pattern(&fields), "alignDashes": false}));
        }
        if widgets::icon_button_enabled(ui, "dc-dash-align", "Fit dashes to corners and ends", on && align_corners, on, 24.0).clicked() {
            set(app, json!({"dash": dash_pattern(&fields), "alignDashes": true}));
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        let fw = ((ui.available_width() - 15.0) / 6.0).clamp(28.0, 40.0);
        let mut nf = fields;
        let mut changed = false;
        for (i, f) in fields.iter().enumerate() {
            let r = ui.add_enabled_ui(dash.is_some(), |ui| widgets::opt_field(ui, ("dash", i), *f, fw)).inner;
            if let Some(x) = r {
                nf[i] = x.map(|v| v.max(0.0));
                changed = true;
            }
        }
        if changed && dash.is_some() {
            set(app, json!({"dash": dash_pattern(&nf), "alignDashes": align_corners}));
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        let fw = ((ui.available_width() - 15.0) / 6.0).clamp(28.0, 40.0);
        for lbl in ["dash", "gap", "dash", "gap", "dash", "gap"] {
            ui.add_sized(vec2(fw, 12.0), egui::Label::new(egui::RichText::new(lbl).size(10.5).color(t.text_dim)));
        }
    });
    widgets::divider(ui);
    // Arrowheads.
    let (sa, ea) = (st.as_ref().and_then(|s| s.start_arrow), st.as_ref().and_then(|s| s.end_arrow));
    ui.horizontal(|ui| {
        row_label(ui, "Arrowheads:");
        if let Some(a) = arrow_dropdown(ui, "arrow-start", sa, true) {
            set(app, json!({"startArrow": a.map(|a| format!("{a:?}"))}));
        }
        if let Some(a) = arrow_dropdown(ui, "arrow-end", ea, false) {
            set(app, json!({"endArrow": a.map(|a| format!("{a:?}"))}));
        }
        if widgets::icon_button_enabled(ui, "arrow-left-right", "Swap start and end arrowheads", false, st.is_some(), 22.0).clicked() {
            app.run("stroke.setAdvanced", json!({"swapArrows": true})).ok();
        }
    });
    let scale = st.as_ref().map(|s| s.arrow_scale).unwrap_or((100.0, 100.0));
    let linked: bool = pstate(ui.ctx(), "arrow-scale-link");
    ui.horizontal(|ui| {
        row_label(ui, "Scale:");
        let on = st.is_some() && (sa.is_some() || ea.is_some());
        let mut ns = None;
        ui.add_enabled_ui(on, |ui| {
            if let Some(v) = widgets::plain_field(ui, "arrow-scale-s", scale.0, "%", 0, 56.0) {
                ns = Some((v, if linked { v } else { scale.1 }));
            }
            if let Some(v) = widgets::plain_field(ui, "arrow-scale-e", scale.1, "%", 0, 56.0) {
                ns = Some((if linked { v } else { scale.0 }, v));
            }
        });
        if widgets::icon_button(ui, if linked { "link" } else { "link-2-off" }, "Link start and end arrowhead scales", linked, 22.0).clicked() {
            set_pstate(ui.ctx(), "arrow-scale-link", !linked);
        }
        if let Some((a, b)) = ns {
            app.run("stroke.setAdvanced", json!({"arrowScale": [a, b]})).ok();
        }
    });
    ui.horizontal(|ui| {
        row_label(ui, "Align:");
        widgets::icon_button_enabled(ui, "dc-cap-square", "Tip extends past the end (on the roadmap)", true, false, 22.0);
        widgets::icon_button_enabled(ui, "dc-cap-butt", "Tip on the end point (on the roadmap)", false, false, 22.0);
    });
    widgets::divider(ui);
    // Profile.
    let pid = profile_id(st.as_ref().and_then(|s| s.profile.as_ref()));
    ui.horizontal(|ui| {
        row_label(ui, "Profile:");
        if let Some(id) = profile_dropdown(ui, pid) {
            set(app, json!({"profile": id}));
        }
        let can_flip = st.as_ref().is_some_and(|s| s.profile.is_some());
        if widgets::icon_button_enabled(ui, "flip-horizontal-2", "Flip Along", false, can_flip, 22.0).clicked() {
            app.run("stroke.setAdvanced", json!({"flipProfile": "along"})).ok();
        }
        if widgets::icon_button_enabled(ui, "flip-vertical-2", "Flip Across", false, can_flip, 22.0).clicked() {
            app.run("stroke.setAdvanced", json!({"flipProfile": "across"})).ok();
        }
    });
}

/// Draw an arrowhead preview: a line with the head at the left (`start`) or right end.
fn paint_arrow(ui: &Ui, r: Rect, a: Option<Arrowhead>, start: bool, color: Color32) {
    let y = r.center().y;
    let (tip_x, back) = if start { (r.left() + 4.0, 1.0) } else { (r.right() - 4.0, -1.0) };
    let s = Stroke::new(1.5, color);
    let line_from = if a.is_some() { tip_x + back * 8.0 } else { tip_x };
    let other = if start { r.right() - 4.0 } else { r.left() + 4.0 };
    ui.painter().line_segment([pos2(line_from, y), pos2(other, y)], s);
    let Some(a) = a else { return };
    let p = |dx: f32, dy: f32| -> Pos2 { pos2(tip_x + back * dx, y + dy) };
    let solid = |pts: Vec<Pos2>| egui::Shape::convex_polygon(pts, color, Stroke::NONE);
    let open = |pts: Vec<Pos2>| egui::Shape::closed_line(pts, s);
    let shape = match a {
        Arrowhead::Triangle => solid(vec![p(0.0, 0.0), p(9.0, -4.5), p(9.0, 4.5)]),
        Arrowhead::TriangleOpen => open(vec![p(0.0, 0.0), p(9.0, -4.5), p(9.0, 4.5)]),
        Arrowhead::Arrow => solid(vec![p(0.0, 0.0), p(10.0, -5.0), p(7.0, 0.0), p(10.0, 5.0)]),
        Arrowhead::ArrowOpen => egui::Shape::line(vec![p(9.0, -5.0), p(0.0, 0.0), p(9.0, 5.0)], s),
        Arrowhead::Circle => egui::Shape::circle_filled(p(4.5, 0.0), 4.5, color),
        Arrowhead::CircleOpen => egui::Shape::circle_stroke(p(4.5, 0.0), 4.0, s),
        Arrowhead::Square => egui::Shape::rect_filled(Rect::from_center_size(p(4.5, 0.0), vec2(8.0, 8.0)), 0.0, color),
        Arrowhead::SquareOpen => egui::Shape::rect_stroke(Rect::from_center_size(p(4.5, 0.0), vec2(8.0, 8.0)), 0.0, s, StrokeKind::Middle),
        Arrowhead::Diamond => solid(vec![p(0.0, 0.0), p(5.0, -5.0), p(10.0, 0.0), p(5.0, 5.0)]),
        Arrowhead::Bar => egui::Shape::line_segment([p(0.0, -6.0), p(0.0, 6.0)], Stroke::new(2.0, color)),
    };
    ui.painter().add(shape);
}

/// Dropdown whose button and items show drawn arrowhead previews. Returns Some(choice).
fn arrow_dropdown(ui: &mut Ui, id: &str, cur: Option<Arrowhead>, start: bool) -> Option<Option<Arrowhead>> {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(64.0, 24.0), Sense::click());
    ui.painter().rect_filled(r, 2, t.input);
    ui.painter().rect_stroke(r, 2, Stroke::new(1.0, if resp.hovered() { t.text } else { t.input_border }), StrokeKind::Inside);
    let body = Rect::from_min_max(r.min + vec2(3.0, 0.0), pos2(r.right() - 16.0, r.bottom()));
    paint_arrow(ui, body, cur, start, t.text_strong);
    icons::paint(ui, "chevron-down", Rect::from_center_size(pos2(r.right() - 8.0, r.center().y), vec2(10.0, 10.0)), t.icon);
    let resp = resp.on_hover_text(if start { "Start arrowhead" } else { "End arrowhead" });
    let mut out = None;
    egui::Popup::menu(&resp).id(egui::Id::new(("arrow-pop", id))).show(|ui| {
        ui.set_min_width(140.0);
        let opts: Vec<Option<Arrowhead>> = std::iter::once(None).chain(Arrowhead::ALL.iter().copied().map(Some)).collect();
        for a in opts {
            let (row, rr) = ui.allocate_exact_size(vec2(140.0, 22.0), Sense::click());
            if a == cur {
                ui.painter().rect_filled(row, 0.0, t.row_selected);
            } else if rr.hovered() {
                ui.painter().rect_filled(row, 0.0, t.hover);
            }
            paint_arrow(ui, Rect::from_min_size(row.min + vec2(4.0, 0.0), vec2(56.0, 22.0)), a, start, t.text_strong);
            ui.painter().text(
                row.left_center() + vec2(66.0, 0.0),
                egui::Align2::LEFT_CENTER,
                arrow_label(a),
                egui::FontId::proportional(11.5),
                t.text,
            );
            if rr.clicked() {
                out = Some(a);
                ui.close();
            }
        }
    });
    out
}

/// Draw a width profile's silhouette into `r`.
pub fn paint_profile(ui: &Ui, r: Rect, prof: Option<&WidthProfile>, color: Color32) {
    let n = 32;
    let half = r.height() / 2.0 - 1.0;
    let mut top = vec![];
    let mut bot = vec![];
    for i in 0..=n {
        let tt = i as f64 / n as f64;
        let (l, rr) = prof.map(|p| p.at(tt)).unwrap_or((0.35, 0.35));
        let x = r.left() + tt as f32 * r.width();
        top.push(pos2(x, r.center().y - l as f32 * half));
        bot.push(pos2(x, r.center().y + rr as f32 * half));
    }
    let mut mesh = egui::Mesh::default();
    for i in 0..=n {
        mesh.colored_vertex(top[i], color);
        mesh.colored_vertex(bot[i], color);
    }
    for i in 0..n as u32 {
        let a = i * 2;
        mesh.add_triangle(a, a + 1, a + 2);
        mesh.add_triangle(a + 1, a + 3, a + 2);
    }
    ui.painter().add(egui::Shape::mesh(mesh));
}

fn profile_dropdown(ui: &mut Ui, cur: &str) -> Option<&'static str> {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(100.0, 24.0), Sense::click());
    ui.painter().rect_filled(r, 2, t.input);
    ui.painter().rect_stroke(r, 2, Stroke::new(1.0, if resp.hovered() { t.text } else { t.input_border }), StrokeKind::Inside);
    let body = Rect::from_min_max(r.min + vec2(6.0, 5.0), pos2(r.right() - 20.0, r.bottom() - 5.0));
    paint_profile(ui, body, profile_of(cur).as_ref(), t.text_strong);
    icons::paint(ui, "chevron-down", Rect::from_center_size(pos2(r.right() - 9.0, r.center().y), vec2(10.0, 10.0)), t.icon);
    let resp = resp.on_hover_text("Variable Width Profile");
    let mut out = None;
    egui::Popup::menu(&resp).show(|ui| {
        for (id, label) in PROFILES {
            let (row, rr) = ui.allocate_exact_size(vec2(170.0, 26.0), Sense::click());
            if id == cur {
                ui.painter().rect_filled(row, 0.0, t.row_selected);
            } else if rr.hovered() {
                ui.painter().rect_filled(row, 0.0, t.hover);
            }
            paint_profile(ui, Rect::from_min_size(row.min + vec2(6.0, 6.0), vec2(70.0, 14.0)), profile_of(id).as_ref(), t.text_strong);
            ui.painter().text(row.left_center() + vec2(84.0, 0.0), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(11.5), t.text);
            if rr.clicked() {
                out = Some(id);
                ui.close();
            }
        }
    });
    out
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let hidden: bool = pstate(ui.ctx(), "stroke-hide-options");
    if menu_item(ui, if hidden { "Show Options" } else { "Hide Options" }, true, false) {
        set_pstate(ui.ctx(), "stroke-hide-options", !hidden);
    }
    menu_item(ui, "Add to Profiles", false, false);
    menu_item(ui, "Delete Profile", false, false);
    if menu_item(ui, "Reset Profile", stroke_now(app).is_some_and(|s| s.profile.is_some()), false) {
        set(app, json!({"profile": "uniform"}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dash_fields_roundtrip() {
        let f = dash_fields(&[12.0, 6.0, 2.0, 3.0]);
        assert_eq!(f, [Some(12.0), Some(6.0), Some(2.0), Some(3.0), None, None]);
        assert_eq!(dash_pattern(&f), vec![12.0, 6.0, 2.0, 3.0]);
    }

    #[test]
    fn dash_without_gap_repeats_dash() {
        assert_eq!(dash_pattern(&[Some(5.0), None, None, Some(9.0), None, None]), vec![5.0, 5.0]);
        assert!(dash_pattern(&[None; 6]).is_empty());
        assert!(dash_pattern(&[Some(0.0), Some(0.0), None, None, None, None]).is_empty());
    }

    #[test]
    fn profile_ids() {
        assert_eq!(profile_id(None), "uniform");
        assert_eq!(profile_id(Some(&WidthProfile::lens())), "lens");
        assert_eq!(profile_id(Some(&WidthProfile::taper_end())), "taperEnd");
        assert_eq!(profile_id(Some(&WidthProfile { points: vec![(0.0, 0.3, 0.3)] })), "custom");
        assert_eq!(arrow_label(Some(Arrowhead::CircleOpen)), "Circle (open)");
    }
}
