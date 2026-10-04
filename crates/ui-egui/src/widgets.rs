//! Shared widgets in Illustrator's panel style. Colours come from theme tokens.

use egui::{Color32, CornerRadius, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use vectorcraft_color::{BlendMode, Paint};
use vectorcraft_doc::Unit;

use crate::icons;
use crate::theme::{self, Tokens};

/// Square icon button; `selected` draws the pressed well.
pub fn icon_button(ui: &mut Ui, icon: &str, tip: &str, selected: bool, size: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let bg = if selected {
        t.tool_active
    } else if resp.hovered() {
        t.hover
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, CornerRadius::same(3), bg);
    let pad = (size * 0.2).round();
    icons::paint(ui, icon, rect.shrink(pad), if selected { t.text } else { t.icon });
    if !tip.is_empty() { resp.on_hover_text(tip) } else { resp }
}

/// Small flat text button (Quick Actions style).
pub fn flat_button(ui: &mut Ui, text: &str, width: f32) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(vec2(width, 24.0), Sense::click());
    let rect = rect.shrink2(vec2(0.0, 0.5));
    if resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(rect, CornerRadius::same(2), t.tool_active);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(2), t.hover);
    }
    ui.painter().rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, if resp.hovered() { t.text } else { t.button_border }),
        StrokeKind::Inside,
    );
    ui.painter().with_clip_rect(rect).text(rect.center(), egui::Align2::CENTER_CENTER, text, egui::FontId::proportional(12.5), t.text_strong);
    resp
}

/// Blue call-to-action pill (dialog OK/Create).
pub fn primary_button(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), theme::semibold(12.5), Color32::WHITE);
    let w = galley.size().x + 32.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w.max(72.0), 28.0), Sense::click());
    let bg = if resp.hovered() { t.accent } else { t.accent_strong };
    ui.painter().rect_filled(rect, CornerRadius::same(14), bg);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, Color32::WHITE);
    resp
}

/// Outlined secondary pill (dialog Cancel).
pub fn secondary_button(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), theme::semibold(12.5), t.text);
    let w = galley.size().x + 32.0;
    let (rect, resp) = ui.allocate_exact_size(vec2(w.max(72.0), 28.0), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(14), t.hover);
    }
    ui.painter().rect_stroke(rect, CornerRadius::same(14), Stroke::new(1.5, t.text_dim), StrokeKind::Inside);
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, t.text);
    resp
}

/// Bold section header (Properties panel).
pub fn section_header(ui: &mut Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(2.0);
    ui.label(egui::RichText::new(text).size(13.0).color(t.text));
    ui.add_space(2.0);
}

/// Full-width 1 px divider with vertical margins.
pub fn divider(ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(rect, 0.0, t.divider);
    ui.add_space(6.0);
}

pub fn dim_label(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).color(t.text).size(12.5))
}

/// A recessed numeric field showing `value` (points) in `unit`. Returns the new value (points)
/// when the user commits (Enter / focus loss). Supports unit suffixes and arithmetic.
pub fn num_field(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, value: Option<f64>, unit: Unit, width: f32) -> Option<f64> {
    let t = Tokens::get(ui.ctx());
    let id = ui.id().with(id);
    let shown = value.map(|v| unit.format(v)).unwrap_or_default();
    let mut buf: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone());
    let editing = ui.memory(|m| m.has_focus(id));
    if !editing {
        buf = shown.clone();
    }
    let resp = ui
        .allocate_ui_with_layout(vec2(width, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(width);
            egui::Frame::NONE
                .fill(t.input)
                .stroke(Stroke::new(1.0, if editing { t.accent } else { t.input_border }))
                .corner_radius(CornerRadius::same(2))
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .desired_width(width - 14.0)
                            .min_size(vec2(width - 14.0, 0.0))
                            .font(egui::FontId::proportional(12.5))
                            .text_color(t.text_strong),
                    )
                })
                .inner
        })
        .inner;
    let commit = resp.lost_focus() && buf != shown;
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    if commit { unit.parse(&buf) } else { None }
}

