//! Embroidery panel: stitch type and parameters of the selected objects, thread,
//! stitch direction (automatic or edited inclination lines), start/stop beads,
//! design information, hoop, fabric and how stitches are shown.

use egui::{Color32, Ui};
use serde_json::{Value, json};
use vectorcraft_embroidery::sf_core::attr::{Embroidery, StitchType};
use vectorcraft_embroidery::sf_core::brush::WidthProfile;
use vectorcraft_embroidery::sf_core::model::{
    Corners, CoverKind, CrossStyle, CustomMotif, FillKind, FillParams, JaggedSides, MAX_PASSES, Motif, MotifColumn, MotifWarp, RunKind, SatinParams,
    TATAMI_LENGTH, TackdownKind, TexturePattern, TieStyle, WarpKind,
};
use vectorcraft_embroidery::sf_core::threads::{Rgb, charts};

use super::{ArtView, StitchView};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets::{dropdown, flat_button, label_row, plain_field, section_header};

/// A flat button sized to its (translated) label.
pub(crate) fn button(ui: &mut Ui, text: &str) -> egui::Response {
    let w = ui.painter().layout_no_wrap(tl!(text).to_string(), egui::FontId::proportional(12.5), egui::Color32::WHITE).size().x;
    flat_button(ui, text, (w + 20.0).min(ui.available_width().max(40.0)))
}

pub const ID: &str = "embroidery";

const LABEL_W: f32 = 112.0;
const FIELD_W: f32 = 74.0;

/// Stitch type menu: (label, `embroidery.set` type).
const TYPES: &[(&str, &str)] = &[
    ("No Stitches", "none"),
    ("Run", "run"),
    ("Bean Run", "bean"),
    ("Backstitch", "backstitch"),
    ("Stem Stitch", "stemstitch"),
    ("Blanket", "blanket"),
    ("Redwork", "redwork"),
    ("Satin", "satin"),
    ("Tatami Fill", "fill"),
    ("Satin Fill", "satinfill"),
    ("Contour Fill", "contour"),
    ("Spiral Fill", "spiral"),
    ("Radial Fill", "radial"),
    ("Cross Fill", "cross"),
    ("Meander Fill", "meander"),
    ("Motif Fill", "motiffill"),
    ("Appliqué", "applique"),
    ("Manual Stitches", "manual"),
];

fn type_index(e: Option<&Embroidery>) -> usize {
    let key = match e.map(|e| &e.stitch) {
        None | Some(StitchType::Branch) => "none",
        Some(st) => st.type_name(),
    };
    TYPES.iter().position(|t| t.1 == key).unwrap_or(0)
}

/// The first selected object that can be stitched (groups: their first path; art in a redwork
/// group: the group), its id and embroidery record.
fn first_stitchable(app: &VectorcraftApp) -> Option<(u64, Option<Embroidery>, bool)> {
    let st = app.session.active()?;
    let d = &st.doc;
    fn find(d: &vectorcraft_doc::Document, id: vectorcraft_doc::NodeId) -> Option<vectorcraft_doc::NodeId> {
        let n = d.node(id)?;
        if vectorcraft_embroidery::stitchable(n) || vectorcraft_embroidery::sews_whole(n) {
            return Some(id);
        }
        n.children()?.iter().find_map(|c| find(d, c.id))
    }
    // the outermost redwork group holding `id`, else `id`
    let owner = |id: vectorcraft_doc::NodeId| {
        let mut out = id;
        let mut at = d.parent_of(id);
        while let Some(p) = at {
            if d.node(p).is_some_and(vectorcraft_embroidery::sews_whole) {
                out = p;
            }
            at = d.parent_of(p);
        }
        out
    };
    st.selection.objects.iter().find_map(|id| find(d, *id)).map(owner).and_then(|id| {
        let n = d.node(id)?;
        Some((id.0, n.embroidery.as_deref().cloned(), vectorcraft_embroidery::is_line_art(n)))
    })
}

fn num(ui: &mut Ui, label: &str, id: &str, value: f64, suffix: &str, decimals: usize) -> Option<f64> {
    let mut out = None;
    label_row(ui, label, LABEL_W, |ui| {
        out = plain_field(ui, ("emb", id), value, suffix, decimals, FIELD_W);
    });
    out
}

fn check(ui: &mut Ui, label: &str, value: bool) -> Option<bool> {
    let mut v = value;
    ui.checkbox(&mut v, label).changed().then_some(v)
}

fn pick(ui: &mut Ui, label: &str, id: &str, current: usize, options: &[&str]) -> Option<usize> {
    let mut out = None;
    label_row(ui, label, LABEL_W, |ui| {
        let cur = options.get(current).copied().unwrap_or_default();
        out = dropdown(ui, ("emb", id), cur, options, 130.0).filter(|i| *i != current);
    });
    out
}

/// [`pick`] for names that are data (styles, fabrics, hoops): shown as they are, never
/// translated.
fn pick_names(ui: &mut Ui, label: &str, id: &str, current: usize, options: &[&str]) -> Option<usize> {
    let mut out = None;
    label_row(ui, label, LABEL_W, |ui| {
        let cur = options.get(current).copied().unwrap_or_default();
        out = crate::widgets::dropdown_names(ui, ("emb", id), cur, options, 130.0).filter(|i| *i != current);
    });
    out
}

/// Whose stitch properties the rows edit: the selected objects (the first one's id), or
/// those new objects of a stitch type take.
#[derive(Clone, Copy)]
enum Target<'a> {
    Selection(u64),
    Current(&'a str),
}

/// Sends a parameter change for the target.
fn set(app: &mut VectorcraftApp, target: Target, params: Value) {
    match target {
        Target::Selection(_) => {
            app.run("embroidery.setParams", json!({ "params": params })).ok();
        }
        Target::Current(ty) => {
            // a run style change files the properties under that style's type
            if let Ok(r) = app.run("embroidery.current.setParams", json!({ "type": ty, "params": params }))
                && let Some(t) = r["type"].as_str()
            {
                app.embroidery.new_type = t.to_string();
            }
        }
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if app.session.active().is_none() {
        crate::panels::empty_state(ui, "waves", tl!("No document"), tl!("Open or create a document to digitize embroidery."));
        return;
    }
    digitize_row(app, ui);
    buttonhole_rows(app, ui);
    ui.add_space(6.0);
    object_section(app, ui);
    ui.add_space(6.0);
    design_section(app, ui, &t);
}

/// Buttonholes: one, or a row of them evenly spaced, in the middle of the first artboard.
fn buttonhole_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    egui::CollapsingHeader::new(tl!("Buttonholes")).id_salt("emb-buttonholes").show(ui, |ui| {
        let mut b = app.embroidery.buttonholes;
        let styles = [tl!("Straight"), tl!("Rounded"), tl!("Keyhole")];
        if let Some(i) = pick(ui, tl!("Style"), "bh-style", b.style, &styles) {
            b.style = i.min(2);
        }
        if let Some(v) = num(ui, tl!("Length"), "bh-length", b.length, " mm", 1) {
            b.length = v.clamp(3.0, 80.0);
        }
        if let Some(v) = num(ui, tl!("Width"), "bh-width", b.width, " mm", 1) {
            b.width = v.clamp(1.5, 12.0);
        }
        if let Some(v) = num(ui, tl!("Count"), "bh-count", f64::from(b.count), "", 0) {
            b.count = v.round().clamp(1.0, 50.0) as u32;
        }
        if b.count > 1
            && let Some(v) = num(ui, tl!("Spacing"), "bh-spacing", b.spacing, " mm", 1)
        {
            b.spacing = v.clamp(1.0, 500.0);
        }
        if let Some(v) = num(ui, tl!("Angle"), "bh-angle", b.angle, "°", 0) {
            b.angle = v.clamp(-360.0, 360.0);
        }
        app.embroidery.buttonholes = b;
        if button(ui, tl!("Add Buttonholes")).clicked() {
            let centre = app.session.active().and_then(|st| st.doc.artboards.first().map(|a| a.rect.center()));
            app.run("embroidery.addButtonholes", b.params(centre.unwrap_or_default())).ok();
        }
    });
}

/// Whether the selection holds an image (Auto-Digitize traces it).
fn selected_image(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| {
        st.selection.objects.iter().any(|id| st.doc.node(*id).is_some_and(|n| matches!(n.kind, vectorcraft_engine::doc::NodeKind::Image(_))))
    })
}

/// Auto-Digitize for a selected image: how many thread colours, then trace and stitch.
fn auto_digitize_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    ui.label(tl!("Turn the image into stitches: narrow parts become satin, the rest tatami fill."));
    ui.add_space(4.0);
    let colors = app.embroidery.digitize_colors;
    if let Some(v) = num(ui, tl!("Thread colors"), "digitize-colors", f64::from(colors), "", 0) {
        app.embroidery.digitize_colors = v.round().clamp(2.0, 16.0) as u32;
    }
    if let Some(v) = check(ui, tl!("Remove background"), app.embroidery.digitize_background) {
        app.embroidery.digitize_background = v;
    }
    ui.add_space(4.0);
    if button(ui, tl!("Auto-Digitize Image")).clicked() {
        let params = json!({ "colors": app.embroidery.digitize_colors, "removeBackground": app.embroidery.digitize_background });
        app.run("embroidery.autoDigitize", params).ok();
    }
    if crate::embroidery::prompt::available(app)
        && button(ui, tl!("Redraw with Claude…"))
            .on_hover_text(tl!("Claude redraws the image as clean flat-colour art, then StitchFoundry digitizes it"))
            .clicked()
    {
        app.run("ui.redrawImage", json!({ "colors": app.embroidery.digitize_colors })).ok();
    }
}

