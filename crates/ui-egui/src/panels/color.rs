//! Color panel: Grayscale / RGB / HSB / CMYK / Web Safe RGB sliders with colour-gradient tracks,
//! value fields, hex field, None/Black/White chips, spectrum ramp and the Fill/Stroke proxy.
//! When the active paint is a gradient, the sliders edit the Gradient panel's selected stop.

use egui::{Color32, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use vectorcraft_color::{Color, Paint};

use super::{active_paint, color_json, live_run, paint_target, pstate, push_recent, set_pstate};
use crate::theme::Tokens;
use crate::widgets::{self, Live, menu_item};
use crate::{VectorcraftApp, icons};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    Grayscale,
    #[default]
    Rgb,
    Hsb,
    Cmyk,
    WebSafe,
}

impl Mode {
    pub const ALL: [(Mode, &'static str); 5] =
        [(Mode::Grayscale, "Grayscale"), (Mode::Rgb, "RGB"), (Mode::Hsb, "HSB"), (Mode::Cmyk, "CMYK"), (Mode::WebSafe, "Web Safe RGB")];

    pub fn labels(self) -> &'static [&'static str] {
        match self {
            Mode::Grayscale => &["K"],
            Mode::Rgb | Mode::WebSafe => &["R", "G", "B"],
            Mode::Hsb => &["H", "S", "B"],
            Mode::Cmyk => &["C", "M", "Y", "K"],
        }
    }
    /// Maximum of each displayed component (minimum is 0).
    pub fn max(self, i: usize) -> f32 {
        match self {
            Mode::Rgb | Mode::WebSafe => 255.0,
            Mode::Hsb if i == 0 => 360.0,
            _ => 100.0,
        }
    }
    pub fn suffix(self, i: usize) -> &'static str {
        match self {
            Mode::Rgb | Mode::WebSafe => "",
            Mode::Hsb if i == 0 => "°",
            _ => "%",
        }
    }
    /// The mode a colour was authored in.
    pub fn of(c: &Color) -> Mode {
        match c {
            Color::Rgb { .. } => Mode::Rgb,
            Color::Cmyk { .. } => Mode::Cmyk,
            Color::Gray { .. } => Mode::Grayscale,
        }
    }
}

/// Displayed component values of `c` in `mode` (RGB 0–255, HSB °/%/%, CMYK %, K %).
pub fn components(mode: Mode, c: &Color) -> Vec<f32> {
    match mode {
        Mode::Grayscale => {
            let k = match *c {
                Color::Gray { k } => k,
                _ => {
                    let [r, g, b] = c.to_rgb();
                    1.0 - (0.3 * r + 0.59 * g + 0.11 * b)
                }
            };
            vec![k * 100.0]
        }
        Mode::Rgb | Mode::WebSafe => c.to_rgb().iter().map(|v| (v * 255.0).clamp(0.0, 255.0)).collect(),
        Mode::Hsb => {
            let [h, s, b] = c.to_hsb();
            vec![h, s * 100.0, b * 100.0]
        }
        Mode::Cmyk => c.to_cmyk().iter().map(|v| v * 100.0).collect(),
    }
}

/// Build a colour from displayed components (inverse of [`components`]).
pub fn from_components(mode: Mode, v: &[f32]) -> Color {
    let g = |i: usize| v.get(i).copied().unwrap_or(0.0);
    match mode {
        Mode::Grayscale => Color::gray((g(0) / 100.0).clamp(0.0, 1.0)),
        Mode::Rgb => Color::rgb((g(0) / 255.0).clamp(0.0, 1.0), (g(1) / 255.0).clamp(0.0, 1.0), (g(2) / 255.0).clamp(0.0, 1.0)),
        Mode::WebSafe => web_safe(&Color::rgb(g(0) / 255.0, g(1) / 255.0, g(2) / 255.0)),
        Mode::Hsb => Color::from_hsb(g(0).rem_euclid(360.0), (g(1) / 100.0).clamp(0.0, 1.0), (g(2) / 100.0).clamp(0.0, 1.0)),
        Mode::Cmyk => Color::cmyk(
            (g(0) / 100.0).clamp(0.0, 1.0),
            (g(1) / 100.0).clamp(0.0, 1.0),
            (g(2) / 100.0).clamp(0.0, 1.0),
            (g(3) / 100.0).clamp(0.0, 1.0),
        ),
    }
}

