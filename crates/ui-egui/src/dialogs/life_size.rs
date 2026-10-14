//! View › Calibrate Life Size… (NEW-33): a ruler and a bank card drawn at the screen's measured
//! points per inch, to hold a real ruler or card against and adjust until they match. Saved, it
//! is the `screenPointsPerInch` preference, and Display Print Size at 100% Zoom is turned on so
//! View › Actual Size shows designs life size.

use egui::{Align2, FontId, Pos2, Stroke, vec2};
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;

/// The points per inch a calibration may set.
pub const RANGE: std::ops::RangeInclusive<f64> = 40.0..=400.0;
/// An ID-1 bank card (ISO/IEC 7810), in millimetres.
const CARD_MM: (f64, f64) = (85.6, 53.98);
/// The ruler's length, in millimetres.
const RULER_MM: u32 = 100;

/// The calibration window: open, and the points per inch being tried.
#[derive(Clone, Debug, Default)]
pub struct LifeSizeState {
    pub open: bool,
    pub ppi: f64,
}

/// `ui.calibrateLifeSize`: with `pointsPerInch`, keeps it and turns life size on; without, opens
/// the window on the value kept.
pub fn run(app: &mut VectorcraftApp, p: &Value) -> Result<Value, String> {
    match p.get("pointsPerInch") {
        Some(v) => {
            let ppi = v.as_f64().filter(|x| x.is_finite() && RANGE.contains(x)).ok_or("`pointsPerInch` is a number from 40 to 400")?;
            keep(app, ppi)?;
            Ok(json!({ "pointsPerInch": ppi }))
        }
        None => {
            app.ui.life_size = LifeSizeState { open: true, ppi: app.session.prefs.screen_points_per_inch };
            Ok(json!({ "open": true }))
        }
    }
}

/// Keeps `ppi` as the screen's points per inch, turns Display Print Size at 100% Zoom on and
/// shows the open design at its new actual size.
fn keep(app: &mut VectorcraftApp, ppi: f64) -> Result<(), String> {
    app.session.execute("prefs.set", &json!({ "values": { "screenPointsPerInch": ppi, "displayPrintSize": true } })).map_err(|e| e.to_string())?;
    if app.session.active().is_some() {
        app.run("view.actualSize", json!({}))?;
    }
    Ok(())
}

/// The window, while open.
pub(super) fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !app.ui.life_size.open {
        return;
    }
    let mut open = true;
    let mut done: Option<bool> = None;
    egui::Window::new(tl!("Calibrate Life Size"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_pos(ctx.content_rect().center() - vec2(300.0, 220.0))
        .show(ctx, |ui| {
            let t = Tokens::get(ui.ctx());
            ui.set_max_width(560.0);
            ui.label(tl!("Hold a ruler, or a bank card, against the screen and change the size until the marks match. Actual Size then shows designs life size."));
            ui.add_space(8.0);
            let ppi = &mut app.ui.life_size.ppi;
            if !(ppi.is_finite() && RANGE.contains(ppi)) {
                *ppi = 96.0;
            }
            let per_mm = (*ppi / 25.4) as f32;
            // the ruler: a mark each millimetre, longer every 5, numbered every 10
            let (rect, _) = ui.allocate_exact_size(vec2(per_mm * RULER_MM as f32 + 24.0, 44.0), egui::Sense::hover());
            let p = ui.painter_at(rect);
            let x0 = rect.left() + 8.0;
            p.line_segment([Pos2::new(x0, rect.top()), Pos2::new(x0 + per_mm * RULER_MM as f32, rect.top())], Stroke::new(1.0, t.text));
            for mm in 0..=RULER_MM {
                let x = x0 + per_mm * mm as f32;
                let len = if mm % 10 == 0 { 18.0 } else if mm % 5 == 0 { 12.0 } else { 7.0 };
                p.line_segment([Pos2::new(x, rect.top()), Pos2::new(x, rect.top() + len)], Stroke::new(1.0, t.text));
                if mm % 10 == 0 {
                    p.text(Pos2::new(x, rect.top() + 20.0), Align2::CENTER_TOP, mm.to_string(), FontId::proportional(11.0), t.text_dim);
                }
            }
            p.text(Pos2::new(x0 + per_mm * RULER_MM as f32, rect.top() + 34.0), Align2::RIGHT_TOP, "mm", FontId::proportional(10.0), t.text_dim);
            ui.add_space(8.0);
            // a bank card's outline
            let card = vec2(per_mm * CARD_MM.0 as f32, per_mm * CARD_MM.1 as f32);
            let (rect, _) = ui.allocate_exact_size(card + vec2(16.0, 8.0), egui::Sense::hover());
            let r = egui::Rect::from_min_size(rect.min + vec2(8.0, 4.0), card);
            ui.painter().rect_stroke(r, 3.0 * per_mm, Stroke::new(1.5, t.accent), egui::StrokeKind::Inside);
            ui.painter().text(r.center(), Align2::CENTER_CENTER, tl!("Bank card"), FontId::proportional(12.0), t.text_dim);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(tl!("Screen points per inch"));
                if ui.small_button("−").clicked() {
                    *ppi = (*ppi - 0.5).max(*RANGE.start());
                }
                ui.add(egui::Slider::new(ppi, RANGE).step_by(0.1).fixed_decimals(1));
                if ui.small_button("+").clicked() {
                    *ppi = (*ppi + 0.5).min(*RANGE.end());
                }
            });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(tl!("Use This Size")).clicked() {
                    done = Some(true);
                }
                if ui.button(tl!("Cancel")).clicked() {
                    done = Some(false);
                }
            });
        });
    if done == Some(true) {
        let ppi = app.ui.life_size.ppi;
        if let Err(e) = keep(app, ppi) {
            app.status(e);
        }
    }
    if !open || done.is_some() {
        app.ui.life_size.open = false;
    }
}