/// The Motif tool's id.
const MOTIF_TOOL: &str = "embroideryMotif";

/// Freehand digitizing: the radial widget and the embroidery brush.
fn digitize_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let open = app.embroidery.widget.open;
    let label = if open { tl!("Hide Digitizing Widget") } else { tl!("Digitizing Widget") };
    if button(ui, label).on_hover_text(tl!("Pick a brush, width, density and shape from a dial, then draw on the canvas")).clicked() {
        app.embroidery.widget.open = !open;
        if !open {
            crate::embroidery::widget::load(app);
        }
    }
    if crate::embroidery::prompt::available(app)
        && button(ui, tl!("Design from Prompt…"))
            .on_hover_text(tl!("Describe a design and Claude draws artwork for it, then StitchFoundry digitizes it"))
            .clicked()
    {
        app.run("ui.designFromPrompt", json!({})).ok();
    }
    // the Motif tool: the motif a click or a drag places
    if app.session.tool_id() == MOTIF_TOOL {
        let ours: Vec<String> = app
            .session
            .active()
            .and_then(|st| st.doc.embroidery.as_deref().map(|c| c.motifs.iter().map(|m| m.name.clone()).collect()))
            .unwrap_or_default();
        let library = with_kept(app, ours);
        let chosen = app.session.tool_options_of(MOTIF_TOOL)["motif"].as_str().unwrap_or("Wave").to_string();
        let built = Motif::ALL.into_iter().find(|m| m.label().eq_ignore_ascii_case(&chosen));
        let custom = built.is_none().then(|| CustomMotif { name: chosen.clone(), ..CustomMotif::default() });
        if let Some(choice) = pick_motif(ui, &library, built.unwrap_or_default(), custom.as_ref()) {
            let name = match choice {
                MotifChoice::BuiltIn(m) => m.label().to_string(),
                MotifChoice::Design(name) => name,
            };
            app.run("tool.setOption", json!({ "tool": MOTIF_TOOL, "values": { "motif": name } })).ok();
        }
    }
    if !open {
        return;
    }
    // Digitize Before / After: where new strokes go in the sewing order.
    let orders = [("top", tl!("Sewn last")), ("before", tl!("Before the selection")), ("after", tl!("After the selection"))];
    let labels: Vec<&str> = orders.iter().map(|o| o.1).collect();
    let cur = orders.iter().position(|o| o.0 == app.embroidery.widget.order).unwrap_or(0);
    if let Some(i) = pick(ui, tl!("New strokes"), "order", cur, &labels)
        && let Some(o) = orders.get(i)
    {
        app.embroidery.widget.order = o.0.to_string();
        crate::embroidery::widget::load(app);
    }
    // Widget preferences: the values each ring offers.
    egui::CollapsingHeader::new(tl!("Widget values")).id_salt("emb-widget-values").show(ui, |ui| {
        let w = &mut app.embroidery.widget;
        for (label, id, list, max) in [
            (tl!("Widths (mm)"), "widths", &mut w.widths, 50.0),
            (tl!("Densities (mm)"), "densities", &mut w.densities, 10.0),
            (tl!("Lengths (mm)"), "lengths", &mut w.lengths, 12.0),
        ] {
            label_row(ui, label, LABEL_W, |ui| {
                let key = egui::Id::new(("emb-widget-list", id));
                let mut text = ui.data(|d| d.get_temp::<String>(key)).unwrap_or_else(|| crate::embroidery::widget::format_values(list));
                let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(FIELD_W * 2.0));
                if resp.lost_focus() {
                    if let Some(v) = crate::embroidery::widget::parse_values(&text, max) {
                        *list = v;
                    }
                    ui.data_mut(|d| d.remove::<String>(key));
                } else if resp.has_focus() {
                    ui.data_mut(|d| d.insert_temp(key, text));
                }
            });
        }
    });
}

/// Whether a branch group (objects sewn as one path) is selected.
fn branch_selected(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| {
        st.selection.objects.iter().filter_map(|id| st.doc.node(*id)).any(|n| {
            matches!(n.kind, vectorcraft_doc::NodeKind::Group { .. })
                && n.embroidery.as_deref().is_some_and(|e| matches!(e.stitch, StitchType::Branch))
        })
    })
}

fn object_section(app: &mut VectorcraftApp, ui: &mut Ui) {
    section_header(ui, tl!("Stitches"));
    if branch_selected(app) {
        ui.label(tl!("These objects are sewn as one path (a branch)."));
        if button(ui, tl!("Break Apart")).on_hover_text(tl!("Sew the objects separately again")).clicked() {
            app.run("embroidery.breakApart", json!({})).ok();
        }
        ui.add_space(4.0);
    }
    if selected_image(app) {
        auto_digitize_rows(app, ui);
        return;
    }
    let Some((id, emb, open)) = first_stitchable(app) else {
        ui.label(tl!("Select a path, shape or compound path to give it a stitch type."));
        ui.add_space(4.0);
        if button(ui, tl!("Convert Artwork to Embroidery")).clicked() {
            app.run("embroidery.convert", json!({})).ok();
        }
        ui.add_space(6.0);
        current_section(app, ui);
        return;
    };
    let labels: Vec<&str> = TYPES.iter().map(|t| tl!(t.0)).collect();
    if let Some(i) = pick(ui, tl!("Stitch type"), "type", type_index(emb.as_ref()), &labels)
        && let Some((_, ty)) = TYPES.get(i)
    {
        app.run("embroidery.set", json!({ "type": ty })).ok();
    }
    let Some(e) = emb else {
        ui.add_space(4.0);
        if button(ui, tl!("Convert Artwork to Embroidery")).clicked() {
            app.run("embroidery.convert", json!({})).ok();
        }
        return;
    };
    thread_row(app, ui, &e);
    style_row(app, ui, &e);
    props_rows(app, ui, Target::Selection(id), &e, open);
}

/// The style the object follows (pick one to apply it), and making its properties the
/// ones new objects take.
fn style_row(app: &mut VectorcraftApp, ui: &mut Ui, e: &Embroidery) {
    let styles: Vec<(String, bool)> = app
        .session
        .active()
        .and_then(|st| st.doc.embroidery.as_deref().map(|c| c.styles.iter().map(|s| (s.name.clone(), s.properties != e.properties())).collect()))
        .unwrap_or_default();
    if !styles.is_empty() {
        let cur = e.style.as_ref().and_then(|n| styles.iter().position(|s| s.0 == *n)).map_or(0, |i| i + 1);
        let mut opts: Vec<String> = vec![tl!("None").to_string()];
        opts.extend(styles.iter().map(|(n, changed)| if *changed && Some(n) == e.style.as_ref() { format!("{n}+") } else { n.clone() }));
        let refs: Vec<&str> = opts.iter().map(String::as_str).collect();
        if let Some(i) = pick_names(ui, tl!("Style"), "style", cur, &refs) {
            if i == 0 {
                app.run("embroidery.style.unlink", json!({})).ok();
            } else if let Some((name, _)) = styles.get(i - 1) {
                app.run("embroidery.style.apply", json!({ "name": name })).ok();
            }
        }
    }
    if !matches!(e.stitch, StitchType::Manual | StitchType::Branch)
        && button(ui, tl!("Make Current")).on_hover_text(tl!("New objects of this stitch type take these properties")).clicked()
    {
        app.run("embroidery.makeCurrent", json!({})).ok();
    }
}

/// With nothing selected: the properties new objects of a stitch type take.
fn current_section(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Some(cfg) = app.session.active().map(|st| st.doc.embroidery.as_deref().cloned().unwrap_or_default()) else { return };
    egui::CollapsingHeader::new(tl!("Properties for New Objects")).id_salt("emb-current").default_open(true).show(ui, |ui| {
        ui.label(tl!("Objects you give a stitch type take these properties."));
        let types: Vec<(&str, &str)> = TYPES.iter().copied().filter(|t| !matches!(t.1, "none" | "manual")).collect();
        let labels: Vec<&str> = types.iter().map(|t| tl!(t.0)).collect();
        let cur = types.iter().position(|t| t.1 == app.embroidery.new_type).unwrap_or(0);
        if let Some(i) = pick(ui, tl!("Stitch type"), "newtype", cur, &labels)
            && let Some(t) = types.get(i)
        {
            app.embroidery.new_type = t.1.to_string();
        }
        let key = types.get(types.iter().position(|t| t.1 == app.embroidery.new_type).unwrap_or(0)).map_or("fill", |t| t.1);
        let Some(e) = cfg.current_for(key).cloned().or_else(|| StitchType::from_name(key).map(Embroidery::new)) else { return };
        props_rows(app, ui, Target::Current(key), &e, true);
        if cfg.current.contains_key(key) && button(ui, tl!("Reset to Defaults")).clicked() {
            app.run("embroidery.current.reset", json!({ "type": key })).ok();
        }
    });
}

