//! The options of TIFF (colour model, byte order, LZW, profile), after the raster export rows it
//! shares with PNG Options ([`super::png_options`]): the `tiffOptions` dialog.

use serde_json::{Map, Value, json};
use vectorcraft_render::encode::jpeg::ColorModel;
use vectorcraft_render::encode::tiff::{ByteOrder, TiffOptions};

use super::form;
use super::png_options::choice;
use crate::state::Dialog;

type Label<'a> = &'a dyn Fn(&mut egui::Ui, &str) -> egui::Response;

/// The options format `id` starts with (`cmyk`: a CMYK document exports CMYK TIFFs).
pub(super) fn defaults(id: &str, cmyk: bool, o: &mut Map<String, Value>) {
    if id == "tiff" {
        let t = TiffOptions::default();
        let model = if cmyk { ColorModel::Cmyk } else { t.color_model };
        o.extend([
            ("colorModel".into(), json!(model.id())),
            ("byteOrder".into(), json!(t.byte_order.id())),
            ("lzw".into(), json!(t.lzw)),
            ("embedIcc".into(), json!(t.embed_icc)),
        ]);
    }
}

/// Does format `id` keep transparency with the dialog's options (else it is flattened on white)?
pub(super) fn keeps_alpha(id: &str, d: &Dialog) -> bool {
    match id {
        "jpg" => false,
        "tiff" => ColorModel::from_id(&d.str("colorModel")).unwrap_or_default() == ColorModel::Rgb,
        _ => true,
    }
}

/// The grid rows of format `id`'s own options.
pub(super) fn rows(ui: &mut egui::Ui, d: &mut Dialog, id: &str, label: Label) {
    if id == "tiff" {
        tiff_rows(ui, d, label);
    }
}

fn tiff_rows(ui: &mut egui::Ui, d: &mut Dialog, label: Label) {
    label(ui, "Color Model:");
    choice(ui, d, "colorModel", &ColorModel::ALL.map(ColorModel::id), &ColorModel::ALL.map(ColorModel::label));
    ui.end_row();

    label(ui, "Byte Order:");
    choice(ui, d, "byteOrder", &ByteOrder::ALL.map(ByteOrder::id), &ByteOrder::ALL.map(ByteOrder::label));
    ui.end_row();

    ui.label("");
    ui.horizontal(|ui| {
        form::check(ui, d, "lzw", "LZW Compression");
        form::check(ui, d, "embedIcc", "Embed ICC Profile");
    });
    ui.end_row();
}