/// A plain number field (percent, degrees, counts) with optional suffix.
pub fn plain_field(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, value: f64, suffix: &str, decimals: usize, width: f32) -> Option<f64> {
    let t = Tokens::get(ui.ctx());
    let id = ui.id().with(id);
    let shown = {
        let s = format!("{:.*}", decimals, value);
        let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
        format!("{s}{suffix}")
    };
    let editing = ui.memory(|m| m.has_focus(id));
    let mut buf: String = if editing { ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone()) } else { shown.clone() };
    let resp = ui
        .allocate_ui_with_layout(vec2(width, 26.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(width);
            egui::Frame::NONE
                .fill(t.input)
                .stroke(Stroke::new(1.0, if editing { t.accent } else { t.input_border }))
                .corner_radius(CornerRadius::same(2))
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .desired_width(width - 14.0)
                            .min_size(vec2(width - 14.0, 0.0))
                            .font(egui::FontId::proportional(12.5))
                            .text_color(t.text_strong),
                    )
                })
                .inner
        })
        .inner;
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    if resp.lost_focus() && buf != shown {
        buf.trim().trim_end_matches(suffix.trim()).trim().trim_end_matches(['%', '°']).parse::<f64>().ok()
    } else {
        None
    }
}

/// Draw a paint preview (swatch chip) into `rect`.
pub fn paint_chip(ui: &Ui, rect: Rect, paint: &Paint) {
    let p = ui.painter();
    match paint {
        Paint::None => {
            p.rect_filled(rect, 0.0, Color32::WHITE);
            p.line_segment([rect.left_bottom(), rect.right_top()], Stroke::new(1.6, Color32::from_rgb(0xe0, 0x20, 0x20)));
        }
        Paint::Solid { color, .. } => {
            let [r, g, b, _] = color.to_rgba8(1.0);
            p.rect_filled(rect, 0.0, Color32::from_rgb(r, g, b));
        }
        Paint::Gradient(gp) => {
            if gp.gradient.kind == vectorcraft_color::GradientKind::Radial {
                // Outer colour fills the corners beyond the largest circle.
                let (c, _) = gp.gradient.sample(1.0);
                let [r, g, b, _] = c.to_rgba8(1.0);
                p.rect_filled(rect, 0.0, Color32::from_rgb(r, g, b));
            }
            let n = 24;
            let w = rect.width() / n as f32;
            for i in 0..n {
                let tt = (i as f32 + 0.5) / n as f32;
                let (c, _) = gp.gradient.sample(tt);
                let [r, g, b, _] = c.to_rgba8(1.0);
                let rr = Rect::from_min_size(pos2(rect.left() + i as f32 * w, rect.top()), vec2(w + 0.5, rect.height()));
                if gp.gradient.kind == vectorcraft_color::GradientKind::Radial {
                    let (c, _) = gp.gradient.sample(1.0 - tt);
                    let [r, g, b, _] = c.to_rgba8(1.0);
                    let s = rect.width().min(rect.height()) * tt / 2.0;
                    p.circle_filled(rect.center(), s.max(0.5), Color32::from_rgb(r, g, b));
                } else {
                    p.rect_filled(rr, 0.0, Color32::from_rgb(r, g, b));
                }
            }
        }
        Paint::Pattern { .. } => {
            p.rect_filled(rect, 0.0, Color32::from_gray(200));
            for i in 0..4 {
                let x = rect.left() + rect.width() * i as f32 / 4.0;
                p.line_segment([pos2(x, rect.bottom()), pos2(x + rect.width() / 4.0, rect.top())], Stroke::new(1.0, Color32::from_gray(90)));
            }
        }
    }
}

