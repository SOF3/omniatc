use bevy::color::{Alpha, Color, ColorToPacked, Srgba};
use bevy_egui::egui;
use serde::Serialize;

use super::{Field, FieldEguiContext, FieldMeta};

#[derive(Default)]
pub struct ColorOpts {
    with_alpha: bool,
}

impl Field for Color {
    type Opts = ColorOpts;

    fn show_egui(
        &mut self,
        meta: FieldMeta<ColorOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        ui.horizontal(|ui| {
            let mut active = false;

            let resp = ui.label(meta.id);
            active = active || resp.hovered() || resp.has_focus();

            let old_value = self.to_srgba();
            let new_value = if meta.opts.with_alpha {
                let mut array = old_value.to_u8_array();
                let resp = ui.color_edit_button_srgba_unmultiplied(&mut array);
                active = active || resp.hovered() || resp.has_focus();
                Srgba::from_u8_array(array)
            } else {
                let mut array = old_value.to_u8_array_no_alpha();
                let resp = ui.color_edit_button_srgb(&mut array);
                active = active || resp.hovered() || resp.has_focus();
                Srgba::from_u8_array_no_alpha(array).with_alpha(old_value.alpha)
            };
            if active {
                meta.doc.clone_into(ctx.doc);
            }

            if old_value != new_value {
                *self = new_value.into();
                *ctx.changed = true;
            }
        });
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}