/// The stitch properties of `e`: for the selected objects (`open`: a line, sewn as a satin
/// line) or for new objects.
fn props_rows(app: &mut VectorcraftApp, ui: &mut Ui, target: Target, e: &Embroidery, open: bool) {
    match &e.stitch {
        StitchType::Run(r) => {
            let kinds = [
                tl!("Single"),
                tl!("Triple"),
                tl!("Blanket"),
                tl!("Zigzag"),
                tl!("Motif"),
                tl!("Backstitch"),
                tl!("Stem stitch"),
                tl!("Hand stitch"),
                tl!("Sculpture run"),
                tl!("String stitch"),
            ];
            let cur = RunKind::ALL.iter().position(|k| *k == r.kind).unwrap_or(0);
            if let Some(i) = pick(ui, tl!("Run style"), "runkind", cur, &kinds)
                && let Some(k) = RunKind::ALL.get(i)
            {
                // string stitching starts with long strings close together
                let params = if *k == RunKind::String { json!({ "kind": k, "width": 8.0, "spacing": 1.0 }) } else { json!({ "kind": k }) };
                set(app, target, params);
            }
            // a sculpture run's stitch length is its triple stitches'
            let length = if r.kind == RunKind::Sculpture { tl!("Triple stitch length") } else { tl!("Stitch length") };
            if let Some(v) = num(ui, length, "len", r.stitch_length, " mm", 2) {
                set(app, target, json!({ "stitch_length": v }));
            }
            // a string stitch's width is how much of the line each string spans
            let width = if r.kind == RunKind::String { tl!("Chord length") } else { tl!("Width") };
            if matches!(r.kind, RunKind::Blanket | RunKind::Zigzag | RunKind::Motif | RunKind::Stem | RunKind::String)
                && let Some(v) = num(ui, width, "rw", r.width, " mm", 2)
            {
                set(app, target, json!({ "width": v }));
            }
            // a hand stitch's spacing is the gap between its stitches, a sculpture run's the length
            // of its single stitches
            let spacing = match r.kind {
                RunKind::Hand => tl!("Gap"),
                RunKind::Sculpture => tl!("Single stitch length"),
                _ => tl!("Spacing"),
            };
            if matches!(r.kind, RunKind::Blanket | RunKind::Zigzag | RunKind::Motif | RunKind::Hand | RunKind::Sculpture | RunKind::String)
                && let Some(v) = num(ui, spacing, "rs", r.spacing, " mm", 2)
            {
                set(app, target, json!({ "spacing": v }));
            }
            if r.kind == RunKind::Sculpture {
                let [triple, single] = r.groups;
                if let Some(v) = num(ui, tl!("Triple stitches"), "rgt", f64::from(triple), "", 0) {
                    set(app, target, json!({ "groups": [v.round().clamp(0.0, 20.0) as u32, single] }));
                }
                if let Some(v) = num(ui, tl!("Single stitches"), "rgs", f64::from(single), "", 0) {
                    set(app, target, json!({ "groups": [triple, v.round().clamp(0.0, 20.0) as u32] }));
                }
            }
            if matches!(r.kind, RunKind::Blanket | RunKind::Motif | RunKind::Stem)
                && let Some(v) = check(ui, tl!("Flip to the other side"), r.flip)
            {
                set(app, target, json!({ "flip": v }));
            }
            let passes = num(ui, tl!("Passes"), "passes", f64::from(r.passes), "", 0);
            if let Some(v) = passes {
                set(app, target, json!({ "passes": v.round().clamp(1.0, f64::from(MAX_PASSES)) as u32 }));
            }
            if r.kind == RunKind::Motif {
                motif_rows(app, ui, target, r.motif, r.custom_motif.as_ref());
                // Graded: the motifs grow or shrink to another width along the line.
                if let Some(v) = check(ui, tl!("Graded"), r.width_end.is_some()) {
                    set(app, target, json!({ "width_end": if v { json!(r.width * 2.0) } else { Value::Null } }));
                }
                if let Some(w) = r.width_end
                    && let Some(v) = num(ui, tl!("Width at end"), "rwend", w, " mm", 2)
                {
                    set(app, target, json!({ "width_end": v.clamp(0.5, 50.0) }));
                }
            }
        }
        StitchType::Satin(s) => {
            if open {
                let w = e.width.unwrap_or(3.0);
                if let Some(v) = num(ui, tl!("Width"), "sw", w, " mm", 2) {
                    set(app, target, json!({ "width": v }));
                }
                let profiles = [tl!("Constant"), tl!("Tapered"), tl!("Calligraphy")];
                let cur = match e.profile {
                    WidthProfile::Constant | WidthProfile::Pressure(_) => 0,
                    WidthProfile::Tapered => 1,
                    WidthProfile::Calligraphy { .. } => 2,
                };
                if let Some(i) = pick(ui, tl!("Width profile"), "profile", cur, &profiles) {
                    let p = match i {
                        1 => json!("Tapered"),
                        2 => json!({ "Calligraphy": { "nib_angle": 145.0 } }),
                        _ => json!("Constant"),
                    };
                    set(app, target, json!({ "profile": p }));
                }
                let corners = [tl!("Automatic"), tl!("Turn"), tl!("Miter"), tl!("Capped")];
                let cur = Corners::ALL.iter().position(|c| *c == s.corners).unwrap_or(0);
                if let Some(i) = pick(ui, tl!("Corners"), "corners", cur, &corners)
                    && let Some(c) = Corners::ALL.get(i)
                {
                    set(app, target, json!({ "corners": c }));
                }
            }
            // a motif column: motifs as wide as the column instead of satin
            let column = s.motif_column.as_deref();
            let sew_as = [tl!("Satin"), tl!("Motifs")];
            if let Some(i) = pick(ui, tl!("Sew as"), "sewas", usize::from(column.is_some()), &sew_as) {
                set(app, target, json!({ "motif_column": if i == 1 { json!(MotifColumn::default()) } else { Value::Null } }));
            }
            if let Some(m) = column {
                let names: Vec<&str> = Motif::ALL.iter().map(|x| motif_name(*x)).collect();
                let cur = Motif::ALL.iter().position(|x| *x == m.motif).unwrap_or(0);
                if let Some(i) = pick(ui, tl!("Motif"), "colmotif", cur, &names)
                    && let Some(x) = Motif::ALL.get(i)
                {
                    set(app, target, json!({ "motif_column": { "motif": x, "custom_motif": null } }));
                }
                if let Some(v) = num(ui, tl!("Motif length (× width)"), "collen", m.length, "", 2) {
                    set(app, target, json!({ "motif_column": { "length": v.clamp(0.2, 10.0) } }));
                }
            }
            if let Some(v) = num(ui, tl!("Density"), "sd", s.spacing, " mm", 2) {
                set(app, target, json!({ "spacing": v }));
            }
            let twin = s.pull_comp_2.is_some();
            let label = if twin { tl!("Pull comp., side 1") } else { tl!("Pull compensation") };
            if let Some(v) = num(ui, label, "spc", s.pull_comp, " mm", 2) {
                set(app, target, json!({ "pull_comp": v }));
            }
            if let Some(side2) = s.pull_comp_2
                && let Some(v) = num(ui, tl!("Pull comp., side 2"), "spc2", side2, " mm", 2)
            {
                set(app, target, json!({ "pull_comp_2": v }));
            }
            if let Some(v) = check(ui, tl!("Different on each side"), twin) {
                set(app, target, json!({ "pull_comp_2": if v { json!(s.pull_comp) } else { Value::Null } }));
            }
            // a tatami border: rows across the column split at the stitch length
            if let Some(v) = check(ui, tl!("Tatami stitches"), s.tatami) {
                let len = if v { TATAMI_LENGTH } else { SatinParams::default().split_length };
                set(app, target, json!({ "tatami": v, "split_length": len }));
            }
            let split = if s.tatami { tl!("Stitch length") } else { tl!("Auto split") };
            if let Some(v) = num(ui, split, "split", s.split_length, " mm", 1) {
                set(app, target, json!({ "split_length": v }));
            }
            let preserve = check(ui, tl!("Preserve long stitches"), s.preserve_long);
            if let Some(v) = preserve {
                set(app, target, json!({ "preserve_long": v }));
            }
            if let Some(v) = check(ui, tl!("Auto spacing"), s.auto_spacing) {
                set(app, target, json!({ "auto_spacing": v }));
            }
            if let Some(v) = check(ui, tl!("Short stitches on curves"), s.short_stitches) {
                set(app, target, json!({ "short_stitches": v }));
            }
            // Jagged edges: stitches end short of the edge at random (fur, grass, feathers).
            let sides = [tl!("None"), tl!("Both sides"), tl!("Side 1"), tl!("Side 2")];
            let cur = s.jagged.and_then(|j| JaggedSides::ALL.iter().position(|x| *x == j.sides)).map_or(0, |i| i + 1);
            if let Some(i) = pick(ui, tl!("Jagged edges"), "jagged", cur, &sides) {
                let depth = s.jagged.unwrap_or_default().depth;
                let v = i.checked_sub(1).and_then(|k| JaggedSides::ALL.get(k)).map_or(Value::Null, |side| json!({ "sides": side, "depth": depth }));
                set(app, target, json!({ "jagged": v }));
            }
            if let Some(j) = s.jagged
                && let Some(v) = num(ui, tl!("Jag depth"), "jagdepth", j.depth, " mm", 1)
            {
                set(app, target, json!({ "jagged": { "depth": v.clamp(0.0, 20.0) } }));
            }
            let u = &s.underlay;
            if let Some(v) = check(ui, tl!("Automatic underlay"), u.auto) {
                set(app, target, json!({ "underlay": { "auto": v } }));
            }
            if !u.auto {
                ui.horizontal(|ui| {
                    for (label, key, val) in [
                        (tl!("Centre run"), "center_run", u.center_run),
                        (tl!("Edge run"), "edge_run", u.edge_run),
                        (tl!("Zigzag"), "zigzag", u.zigzag),
                    ] {
                        if let Some(v) = check(ui, label, val) {
                            set(app, target, json!({ "underlay": { key: v } }));
                        }
                    }
                });
                if u.zigzag
                    && let Some(v) = check(ui, tl!("Double zigzag"), u.double_zigzag)
                {
                    set(app, target, json!({ "underlay": { "double_zigzag": v } }));
                }
            }
            if !open && let Target::Selection(id) = target {
                direction_rows(app, ui, id, e);
            }
        }
        StitchType::Fill(f) => {
            let kinds: Vec<&str> = FillKind::ALL.iter().map(|k| tl!(k.label())).collect();
            let cur = FillKind::ALL.iter().position(|k| *k == f.kind).unwrap_or(0);
            if let Some(i) = pick(ui, tl!("Fill style"), "fillkind", cur, &kinds)
                && let Some(k) = FillKind::ALL.get(i)
            {
                set(app, target, json!({ "kind": k }));
            }
            let (spacing_label, decimals) = match f.kind {
                FillKind::Meander => (tl!("Path spacing"), 1),
                FillKind::Motif => (tl!("Row spacing"), 1),
                _ => (tl!("Density"), 2),
            };
            if let Some(v) = num(ui, spacing_label, "fd", f.spacing, " mm", decimals) {
                set(app, target, json!({ "spacing": v }));
            }
            if !matches!(f.kind, FillKind::Satin | FillKind::Radial) {
                let label = if f.kind == FillKind::Cross { tl!("Cell size") } else { tl!("Stitch length") };
                if let Some(v) = num(ui, label, "fl", f.stitch_length, " mm", 2) {
                    set(app, target, json!({ "stitch_length": v }));
                }
            }
            if f.kind == FillKind::Cross {
                let styles = [tl!("Full cross"), tl!("Half cross"), tl!("Upright cross"), tl!("Double cross")];
                let cur = CrossStyle::ALL.iter().position(|c| *c == f.cross).unwrap_or(0);
                if let Some(i) = pick(ui, tl!("Cross style"), "fcross", cur, &styles)
                    && let Some(c) = CrossStyle::ALL.get(i)
                {
                    set(app, target, json!({ "cross": c }));
                }
            }
            // an offset fill: only so many rings in from the outline
            if f.kind == FillKind::Contour
                && let Some(v) = num(ui, tl!("Rings (0: all)"), "frings", f64::from(f.rings), "", 0)
            {
                set(app, target, json!({ "rings": v.round().clamp(0.0, 500.0) as u32 }));
            }
            if f.kind == FillKind::Motif {
                motif_rows(app, ui, target, f.motif, f.custom_motif.as_ref());
                // a paired motif fill: a second motif in turn with the first along the rows
                let mut pairs = vec![tl!("None")];
                pairs.extend(Motif::ALL.iter().map(|m| motif_name(*m)));
                let cur = f.motif_pair.and_then(|m| Motif::ALL.iter().position(|x| *x == m)).map_or(0, |i| i + 1);
                if let Some(i) = pick(ui, tl!("Paired with"), "fpair", cur, &pairs) {
                    let v = i.checked_sub(1).and_then(|k| Motif::ALL.get(k)).map_or(Value::Null, |m| json!(m));
                    set(app, target, json!({ "motif_pair": v }));
                }
                if let Some(v) = num(ui, tl!("Motif size"), "fms", f.motif_size, " mm", 1) {
                    set(app, target, json!({ "motif_size": v.clamp(1.0, 50.0) }));
                }
                if let Some(v) = num(ui, tl!("Row offset"), "fmo", f.offset, "", 2) {
                    set(app, target, json!({ "offset": v }));
                }
                // a 3D motif fill: sculpted as a dome or a bowl, or receding
                let warps = [tl!("None"), tl!("Globe out"), tl!("Globe in"), tl!("Perspective")];
                let cur = f.warp.and_then(|w| WarpKind::ALL.iter().position(|k| *k == w.kind)).map_or(0, |i| i + 1);
                if let Some(i) = pick(ui, tl!("3D effect"), "fwarp", cur, &warps) {
                    let v = i
                        .checked_sub(1)
                        .and_then(|k| WarpKind::ALL.get(k))
                        .map_or(Value::Null, |k| json!(MotifWarp { kind: *k, ..f.warp.unwrap_or_default() }));
                    set(app, target, json!({ "warp": v }));
                }
                if let Some(w) = f.warp {
                    if let Some(v) = num(ui, tl!("Strength"), "fwamt", w.amount, "", 2) {
                        set(app, target, json!({ "warp": { "amount": v.clamp(0.0, 0.8) } }));
                    }
                    if w.kind == WarpKind::Perspective
                        && let Some(v) = num(ui, tl!("Direction"), "fwang", w.angle, "°", 0)
                    {
                        set(app, target, json!({ "warp": { "angle": v } }));
                    }
                }
            }
            if f.kind == FillKind::Tatami {
                if let Some(v) = num(ui, tl!("Row offset"), "fo", f.offset, "", 2) {
                    set(app, target, json!({ "offset": v }));
                }
                // a spacing profile replaces the gradient
                if f.spacing_profile.is_empty()
                    && let Some(v) = num(ui, tl!("Gradient to"), "fg", f.spacing_end, " mm", 2)
                {
                    set(app, target, json!({ "spacing_end": v }));
                }
                profile_rows(app, ui, target, f);
            }
            if matches!(f.kind, FillKind::Tatami | FillKind::Meander)
                && let Some(v) = num(ui, tl!("Randomness"), "fr", f.random, "", 2)
            {
                set(app, target, json!({ "random": v }));
            }
            if f.kind == FillKind::Tatami {
                texture_rows(app, ui, target, f, e);
                if let Some(v) = check(ui, tl!("Trapunto (travel along the outline)"), f.trapunto) {
                    set(app, target, json!({ "trapunto": v }));
                }
            }
            if f.kind == FillKind::Satin
                && let Some(v) = num(ui, tl!("Auto split"), "fsplit", f.split_length, " mm", 1)
            {
                set(app, target, json!({ "split_length": v }));
            }
            if let Some(v) = num(ui, tl!("Pull compensation"), "fpc", f.pull_comp, " mm", 2) {
                set(app, target, json!({ "pull_comp": v }));
            }
            // cross, meander and motif fills are sewn without underlay
            let u = &f.underlay;
            if !matches!(f.kind, FillKind::Cross | FillKind::Meander | FillKind::Motif) {
                ui.horizontal(|ui| {
                    if let Some(v) = check(ui, tl!("Edge run underlay"), u.edge_run) {
                        set(app, target, json!({ "underlay": { "edge_run": v } }));
                    }
                    if let Some(v) = check(ui, tl!("Tatami underlay"), u.tatami) {
                        set(app, target, json!({ "underlay": { "tatami": v } }));
                    }
                });
            }
            if matches!(f.kind, FillKind::Tatami | FillKind::Satin | FillKind::Cross)
                && let Target::Selection(id) = target
            {
                direction_rows(app, ui, id, e);
            }
        }
        StitchType::Applique(a) => {
            if let Some(v) = check(ui, tl!("Placement line and stop"), a.placement) {
                set(app, target, json!({ "placement": v }));
            }
            let tack = [tl!("None"), tl!("Run"), tl!("Zigzag"), tl!("Blanket")];
            let cur = [TackdownKind::None, TackdownKind::Run, TackdownKind::Zigzag, TackdownKind::Blanket]
                .iter()
                .position(|k| *k == a.tackdown)
                .unwrap_or(2);
            if let Some(i) = pick(ui, tl!("Tack-down"), "tack", cur, &tack) {
                let k = [TackdownKind::None, TackdownKind::Run, TackdownKind::Zigzag, TackdownKind::Blanket].get(i).copied().unwrap_or_default();
                set(app, target, json!({ "tackdown": k }));
            }
            if let Some(v) = check(ui, tl!("Stop to trim fabric"), a.trim_stop) {
                set(app, target, json!({ "trim_stop": v }));
            }
            let cover = [tl!("None"), tl!("Satin"), tl!("Blanket")];
            let cur = [CoverKind::None, CoverKind::Satin, CoverKind::Blanket].iter().position(|k| *k == a.cover).unwrap_or(1);
            if let Some(i) = pick(ui, tl!("Cover stitch"), "cover", cur, &cover) {
                let k = [CoverKind::None, CoverKind::Satin, CoverKind::Blanket].get(i).copied().unwrap_or_default();
                set(app, target, json!({ "cover": k }));
            }
            if let Some(v) = num(ui, tl!("Cover width"), "cw", a.cover_width, " mm", 2) {
                set(app, target, json!({ "cover_width": v }));
            }
            if let Some(v) = num(ui, tl!("Density"), "cd", a.cover_spacing, " mm", 2) {
                set(app, target, json!({ "cover_spacing": v }));
            }
        }
        StitchType::Manual => {
            ui.label(tl!("Each anchor point is a needle penetration."));
        }
        StitchType::Branch => {}
        StitchType::Redwork(r) => {
            // redwork lines are sewn in one of the outline stitches
            let kinds = [RunKind::Single, RunKind::Triple, RunKind::Back, RunKind::Stem];
            let names = [tl!("Single"), tl!("Triple"), tl!("Backstitch"), tl!("Stem stitch")];
            let cur = kinds.iter().position(|k| *k == r.kind).unwrap_or(0);
            if let Some(i) = pick(ui, tl!("Run style"), "rwkind", cur, &names)
                && let Some(k) = kinds.get(i)
            {
                set(app, target, json!({ "kind": k }));
            }
            if let Some(v) = num(ui, tl!("Stitch length"), "rwlen", r.stitch_length, " mm", 2) {
                set(app, target, json!({ "stitch_length": v }));
            }
            ui.label(tl!("Sewn as one line that ends where it starts; the way between lines is retraced along them."));
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if let Target::Selection(id) = target
            && button(ui, tl!("Reset Start and Stop")).on_hover_text(tl!("Let sewing start and stop at the closest points again")).clicked()
        {
            app.run("embroidery.setBeads", json!({ "id": id, "entry": null, "exit": null })).ok();
        }
        if let Some(v) = check(ui, tl!("Trim after"), e.trim_after) {
            set(app, target, json!({ "trim_after": v }));
        }
        if let Some(v) = check(ui, tl!("Stop after"), e.stop_after) {
            set(app, target, json!({ "stop_after": v }));
        }
        if let Some(v) = check(ui, tl!("Omit last stitch"), e.omit_last) {
            set(app, target, json!({ "omit_last": v }));
        }
    });
    // Tie-in and tie-off: the design's setting unless this object says otherwise.
    let ties = [tl!("Design setting"), tl!("On"), tl!("Off")];
    let index = |v: Option<bool>| match v {
        None => 0,
        Some(true) => 1,
        Some(false) => 2,
    };
    let value = |i: usize| match i {
        1 => json!(true),
        2 => json!(false),
        _ => Value::Null,
    };
    if let Some(i) = pick(ui, tl!("Tie in"), "tiein", index(e.tie_in), &ties) {
        set(app, target, json!({ "tie_in": value(i) }));
    }
    if let Some(i) = pick(ui, tl!("Tie off"), "tieoff", index(e.tie_off), &ties) {
        set(app, target, json!({ "tie_off": value(i) }));
    }
    // Remove underlying stitching: the fills below leave out what this object covers.
    if let Some(v) = check(ui, tl!("Remove underlying stitching"), e.remove_underlying) {
        set(app, target, json!({ "remove_underlying": v }));
    }
    if e.remove_underlying
        && let Some(v) = num(ui, tl!("Overlap"), "uover", e.cut_overlap(), " mm", 2)
    {
        set(app, target, json!({ "underlying_overlap": v.clamp(0.0, 10.0) }));
    }
}

