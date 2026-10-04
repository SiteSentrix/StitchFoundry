//! Swatches panel: proxy, Recent Colors, thumbnail grid (15.5 pt tiles on a 17 pt pitch) or list,
//! colour groups as folders, None/Registration first, bottom bar and panel menu.

use egui::{Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_color::Paint;

use super::{active_paint, pstate, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, menu_item, swatch_tile};
use crate::{VectorcraftApp, icons};

/// View modes of the swatch list.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum View {
    SmallThumb,
    #[default]
    MediumThumb,
    LargeThumb,
    SmallList,
    LargeList,
}

impl View {
    const ALL: [(View, &'static str); 5] = [
        (View::SmallThumb, "Small Thumbnail View"),
        (View::MediumThumb, "Medium Thumbnail View"),
        (View::LargeThumb, "Large Thumbnail View"),
        (View::SmallList, "Small List View"),
        (View::LargeList, "Large List View"),
    ];
    fn is_list(self) -> bool {
        matches!(self, View::SmallList | View::LargeList)
    }
    /// (tile, pitch) in points. Medium is the measured 15.5 / 17 pt.
    fn tile(self) -> (f32, f32) {
        match self {
            View::SmallThumb => (11.0, 12.5),
            View::MediumThumb => (15.5, 17.0),
            View::LargeThumb => (30.0, 32.0),
            View::SmallList => (12.0, 17.0),
            View::LargeList => (18.0, 25.0),
        }
    }
}

/// Swatch-kind filter (Show Swatch Kinds menu).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    All,
    Color,
    Gradient,
    Pattern,
    Groups,
}

