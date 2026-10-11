//! Stitch List panel: every record of the stitch plan (needle penetrations, jumps,
//! trims, colour changes and stops) in sewing order, with its position and length
//! and the object that sews it. Click a row to play the design up to it and select
//! its object; the row the stitch player is at is highlighted.

use egui::{Color32, Sense, Ui, vec2};
use serde_json::json;
use vectorcraft_embroidery::sf_core::Cmd;

use crate::VectorcraftApp;
use crate::theme::Tokens;

pub const ID: &str = "stitchList";

const ROW_H: f32 = 18.0;

fn function(c: Cmd) -> &'static str {
    match c {
        Cmd::Stitch => tl!("Stitch"),
        Cmd::Jump => tl!("Jump"),
        Cmd::Trim => super::machine_word("Trim"),
        Cmd::ColorChange => tl!("Color change"),
        Cmd::Stop => tl!("Stop"),
        Cmd::End => tl!("End"),
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(plan) = app.session.embroidery_plan() else {
        crate::panels::empty_state(ui, "dc-list-view", tl!("No document"), tl!("Open a document to see its stitches."));
        return;
    };
    let recs = &plan.pattern().stitches;
    if recs.is_empty() {
        crate::panels::empty_state(ui, "dc-list-view", tl!("Nothing stitched yet"), tl!("Give objects a stitch type in the Embroidery panel."));
        return;
    }
    let at = app.embroidery.player.active.then(|| (app.embroidery.player.pos as usize).min(recs.len()));
    // Header.
    let cols = [44.0, 84.0, 64.0, 64.0, 52.0];
    let header = [tl!("#"), tl!("Function"), tl!("X (mm)"), tl!("Y (mm)"), tl!("Length")];
    ui.horizontal(|ui| {
        for (w, h) in cols.iter().zip(header) {
            ui.add_sized(vec2(*w, ROW_H), egui::Label::new(egui::RichText::new(h).color(t.text_dim).size(11.5)));
        }
        ui.label(egui::RichText::new(tl!("Object")).color(t.text_dim).size(11.5));
    });
    let mut clicked: Option<usize> = None;
    let mut scroll = egui::ScrollArea::vertical().auto_shrink([false, false]);
    if app.embroidery.player.playing
        && let Some(i) = at
    {
        // keep the player's stitch in view while it plays
        scroll = scroll.vertical_scroll_offset((i as f32 * ROW_H - ui.available_height() * 0.5).max(0.0));
    }
    scroll.show_rows(ui, ROW_H, recs.len(), |ui, range| {
        for i in range {
            let r = recs[i];
            let prev = recs[..i].iter().rev().find(|q| q.is_move()).map(|q| q.pos());
            let len = if r.is_move() { prev.map(|q| q.distance(r.pos())) } else { None };
            let name = plan
                .node_at_record(i)
                .and_then(|id| plan.design.objects.get(plan.built.object_at(i).map_or(usize::MAX, |s| s.index)).map(|o| (id, o.name.clone())));
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), ROW_H), Sense::click());
            if at == Some(i) {
                ui.painter().rect_filled(rect, 2.0, t.accent.gamma_multiply(0.25));
            } else if resp.hovered() {
                ui.painter().rect_filled(rect, 2.0, Color32::from_black_alpha(20));
            }
            let color = match r.cmd {
                Cmd::Stitch => t.text,
                Cmd::Jump => Color32::from_rgb(40, 110, 220),
                _ => Color32::from_rgb(200, 70, 40),
            };
            let cells = [
                i.to_string(),
                function(r.cmd).to_string(),
                format!("{:.2}", r.x),
                format!("{:.2}", r.y),
                len.map_or_else(String::new, |l| format!("{l:.2}")),
                name.map(|n| n.1).unwrap_or_default(),
            ];
            let mut x = rect.left() + 2.0;
            for (k, c) in cells.iter().enumerate() {
                let w = cols.get(k).copied().unwrap_or(rect.right() - x);
                ui.painter().text(egui::pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, c, egui::FontId::proportional(12.0), color);
                x += w;
            }
            if resp.clicked() {
                clicked = Some(i);
            }
        }
    });
    if let Some(i) = clicked {
        let p = &mut app.embroidery.player;
        p.active = true;
        p.playing = false;
        p.pos = (i + 1) as f64;
        if let Some(id) = plan.node_at_record(i) {
            app.run("select.set", json!({ "ids": [id.0] })).ok();
        }
    }
}