/// The Fill/Stroke proxy pair (Illustrator's overlapping squares). Returns (fill clicked, stroke clicked, swap, default).
pub fn fill_stroke_proxy(ui: &mut Ui, fill: &Paint, stroke: &Paint, fill_active: bool, size: f32) -> (bool, bool, bool, bool) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
    let s = size * 0.62;
    let fill_r = Rect::from_min_size(rect.min + vec2(0.0, 0.0), Vec2::splat(s));
    let stroke_r = Rect::from_min_size(rect.max - Vec2::splat(s), Vec2::splat(s));
    let stroke_resp = ui.interact(stroke_r, ui.id().with("stroke-proxy"), Sense::click());
    let fill_resp = ui.interact(fill_r, ui.id().with("fill-proxy"), Sense::click());
    let draw_fill = |ui: &Ui| {
        paint_chip(ui, fill_r, fill);
        ui.painter().rect_stroke(fill_r, 0.0, Stroke::new(1.0, Color32::from_gray(20)), StrokeKind::Inside);
        ui.painter().rect_stroke(fill_r.expand(1.0), 0.0, Stroke::new(1.0, Color32::from_gray(150)), StrokeKind::Outside);
    };
    let draw_stroke = |ui: &Ui| {
        let w = s * 0.28;
        paint_chip(ui, stroke_r, stroke);
        let inner = stroke_r.shrink(w);
        ui.painter().rect_filled(inner, 0.0, t.panel);
        ui.painter().rect_stroke(stroke_r, 0.0, Stroke::new(1.0, Color32::from_gray(20)), StrokeKind::Inside);
        ui.painter().rect_stroke(inner, 0.0, Stroke::new(1.0, Color32::from_gray(20)), StrokeKind::Outside);
        ui.painter().rect_stroke(stroke_r.expand(1.0), 0.0, Stroke::new(1.0, Color32::from_gray(150)), StrokeKind::Outside);
    };
    if fill_active {
        draw_stroke(ui);
        draw_fill(ui);
    } else {
        draw_fill(ui);
        draw_stroke(ui);
    }
    // Swap arrow (top-right) and default (bottom-left) mini buttons.
    let swap_r = Rect::from_min_size(pos2(rect.right() - size * 0.3, rect.top()), Vec2::splat(size * 0.3));
    let def_r = Rect::from_min_size(pos2(rect.left(), rect.bottom() - size * 0.3), Vec2::splat(size * 0.3));
    let swap = ui.interact(swap_r, ui.id().with("swap-proxy"), Sense::click());
    let def = ui.interact(def_r, ui.id().with("default-proxy"), Sense::click());
    icons::paint(ui, "dc-swap", swap_r.shrink(1.0), if swap.hovered() { t.text } else { t.icon });
    let a = Rect::from_min_size(def_r.min + vec2(1.0, 1.0), Vec2::splat(size * 0.14));
    let b = Rect::from_min_size(def_r.min + vec2(size * 0.1, size * 0.1), Vec2::splat(size * 0.14));
    ui.painter().rect_filled(b, 0.0, Color32::BLACK);
    ui.painter().rect_stroke(b, 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
    ui.painter().rect_filled(a, 0.0, Color32::WHITE);
    ui.painter().rect_stroke(a, 0.0, Stroke::new(1.0, Color32::BLACK), StrokeKind::Inside);
    (
        fill_resp.on_hover_text("Fill (X)").clicked(),
        stroke_resp.on_hover_text("Stroke (X)").clicked(),
        swap.on_hover_text("Swap Fill and Stroke (Shift+X)").clicked(),
        def.on_hover_text("Default Fill and Stroke (D)").clicked(),
    )
}

/// A compact dropdown. Returns the chosen index.
pub fn dropdown(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, current: &str, options: &[&str], width: f32) -> Option<usize> {
    combo(ui, id, current, width, |ui| {
        let mut chosen = None;
        for (i, o) in options.iter().enumerate() {
            if ui.selectable_label(*o == current, *o).clicked() {
                chosen = Some(i);
            }
        }
        chosen
    })
}