/// The nearest web-safe colour (each channel a multiple of 0x33).
pub fn web_safe(c: &Color) -> Color {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 5.0).round() / 5.0;
    let [r, g, b] = c.to_rgb();
    Color::rgb(q(r), q(g), q(b))
}

pub fn is_web_safe(c: &Color) -> bool {
    web_safe(c).to_hex() == c.to_hex()
}

/// Track colour of slider `i` at position `t` (0..1) with the other components fixed —
/// Illustrator's dynamic colour sliders.
pub fn track_color(mode: Mode, comps: &[f32], i: usize, t: f32) -> Color {
    let mut v = comps.to_vec();
    if i < v.len() {
        v[i] = t * mode.max(i);
    }
    // HSB hue track at full saturation/brightness reads better when S or B are 0.
    let m = if mode == Mode::WebSafe { Mode::Rgb } else { mode };
    from_components(m, &v)
}

/// Colour at a point of the spectrum ramp: hue across; white at the top, full colour in the
/// middle, black at the bottom. In Grayscale mode the ramp is a grey ramp.
pub fn spectrum_at(mode: Mode, x: f32, y: f32) -> Color {
    let x = x.clamp(0.0, 1.0);
    let y = y.clamp(0.0, 1.0);
    if mode == Mode::Grayscale {
        return Color::gray(x);
    }
    let h = x * 360.0;
    let c = if y < 0.5 { Color::from_hsb(h, y * 2.0, 1.0) } else { Color::from_hsb(h, 1.0, 1.0 - (y - 0.5) * 2.0) };
    match mode {
        Mode::Cmyk => {
            let [c0, m, yy, k] = c.to_cmyk();
            Color::cmyk(c0, m, yy, k)
        }
        Mode::WebSafe => web_safe(&c),
        _ => c,
    }
}

/// Parse a hex field (`E67828`, `#e67828`, `fff`).
pub fn parse_hex(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    if !(s.len() == 3 || s.len() == 6) || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Color::from_hex(s)
}

fn to32(c: &Color) -> Color32 {
    super::c32(c)
}

/// What the panel is editing: the active proxy's solid colour, or a selected gradient stop.
enum Target {
    Paint(Option<Color>),
    Stop { paint: Paint, index: usize, color: Color },
}

fn target(app: &VectorcraftApp) -> Target {
    let p = active_paint(app);
    match &p {
        Paint::Gradient(g) => {
            let i = app.session.gradient_stop.unwrap_or(0).min(g.gradient.stops.len().saturating_sub(1));
            let color = g.gradient.stops.get(i).map(|s| s.color).unwrap_or(Color::BLACK);
            Target::Stop { paint: p.clone(), index: i, color }
        }
        Paint::Solid { color, .. } => Target::Paint(Some(*color)),
        _ => Target::Paint(None),
    }
}