impl Kind {
    const ALL: [(Kind, &'static str); 5] = [
        (Kind::All, "Show All Swatches"),
        (Kind::Color, "Show Color Swatches"),
        (Kind::Gradient, "Show Gradient Swatches"),
        (Kind::Pattern, "Show Pattern Swatches"),
        (Kind::Groups, "Show Color Groups"),
    ];
    /// Does a swatch with `paint` (in a group or not) pass the filter?
    pub fn accepts(self, paint: &Paint, in_group: bool) -> bool {
        match self {
            Kind::All => true,
            Kind::Color => matches!(paint, Paint::Solid { .. } | Paint::None),
            Kind::Gradient => matches!(paint, Paint::Gradient(_)),
            Kind::Pattern => matches!(paint, Paint::Pattern { .. }),
            Kind::Groups => in_group,
        }
    }
}

/// A row of the swatch list: special tiles, plain swatches and group folders.
#[derive(Clone, Debug)]
enum Entry {
    Registration,
    Swatch { name: String, paint: Paint, global: bool, spot: bool },
    Folder(String),
}

const REGISTRATION: &str = "[Registration]";

fn entries(app: &VectorcraftApp, kind: Kind) -> Vec<Entry> {
    let Some(st) = app.session.active() else { return vec![] };
    let d = &st.doc;
    let mut out = vec![];
    let sw = |s: &vectorcraft_color::Swatch| Entry::Swatch { name: s.name.clone(), paint: s.paint.clone(), global: s.global, spot: s.spot };
    // None first, then Registration, then the rest (Illustrator's order).
    let (specials, rest): (Vec<_>, Vec<_>) = d.swatches.iter().partition(|s| s.paint.is_none());
    for s in specials.iter().filter(|s| kind.accepts(&s.paint, false)) {
        out.push(sw(s));
    }
    if matches!(kind, Kind::All | Kind::Color) {
        out.push(Entry::Registration);
    }
    for s in rest.iter().filter(|s| kind.accepts(&s.paint, false)) {
        out.push(sw(s));
    }
    for g in &d.swatch_groups {
        let items: Vec<_> = g.swatches.iter().filter(|s| kind.accepts(&s.paint, true)).collect();
        if items.is_empty() && kind != Kind::Groups && kind != Kind::All {
            continue;
        }
        out.push(Entry::Folder(g.name.clone()));
        out.extend(items.into_iter().map(sw));
    }
    out
}

/// Apply a clicked swatch to the active proxy (Alt: the inactive one).
fn apply(app: &mut VectorcraftApp, ui: &Ui, e: &Entry) {
    let params = match e {
        Entry::Registration => json!({"color": {"c": 1.0, "m": 1.0, "y": 1.0, "k": 1.0}}),
        Entry::Swatch { paint, .. } if paint.is_none() => json!({"none": true}),
        Entry::Swatch { name, .. } => json!({"swatch": name}),
        Entry::Folder(_) => return,
    };
    super::apply_click(app, ui, params);
}

/// A pattern swatch drawn as a rendered tile (cached by the definition's identity and size).
fn pattern_thumb(app: &VectorcraftApp, ui: &Ui, r: Rect, paint: &Paint) {
    use std::cell::RefCell;
    use std::collections::HashMap;
    type Key = (String, Vec<usize>, String, u32);
    thread_local! {
        static CACHE: RefCell<HashMap<Key, Option<egui::TextureHandle>>> = RefCell::new(HashMap::new());
    }
    let Paint::Pattern { pattern, .. } = paint else { return };
    let Some(st) = app.session.active() else { return };
    let Some(def) = st.doc.pattern(pattern) else { return };
    let px = (r.width() * ui.ctx().pixels_per_point()).round().max(4.0) as u32;
    let key = (
        pattern.clone(),
        def.art.iter().map(|a| std::sync::Arc::as_ptr(a) as usize).collect(),
        format!("{:?}{:?}{:?}", def.tile, def.tile_type, def.overlap),
        px,
    );
    let tex = CACHE.with(|c| c.borrow().get(&key).cloned()).unwrap_or_else(|| {
        let tex = vectorcraft_render::render_pattern_swatch(&st.doc, pattern, px).map(|img| {
            let color = egui::ColorImage::from_rgba_premultiplied([img.width as usize, img.height as usize], &img.pixels);
            ui.ctx().load_texture(format!("pattern-swatch-{pattern}-{px}"), color, egui::TextureOptions::LINEAR)
        });
        CACHE.with(|c| {
            let mut c = c.borrow_mut();
            if c.len() > 256 {
                c.clear();
            }
            c.insert(key, tex.clone());
        });
        tex
    });
    if let Some(tex) = tex {
        ui.painter().rect_filled(r, 0.0, egui::Color32::WHITE);
        ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
    }
}

fn draw_registration(ui: &Ui, r: Rect) {
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_filled(r, 0.0, egui::Color32::WHITE);
    let c = r.center();
    let rad = r.width() * 0.28;
    let s = Stroke::new(1.0, egui::Color32::BLACK);
    ui.painter().circle_stroke(c, rad, s);
    ui.painter().line_segment([pos2(r.left() + 2.0, c.y), pos2(r.right() - 2.0, c.y)], s);
    ui.painter().line_segment([pos2(c.x, r.top() + 2.0), pos2(c.x, r.bottom() - 2.0)], s);
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
}

fn draw_folder(ui: &Ui, r: Rect) {
    let t = Tokens::get(ui.ctx());
    icons::paint(ui, "dc-folder", r.expand(1.0), t.icon);
}

fn selected_name(ui: &Ui) -> Option<String> {
    pstate::<Option<String>>(ui.ctx(), "swatch-selected")
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if app.session.active().is_none() {
        super::empty_state(ui, "swatch-book", "No document", "Open a document to see its swatches.");
        return;
    }
    let view: View = pstate(ui.ctx(), "swatch-view");
    let kind: Kind = pstate(ui.ctx(), "swatch-kind");
    // Top row: proxy at left, list / grid view toggles at right.
    ui.horizontal(|ui| {
        super::proxy(app, ui, 34.0);
        ui.add_space(ui.available_width() - 58.0);
        if widgets::icon_button(ui, "dc-list-view", "Show List View", view.is_list(), 26.0).clicked() {
            set_pstate(ui.ctx(), "swatch-view", View::SmallList);
        }
        if widgets::icon_button(ui, "dc-grid-view", "Show Thumbnail View", !view.is_list(), 26.0).clicked() {
            set_pstate(ui.ctx(), "swatch-view", View::MediumThumb);
        }
    });
    ui.add_space(4.0);
    super::recent_colors_row(app, ui);
    widgets::divider(ui);
    widgets::subheader(ui, "Swatch Tiles");
    let items = entries(app, kind);
    let active = active_paint(app);
    let active_swatch = match &active {
        Paint::Solid { swatch: Some(n), .. } => Some(n.clone()),
        Paint::None => Some("[None]".to_string()),
        _ => None,
    };
    let sel = selected_name(ui).or(active_swatch);
    let (tile, pitch) = view.tile();
    let mut clicked: Option<Entry> = None;
    let mut edit_pattern: Option<String> = None;
    widgets::list_box(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("swatch-scroll").max_height(if view == View::LargeThumb { 200.0 } else { 150.0 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if view.is_list() {
                for e in &items {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), pitch), Sense::click());
                    let name = match e {
                        Entry::Registration => REGISTRATION.to_string(),
                        Entry::Swatch { name, .. } => name.clone(),
                        Entry::Folder(n) => n.clone(),
                    };
                    let is_sel = sel.as_deref() == Some(name.as_str());
                    if is_sel {
                        ui.painter().rect_filled(r, 0.0, t.row_selected);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 0.0, t.hover);
                    }
                    let chip = Rect::from_min_size(r.left_center() + vec2(4.0, -tile / 2.0), vec2(tile, tile));
                    match e {
                        Entry::Registration => draw_registration(ui, chip),
                        Entry::Swatch { paint, .. } => {
                            swatch_tile(ui, chip, paint, false, false);
                            pattern_thumb(app, ui, chip, paint);
                        }
                        Entry::Folder(_) => draw_folder(ui, chip),
                    }
                    ui.painter().text(
                        pos2(chip.right() + 8.0, r.center().y),
                        egui::Align2::LEFT_CENTER,
                        &name,
                        egui::FontId::proportional(12.0),
                        t.text,
                    );
                    // Kind markers at the right: global (white corner) / spot (dot) / process.
                    if let Entry::Swatch { global, spot, paint, .. } = e {
                        let mk = Rect::from_center_size(r.right_center() - vec2(12.0, 0.0), vec2(10.0, 10.0));
                        if *spot {
                            ui.painter().circle_filled(mk.center(), 3.5, t.icon);
                        } else if matches!(paint, Paint::Gradient(_)) {
                            icons::paint(ui, "dc-grad-linear", mk, t.icon);
                        } else if *global {
                            ui.painter().add(egui::Shape::convex_polygon(
                                vec![mk.left_bottom(), mk.right_bottom(), mk.right_top()],
                                t.icon,
                                Stroke::NONE,
                            ));
                        }
                    }
                    if resp.on_hover_text(&name).clicked() && !matches!(e, Entry::Folder(_)) {
                        clicked = Some(e.clone());
                    }
                }
            } else {
                let w = ui.available_width();
                let per_row = ((w - 2.0) / pitch).floor().max(1.0) as usize;
                let mut col = 0usize;
                let mut rows: Vec<Vec<(usize, &Entry)>> = vec![vec![]];
                for (i, e) in items.iter().enumerate() {
                    // A group folder starts a new row.
                    if matches!(e, Entry::Folder(_)) && col > 0 {
                        rows.push(vec![]);
                        col = 0;
                    }
                    if col == per_row {
                        rows.push(vec![]);
                        col = 0;
                    }
                    rows.last_mut().unwrap().push((i, e));
                    col += 1;
                }
                for row in rows {
                    let (r, _) = ui.allocate_exact_size(vec2(w, pitch), Sense::hover());
                    for (c, (i, e)) in row.into_iter().enumerate() {
                        let cell = Rect::from_min_size(r.min + vec2(1.0 + c as f32 * pitch, (pitch - tile) / 2.0), vec2(tile, tile));
                        let resp = ui.interact(cell, ui.id().with(("sw", i)), Sense::click());
                        let (name, tip) = match e {
                            Entry::Registration => (REGISTRATION.to_string(), "[Registration]".to_string()),
                            Entry::Swatch { name, .. } => (name.clone(), name.clone()),
                            Entry::Folder(n) => (n.clone(), format!("Color Group: {n}")),
                        };
                        match e {
                            Entry::Registration => draw_registration(ui, cell),
                            Entry::Swatch { paint, global, .. } => {
                                swatch_tile(ui, cell, paint, sel.as_deref() == Some(name.as_str()), resp.hovered());
                                pattern_thumb(app, ui, cell.shrink(1.0), paint);
                                if resp.double_clicked()
                                    && let Paint::Pattern { pattern, .. } = paint
                                {
                                    edit_pattern = Some(pattern.clone());
                                }
                                if *global {
                                    let k = cell.shrink(1.0);
                                    ui.painter().add(egui::Shape::convex_polygon(
                                        vec![k.right_bottom(), k.right_bottom() - vec2(5.0, 0.0), k.right_bottom() - vec2(0.0, 5.0)],
                                        egui::Color32::WHITE,
                                        Stroke::NONE,
                                    ));
                                }
                            }
                            Entry::Folder(_) => draw_folder(ui, cell),
                        }
                        if resp.on_hover_text(tip).clicked() && !matches!(e, Entry::Folder(_)) {
                            clicked = Some(e.clone());
                        }
                    }
                }
            }
        });
    });
    if let Some(name) = edit_pattern {
        app.run("object.pattern.edit", json!({"name": name})).ok();
        app.ui.open_panel = Some("patternOptions".into());
    }
    if let Some(e) = clicked {
        let name = match &e {
            Entry::Registration => REGISTRATION.to_string(),
            Entry::Swatch { name, .. } => name.clone(),
            Entry::Folder(n) => n.clone(),
        };
        set_pstate(ui.ctx(), "swatch-selected", Some(name));
        apply(app, ui, &e);
    }
    bottom(app, ui);
}