/// The spacing profile of a tatami fill: presets from its density (even, accordion, fading out
/// or in across the rows), and Make Blend, which turns it into layers blending its thread into
/// another colour.
fn profile_rows(app: &mut VectorcraftApp, ui: &mut Ui, target: Target, f: &FillParams) {
    let d = f.spacing.max(0.1);
    let presets: [Vec<[f64; 2]>; 4] =
        [Vec::new(), vec![[0.0, d], [0.25, d * 4.0], [0.5, d], [0.75, d * 4.0], [1.0, d]], vec![[0.0, d], [1.0, 0.0]], vec![[0.0, 0.0], [1.0, d]]];
    let mut names = vec![tl!("Even"), tl!("Accordion"), tl!("Fade out"), tl!("Fade in")];
    let cur = presets.iter().position(|p| *p == f.spacing_profile).unwrap_or_else(|| {
        names.push(tl!("Custom"));
        names.len() - 1
    });
    if let Some(i) = pick(ui, tl!("Spacing profile"), "fprofile", cur, &names)
        && let Some(p) = presets.get(i)
    {
        set(app, target, json!({ "spacing_profile": p }));
    }
    let Target::Selection(id) = target else { return };
    ui.horizontal(|ui| {
        ui.label(tl!("Blend into"));
        egui::color_picker::color_edit_button_srgb(ui, &mut app.embroidery.blend_to);
        if button(ui, tl!("Make Blend")).clicked() {
            let from = app.session.active().and_then(|st| {
                let n = st.doc.node(vectorcraft_doc::NodeId(id))?;
                Some(vectorcraft_embroidery::node_thread(n, n.embroidery.as_deref()?).color)
            });
            let [r, g, b] = app.embroidery.blend_to;
            if let Some(from) = from {
                let to = Rgb::new(r, g, b);
                app.run("embroidery.makeBlend", json!({ "ids": [id], "colors": [from.to_hex(), to.to_hex()] })).ok();
            }
        }
    });
}

