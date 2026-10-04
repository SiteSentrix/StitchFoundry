//! The gradient stop popover: double-clicking a stop on the Gradient tool's annotator edits it
//! next to its chip — its colour (the Color panel's controls, or a document swatch), opacity and
//! location. It edits the selected stop (`gradient.selectStop`) through `paint.editGradient`, and
//! closes on Escape or a click elsewhere.
//!
//! Fields: `index`, `x`, `y` (the chip, document coordinates) and `tab` (`color` or `swatches`).
//! Agents can set `color` (hex), `opacity` and `location` (percentages) and confirm.

use egui::{Sense, vec2};
use serde_json::{Value, json};
use vectorcraft_color::gradient::move_stop;
use vectorcraft_color::{Color, GradientStop, Paint};
use vectorcraft_geom::Point;

use super::{DialogResult, DialogSpec};
use crate::VectorcraftApp;
use crate::canvas::Xf;
use crate::panels::gradient::stops_json;
use crate::panels::{active_paint, live_run};
use crate::state::Dialog;
use crate::theme::Tokens;
use crate::widgets::{self, Live};

pub(super) const SPEC: DialogSpec = DialogSpec::window(show, confirm);

/// The stops of the gradient behind the active proxy and the selected stop's index.
fn selected(app: &VectorcraftApp) -> Option<(Vec<GradientStop>, usize)> {
    let Paint::Gradient(g) = active_paint(app) else { return None };
    let i = app.session.selected_stop().filter(|i| *i < g.gradient.stops.len())?;
    Some((g.gradient.stops, i))
}

/// Write `stops` to the gradient and select stop `i`.
fn apply(app: &mut VectorcraftApp, stops: &[GradientStop], i: usize, phase: Live) {
    let params = json!({ "stops": stops_json(stops), "stroke": !app.session.fill_active });
    live_run(app, "Gradient", "paint.editGradient", params, phase);
    if phase == Live::Released && app.session.selected_stop() != Some(i) {
        app.run("gradient.selectStop", json!({ "index": i })).ok();
    }
}

/// `stops` with stop `i` given `color`.
fn recolor(mut stops: Vec<GradientStop>, i: usize, color: Color) -> Vec<GradientStop> {
    stops[i].color = color;
    stops
}

/// Solid colours of the document's swatches (groups included), with their names.
fn swatch_colors(app: &VectorcraftApp) -> Vec<(String, Color)> {
    let Some(st) = app.session.active() else { return vec![] };
    let d = &st.doc;
    d.swatches.iter().chain(d.swatch_groups.iter().flat_map(|g| g.swatches.iter())).filter_map(|s| Some((s.name.clone(), s.paint.color()?))).collect()
}

fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    let Some(d) = app.ui.dialog.clone() else { return };
    // The stop went away (undo, another object selected): nothing left to edit.
    let Some((stops, i)) = selected(app) else {
        app.ui.dialog = None;
        return;
    };
    let t = Tokens::get(ctx);
    let chip = Point::new(d.f64("x", 0.0), d.f64("y", 0.0));
    let pos =
        app.canvas_rect.zip(app.view().copied()).map_or(ctx.content_rect().center(), |(r, v)| Xf::new(r, &v).to_screen(chip) + vec2(14.0, 14.0));
    let swatches = d.str("tab") == "swatches";
    let mut tab = None;
    let resp = egui::Area::new(egui::Id::new("gradient-stop-popover"))
        .order(egui::Order::Foreground)
        .fixed_pos(pos)
        .constrain(true)
        .show(ctx, |ui| {
            egui::Frame::popup(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                ui.set_width(250.0);
                ui.horizontal(|ui| {
                    for (icon, tip, on) in [("palette", "Color", !swatches), ("swatch-book", "Swatches", swatches)] {
                        if widgets::icon_button(ui, icon, tip, on, 24.0).clicked() {
                            tab = Some(if icon == "palette" { "color" } else { "swatches" });
                        }
                    }
                });
                ui.add_space(6.0);
                if swatches {
                    swatch_grid(app, ui, &stops, i);
                } else {
                    // The Color panel edits the selected stop of the active gradient.
                    crate::panels::color::show(app, ui);
                }
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    let s = stops[i];
                    widgets::dim_label(ui, "Opacity:");
                    if let Some(v) = widgets::plain_field(ui, "stop-pop-op", s.opacity as f64 * 100.0, "%", 0, 54.0) {
                        let mut v2 = stops.clone();
                        v2[i].opacity = (v / 100.0).clamp(0.0, 1.0) as f32;
                        apply(app, &v2, i, Live::Released);
                    }
                    widgets::dim_label(ui, "Location:");
                    if let Some(v) = widgets::plain_field(ui, "stop-pop-loc", s.offset as f64 * 100.0, "%", 1, 54.0) {
                        let (v2, ni) = move_stop(&stops, i, (v / 100.0) as f32);
                        apply(app, &v2, ni, Live::Released);
                    }
                });
            });
        })
        .response;
    if let (Some(tab), Some(d)) = (tab, app.ui.dialog.as_mut()) {
        d.fields.insert("tab".into(), json!(tab));
    }
    // The double-click that opened the popover lands on its chip, outside it: only a later click
    // elsewhere closes it.
    if resp.clicked_elsewhere() && !ctx.input(|i| i.pointer.button_double_clicked(egui::PointerButton::Primary)) {
        app.ui.dialog = None;
    }
}