fn deletable(ui: &Ui) -> Option<String> {
    selected_name(ui).filter(|n| !n.starts_with('['))
}

fn bottom(app: &mut VectorcraftApp, ui: &mut Ui) {
    let kind: Kind = pstate(ui.ctx(), "swatch-kind");
    widgets::bottom_bar(ui, |ui| {
        widgets::icon_button_enabled(ui, "library", "Swatch Libraries (on the roadmap)", false, false, 24.0);
        let kr = widgets::icon_button(ui, "dc-swatch-kinds", "Show Swatch Kinds", kind != Kind::All, 24.0);
        egui::Popup::menu(&kr).show(|ui| {
            for (k, label) in Kind::ALL {
                if menu_item(ui, label, true, k == kind) {
                    set_pstate(ui.ctx(), "swatch-kind", k);
                }
            }
        });
        widgets::icon_button_enabled(ui, "dc-options", "Swatch Options (on the roadmap)", false, false, 24.0);
        ui.add_space((ui.available_width() - 3.0 * 28.0).max(0.0));
        if widgets::icon_button(ui, "dc-folder", "New Color Group", false, 24.0).clicked() {
            let names: Vec<String> = deletable(ui).into_iter().collect();
            app.run("swatch.newGroup", json!({"swatches": names})).ok();
        }
        if widgets::icon_button(ui, "dc-new-item", "New Swatch", false, 24.0).clicked() {
            new_swatch(app);
        }
        let del = deletable(ui);
        if widgets::icon_button_enabled(ui, "trash-2", "Delete Swatch", false, del.is_some(), 24.0).clicked()
            && let Some(n) = del
            && app.run("swatch.delete", json!({"name": n})).is_ok()
        {
            set_pstate::<Option<String>>(ui.ctx(), "swatch-selected", None);
        }
    });
}