/// The motif of a motif run or fill: a dropdown of the built-in motifs, then the design's own
/// (Make Motif), and Delete Motif when it sews one of those. `custom` is the design motif it
/// sews, if any: shown even when the design no longer has it.
fn motif_rows(app: &mut VectorcraftApp, ui: &mut Ui, target: Target, current: Motif, custom: Option<&CustomMotif>) {
    let ours: Vec<String> = app
        .session
        .active()
        .and_then(|st| st.doc.embroidery.as_deref().map(|c| c.motifs.iter().map(|m| m.name.clone()).collect()))
        .unwrap_or_default();
    let library = with_kept(app, ours.clone());
    match pick_motif(ui, &library, current, custom) {
        Some(MotifChoice::BuiltIn(m)) => set(app, target, json!({ "motif": m, "custom_motif": null })),
        Some(MotifChoice::Design(name)) => use_motif(app, target, &name),
        None => {}
    }
    if let Some(c) = custom.filter(|c| ours.contains(&c.name)) {
        ui.horizontal(|ui| {
            if button(ui, tl!("Delete Motif")).on_hover_text(tl!("Remove the motif from the design; what sews it keeps it.")).clicked() {
                app.run("embroidery.motif.delete", json!({ "name": c.name })).ok();
            }
            if !app.session.prefs.embroidery_motifs.iter().any(|m| m.name == c.name)
                && button(ui, tl!("Keep Motif")).on_hover_text(tl!("Keep the motif in My Motifs, for any design")).clicked()
            {
                app.run("embroidery.motif.keep", json!({ "name": c.name })).ok();
            }
        });
    }
}

/// The design's motifs, then those of My Motifs it hasn't.
fn with_kept(app: &VectorcraftApp, mut names: Vec<String>) -> Vec<String> {
    for m in &app.session.prefs.embroidery_motifs {
        if !names.contains(&m.name) {
            names.push(m.name.clone());
        }
    }
    names
}

/// A built-in motif's name in the UI language.
fn motif_name(m: Motif) -> &'static str {
    match m {
        Motif::Wave => tl!("Wave"),
        Motif::Scallop => tl!("Scallop"),
        Motif::Diamond => tl!("Diamond"),
        Motif::Cross => tl!("Cross"),
        Motif::Feather => tl!("Feather"),
        Motif::Heart => tl!("Heart"),
        Motif::Star => tl!("Star"),
        Motif::Leaf => tl!("Leaf"),
        Motif::Loop => tl!("Loop"),
        Motif::Zigzag => tl!("Zigzag"),
        Motif::EStitch => tl!("E-stitch"),
    }
}

/// A motif picked from the motif dropdown.
enum MotifChoice {
    BuiltIn(Motif),
    /// One of the design's motifs, by name.
    Design(String),
}

/// The motif dropdown over the built-in motifs and the design's (`library`): the motif picked,
/// if it changed.
fn pick_motif(ui: &mut Ui, library: &[String], current: Motif, custom: Option<&CustomMotif>) -> Option<MotifChoice> {
    let mut names: Vec<&str> = Motif::ALL.iter().map(|m| motif_name(*m)).collect();
    let builtin = names.len();
    names.extend(library.iter().map(String::as_str));
    if let Some(c) = custom
        && !library.contains(&c.name)
    {
        names.push(&c.name);
    }
    let cur = match custom {
        Some(c) => names.iter().skip(builtin).position(|n| *n == c.name).map_or(0, |i| i + builtin),
        None => Motif::ALL.iter().position(|m| *m == current).unwrap_or(0),
    };
    let i = pick_names(ui, tl!("Motif"), "motif", cur, &names)?;
    match Motif::ALL.get(i) {
        Some(m) => Some(MotifChoice::BuiltIn(*m)),
        None => library.get(i.checked_sub(builtin)?).cloned().map(MotifChoice::Design),
    }
}

/// Sew one of the design's motifs: the selected objects through Apply Motif (which sizes them
/// for it), new objects through the properties they take.
fn use_motif(app: &mut VectorcraftApp, target: Target, name: &str) {
    match target {
        Target::Selection(_) => {
            app.run("embroidery.motif.apply", json!({ "name": name })).ok();
        }
        Target::Current(_) => {
            let m = app.session.active().and_then(|st| st.doc.embroidery.as_deref()?.motif(name).cloned());
            if let Some(m) = m {
                set(app, target, json!({ "custom_motif": m }));
            }
        }
    }
}

/// A texture pattern's name in the UI language.
fn texture_name(p: TexturePattern) -> &'static str {
    match p {
        TexturePattern::Diamonds => tl!("Diamonds"),
        TexturePattern::Waves => tl!("Waves"),
        TexturePattern::Herringbone => tl!("Herringbone"),
        TexturePattern::Bricks => tl!("Bricks"),
        TexturePattern::Circles => tl!("Circles"),
        TexturePattern::Scales => tl!("Scales"),
        TexturePattern::Hexagons => tl!("Hexagons"),
        TexturePattern::Chevrons => tl!("Chevrons"),
        TexturePattern::Zigzag => tl!("Zigzag"),
        TexturePattern::Ogee => tl!("Ogee"),
        TexturePattern::BasketWeave => tl!("Basket weave"),
        TexturePattern::Stars => tl!("Stars"),
        TexturePattern::Hearts => tl!("Hearts"),
        TexturePattern::Leaves => tl!("Leaves"),
        TexturePattern::Stripes => tl!("Stripes"),
        TexturePattern::Slants => tl!("Slants"),
        TexturePattern::Pinstripes => tl!("Pinstripes"),
        TexturePattern::Lattice => tl!("Lattice"),
        TexturePattern::Arches => tl!("Arches"),
        TexturePattern::Rings => tl!("Rings"),
        TexturePattern::Dots => tl!("Dots"),
        TexturePattern::Spirals => tl!("Spirals"),
        TexturePattern::Pyramids => tl!("Pyramids"),
        TexturePattern::Ripples => tl!("Ripples"),
        TexturePattern::GreekKey => tl!("Greek key"),
        TexturePattern::Crosses => tl!("Crosses"),
        TexturePattern::Quatrefoils => tl!("Quatrefoils"),
        TexturePattern::Flowers => tl!("Flowers"),
        TexturePattern::Trefoils => tl!("Trefoils"),
        TexturePattern::Moons => tl!("Moons"),
        TexturePattern::Arrows => tl!("Arrows"),
        TexturePattern::NestedDiamonds => tl!("Nested diamonds"),
        TexturePattern::Argyle => tl!("Argyle"),
        TexturePattern::Wicker => tl!("Wicker"),
        TexturePattern::Feathers => tl!("Feathers"),
        TexturePattern::Paisley => tl!("Paisley"),
        TexturePattern::Swirls => tl!("Swirls"),
        TexturePattern::Snowflakes => tl!("Snowflakes"),
        TexturePattern::Bubbles => tl!("Bubbles"),
        TexturePattern::Sunbursts => tl!("Sunbursts"),
    }
}