/// Document swatches as tiles: a click gives the stop that colour.
fn swatch_grid(app: &mut VectorcraftApp, ui: &mut egui::Ui, stops: &[GradientStop], i: usize) {
    let colors = swatch_colors(app);
    if colors.is_empty() {
        widgets::dim_label(ui, "The document has no colour swatches.");
        return;
    }
    let mut pick = None;
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
        for (name, c) in &colors {
            let (r, resp) = ui.allocate_exact_size(vec2(18.0, 18.0), Sense::click());
            widgets::swatch_tile(ui, r, &Paint::solid(*c), stops[i].color == *c, resp.hovered());
            if resp.on_hover_text(name).clicked() {
                pick = Some(*c);
            }
        }
    });
    if let Some(c) = pick {
        apply(app, &recolor(stops.to_vec(), i, c), i, Live::Released);
    }
}

/// OK (agents): apply the `color`, `opacity` and `location` fields that are set, then close.
fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> DialogResult {
    let (mut stops, mut i) = selected(app).ok_or("no gradient stop is selected")?;
    if let Some(hex) = d.fields.get("color").and_then(Value::as_str) {
        let c = crate::panels::color::parse_hex(hex).ok_or_else(|| format!("bad colour `{hex}` (#rrggbb)"))?;
        stops = recolor(stops, i, c);
    }
    if d.fields.contains_key("opacity") {
        stops[i].opacity = (d.f64("opacity", 100.0) / 100.0).clamp(0.0, 1.0) as f32;
    }
    if d.fields.contains_key("location") {
        (stops, i) = move_stop(&stops, i, (d.f64("location", 0.0) / 100.0) as f32);
    }
    apply(app, &stops, i, Live::Released);
    app.ui.dialog = None;
    Ok(json!({ "index": i }))
}

#[cfg(test)]
mod tests {
    use vectorcraft_engine::Session;
    use vectorcraft_tools::{PointerEvent, PointerKind};

    use super::*;