/// The recessed combo box of [`dropdown`] showing `current`; `list` draws the options and returns
/// the chosen one.
fn combo<R>(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    current: &str,
    width: f32,
    list: impl FnOnce(&mut Ui) -> Option<R>,
) -> Option<R> {
    let t = Tokens::get(ui.ctx());
    egui::Frame::NONE
        .fill(t.input)
        .stroke(Stroke::new(1.0, t.input_border))
        .corner_radius(CornerRadius::same(3))
        .show(ui, |ui| {
            egui::ComboBox::from_id_salt(ui.id().with(id))
                .selected_text(egui::RichText::new(current).size(12.0))
                .width(width - 4.0)
                .show_ui(ui, list)
                .inner
                .flatten()
        })
        .inner
}

/// Whether the blend-mode list draws a separator above `BlendMode::ALL[i]`: where a
/// [`BlendMode::group`] starts.
pub fn blend_separator_before(i: usize) -> bool {
    i > 0 && i < BlendMode::ALL.len() && BlendMode::ALL[i].group() != BlendMode::ALL[i - 1].group()
}

/// The blend-mode dropdown, its groups separated (Normal | darken | lighten | contrast | inversion
/// | component modes). Returns the chosen mode.
pub fn blend_dropdown(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, current: BlendMode, width: f32) -> Option<BlendMode> {
    combo(ui, id, current.label(), width, |ui| {
        let mut chosen = None;
        for (i, m) in BlendMode::ALL.into_iter().enumerate() {
            if blend_separator_before(i) {
                ui.separator();
            }
            if ui.selectable_label(m == current, m.label()).clicked() {
                chosen = Some(m);
            }
        }
        chosen
    })
}

/// The blend mode an effect parameter names (`"mode": "multiply"`), which editors show as a
/// [`blend_param_dropdown`].
pub fn blend_param(key: &str, value: &serde_json::Value) -> Option<BlendMode> {
    value.as_str().filter(|_| key == "mode").and_then(BlendMode::parse)
}

/// [`blend_dropdown`] for a blend-mode parameter; returns the chosen mode's parameter value.
pub fn blend_param_dropdown(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, current: BlendMode) -> Option<serde_json::Value> {
    blend_dropdown(ui, id, current, 120.0).map(|m| serde_json::Value::String(m.label().to_ascii_lowercase()))
}

/// An edit made with [`opacity_blend`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransparencyEdit {
    Blend(BlendMode),
    /// Opacity in percent, with the slider's drag phase (`Released` for a typed value).
    Opacity(f64, Live),
}

/// The transparency controls shared by the Transparency panel and the Appearance panel's Opacity
/// popups: the grouped blend-mode dropdown, the opacity percent field and its slider popup.
pub fn opacity_blend(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    opacity: f32,
    blend: BlendMode,
    enabled: bool,
) -> Option<TransparencyEdit> {
    let t = Tokens::get(ui.ctx());
    let mut edit = None;
    ui.horizontal(|ui| {
        ui.add_enabled_ui(enabled, |ui| {
            if let Some(m) = blend_dropdown(ui, (id, "blend"), blend, 104.0) {
                edit = Some(TransparencyEdit::Blend(m));
            }
        });
        dim_label(ui, "Opacity:");
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.add_enabled_ui(enabled, |ui| {
            if let Some(o) = plain_field(ui, (id, "opacity"), opacity as f64 * 100.0, "%", 0, 50.0) {
                edit = Some(TransparencyEdit::Opacity(o.clamp(0.0, 100.0), Live::Released));
            }
        });
        let (r, resp) = ui.allocate_exact_size(vec2(18.0, 26.0), if enabled { Sense::click() } else { Sense::hover() });
        ui.painter().rect_stroke(r, 2, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
        icons::paint(ui, "chevron-right", r.shrink2(vec2(3.0, 7.0)), if enabled { t.icon } else { t.text_disabled });
        egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            let mut o = opacity * 100.0;
            let r = ui.add(egui::Slider::new(&mut o, 0.0..=100.0).show_value(false));
            let phase = if r.drag_stopped() || (r.changed() && !r.dragged()) {
                Live::Released
            } else if r.changed() {
                Live::Dragging
            } else {
                return;
            };
            edit = Some(TransparencyEdit::Opacity(o.round() as f64, phase));
        });
    });
    edit
}