/// Tatami textures: programmed splits (a pattern over the fill), and the split lines carved
/// into the fill.
fn texture_rows(app: &mut VectorcraftApp, ui: &mut Ui, target: Target, f: &FillParams, e: &Embroidery) {
    let mut names = vec![tl!("None")];
    names.extend(TexturePattern::ALL.iter().map(|p| texture_name(*p)));
    let cur = f.texture.and_then(|t| TexturePattern::ALL.iter().position(|p| *p == t.pattern)).map_or(0, |i| i + 1);
    if let Some(i) = pick(ui, tl!("Texture"), "texture", cur, &names) {
        let v = match (i.checked_sub(1).and_then(|k| TexturePattern::ALL.get(k)), f.texture) {
            // another pattern keeps the size and turn already set
            (Some(p), Some(t)) => json!({ "pattern": p, "size": t.size, "angle": t.angle }),
            (Some(p), None) => json!({ "pattern": p, "size": p.default_size(f.stitch_length), "angle": 0.0 }),
            (None, _) => Value::Null,
        };
        set(app, target, json!({ "texture": v }));
    }
    if let Some(t) = f.texture {
        if let Some(v) = num(ui, tl!("Texture size"), "tsize", t.size, " mm", 1) {
            set(app, target, json!({ "texture": { "size": v.clamp(1.0, 100.0) } }));
        }
        if let Some(v) = num(ui, tl!("Texture angle"), "tangle", t.angle, "°", 1) {
            set(app, target, json!({ "texture": { "angle": v } }));
        }
    }
    if let Target::Selection(id) = target
        && !e.splits.is_empty()
    {
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", tl!("Split lines:"), e.splits.len()));
            if button(ui, tl!("Clear Split Lines")).clicked() {
                app.run("embroidery.clearSplits", json!({ "ids": [id] })).ok();
            }
        });
    }
    // rows following flow lines (a Florentine or liquid effect)
    if let Target::Selection(id) = target
        && !e.flow.is_empty()
    {
        ui.horizontal(|ui| {
            ui.label(format!("{} {}", tl!("Rows follow lines:"), e.flow.len()));
            if button(ui, tl!("Clear Flow Lines")).clicked() {
                app.run("embroidery.clearFlowLines", json!({ "ids": [id] })).ok();
            }
        });
    }
}

/// Stitch direction: automatic, or inclination lines the user can edit.
fn direction_rows(app: &mut VectorcraftApp, ui: &mut Ui, id: u64, e: &Embroidery) {
    ui.add_space(4.0);
    let n = e.guides.len();
    let status = match n {
        0 if e.auto_direction => tl!("Direction: automatic").to_string(),
        0 => tl!("Direction: fixed angle").to_string(),
        1 => tl!("Direction: one inclination line").to_string(),
        n => format!("{} {n} {}", tl!("Direction: turning along"), tl!("inclination lines")),
    };
    ui.label(status);
    if let StitchType::Fill(f) = &e.stitch
        && n == 0
        && let Some(v) = num(ui, tl!("Angle"), "angle", f.angle, "°", 1)
    {
        set(app, Target::Selection(id), json!({ "angle": v, "auto_direction": false }));
    }
    ui.horizontal_wrapped(|ui| {
        if button(ui, tl!("Auto Direction")).on_hover_text(tl!("Place editable inclination lines (or pick the angle) automatically")).clicked() {
            app.run("embroidery.autoDirection", json!({})).ok();
        }
        if button(ui, tl!("Edit Lines")).on_hover_text(tl!("Drag across the shape to add a line; drag a bead to move it")).clicked() {
            app.run("tool.select", json!({ "tool": "embroideryDirection" })).ok();
        }
        if n > 0 && button(ui, tl!("Clear Lines")).clicked() {
            app.run("embroidery.clearGuides", json!({ "ids": [id] })).ok();
        }
    });
    // Stitches edited by hand.
    ui.add_space(4.0);
    if e.frozen.is_some() {
        ui.label(tl!("Stitches edited by hand: the shape no longer changes them."));
    }
    ui.horizontal_wrapped(|ui| {
        if button(ui, tl!("Edit Stitches")).on_hover_text(tl!("Pick, move, add and delete single stitches")).clicked() {
            app.run("tool.select", json!({ "tool": "embroideryStitches" })).ok();
        }
        // one needle point picked with the Stitch Edit tool: split the object there
        let opts = if app.session.tool_id() == "embroideryStitches" { app.session.tool_options() } else { Value::Null };
        if opts["id"] == id
            && let Some([at]) = opts["picked"].as_array().map(Vec::as_slice)
            && button(ui, tl!("Split Here")).on_hover_text(tl!("Split the object in two at the picked stitch")).clicked()
        {
            app.run("embroidery.stitches.split", json!({ "id": id, "at": at })).ok();
        }
        if e.frozen.is_some() && button(ui, tl!("Regenerate Stitches")).on_hover_text(tl!("Drop the hand edits and stitch the shape again")).clicked()
        {
            app.run("embroidery.regenerate", json!({ "ids": [id] })).ok();
        }
    });
}

fn thread_row(app: &mut VectorcraftApp, ui: &mut Ui, e: &Embroidery) {
    label_row(ui, tl!("Thread"), LABEL_W, |ui| {
        let current = e.thread.as_ref().map(|t| t.color);
        let mut rgb = current.map(|c| [c.r, c.g, c.b]).unwrap_or([26, 62, 140]);
        if egui::color_picker::color_edit_button_srgb(ui, &mut rgb).changed() {
            let hex = Rgb::new(rgb[0], rgb[1], rgb[2]).to_hex();
            app.run("embroidery.setThread", json!({ "color": hex })).ok();
        }
        // pick a named thread from a chart
        let chart_name = app.session.active().and_then(|s| s.doc.embroidery.as_ref().map(|c| c.chart.clone())).unwrap_or_else(|| "Generic".into());
        let all = charts();
        let chart = all.iter().find(|c| c.name == chart_name).or(all.first());
        if let Some(chart) = chart {
            let label = e.thread.as_ref().map(|t| t.label()).unwrap_or_else(|| tl!("Object color").to_string());
            egui::ComboBox::from_id_salt(("emb", "thread")).selected_text(label).width(150.0).height(320.0).show_ui(ui, |ui| {
                for th in &chart.threads {
                    let c = th.color;
                    let r = ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 2.0, Color32::from_rgb(c.r, c.g, c.b));
                        ui.selectable_label(false, th.label())
                    });
                    if r.inner.clicked() {
                        app.run("embroidery.setThread", json!({ "color": c.to_hex(), "name": th.name, "code": th.code, "chart": th.chart })).ok();
                    }
                }
            });
        }
    });
}