fn apply(app: &mut VectorcraftApp, ui: &Ui, tgt: &Target, c: Color, phase: Live) {
    match tgt {
        Target::Paint(_) => {
            let cmd = paint_target(app);
            live_run(app, "Color", cmd, json!({"color": color_json(&c)}), phase);
        }
        Target::Stop { paint, index, .. } => {
            if let Paint::Gradient(g) = paint {
                let mut stops = g.gradient.stops.clone();
                if let Some(s) = stops.get_mut(*index) {
                    s.color = c;
                }
                let params = json!({"stroke": !app.session.fill_active, "stops": super::gradient::stops_json(&stops)});
                live_run(app, "Gradient", "paint.editGradient", params, phase);
            }
        }
    }
    if phase == Live::Released {
        push_recent(ui.ctx(), c);
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let tgt = target(app);
    let color = match &tgt {
        Target::Paint(c) => *c,
        Target::Stop { color, .. } => Some(*color),
    };
    let stored: Option<Mode> = pstate(ui.ctx(), "color-mode");
    let mode = stored.unwrap_or_else(|| color.as_ref().map(Mode::of).unwrap_or_default());
    let show_options: bool = !pstate::<bool>(ui.ctx(), "color-hide-options");
    // Keep the displayed components while the colour is unchanged (hue survives S = 0 etc.).
    let key = color.map(|c| (c.to_hex(), mode as u8));
    let comps: Vec<f32> = match (color, pstate::<Option<((String, u8), Vec<f32>)>>(ui.ctx(), "color-comps")) {
        (Some(_), Some((k, v))) if Some(&k) == key.as_ref() => v,
        (Some(c), _) => components(mode, &c),
        (None, _) => vec![0.0; mode.labels().len()],
    };
    if show_options {
        super::recent_colors_row(app, ui);
        widgets::divider(ui);
    }
    let mut new: Option<(Color, Live, Vec<f32>)> = None;
    ui.horizontal(|ui| {
        // Left column: proxy + web-safe warning.
        ui.vertical(|ui| {
            ui.set_width(44.0);
            super::proxy(app, ui, 40.0);
            if let Some(c) = color
                && mode != Mode::WebSafe
                && mode != Mode::Grayscale
                && !is_web_safe(&c)
            {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;
                    icons::icon(ui, "dc-cube", 16.0, t.icon).on_hover_text("Out of Web Color Warning");
                    let ws = web_safe(&c);
                    let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                    widgets::swatch_tile(ui, r, &Paint::solid(ws), false, resp.hovered());
                    if resp.on_hover_text(format!("Click to correct to the closest web color ({})", ws.to_hex())).clicked() {
                        new = Some((ws, Live::Released, components(mode, &ws)));
                    }
                });
            }
        });
        // Sliders.
        ui.vertical(|ui| {
            let labels = mode.labels();
            let field_w = 48.0;
            let slider_w = (ui.available_width() - field_w - 24.0).max(60.0);
            for (i, lbl) in labels.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.add_sized(vec2(12.0, 22.0), egui::Label::new(egui::RichText::new(*lbl).size(12.5).color(t.text)));
                    let max = mode.max(i);
                    let v = comps.get(i).copied().unwrap_or(0.0);
                    let enabled = color.is_some();
                    let track = |x: f32| to32(&track_color(mode, &comps, i, x));
                    let grey = |_x: f32| t.input;
                    let (nv, phase) = if enabled {
                        widgets::color_slider(ui, ("color-slider", i), v / max, slider_w, &track)
                    } else {
                        widgets::color_slider(ui, ("color-slider", i), 0.0, slider_w, &grey)
                    };
                    if let Some(nv) = nv
                        && enabled
                    {
                        let mut c2 = comps.clone();
                        c2[i] = if mode == Mode::WebSafe { ((nv * 5.0).round() / 5.0) * max } else { (nv * max).round() };
                        new = Some((from_components(mode, &c2), phase, c2));
                    }
                    let dec = 0;
                    if let Some(fv) = widgets::plain_field(ui, ("color-field", i), v as f64, mode.suffix(i), dec, field_w)
                        && enabled
                    {
                        let mut c2 = comps.clone();
                        c2[i] = (fv as f32).clamp(0.0, max);
                        new = Some((from_components(mode, &c2), Live::Released, c2));
                    }
                });
            }
        });
    });
    ui.add_space(4.0);
    // None / Black / White chips and the hex field.
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        for (p, tip) in [(Paint::None, "None"), (Paint::solid(Color::BLACK), "Black"), (Paint::solid(Color::WHITE), "White")] {
            let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
            widgets::swatch_tile(ui, r, &p, false, resp.hovered());
            if resp.on_hover_text(tip).clicked() {
                match p.color() {
                    Some(c) => new = Some((c, Live::Released, components(mode, &c))),
                    None => {
                        app.run(paint_target(app), json!({"none": true})).ok();
                    }
                }
            }
        }
        ui.spacing_mut().item_spacing.x = 6.0;
        if matches!(mode, Mode::Rgb | Mode::WebSafe | Mode::Hsb) {
            ui.add_space((ui.available_width() - 96.0).max(4.0));
            ui.label(egui::RichText::new("#").size(15.0).color(t.text));
            let hex = color.map(|c| c.to_hex().trim_start_matches('#').to_uppercase()).unwrap_or_default();
            let id = ui.id().with("hex");
            let editing = ui.memory(|m| m.has_focus(id));
            let mut buf: String = if editing { ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| hex.clone()) } else { hex.clone() };
            let resp = egui::Frame::NONE
                .fill(t.input)
                .stroke(Stroke::new(1.0, if editing { t.accent } else { t.input_border }))
                .corner_radius(2)
                .inner_margin(egui::Margin::symmetric(6, 4))
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut buf)
                            .id(id)
                            .frame(egui::Frame::NONE)
                            .desired_width(62.0)
                            .char_limit(7)
                            .font(egui::FontId::proportional(12.5))
                            .text_color(t.text_strong),
                    )
                })
                .inner;
            ui.data_mut(|d| d.insert_temp(id, buf.clone()));
            if resp.lost_focus()
                && buf != hex
                && let Some(c) = parse_hex(&buf)
            {
                new = Some((c, Live::Released, components(mode, &c)));
            }
        }
    });
    ui.add_space(4.0);
    // Spectrum ramp.
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 46.0), Sense::click_and_drag());
    let cols = 72;
    let rows = 12;
    let (cw, rh) = (r.width() / cols as f32, r.height() / rows as f32);
    let mut mesh = egui::Mesh::default();
    for j in 0..=rows {
        for i in 0..=cols {
            let c = spectrum_at(mode, i as f32 / cols as f32, j as f32 / rows as f32);
            mesh.colored_vertex(pos2(r.left() + i as f32 * cw, r.top() + j as f32 * rh), to32(&c));
        }
    }
    for j in 0..rows {
        for i in 0..cols {
            let a = (j * (cols + 1) + i) as u32;
            let b = a + 1;
            let c = a + (cols + 1) as u32;
            let d = c + 1;
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(b, d, c);
        }
    }
    ui.painter().add(egui::Shape::mesh(mesh));
    ui.painter().rect_stroke(r, 0.0, Stroke::new(1.0, t.border), StrokeKind::Outside);
    if (resp.dragged() || resp.clicked() || resp.drag_stopped())
        && let Some(p) = resp.interact_pointer_pos()
    {
        let c = spectrum_at(mode, (p.x - r.left()) / r.width(), (p.y - r.top()) / r.height());
        let phase = if resp.dragged() && !resp.drag_stopped() { Live::Dragging } else { Live::Released };
        new = Some((c, phase, components(mode, &c)));
    }
    if resp.hovered()
        && let Some(p) = resp.hover_pos()
    {
        ui.painter().rect_stroke(Rect::from_center_size(p, vec2(5.0, 5.0)), 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Middle);
    }
    if let Some((c, phase, comps2)) = new {
        let key = (c.to_hex(), mode as u8);
        set_pstate(ui.ctx(), "color-comps", Some((key, comps2)));
        apply(app, ui, &tgt, c, phase);
    }
}