/// The 3×3 reference point locator. Returns a new index when clicked.
pub fn reference_point(ui: &mut Ui, current: usize) -> Option<usize> {
    let t = Tokens::get(ui.ctx());
    let size = 22.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let mut out = None;
    let step = size / 2.0 - 2.5;
    ui.painter().rect_stroke(rect.shrink(4.5), 0.0, Stroke::new(1.0, t.text_dim), StrokeKind::Middle);
    for i in 0..9 {
        let c = Pos2::new(rect.left() + 3.0 + (i % 3) as f32 * step + 1.5, rect.top() + 3.0 + (i / 3) as f32 * step + 1.5);
        let r = Rect::from_center_size(c, Vec2::splat(5.0));
        let resp = ui.interact(r.expand(1.5), ui.id().with(("refpt", i)), Sense::click());
        if resp.clicked() {
            out = Some(i);
        }
        if i == current {
            ui.painter().rect_filled(r, 0.0, t.text);
        } else {
            ui.painter().rect_filled(r, 0.0, t.panel);
            ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t.text_dim), StrokeKind::Inside);
        }
    }
    out
}

/// A toggle icon (e.g. eye / lock columns) — returns clicked.
pub fn toggle_icon(ui: &mut Ui, on_icon: &str, on: bool, size: f32, tip: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    if on {
        icons::paint(ui, on_icon, rect.shrink(size * 0.2), if resp.hovered() { t.text } else { t.icon });
    } else if resp.hovered() {
        icons::paint(ui, on_icon, rect.shrink(size * 0.2), t.text_disabled);
    }
    resp.on_hover_text(tip).clicked()
}

// ---------- panel widgets (Swatches, Color, Stroke, Gradient, Appearance, …) ----------

/// Icon button that can be disabled (greyed, no hover, never clicked).
pub fn icon_button_enabled(ui: &mut Ui, icon: &str, tip: &str, selected: bool, enabled: bool, size: f32) -> Response {
    if enabled {
        return icon_button(ui, icon, tip, selected, size);
    }
    let t = Tokens::get(ui.ctx());
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let pad = (size * 0.2).round();
    icons::paint(ui, icon, rect.shrink(pad), t.text_disabled);
    if tip.is_empty() { resp } else { resp.on_hover_text(tip) }
}

/// Regular-weight panel sub-header ("Shape Modes:", "Align Objects:").
pub fn subheader(ui: &mut Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(text).size(12.5).color(t.text));
}

/// A label drawn with a dotted underline (Illustrator's link labels: "Stroke:", "Opacity:").
pub fn link_label(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(text.to_string(), egui::FontId::proportional(12.5), t.text_strong);
    let (rect, resp) = ui.allocate_exact_size(galley.size() + vec2(0.0, 3.0), Sense::click());
    let y = rect.top() + galley.size().y + 1.0;
    ui.painter().galley(rect.min, galley, t.text_strong);
    let mut x = rect.left();
    while x < rect.right() - 1.0 {
        ui.painter().line_segment([pos2(x, y), pos2((x + 1.0).min(rect.right()), y)], Stroke::new(1.0, t.text_dim));
        x += 2.5;
    }
    resp
}

/// The row of a checkbox or radio button: allocates a 13 pt box plus `label`, draws the label and
/// returns the box, the response and the box's border colour.
fn choice_row(ui: &mut Ui, label: &str, enabled: bool) -> (Rect, Response, Color32) {
    let t = Tokens::get(ui.ctx());
    let galley = ui.painter().layout_no_wrap(label.to_string(), egui::FontId::proportional(12.5), if enabled { t.text } else { t.text_disabled });
    let (rect, resp) =
        ui.allocate_exact_size(vec2(18.0 + galley.size().x, 20.0f32.max(galley.size().y)), if enabled { Sense::click() } else { Sense::hover() });
    let bx = Rect::from_min_size(pos2(rect.left(), rect.center().y - 6.5), Vec2::splat(13.0));
    ui.painter().galley(pos2(bx.right() + 5.0, rect.center().y - galley.size().y / 2.0), galley, t.text);
    let border = if !enabled {
        t.divider
    } else if resp.hovered() {
        t.text
    } else {
        t.button_border
    };
    (bx, resp, border)
}

