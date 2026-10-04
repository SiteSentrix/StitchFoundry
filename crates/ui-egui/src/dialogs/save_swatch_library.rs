//! Save Swatch Library: a name, a file format and where it goes: the user library folder (listed
//! under Window → Swatch Libraries → User Defined) or a file picked in a save dialog (a download
//! on the web). OK runs `swatch.library.save`.
//!
//! Fields: `name`, `format` (`vcswatches`, `gpl` or `css`), `user` (save to the user library
//! folder), `selectedOnly` and `names` (the swatches selected in the Swatches panel), `__user`
//! (there is a user library folder).

use serde_json::{Value, json};
use vectorcraft_color::palette_io::PaletteFormat;

use super::swatch_options::{grid, label};
use super::{DialogSpec, form, run_and_close};
use crate::state::Dialog;
use crate::{VectorcraftApp, widgets};

/// The dialog kind of Save Swatch Library.
pub const KIND: &str = "saveSwatchLibrary";

pub(super) const SPEC: DialogSpec = DialogSpec { heading: |_| "Save Swatch Library".into(), body, confirm, min_width: 360.0, ..DialogSpec::FORM };

/// Open Save Swatch Library for the document's swatches (`names`: the ones selected in the panel).
pub fn open(app: &mut VectorcraftApp, names: Vec<String>) -> Result<Value, String> {
    let st = app.session.active().ok_or("no document open")?;
    let title = st.title();
    let name = title.rsplit_once('.').map_or(title.as_str(), |(s, _)| s).to_string();
    let user = app.session.swatch_libraries.user_dir().is_some();
    let fields = json!({
        "name": name, "format": PaletteFormat::Native.id(), "user": user, "__user": user,
        "selectedOnly": false, "names": names,
    });
    app.ui.dialog = Some(Dialog::new(KIND, fields));
    Ok(Value::Null)
}

fn format_of(d: &Dialog) -> PaletteFormat {
    PaletteFormat::parse(&d.str("format")).unwrap_or(PaletteFormat::Native)
}

fn body(_: &mut VectorcraftApp, ui: &mut egui::Ui, d: &mut Dialog) -> bool {
    let labels = PaletteFormat::ALL.map(PaletteFormat::label);
    let names = d.fields.get("names").and_then(Value::as_array).map_or(0, Vec::len);
    let set = |d: &mut Dialog, key: &str, v: Value| {
        d.fields.insert(key.into(), v);
    };
    grid(ui, |ui| {
        label(ui, "Name:");
        form::text(ui, d, "name", 220.0);
        ui.end_row();
        label(ui, "Format:");
        if let Some(i) = widgets::dropdown(ui, "library-format", format_of(d).label(), &labels, 230.0) {
            set(d, "format", json!(PaletteFormat::ALL[i].id()));
        }
        ui.end_row();
        label(ui, "Save To:");
        let (user, has_user) = (d.bool("user"), d.bool("__user"));
        if widgets::radio(ui, "User Defined Libraries", user, has_user) {
            set(d, "user", json!(true));
        }
        ui.end_row();
        ui.label("");
        if widgets::radio(ui, "A File…", !user, true) {
            set(d, "user", json!(false));
        }
        ui.end_row();
        ui.label("");
        let only = d.bool("selectedOnly");
        if widgets::check(ui, &format!("Selected Swatches Only ({names})"), only, names > 0) {
            set(d, "selectedOnly", json!(!only));
        }
        ui.end_row();
    });
    if format_of(d) == PaletteFormat::Gpl {
        widgets::dim_label(ui, "GPL palettes keep solid colours only, as RGB.");
    }
    false
}

/// `swatch.library.save` parameters from the fields.
fn params(d: &Dialog) -> Value {
    let mut p = json!({"name": d.str("name"), "format": format_of(d).id(), "user": d.bool("user") && d.bool("__user")});
    if d.bool("selectedOnly") {
        p["names"] = d.fields.get("names").cloned().unwrap_or_else(|| json!([]));
    }
    p
}

fn confirm(app: &mut VectorcraftApp, d: &Dialog) -> Result<Value, String> {
    let p = params(d);
    if p["user"] == json!(true) {
        let r = run_and_close(app, "swatch.library.save", p)?;
        app.status(format!("Saved to User Defined: {}", d.str("name")));
        return Ok(r);
    }
    app.ui.dialog = None;
    crate::io::save_command_output(app, "swatch.library.save", format_of(d).id(), p).map(|path| json!({ "path": path }))
}