pub fn menu(app: &mut VectorcraftApp, ui: &mut Ui) {
    let hidden: bool = pstate(ui.ctx(), "color-hide-options");
    if menu_item(ui, if hidden { "Show Options" } else { "Hide Options" }, true, false) {
        set_pstate(ui.ctx(), "color-hide-options", !hidden);
    }
    ui.separator();
    let tgt = target(app);
    let color = match &tgt {
        Target::Paint(c) => *c,
        Target::Stop { color, .. } => Some(*color),
    };
    let cur: Option<Mode> = pstate(ui.ctx(), "color-mode");
    let cur = cur.unwrap_or_else(|| color.as_ref().map(Mode::of).unwrap_or_default());
    for (m, label) in Mode::ALL {
        if menu_item(ui, label, true, m == cur) {
            set_pstate(ui.ctx(), "color-mode", Some(m));
            // Converting the colour's model follows Illustrator (picking CMYK converts the colour).
            if let Some(c) = color {
                let conv = from_components(if m == Mode::Hsb { Mode::Rgb } else { m }, &components(if m == Mode::Hsb { Mode::Rgb } else { m }, &c));
                if conv != c {
                    apply(app, ui, &tgt, conv, Live::Released);
                }
            }
        }
    }
    ui.separator();
    if menu_item(ui, "Invert", color.is_some(), false)
        && let Some(c) = color
    {
        apply(app, ui, &tgt, c.invert(), Live::Released);
    }
    if menu_item(ui, "Complement", color.is_some(), false)
        && let Some(c) = color
    {
        apply(app, ui, &tgt, c.complement(), Live::Released);
    }
    ui.separator();
    if menu_item(ui, "Create New Swatch…", color.is_some(), false)
        && let Some(c) = color
    {
        app.run("swatch.new", json!({"color": color_json(&c)})).ok();
    }
    if menu_item(ui, "Copy Color Value (Hex)", color.is_some(), false)
        && let Some(c) = color
    {
        ui.ctx().copy_text(c.to_hex());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgb_roundtrip() {
        let c = Color::rgb8(230, 120, 40);
        let v = components(Mode::Rgb, &c);
        assert_eq!(v.iter().map(|x| x.round() as i32).collect::<Vec<_>>(), vec![230, 120, 40]);
        assert_eq!(from_components(Mode::Rgb, &v).to_hex(), c.to_hex());
    }

    #[test]
    fn hsb_roundtrip_and_ranges() {
        let c = Color::rgb8(230, 120, 40);
        let v = components(Mode::Hsb, &c);
        assert!(v[0] > 0.0 && v[0] < 360.0 && v[1] <= 100.0 && v[2] <= 100.0);
        assert_eq!(from_components(Mode::Hsb, &v).to_hex(), c.to_hex());
        assert_eq!(Mode::Hsb.max(0), 360.0);
        assert_eq!(Mode::Hsb.suffix(0), "°");
    }

    #[test]
    fn cmyk_and_gray_keep_model() {
        let c = from_components(Mode::Cmyk, &[10.0, 20.0, 30.0, 40.0]);
        assert!(matches!(c, Color::Cmyk { .. }));
        let v = components(Mode::Cmyk, &c);
        assert!((v[3] - 40.0).abs() < 1e-3);
        let g = from_components(Mode::Grayscale, &[25.0]);
        assert_eq!(g, Color::gray(0.25));
        assert!((components(Mode::Grayscale, &g)[0] - 25.0).abs() < 1e-4);
        // Grayscale view of an RGB colour uses luminance.
        assert!((components(Mode::Grayscale, &Color::WHITE)[0]).abs() < 1e-4);
    }

    #[test]
    fn web_safe_snaps() {
        let c = Color::rgb8(230, 120, 40);
        let w = web_safe(&c);
        assert_eq!(w.to_hex(), "#ff6633");
        assert!(is_web_safe(&w));
        assert!(!is_web_safe(&c));
        assert_eq!(from_components(Mode::WebSafe, &[230.0, 120.0, 40.0]).to_hex(), "#ff6633");
    }

    #[test]
    fn slider_tracks_vary_one_component() {
        let comps = [230.0, 120.0, 40.0];
        let a = track_color(Mode::Rgb, &comps, 0, 0.0).to_rgba8(1.0);
        let b = track_color(Mode::Rgb, &comps, 0, 1.0).to_rgba8(1.0);
        assert_eq!((a[0], a[1], a[2]), (0, 120, 40));
        assert_eq!((b[0], b[1], b[2]), (255, 120, 40));
        let h = track_color(Mode::Hsb, &[0.0, 100.0, 100.0], 0, 1.0 / 3.0).to_hex();
        assert_eq!(h, "#00ff00");
    }

    #[test]
    fn spectrum_and_hex() {
        assert_eq!(spectrum_at(Mode::Rgb, 0.0, 0.0).to_hex(), "#ffffff");
        assert_eq!(spectrum_at(Mode::Rgb, 0.0, 0.5).to_hex(), "#ff0000");
        assert_eq!(spectrum_at(Mode::Rgb, 0.3, 1.0).to_hex(), "#000000");
        assert_eq!(spectrum_at(Mode::Grayscale, 1.0, 0.3), Color::gray(1.0));
        assert_eq!(parse_hex("E67828").unwrap().to_hex(), "#e67828");
        assert_eq!(parse_hex("#fff").unwrap().to_hex(), "#ffffff");
        assert!(parse_hex("zz").is_none());
        assert!(parse_hex("12345").is_none());
    }
}