fn design_section(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens) {
    section_header(ui, tl!("Design"));
    let info = app.run("embroidery.info", json!({})).unwrap_or(Value::Null);
    let stitches = info["stitches"].as_u64().unwrap_or(0);
    if stitches > 0 {
        let w = info["widthMm"].as_f64().unwrap_or(0.0);
        let h = info["heightMm"].as_f64().unwrap_or(0.0);
        let row = |ui: &mut Ui, k: &str, v: String| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(k).color(t.text_dim));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| ui.label(v));
            });
        };
        row(ui, tl!("Stitches"), stitches.to_string());
        row(ui, tl!("Size"), format!("{w:.1} × {h:.1} mm ({:.2} × {:.2} in)", w / 25.4, h / 25.4));
        row(ui, tl!("Colors"), info["colors"].as_array().map_or(0, Vec::len).to_string());
        row(ui, tl!("Trims"), info["trims"].as_u64().unwrap_or(0).to_string());
        row(ui, tl!("Sewing time"), info["time"].as_str().unwrap_or_default().to_string());
        row(ui, tl!("Thread"), format!("{:.1} m", info["threadM"].as_f64().unwrap_or(0.0)));
        if let Some(fits) = info["fits"].as_bool() {
            let (txt, col) = if fits { (tl!("Fits the hoop"), t.accent) } else { (tl!("Larger than the hoop"), t.error) };
            ui.label(egui::RichText::new(txt).color(col));
        }
    } else {
        ui.label(egui::RichText::new(tl!("No stitches yet.")).color(t.text_dim));
    }
    checks(app, ui, t);
    // Hoop and fabric.
    let hoops: Vec<String> =
        info["hoops"].as_array().map(|a| a.iter().filter_map(|h| h["name"].as_str().map(str::to_string)).collect()).unwrap_or_default();
    let mut hoop_opts: Vec<&str> = vec![tl!("No hoop")];
    hoop_opts.extend(hoops.iter().map(String::as_str));
    let cur_hoop = info["hoop"].as_str().and_then(|h| hoops.iter().position(|x| x == h)).map_or(0, |i| i + 1);
    if let Some(i) = pick_names(ui, tl!("Hoop"), "hoop", cur_hoop, &hoop_opts) {
        let v = if i == 0 { Value::Null } else { json!(hoop_opts.get(i).copied().unwrap_or_default()) };
        app.run("embroidery.settings", json!({ "hoop": v })).ok();
    }
    let fabrics: Vec<String> =
        info["fabrics"].as_array().map(|a| a.iter().filter_map(|f| f.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let mut fabric_opts: Vec<&str> = vec![tl!("Not set")];
    fabric_opts.extend(fabrics.iter().map(String::as_str));
    let cur_fabric = info["fabric"].as_str().and_then(|f| fabrics.iter().position(|x| x == f)).map_or(0, |i| i + 1);
    if let Some(i) = pick_names(ui, tl!("Fabric"), "fabric", cur_fabric, &fabric_opts)
        && i > 0
        && let Some(name) = fabric_opts.get(i)
    {
        app.run("embroidery.applyFabric", json!({ "name": name })).ok();
    }
    fabric_rows(app, ui, &info);
    // the chart threads are picked from, and matching the design's threads to it
    let charts: Vec<&str> = info["charts"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let chart = app.session.active().and_then(|s| s.doc.embroidery.as_ref().map(|c| c.chart.clone())).unwrap_or_default();
    let cur = charts.iter().position(|c| *c == chart).unwrap_or(0);
    if let Some(i) = pick_names(ui, tl!("Thread chart"), "chart", cur, &charts)
        && let Some(name) = charts.get(i)
    {
        app.run("embroidery.settings", json!({ "chart": name })).ok();
    }
    if stitches > 0 && button(ui, tl!("Match Threads to Chart")).on_hover_text(tl!("Give every stitched object the chart's closest thread")).clicked()
    {
        app.run("embroidery.matchThreads", json!({ "all": true })).ok();
    }
    machine_rows(app, ui, &info);
    // How stitches are shown.
    ui.add_space(4.0);
    let views = [tl!("Hidden"), tl!("Stitch Lines"), tl!("TrueView")];
    let cur = match app.embroidery.stitches {
        StitchView::Off => 0,
        StitchView::Lines => 1,
        StitchView::TrueView => 2,
    };
    if let Some(i) = pick(ui, tl!("Show stitches"), "view", cur, &views) {
        app.embroidery.stitches = [StitchView::Off, StitchView::Lines, StitchView::TrueView].get(i).copied().unwrap_or_default();
    }
    let arts = [tl!("Normal"), tl!("Dimmed"), tl!("Hidden")];
    let cur = match app.embroidery.art {
        ArtView::Normal => 0,
        ArtView::Dimmed => 1,
        ArtView::Hidden => 2,
    };
    if let Some(i) = pick(ui, tl!("Artwork"), "art", cur, &arts) {
        app.embroidery.art = [ArtView::Normal, ArtView::Dimmed, ArtView::Hidden].get(i).copied().unwrap_or_default();
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut app.embroidery.connectors, tl!("Connectors"));
        ui.checkbox(&mut app.embroidery.needle_points, tl!("Needle points"));
        ui.checkbox(&mut app.embroidery.beads, tl!("Start and stop"));
        ui.checkbox(&mut app.embroidery.guides, tl!("Inclination lines"));
        let mut fabric = app.embroidery.fabric;
        if egui::color_picker::color_edit_button_srgba(ui, &mut fabric, egui::color_picker::Alpha::Opaque)
            .on_hover_text(tl!("Fabric color"))
            .changed()
        {
            app.embroidery.fabric = fabric;
        }
    });
    // the whole design repeated in a grid: a preview of a repeated layout, not sewn
    let [cols, rows] = app.embroidery.repeats;
    if let Some(v) = num(ui, tl!("Repeats across"), "rep-cols", f64::from(cols), "", 0) {
        app.run("ui.embroideryRepeats", json!({ "columns": v })).ok();
    }
    if let Some(v) = num(ui, tl!("Repeats down"), "rep-rows", f64::from(rows), "", 0) {
        app.run("ui.embroideryRepeats", json!({ "rows": v })).ok();
    }
    if cols * rows > 1
        && let Some(v) = num(ui, tl!("Gap between repeats"), "rep-gap", app.embroidery.repeat_gap, " mm", 1)
    {
        app.run("ui.embroideryRepeats", json!({ "gap": v })).ok();
    }
}

/// The design fabric's settings: editable when the fabric was made for the design; a
/// built-in one can be copied to change.
fn fabric_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let Some(name) = info["fabric"].as_str().filter(|n| !n.is_empty()).map(str::to_string) else { return };
    let Some(f) = info["fabricDetails"].as_array().and_then(|a| a.iter().find(|f| f["name"] == name.as_str())).cloned() else { return };
    let custom = f["custom"].as_bool().unwrap_or(false);
    let names: Vec<&str> = info["fabrics"].as_array().map(|a| a.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    egui::CollapsingHeader::new(tl!("Fabric settings")).id_salt("emb-fabric").show(ui, |ui| {
        let mut change: Option<Value> = None;
        ui.add_enabled_ui(custom, |ui| {
            for (label, key, id, decimals) in [
                (tl!("Satin density"), "satinSpacing", "fab-ss", 2),
                (tl!("Fill density"), "fillSpacing", "fab-fs", 2),
                (tl!("Fill stitch length"), "fillStitchLength", "fab-fl", 1),
                (tl!("Pull compensation"), "pullComp", "fab-pc", 2),
                (tl!("Underlay spacing"), "underlaySpacing", "fab-us", 1),
            ] {
                if let Some(v) = num(ui, label, id, f[key].as_f64().unwrap_or(0.0), " mm", decimals) {
                    change = Some(json!({ key: v }));
                }
            }
            ui.horizontal(|ui| {
                if let Some(v) = check(ui, tl!("Fill underlay"), f["fillUnderlay"].as_bool().unwrap_or(true)) {
                    change = Some(json!({ "fillUnderlay": v }));
                }
                if let Some(v) = check(ui, tl!("Topping"), f["topping"].as_bool().unwrap_or(false)) {
                    change = Some(json!({ "topping": v }));
                }
            });
        });
        if let Some(mut c) = change {
            c["name"] = json!(name);
            if app.run("embroidery.fabric.save", c).is_ok() {
                app.run("embroidery.applyFabric", json!({ "name": name })).ok();
            }
        }
        ui.horizontal(|ui| {
            if button(ui, tl!("Save as New Fabric")).on_hover_text(tl!("A copy of this fabric to change")).clicked() {
                let base = name.trim_end_matches(|c: char| c.is_ascii_digit()).trim_end();
                let fresh = (2..).map(|k| format!("{base} {k}")).find(|n| !names.contains(&n.as_str())).unwrap_or_default();
                if app.run("embroidery.fabric.save", json!({ "name": fresh, "from": name })).is_ok() {
                    app.run("embroidery.applyFabric", json!({ "name": fresh })).ok();
                }
            }
            if custom && button(ui, tl!("Delete Fabric")).clicked() {
                app.run("embroidery.fabric.delete", json!({ "name": name })).ok();
            }
        });
    });
}

/// A machine profile's name as the Machine menu shows it: "PES: home machines" for a
/// standard one, the name of one of the user's.
fn machine_label(m: &Value) -> String {
    let name = m["name"].as_str().unwrap_or_default();
    if m["standard"].as_bool() != Some(true) {
        return name.to_string();
    }
    let kind = if m["kind"].as_str() == Some("commercial") { tl!("commercial machines") } else { tl!("home machines") };
    format!("{}: {kind}", name.to_uppercase())
}

/// The machine the design is for (its file format, stitch limits and speed), the design's
/// machine settings, and keeping them in a machine profile.
fn machine_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let machines: Vec<Value> = info["machines"].as_array().cloned().unwrap_or_default();
    let current = info["machine"].as_str().unwrap_or_default().to_string();
    let mut rows: Vec<(String, String)> = machines.iter().filter_map(|m| Some((m["name"].as_str()?.to_string(), machine_label(m)))).collect();
    // a machine the design names that isn't kept here (made on another computer, or deleted)
    if !current.is_empty() && !rows.iter().any(|r| r.0.eq_ignore_ascii_case(&current)) {
        rows.push((current.clone(), current.clone()));
    }
    let mut opts: Vec<&str> = vec![tl!("Not set")];
    opts.extend(rows.iter().map(|r| r.1.as_str()));
    let cur = rows.iter().position(|r| r.0.eq_ignore_ascii_case(&current)).map_or(0, |i| i + 1);
    if let Some(i) = pick_names(ui, tl!("Machine"), "machine", cur, &opts) {
        let v = i.checked_sub(1).and_then(|k| rows.get(k)).map_or(Value::Null, |r| json!(r.0));
        app.run("embroidery.settings", json!({ "machine": v })).ok();
    }
    let s = &info["settings"];
    egui::CollapsingHeader::new(tl!("Machine settings")).id_salt("emb-machine-settings").show(ui, |ui| {
        let lengths = [
            (tl!("Longest stitch"), "maxStitch", "mxs"),
            (tl!("Longest jump"), "maxJump", "mxj"),
            (tl!("Shortest stitch"), "minStitch", "mns"),
            (tl!("Trim jumps longer than"), "trimThreshold", "trt"),
            (tl!("Tie length"), "tieLength", "til"),
        ];
        for (label, key, id) in lengths {
            if let Some(v) = num(ui, label, id, s[key].as_f64().unwrap_or(0.0), " mm", 2) {
                app.run("embroidery.settings", json!({ key: v })).ok();
            }
        }
        for (label, key, id, decimals) in [
            (tl!("Stitches per minute"), "machineSpeed", "spm", 0),
            (tl!("Seconds per trim"), "trimSeconds", "tsec", 1),
            (tl!("Seconds per color change"), "colorChangeSeconds", "ccsec", 0),
        ] {
            if let Some(v) = num(ui, label, id, s[key].as_f64().unwrap_or(0.0), "", decimals) {
                app.run("embroidery.settings", json!({ key: v })).ok();
            }
        }
        let styles: Vec<&str> = TieStyle::ALL.iter().map(|t| tl!(t.label())).collect();
        let cur = TieStyle::ALL.iter().position(|t| s["tieStyle"].as_str() == Some(t.id())).unwrap_or(1);
        if let Some(i) = pick(ui, tl!("Tie style"), "tiestyle", cur, &styles)
            && let Some(t) = TieStyle::ALL.get(i)
        {
            app.run("embroidery.settings", json!({ "tieStyle": t.id() })).ok();
        }
        ui.horizontal(|ui| {
            if let Some(v) = check(ui, tl!("Tie in"), s["tieIn"].as_bool().unwrap_or(true)) {
                app.run("embroidery.settings", json!({ "tieIn": v })).ok();
            }
            if let Some(v) = check(ui, tl!("Tie off"), s["tieOff"].as_bool().unwrap_or(true)) {
                app.run("embroidery.settings", json!({ "tieOff": v })).ok();
            }
        });
        machine_profile_rows(app, ui, &machines, &current, info["machineFormat"].as_str().unwrap_or("dst"));
    });
}