/// Panel-style checkbox with a disabled state. Returns true when toggled.
pub fn check(ui: &mut Ui, label: &str, value: bool, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let (bx, resp, border) = choice_row(ui, label, enabled);
    ui.painter().rect_filled(bx, CornerRadius::same(2), if value && enabled { t.accent_strong } else { t.input });
    ui.painter().rect_stroke(bx, CornerRadius::same(2), Stroke::new(1.0, border), StrokeKind::Inside);
    if value {
        let c = if enabled { Color32::WHITE } else { t.text_disabled };
        ui.painter().line_segment([bx.left_center() + vec2(3.0, 0.0), bx.center_bottom() + vec2(-1.0, -3.5)], Stroke::new(1.6, c));
        ui.painter().line_segment([bx.center_bottom() + vec2(-1.0, -3.5), bx.right_top() + vec2(-3.0, 3.0)], Stroke::new(1.6, c));
    }
    enabled && resp.clicked()
}

/// Radio button in the style of [`check`], with a disabled state. Returns true when clicked.
pub fn radio(ui: &mut Ui, label: &str, selected: bool, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let (bx, resp, border) = choice_row(ui, label, enabled);
    let c = bx.center();
    ui.painter().circle(c, 6.0, if selected && enabled { t.accent_strong } else { t.input }, Stroke::new(1.0, border));
    if selected {
        ui.painter().circle_filled(c, 2.5, if enabled { Color32::WHITE } else { t.text_disabled });
    }
    enabled && resp.clicked()
}

/// Numeric field with an up/down spinner on the left and a preset dropdown on the right
/// (Stroke weight, font size, leading…). `presets` empty = no dropdown. Returns the committed value.
#[allow(clippy::too_many_arguments)]
pub fn spin_field(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    value: Option<f64>,
    unit: Unit,
    width: f32,
    step: f64,
    min: f64,
    presets: &[f64],
) -> Option<f64> {
    spin_generic(ui, value, width, step, min, presets, &|v| unit.format(v), &mut |ui, fw| num_field(ui, id, value, unit, fw))
}

/// [`spin_field`] for unitless values (percent, degrees, 1/1000 em) with a display suffix.
#[allow(clippy::too_many_arguments)]
pub fn spin_plain(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug + Copy,
    value: f64,
    suffix: &str,
    decimals: usize,
    width: f32,
    step: f64,
    min: f64,
    presets: &[f64],
) -> Option<f64> {
    let fmt = |v: f64| format!("{v}{suffix}");
    spin_generic(ui, Some(value), width, step, min, presets, &fmt, &mut |ui, fw| plain_field(ui, id, value, suffix, decimals, fw))
}

