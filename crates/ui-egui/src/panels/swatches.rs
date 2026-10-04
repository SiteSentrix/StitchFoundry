//! Swatches panel: proxy, Recent Colors, thumbnail grid (15.5 pt tiles on a 17 pt pitch) or list,
//! colour groups as folders, None/Registration first, bottom bar and panel menu.

use egui::{Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_color::Paint;

use super::{active_paint, paint_target, pstate, push_recent, set_pstate};
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

impl Entry {
    fn name(&self) -> &str {
        match self {
            Entry::Registration => REGISTRATION,
            Entry::Swatch { name, .. } | Entry::Folder(name) => name,
        }
    }
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

fn apply(app: &mut VectorcraftApp, ui: &Ui, e: &Entry) {
    let target = paint_target(app);
    match e {
        Entry::Registration => {
            app.run(target, json!({"color": {"c": 1.0, "m": 1.0, "y": 1.0, "k": 1.0}})).ok();
        }
        Entry::Swatch { name, paint, .. } => {
            let r = if paint.is_none() { app.run(target, json!({"none": true})) } else { app.run(target, json!({"swatch": name})) };
            if r.is_ok()
                && let Some(c) = paint.color()
            {
                push_recent(ui.ctx(), c);
            }
        }
        Entry::Folder(_) => {}
    }
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

/// The swatches and colour groups selected in the panel, in click order, without names that no
/// longer exist (deleted, renamed or undone).
fn selection(app: &VectorcraftApp, ui: &Ui) -> Vec<String> {
    let Some(st) = app.session.active() else { return vec![] };
    let mut names: Vec<String> = pstate(ui.ctx(), "swatch-selected");
    names.retain(|n| n == REGISTRATION || st.doc.swatch_name_taken(n));
    names
}

/// The selection after a click on `name`: Cmd/Ctrl toggles it, Shift extends from the last clicked
/// name over `order` (the names as displayed), a plain click selects it alone.
fn click_selection(ui: &Ui, mut sel: Vec<String>, order: &[&str], name: &str, m: egui::Modifiers) -> Vec<String> {
    let anchor: String = pstate(ui.ctx(), "swatch-anchor");
    let pos = |n: &str| order.iter().position(|o| *o == n);
    if m.shift
        && let (Some(a), Some(b)) = (pos(&anchor), pos(name))
    {
        for n in &order[a.min(b)..=a.max(b)] {
            if !sel.iter().any(|s| s == n) {
                sel.push(n.to_string());
            }
        }
        return sel;
    }
    set_pstate(ui.ctx(), "swatch-anchor", name.to_string());
    if !m.command {
        return vec![name.to_string()];
    }
    match sel.iter().position(|s| s == name) {
        Some(i) => {
            sel.remove(i);
        }
        None => sel.push(name.to_string()),
    }
    sel
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
    let selected = selection(app, ui);
    // Without a selection the swatch of the active paint is highlighted.
    let sel: Vec<String> = if selected.is_empty() { active_swatch.into_iter().collect() } else { selected.clone() };
    let is_sel = |name: &str| sel.iter().any(|s| s == name);
    let (tile, pitch) = view.tile();
    let mut clicked: Option<(Entry, egui::Modifiers)> = None;
    // Double-click opens the swatch's editor (Swatch Options, Gradient panel or pattern editing).
    let mut edit: Option<String> = None;
    widgets::list_box(ui, |ui| {
        egui::ScrollArea::vertical().id_salt("swatch-scroll").max_height(if view == View::LargeThumb { 200.0 } else { 150.0 }).show(ui, |ui| {
            ui.set_width(ui.available_width());
            if view.is_list() {
                for e in &items {
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), pitch), Sense::click());
                    let name = e.name();
                    if is_sel(name) {
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
                        name,
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
                    if matches!(e, Entry::Swatch { .. }) && resp.double_clicked() {
                        edit = Some(name.to_string());
                    }
                    if resp.on_hover_text(name).clicked() {
                        clicked = Some((e.clone(), ui.input(|i| i.modifiers)));
                    }
                }
            } else {
                let w = ui.available_width();
                let per_row = ((w - 2.0) / pitch).floor().max(1.0) as usize;
                let mut col = 0usize;
                let mut rows: Vec<Vec<&Entry>> = vec![vec![]];
                for e in &items {
                    // A group folder starts a new row.
                    if matches!(e, Entry::Folder(_)) && col > 0 {
                        rows.push(vec![]);
                        col = 0;
                    }
                    if col == per_row {
                        rows.push(vec![]);
                        col = 0;
                    }
                    rows.last_mut().unwrap().push(e);
                    col += 1;
                }
                for row in rows {
                    let (r, _) = ui.allocate_exact_size(vec2(w, pitch), Sense::hover());
                    for (c, e) in row.into_iter().enumerate() {
                        let cell = Rect::from_min_size(r.min + vec2(1.0 + c as f32 * pitch, (pitch - tile) / 2.0), vec2(tile, tile));
                        let resp = ui.interact(cell, tile_id(e), Sense::click());
                        let name = e.name();
                        match e {
                            Entry::Registration => draw_registration(ui, cell),
                            Entry::Swatch { paint, global, .. } => {
                                swatch_tile(ui, cell, paint, is_sel(name), resp.hovered());
                                pattern_thumb(app, ui, cell.shrink(1.0), paint);
                                if resp.double_clicked() {
                                    edit = Some(name.to_string());
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
                            Entry::Folder(_) => {
                                draw_folder(ui, cell);
                                if is_sel(name) {
                                    ui.painter().rect_stroke(cell.expand(1.0), 0.0, Stroke::new(1.5, t.accent), StrokeKind::Outside);
                                }
                            }
                        }
                        let resp = match e {
                            Entry::Folder(n) => resp.on_hover_text(format!("Color Group: {n}")),
                            _ => resp.on_hover_text(name),
                        };
                        if resp.clicked() {
                            clicked = Some((e.clone(), ui.input(|i| i.modifiers)));
                        }
                    }
                }
            }
        });
    });
    if let Some(name) = edit {
        app.run("ui.swatchOptions", json!({"name": name})).ok();
    }
    let selected = match clicked {
        Some((e, m)) => {
            let order: Vec<&str> = items.iter().map(Entry::name).collect();
            let sel = click_selection(ui, selected, &order, e.name(), m);
            set_pstate(ui.ctx(), "swatch-selected", sel.clone());
            // A plain click also applies the swatch; modifier clicks only select.
            if !(m.shift || m.command) {
                apply(app, ui, &e);
            }
            sel
        }
        None => selected,
    };
    bottom(app, ui, &selected);
}

/// The interaction id of a tile in the thumbnail views (stable per swatch or group name).
fn tile_id(e: &Entry) -> egui::Id {
    egui::Id::new(("swatch-tile", e.name()))
}

/// The selected swatches and groups that can be deleted (all but None and Registration).
fn deletable(sel: &[String]) -> Vec<String> {
    sel.iter().filter(|n| !n.starts_with('[')).cloned().collect()
}

/// The selected swatch when exactly one swatch with an editor is selected (not None, Registration
/// or a group).
fn editable(app: &VectorcraftApp, sel: &[String]) -> Option<String> {
    match sel {
        [n] if !n.starts_with('[') && app.session.active().is_some_and(|st| st.doc.swatch(n).is_some()) => Some(n.clone()),
        _ => None,
    }
}

/// Delete `names` (swatches and colour groups) after asking, or at once with `now` (Alt-click).
fn delete(app: &mut VectorcraftApp, names: Vec<String>, now: bool) {
    let params = json!({"names": names});
    if now {
        app.run("swatch.delete", params).ok();
        return;
    }
    let message = match names.as_slice() {
        [n] => format!("Delete “{n}”?"),
        _ => format!("Delete these {} swatches and groups?", names.len()),
    };
    crate::dialogs::confirm::ask(app, &message, "Art using a deleted global swatch keeps its colour.", "swatch.delete", params);
}

fn bottom(app: &mut VectorcraftApp, ui: &mut Ui, sel: &[String]) {
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
        let opts = editable(app, sel);
        if widgets::icon_button_enabled(ui, "dc-options", "Swatch Options", false, opts.is_some(), 24.0).clicked()
            && let Some(n) = opts
        {
            app.run("ui.swatchOptions", json!({"name": n})).ok();
        }
        ui.add_space((ui.available_width() - 3.0 * 28.0).max(0.0));
        if widgets::icon_button(ui, "dc-folder", "New Color Group", false, 24.0).clicked() {
            app.run("swatch.newGroup", json!({"swatches": deletable(sel)})).ok();
        }
        // Ctrl/Cmd-click makes a spot colour (Alt-click skips the dialog).
        if widgets::icon_button(ui, "dc-new-item", "New Swatch", false, 24.0).clicked() {
            new_swatch(app, ui.input(|i| i.modifiers.command));
        }
        // Alt-click deletes without asking.
        let del = deletable(sel);
        if widgets::icon_button_enabled(ui, "trash-2", "Delete Swatch", false, !del.is_empty(), 24.0).clicked() {
            delete(app, del, ui.input(|i| i.modifiers.alt));
        }
    });
}

/// Save the active paint as a new swatch (a spot colour with `spot`).
fn new_swatch(app: &mut VectorcraftApp, spot: bool) {
    let mut params = super::paint_params(&active_paint(app));
    params["spot"] = json!(spot);
    app.run("swatch.new", params).ok();
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let view: View = pstate(ui.ctx(), "swatch-view");
    let selected = selection(app, ui);
    let del = deletable(&selected);
    let opts = editable(app, &selected);
    if menu_item(ui, "New Swatch…", true, false) {
        new_swatch(app, false);
    }
    if menu_item(ui, "New Color Group…", true, false) {
        app.run("swatch.newGroup", json!({"swatches": del})).ok();
    }
    if menu_item(ui, "Duplicate Swatch", opts.is_some(), false)
        && let Some(n) = &opts
    {
        app.run("swatch.duplicate", json!({"name": n})).ok();
    }
    menu_item(ui, "Merge Swatches", false, false);
    if menu_item(ui, "Delete Swatch", !del.is_empty(), false) {
        delete(app, del, false);
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
    if menu_item(ui, "Swatch Options…", opts.is_some(), false)
        && let Some(n) = opts
    {
        app.run("ui.swatchOptions", json!({"name": n})).ok();
    }
    menu_item(ui, "Spot Colors…", false, false);
    ui.separator();
    menu_item(ui, "Open Swatch Library", false, false);
    menu_item(ui, "Save Swatch Library…", false, false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogs::swatch_options::KIND;
    use egui::{Event, Modifiers, PointerButton, Pos2};
    use vectorcraft_color::Color;
    use vectorcraft_engine::Session;

    fn app() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(Session::new(), Default::default());
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app
    }

    /// One frame of `draw` on a persistent context (clicks need the previous frame's layout).
    fn frame(app: &mut VectorcraftApp, ctx: &egui::Context, events: Vec<Event>, time: f64, draw: fn(&mut VectorcraftApp, &mut Ui)) {
        frame_with(app, ctx, events, Modifiers::NONE, time, draw);
    }

    /// [`frame`] with `modifiers` held.
    fn frame_with(
        app: &mut VectorcraftApp,
        ctx: &egui::Context,
        events: Vec<Event>,
        modifiers: Modifiers,
        time: f64,
        draw: fn(&mut VectorcraftApp, &mut Ui),
    ) {
        let screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vec2(280.0, 700.0)));
        let events = std::iter::once(Event::ModifiersChanged(modifiers)).chain(events).collect();
        let input = egui::RawInput { events, time: Some(time), screen_rect, ..Default::default() };
        let mut out = ctx.run_ui(input, |ui| draw(app, ui));
        out.textures_delta.clear();
    }

    fn context() -> egui::Context {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        ctx
    }

    fn double_click(at: Pos2) -> Vec<Event> {
        let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Default::default() };
        vec![Event::PointerMoved(at), button(true), button(false), button(true), button(false)]
    }

    /// Double-click the thumbnail of swatch `name` in the panel.
    fn double_click_tile(app: &mut VectorcraftApp, ctx: &egui::Context, name: &str) {
        frame(app, ctx, vec![], 0.0, show);
        let tile = ctx.read_response(egui::Id::new(("swatch-tile", name))).unwrap_or_else(|| panic!("no tile for {name}")).rect;
        frame(app, ctx, double_click(tile.center()), 1.0, show);
    }

    /// Click the thumbnail of swatch or group `name` with `modifiers` held (frames at `time`, `time + 0.5`).
    fn click_tile(app: &mut VectorcraftApp, ctx: &egui::Context, name: &str, modifiers: Modifiers, time: f64) {
        frame(app, ctx, vec![], time, show);
        let at = ctx.read_response(egui::Id::new(("swatch-tile", name))).unwrap_or_else(|| panic!("no tile for {name}")).rect.center();
        let button = |pressed| Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers };
        frame_with(app, ctx, vec![Event::PointerMoved(at), button(true), button(false)], modifiers, time + 0.5, show);
    }

    fn selected(ctx: &egui::Context) -> Vec<String> {
        pstate(ctx, "swatch-selected")
    }

    fn dialog_frame(app: &mut VectorcraftApp, ctx: &egui::Context) {
        frame(app, ctx, vec![], 2.0, |app, ui| crate::dialogs::show(app, ui.ctx()));
    }

    #[test]
    fn double_click_opens_swatch_options_and_ok_edits_linked_art() {
        let mut app = app();
        app.run("swatch.edit", json!({"name": "Red", "global": true})).unwrap();
        let id = app.run("shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50})).unwrap()["id"].clone();
        app.run("paint.setFill", json!({"ids": [id], "swatch": "Red"})).unwrap();
        let ctx = context();
        double_click_tile(&mut app, &ctx, "Red");
        let d = app.ui.dialog.as_ref().expect("double-click opens Swatch Options");
        assert_eq!((d.kind.as_str(), d.str("__swatch").as_str(), d.str("mode").as_str()), (KIND, "Red", "rgb"));
        // The dialog previews live on the canvas; OK keeps the edit as one undo step.
        let undo = app.session.doc().unwrap().history.undo.len();
        dialog_frame(&mut app, &ctx);
        assert!(app.session.in_interaction(), "Preview runs as an interaction");
        let d = app.ui.dialog.as_mut().unwrap();
        d.fields.insert("color".into(), json!("#00ff00"));
        d.fields.insert("name".into(), json!("Signal"));
        crate::dialogs::confirm(&mut app).unwrap();
        assert!(app.ui.dialog.is_none() && !app.session.in_interaction());
        let doc = &app.session.doc().unwrap().doc;
        assert_eq!(doc.swatch("Signal").and_then(|w| w.paint.color()).map(|c| c.to_hex()), Some("#00ff00".into()));
        let fill = doc.node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap().appearance.fill_paint();
        assert_eq!(fill, Paint::Solid { color: Color::from_hex("#00ff00").unwrap(), swatch: Some("Signal".into()) });
        assert_eq!(app.session.doc().unwrap().history.undo.len(), undo + 1);
    }

    #[test]
    fn cancel_rolls_the_preview_back_and_other_kinds_open_their_editors() {
        let mut app = app();
        let ctx = context();
        let before = app.session.doc().unwrap().doc.clone();
        app.run("ui.swatchOptions", json!({"name": "Bright Blue"})).unwrap();
        app.ui.dialog.as_mut().unwrap().fields.insert("color".into(), json!("#000000"));
        dialog_frame(&mut app, &ctx);
        assert_ne!(app.session.doc().unwrap().doc, before, "previewed");
        crate::dialogs::cancel(&mut app);
        assert!(app.ui.dialog.is_none() && !app.session.in_interaction());
        assert_eq!(app.session.doc().unwrap().doc, before);
        // Gradients open the Gradient panel, patterns pattern editing, None nothing.
        double_click_tile(&mut app, &ctx, "Sunset");
        assert!(app.ui.dialog.is_none());
        assert_eq!(app.ui.open_panel.as_deref(), Some("gradient"));
        assert!(app.run("ui.swatchOptions", json!({"name": "[None]"})).is_err());
        assert!(app.run("ui.swatchOptions", json!({})).is_err());
    }

    #[test]
    fn modifier_clicks_select_several_swatches_and_groups() {
        let mut app = app();
        let ctx = context();
        click_tile(&mut app, &ctx, "Red", Modifiers::NONE, 0.0);
        assert_eq!(selected(&ctx), ["Red"]);
        assert_eq!(app.session.paint.fill.color().map(|c| c.to_hex()), Some("#ed1c24".into()), "a plain click applies");
        click_tile(&mut app, &ctx, "Amber", Modifiers::SHIFT, 2.0);
        assert_eq!(selected(&ctx), ["Red", "Orange Red", "Orange", "Amber"], "Shift extends over the shown order");
        click_tile(&mut app, &ctx, "Orange", Modifiers::COMMAND, 4.0);
        click_tile(&mut app, &ctx, "Brights", Modifiers::COMMAND, 6.0);
        assert_eq!(selected(&ctx), ["Red", "Orange Red", "Amber", "Brights"], "Cmd toggles swatches and colour groups");
        assert_eq!(app.session.paint.fill.color().map(|c| c.to_hex()), Some("#ed1c24".into()), "modifier clicks only select");
        click_tile(&mut app, &ctx, "Grays", Modifiers::NONE, 8.0);
        assert_eq!(selected(&ctx), ["Grays"], "a plain click on a group selects it alone");
    }

    #[test]
    fn delete_asks_then_removes_the_selection_and_unlinks_art() {
        let mut app = app();
        let ctx = context();
        app.run("swatch.edit", json!({"name": "Red", "global": true})).unwrap();
        let id = app.run("shape.rectangle", json!({"x": 0, "y": 0, "width": 50, "height": 50})).unwrap()["id"].clone();
        app.run("paint.setFill", json!({"ids": [id], "swatch": "Red"})).unwrap();
        delete(&mut app, vec!["Red".into(), "Brights".into()], false);
        let d = app.ui.dialog.as_ref().expect("delete asks first");
        assert_eq!((d.kind.as_str(), d.str("message").as_str()), (crate::dialogs::confirm::KIND, "Delete these 2 swatches and groups?"));
        dialog_frame(&mut app, &ctx);
        assert!(app.session.doc().unwrap().doc.swatch("Red").is_some(), "nothing is deleted before OK");
        crate::dialogs::confirm(&mut app).unwrap();
        assert!(app.ui.dialog.is_none());
        let doc = &app.session.doc().unwrap().doc;
        assert!(doc.swatch("Red").is_none() && doc.swatch("Bright Red").is_none() && doc.swatch_groups.iter().all(|g| g.name != "Brights"));
        let fill = doc.node(vectorcraft_doc::NodeId(id.as_u64().unwrap())).unwrap().appearance.fill_paint();
        assert_eq!(fill, Paint::solid(Color::from_hex("#ed1c24").unwrap()), "the art keeps its colour, unlinked");
        // Cancel keeps the swatch; Alt-click (now) deletes without asking.
        delete(&mut app, vec!["Orange".into()], false);
        assert_eq!(app.ui.dialog.as_ref().unwrap().str("message"), "Delete “Orange”?");
        crate::dialogs::cancel(&mut app);
        assert!(app.session.doc().unwrap().doc.swatch("Orange").is_some());
        delete(&mut app, vec!["Orange".into()], true);
        assert!(app.ui.dialog.is_none() && app.session.doc().unwrap().doc.swatch("Orange").is_none());
        assert_eq!(deletable(&["[None]".into(), REGISTRATION.into(), "Grays".into()]), ["Grays"]);
    }

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