/// Keeping the design's machine settings in its machine's profile or a new one of the user's;
/// renaming the user's own, and putting a standard one back to how it ships.
fn machine_profile_rows(app: &mut VectorcraftApp, ui: &mut Ui, machines: &[Value], current: &str, format: &str) {
    let profile = machines.iter().find(|m| m["name"].as_str().is_some_and(|n| !current.is_empty() && n.eq_ignore_ascii_case(current)));
    let own = profile.filter(|m| m["standard"].as_bool() != Some(true));
    ui.add_space(4.0);
    if let Some(m) = own {
        let name = m["name"].as_str().unwrap_or_default().to_string();
        label_row(ui, tl!("Name"), LABEL_W, |ui| {
            let key = egui::Id::new("emb-machine-name");
            let mut text = ui.data(|d| d.get_temp::<String>(key)).unwrap_or_else(|| name.clone());
            let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(130.0));
            if resp.lost_focus() {
                let new = text.trim();
                if !new.is_empty() && new != name {
                    app.run("embroidery.machine.save", json!({ "name": name, "newName": new })).ok();
                }
                ui.data_mut(|d| d.remove::<String>(key));
            } else if resp.has_focus() {
                ui.data_mut(|d| d.insert_temp(key, text));
            }
        });
        let formats: Vec<String> =
            machines.iter().filter(|m| m["standard"].as_bool() == Some(true)).filter_map(|m| m["format"].as_str().map(str::to_uppercase)).collect();
        let opts: Vec<&str> = formats.iter().map(String::as_str).collect();
        let cur = formats.iter().position(|f| f.eq_ignore_ascii_case(m["format"].as_str().unwrap_or_default())).unwrap_or(0);
        if let Some(i) = pick_names(ui, tl!("Format"), "machine-format", cur, &opts)
            && let Some(f) = formats.get(i)
        {
            app.run("embroidery.machine.save", json!({ "name": name, "format": f.to_lowercase() })).ok();
        }
    }
    ui.horizontal_wrapped(|ui| {
        if let Some(m) = profile {
            let name = m["name"].as_str().unwrap_or_default().to_string();
            if button(ui, tl!("Save to Machine")).on_hover_text(tl!("Keep these settings in the machine's profile, for new designs")).clicked() {
                app.run("embroidery.machine.save", json!({ "name": name, "fromDesign": true })).ok();
            }
            if m["edited"].as_bool() == Some(true)
                && button(ui, tl!("Reset Machine")).on_hover_text(tl!("Put the machine's profile back to how it ships, and use it")).clicked()
                && app.run("embroidery.machine.delete", json!({ "name": name })).is_ok()
            {
                app.run("embroidery.settings", json!({ "machine": name })).ok();
            }
            if own.is_some() && button(ui, tl!("Delete Machine")).clicked() {
                app.run("embroidery.machine.delete", json!({ "name": name })).ok();
            }
        }
        if button(ui, tl!("Save as New Machine")).on_hover_text(tl!("A machine profile of your own with these settings")).clicked() {
            // a copy of one of the user's is numbered; a machine the design names but that isn't
            // kept here comes back under its name
            let (base, first) = match (profile, own) {
                (_, Some(m)) => (m["name"].as_str().unwrap_or_default().trim_end_matches(|c: char| c.is_ascii_digit()).trim_end().to_string(), 2),
                (None, None) if !current.is_empty() => (current.to_string(), 1),
                _ => (tl!("My Machine").to_string(), 1),
            };
            let taken = |n: &str| machines.iter().any(|m| m["name"].as_str().is_some_and(|x| x.eq_ignore_ascii_case(n)));
            let fresh = (first..).map(|k| if k == 1 { base.clone() } else { format!("{base} {k}") }).find(|n| !taken(n)).unwrap_or_default();
            if app.run("embroidery.machine.save", json!({ "name": fresh, "format": format, "fromDesign": true })).is_ok() {
                app.run("embroidery.settings", json!({ "machine": fresh })).ok();
            }
        }
    });
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let open = app.embroidery.widget.open;
    if ui.button(if open { tl!("Hide Digitizing Widget") } else { tl!("Digitizing Widget") }).clicked() {
        app.embroidery.widget.open = !open;
        if !open {
            crate::embroidery::widget::load(app);
        }
        ui.close();
    }
    if ui.button(tl!("Convert Artwork to Embroidery")).clicked() {
        app.run("embroidery.convert", json!({})).ok();
        ui.close();
    }
    if ui.button(tl!("Auto Direction")).clicked() {
        app.run("embroidery.autoDirection", json!({})).ok();
        ui.close();
    }
}

/// Check Design: runs `embroidery.check`, keeps the findings for the panel and
/// shows the panel → how many it found.
pub fn run_check(app: &mut VectorcraftApp) -> Result<usize, String> {
    let r = app.run("embroidery.check", json!({}))?;
    let findings = r["findings"].as_array().cloned().unwrap_or_default();
    let n = findings.len();
    app.embroidery.checks = Some(findings);
    // show the findings: open the panel (Window › Embroidery toggles, so not when it is open)
    if app.ui.open_panel.as_deref() != Some(ID) {
        app.run("window.panel", json!({ "panel": ID })).ok();
    }
    Ok(n)
}

/// A finding in the interface language (the engine's message is English).
fn finding_text(f: &Value) -> String {
    let num = |k: &str| f[k].as_f64().map(|v| format!("{v:.1}")).unwrap_or_default();
    match f["kind"].as_str().unwrap_or_default() {
        "dense" => crate::i18n::fmt(tl!("Very dense: about {n} layers of thread here"), &[("n", &num("value"))]),
        "longStitches" => crate::i18n::fmt(tl!("Stitches up to {n} mm long can snag"), &[("n", &num("value"))]),
        "tiny" => tl!("Tiny object: may not sew cleanly").to_string(),
        "smallLettering" => crate::i18n::fmt(tl!("Lettering only {n} mm tall is hard to sew clearly"), &[("n", &num("value"))]),
        "noStitches" => tl!("Makes no stitches").to_string(),
        "hoop" => tl!("The design is larger than the hoop").to_string(),
        _ => f["message"].as_str().unwrap_or_default().to_string(),
    }
}

/// The Check Design button and its findings: click one to select its objects and
/// bring the spot into view.
fn checks(app: &mut VectorcraftApp, ui: &mut Ui, t: &Tokens) {
    ui.add_space(4.0);
    let label = if app.embroidery.checks.is_some() { tl!("Check Again") } else { tl!("Check Design") };
    if button(ui, label).on_hover_text(tl!("Look for dense spots, long stitches, tiny objects and small lettering")).clicked() {
        run_check(app).ok();
    }
    // what counts as a problem: the design's own limits
    egui::CollapsingHeader::new(tl!("Check settings")).id_salt("emb-check-limits").show(ui, |ui| {
        let lim = app.session.active().and_then(|st| st.doc.embroidery.as_deref().map(|e| e.checks.clone())).unwrap_or_default();
        for (label, key, id, value, suffix) in [
            (tl!("Dense from (layers)"), "denseLayers", "chk-dense", lim.dense_layers, ""),
            (tl!("Long stitches from"), "longStitch", "chk-long", lim.long_stitch, " mm"),
            (tl!("Small lettering under"), "smallLettering", "chk-text", lim.small_lettering, " mm"),
            (tl!("Tiny objects under"), "tinyObject", "chk-tiny", lim.tiny_object, " mm"),
        ] {
            if let Some(v) = num(ui, label, id, value, suffix, 1) {
                app.run("embroidery.settings", json!({ "checks": { key: v } })).ok();
            }
        }
    });
    let Some(findings) = app.embroidery.checks.clone() else { return };
    if findings.is_empty() {
        ui.label(egui::RichText::new(tl!("No problems found.")).color(t.accent));
        return;
    }
    for f in &findings {
        let warning = f["severity"] == "warning";
        let (mark, color) = if warning { ("▲", t.warning) } else { ("●", t.text_dim) };
        let text = egui::RichText::new(format!("{mark} {}", finding_text(f))).color(color);
        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click())).on_hover_text(tl!("Click to select and show it"));
        if resp.clicked() {
            let ids: Vec<u64> = f["ids"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
            if !ids.is_empty() {
                app.run("select.set", json!({ "ids": ids })).ok();
            }
            if let Some(at) = f["at"].as_array()
                && let (Some(x), Some(y)) = (at.first().and_then(Value::as_f64), at.get(1).and_then(Value::as_f64))
                && let Some(v) = app.view_mut()
            {
                v.center = vectorcraft_geom::Point::new(x, y);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn finding_text_for_test(f: &Value) -> String {
    finding_text(f)
}