fn new_swatch(app: &mut VectorcraftApp) {
    let p = active_paint(app);
    let params = match &p {
        Paint::None => json!({}),
        other => super::paint_params(other),
    };
    app.run("swatch.new", params).ok();
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let view: View = pstate(ui.ctx(), "swatch-view");
    let sel = deletable(ui);
    if menu_item(ui, "New Swatch…", true, false) {
        new_swatch(app);
    }
    if menu_item(ui, "New Color Group…", true, false) {
        app.run("swatch.newGroup", json!({"swatches": sel.clone().into_iter().collect::<Vec<_>>()})).ok();
    }
    if menu_item(ui, "Duplicate Swatch", sel.is_some(), false)
        && let Some(n) = &sel
    {
        app.run("swatch.duplicate", json!({"name": n})).ok();
    }
    menu_item(ui, "Merge Swatches", false, false);
    if menu_item(ui, "Delete Swatch", sel.is_some(), false)
        && let Some(n) = &sel
    {
        app.run("swatch.delete", json!({"name": n})).ok();
    }
    menu_item(ui, "Ungroup Color Group", false, false);
    menu_item(ui, "Select All Unused", false, false);
    menu_item(ui, "Add Used Colors", false, false);
    ui.separator();
    if menu_item(ui, "Sort by Name", true, false) {
        app.run("swatch.sortByName", json!({})).ok();
    }
    menu_item(ui, "Sort by Kind", false, false);
    ui.separator();
    for (v, label) in View::ALL {
        if menu_item(ui, label, true, v == view) {
            set_pstate(ui.ctx(), "swatch-view", v);
        }
    }
    ui.separator();
    menu_item(ui, "Swatch Options…", false, false);
    menu_item(ui, "Spot Colors…", false, false);
    ui.separator();
    menu_item(ui, "Open Swatch Library", false, false);
    menu_item(ui, "Save Swatch Library…", false, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use vectorcraft_color::Color;

    #[test]
    fn kind_filter() {
        let solid = Paint::solid(Color::BLACK);
        let grad = Paint::Gradient(Box::new(vectorcraft_color::GradientPaint::new(Default::default())));
        assert!(Kind::All.accepts(&grad, false));
        assert!(Kind::Color.accepts(&solid, false));
        assert!(!Kind::Color.accepts(&grad, false));
        assert!(Kind::Gradient.accepts(&grad, true));
        assert!(Kind::Groups.accepts(&solid, true));
        assert!(!Kind::Groups.accepts(&solid, false));
    }

    #[test]
    fn medium_tiles_match_measured_metrics() {
        assert_eq!(View::MediumThumb.tile(), (15.5, 17.0));
        assert!(View::SmallList.is_list() && !View::LargeThumb.is_list());
    }
}