#[allow(clippy::too_many_arguments)]
fn spin_generic(
    ui: &mut Ui,
    value: Option<f64>,
    width: f32,
    step: f64,
    min: f64,
    presets: &[f64],
    fmt: &dyn Fn(f64) -> String,
    field: &mut dyn FnMut(&mut Ui, f32) -> Option<f64>,
) -> Option<f64> {
    let t = Tokens::get(ui.ctx());
    let mut out = None;
    let h = 26.0;
    let enabled = ui.is_enabled();
    ui.scope(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        let (sr, sresp) = ui.allocate_exact_size(vec2(16.0, h), Sense::click());
        ui.painter().rect_filled(sr, CornerRadius { nw: 2, sw: 2, ne: 0, se: 0 }, t.input);
        ui.painter().rect_stroke(sr, CornerRadius { nw: 2, sw: 2, ne: 0, se: 0 }, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
        let up = Rect::from_min_max(sr.min, pos2(sr.right(), sr.center().y));
        let down = Rect::from_min_max(pos2(sr.left(), sr.center().y), sr.max);
        let hover = sresp.hover_pos();
        for (r, is_up) in [(up, true), (down, false)] {
            let c = if !enabled {
                t.text_disabled
            } else if hover.is_some_and(|p| r.contains(p)) {
                t.text_strong
            } else {
                t.icon
            };
            let m = r.center();
            let d = if is_up { -1.5 } else { 1.5 };
            ui.painter().line_segment([m + vec2(-3.0, -d), m + vec2(0.0, d)], Stroke::new(1.2, c));
            ui.painter().line_segment([m + vec2(0.0, d), m + vec2(3.0, -d)], Stroke::new(1.2, c));
        }
        if sresp.clicked()
            && let Some(p) = sresp.interact_pointer_pos()
        {
            let v = value.unwrap_or(0.0);
            let nv = if up.contains(p) { v + step } else { v - step };
            out = Some(nv.max(min));
        }
        let fw = if presets.is_empty() { width - 16.0 } else { width - 36.0 };
        if let Some(v) = field(ui, fw) {
            out = Some(v.max(min));
        }
        if !presets.is_empty() {
            let (dr, dresp) = ui.allocate_exact_size(vec2(20.0, h), Sense::click());
            ui.painter().rect_filled(dr, CornerRadius { nw: 0, sw: 0, ne: 2, se: 2 }, t.input);
            ui.painter().rect_stroke(dr, CornerRadius { nw: 0, sw: 0, ne: 2, se: 2 }, Stroke::new(1.0, t.input_border), StrokeKind::Inside);
            let c = if !enabled {
                t.text_disabled
            } else if dresp.hovered() {
                t.text_strong
            } else {
                t.icon
            };
            icons::paint(ui, "chevron-down", Rect::from_center_size(dr.center(), Vec2::splat(12.0)), c);
            egui::Popup::menu(&dresp).show(|ui| {
                ui.set_min_width(width - 10.0);
                for p in presets {
                    if ui.selectable_label(value.is_some_and(|v| (v - p).abs() < 1e-6), fmt(*p)).clicked() {
                        out = Some(*p);
                    }
                }
            });
        }
    });
    out
}

/// Drag phase of a live slider/handle: previews while dragging, commits on release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Live {
    Idle,
    Dragging,
    Released,
}

/// A colour slider with a gradient track (`track(t)` gives the colour at 0..1) and a triangular
/// thumb below it, like Illustrator's Color panel. Returns the new normalized value and the phase.
pub fn color_slider(
    ui: &mut Ui,
    id: impl std::hash::Hash + std::fmt::Debug,
    value: f32,
    width: f32,
    track: &dyn Fn(f32) -> Color32,
) -> (Option<f32>, Live) {
    let t = Tokens::get(ui.ctx());
    let (rect, _) = ui.allocate_exact_size(vec2(width, 22.0), Sense::hover());
    let bar = Rect::from_min_size(pos2(rect.left() + 4.0, rect.top() + 4.0), vec2(width - 8.0, 7.0));
    let resp = ui.interact(rect, ui.id().with(id), Sense::click_and_drag());
    let n = (bar.width() / 2.0).ceil().max(2.0) as usize;
    let seg = bar.width() / n as f32;
    for i in 0..n {
        let r = Rect::from_min_size(pos2(bar.left() + i as f32 * seg, bar.top()), vec2(seg + 0.6, bar.height()));
        ui.painter().rect_filled(r, 0.0, track((i as f32 + 0.5) / n as f32));
    }
    ui.painter().rect_stroke(bar, 0.0, Stroke::new(1.0, t.border), StrokeKind::Outside);
    let v = value.clamp(0.0, 1.0);
    let x = bar.left() + v * bar.width();
    let tip = pos2(x, bar.bottom() - 1.0);
    let thumb = vec![tip, pos2(x + 5.5, tip.y + 6.0), pos2(x + 5.5, tip.y + 10.0), pos2(x - 5.5, tip.y + 10.0), pos2(x - 5.5, tip.y + 6.0)];
    let fill = if resp.dragged() || resp.hovered() { Color32::WHITE } else { t.icon };
    ui.painter().add(egui::Shape::convex_polygon(thumb, fill, Stroke::new(1.0, t.border)));
    let mut out = None;
    let mut phase = Live::Idle;
    if (resp.dragged() || resp.clicked() || resp.drag_stopped())
        && let Some(p) = resp.interact_pointer_pos()
    {
        out = Some(((p.x - bar.left()) / bar.width()).clamp(0.0, 1.0));
        phase = if resp.dragged() && !resp.drag_stopped() { Live::Dragging } else { Live::Released };
    }
    (out, phase)
}