    /// A rectangle with a three-stop gradient from (100, 150) to (200, 150), the Gradient tool
    /// and stop 1 selected.
    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 100, "height": 100})).unwrap();
        let stops = json!([{"offset": 0, "color": "#ffffff"}, {"offset": 0.5, "color": "#00ff00"}, {"offset": 1, "color": "#000000"}]);
        app.run("paint.setFill", json!({"gradient": {"stops": stops, "start": [100, 150], "end": [200, 150]}})).unwrap();
        app.select_tool("gradient");
        app.run("gradient.selectStop", json!({"index": 1})).unwrap();
        app
    }

    fn stops(app: &VectorcraftApp) -> Vec<GradientStop> {
        selected(app).unwrap().0
    }

    /// One headless frame of the dialog layer and the shortcuts.
    fn frame(app: &mut VectorcraftApp, events: Vec<egui::Event>) {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut out = ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
            crate::shortcuts::handle(app, ui.ctx());
            super::super::show(app, ui.ctx());
        });
        out.textures_delta.clear();
    }

    fn key(k: egui::Key) -> egui::Event {
        egui::Event::Key { key: k, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }
    }

    #[test]
    fn double_clicking_a_stop_opens_the_popover_which_draws_both_tabs() {
        let mut app = app();
        let view = app.view_info();
        let chip = Point::new(150.0, 150.0 + 10.0 / view.zoom);
        crate::canvas::dispatch(&mut app, &PointerEvent { kind: PointerKind::DoubleClick, pos: chip, mods: Default::default(), pressure: 1.0 }, view);
        let d = app.ui.dialog.clone().expect("the popover opened");
        assert_eq!((d.kind.as_str(), d.fields["index"].clone(), d.str("tab")), ("gradientStop", json!(1), "color".into()));
        assert_eq!(app.session.selected_stop(), Some(1));
        frame(&mut app, vec![]);
        app.ui.dialog.as_mut().unwrap().fields.insert("tab".into(), json!("swatches"));
        frame(&mut app, vec![]);
        assert!(app.ui.dialog.is_some(), "still open");
        // Escape closes it.
        frame(&mut app, vec![key(egui::Key::Escape)]);
        assert!(app.ui.dialog.is_none());
    }

    #[test]
    fn confirm_applies_colour_opacity_and_location() {
        let mut app = app();
        app.ui.dialog = Some(Dialog::new("gradientStop", json!({"index": 1, "x": 150, "y": 160, "color": "#ff0000", "opacity": 40, "location": 80})));
        assert_eq!(super::super::confirm(&mut app).unwrap(), json!({"index": 1}));
        let s = stops(&app);
        assert_eq!((s[1].color.to_hex(), s[1].opacity, (s[1].offset * 100.0).round()), ("#ff0000".to_string(), 0.4, 80.0));
        assert!(app.ui.dialog.is_none());
        // Moving it past the last stop keeps it selected at its new index.
        app.ui.dialog = Some(Dialog::new("gradientStop", json!({"location": 100})));
        super::super::confirm(&mut app).unwrap();
        assert_eq!((app.session.selected_stop(), stops(&app)[2].color.to_hex()), (Some(2), "#ff0000".to_string()));
        app.ui.dialog = Some(Dialog::new("gradientStop", json!({"color": "red"})));
        assert!(super::super::confirm(&mut app).is_err());
    }

    fn click(at: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    #[test]
    fn the_opening_double_click_keeps_it_open_and_a_later_click_elsewhere_closes_it() {
        let mut app = app();
        app.ui.dialog = Some(Dialog::new("gradientStop", json!({"index": 1, "x": 150, "y": 160})));
        // The frame the double-click lands in (the popover opens beside the pointer, not under it).
        let at = egui::pos2(5.0, 5.0);
        let double = vec![egui::Event::PointerMoved(at), click(at, true), click(at, false), click(at, true), click(at, false)];
        frame(&mut app, double);
        assert!(app.ui.dialog.is_some(), "the opening double-click must not close it");
        frame(&mut app, vec![egui::Event::PointerMoved(at), click(at, true), click(at, false)]);
        assert!(app.ui.dialog.is_none(), "a click elsewhere closes it");
    }

    #[test]
    fn the_popover_closes_when_its_stop_goes_away() {
        let mut app = app();
        app.ui.dialog = Some(Dialog::new("gradientStop", json!({"index": 1, "x": 150, "y": 160})));
        app.run("gradient.selectStop", json!({"index": null})).unwrap();
        frame(&mut app, vec![]);
        assert!(app.ui.dialog.is_none());
    }

    #[test]
    fn delete_removes_the_selected_stop_instead_of_the_object() {
        let mut app = app();
        frame(&mut app, vec![key(egui::Key::Delete)]);
        assert_eq!(stops(&app).len(), 2);
        frame(&mut app, vec![key(egui::Key::Backspace)]);
        assert_eq!(stops(&app).len(), 2, "never below two stops");
        assert_eq!(app.session.doc().unwrap().selection.objects.len(), 1, "the rectangle stays");
        // The arrows nudge it.
        let before = stops(&app)[1].offset;
        frame(&mut app, vec![key(egui::Key::ArrowLeft)]);
        assert!((stops(&app)[1].offset - (before - 0.01)).abs() < 1e-6);
    }
}