/// One swatch tile (Illustrator: 1 px dark frame, white inset on hover/selection).
pub fn swatch_tile(ui: &Ui, rect: Rect, paint: &Paint, selected: bool, hovered: bool) {
    let t = Tokens::get(ui.ctx());
    paint_chip(ui, rect, paint);
    ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
    if selected || hovered {
        ui.painter().rect_stroke(rect.shrink(1.0), 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
        ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, if selected { t.accent } else { t.text }), StrokeKind::Outside);
    }
}

/// A bordered list box (Swatches tiles, Appearance rows, Artboards list).
pub fn list_box<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let t = Tokens::get(ui.ctx());
    egui::Frame::NONE.stroke(Stroke::new(1.0, t.input_border)).inner_margin(egui::Margin::same(1)).show(ui, add).inner
}

/// A bottom button bar separated from the content by a divider (panel footers).
pub fn bottom_bar(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, 0.0, t.divider);
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        add(ui)
    });
}

/// A menu row for panel (≡) menus: label, optional check mark, disabled when not implemented.
pub fn menu_item(ui: &mut Ui, label: &str, enabled: bool, checked: bool) -> bool {
    let text = if checked { format!("✓ {label}") } else { format!("   {label}") };
    ui.add_enabled(enabled, egui::Button::new(egui::RichText::new(text).size(12.5)).frame(false)).clicked()
}

/// A number field that may be empty (Stroke dash/gap). Returns `Some(new)` on commit, where
/// `new` is `None` when the field was cleared.
pub fn opt_field(ui: &mut Ui, id: impl std::hash::Hash + std::fmt::Debug, value: Option<f64>, width: f32) -> Option<Option<f64>> {
    let t = Tokens::get(ui.ctx());
    let id = ui.id().with(id);
    let shown = value.map(|v| format!("{v}")).unwrap_or_default();
    let editing = ui.memory(|m| m.has_focus(id));
    let mut buf: String = if editing { ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone()) } else { shown.clone() };
    let enabled = ui.is_enabled();
    let resp = egui::Frame::NONE
        .fill(t.input)
        .stroke(Stroke::new(1.0, if editing { t.accent } else { t.input_border }))
        .corner_radius(CornerRadius::same(2))
        .inner_margin(egui::Margin::symmetric(4, 4))
        .show(ui, |ui| {
            ui.add(
                egui::TextEdit::singleline(&mut buf)
                    .id(id)
                    .frame(egui::Frame::NONE)
                    .desired_width(width - 10.0)
                    .font(egui::FontId::proportional(12.5))
                    .text_color(if enabled { t.text_strong } else { t.text_disabled }),
            )
        })
        .inner;
    ui.data_mut(|d| d.insert_temp(id, buf.clone()));
    if resp.lost_focus() && buf != shown {
        let s = buf.trim().trim_end_matches("pt").trim();
        if s.is_empty() { Some(None) } else { s.parse::<f64>().ok().map(Some) }
    } else {
        None
    }
}
