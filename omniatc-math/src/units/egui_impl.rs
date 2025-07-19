use bevy_mod_config::manager::egui::{DefaultStyle, Editable};
use bevy_mod_config::ConfigField;
use strum::IntoEnumIterator;

use super::{Angle, Length, LengthUnit};
use crate::{QuantityMetadataWithUnit, QuantityTrait, Speed, SpeedUnit, UnitEnum};

impl Editable<DefaultStyle> for Length<f32> {
    type TempData = LengthUnit;

    fn show(
        ui: &mut egui::Ui,
        value: &mut Self,
        metadata: &Self::Metadata,
        temp: &mut Option<Self::TempData>,
        id_salt: impl std::hash::Hash,
        _: &DefaultStyle,
    ) -> egui::Response {
        show_editable(ui, value, metadata, temp, id_salt)
    }
}

impl Editable<DefaultStyle> for Speed<f32> {
    type TempData = SpeedUnit;

    fn show(
        ui: &mut egui::Ui,
        value: &mut Self,
        metadata: &Self::Metadata,
        temp: &mut Option<Self::TempData>,
        id_salt: impl std::hash::Hash,
        _: &DefaultStyle,
    ) -> egui::Response {
        show_editable(ui, value, metadata, temp, id_salt)
    }
}

fn show_editable<T, U>(
    ui: &mut egui::Ui,
    value: &mut T,
    metadata: &T::Metadata,
    temp: &mut Option<U>,
    id_salt: impl std::hash::Hash,
) -> egui::Response
where
    T: QuantityTrait + Copy + ConfigField<Metadata = QuantityMetadataWithUnit<T, U>>,
    U: UnitEnum<Quantity = T>,
{
    let unit = temp.get_or_insert_with(|| metadata.unit);
    let quantity_to_float = unit.quantity_to_float();
    let mut edited = quantity_to_float(*value);
    let resp = ui.horizontal(|ui| {
        let mut slider_resp = ui.add(
            egui::Slider::new(
                &mut edited,
                quantity_to_float(metadata.min)..=quantity_to_float(metadata.max),
            )
            .suffix(unit.to_str()),
        );

        let unit_changed = egui::ComboBox::from_id_salt(id_salt)
            .selected_text(unit.to_str())
            .show_ui(ui, |ui| {
                for variant in U::iter() {
                    ui.selectable_value(unit, variant, variant.to_str());
                }
            })
            .response
            .changed();
        if unit_changed {
            slider_resp.mark_changed();
        }

        slider_resp
    });

    if resp.inner.changed() {
        // do not perform unnecessary updates,
        // otherwise we will be accumulating numerical errors in a loop.
        *value = unit.float_to_quantity()(edited);
    }

    resp.inner
}

impl Editable<DefaultStyle> for Angle {
    type TempData = ();

    fn show(
        ui: &mut egui::Ui,
        value: &mut Self,
        metadata: &Self::Metadata,
        _: &mut Option<Self::TempData>,
        _: impl std::hash::Hash,
        _: &DefaultStyle,
    ) -> egui::Response {
        let mut edited = value.into_degrees();
        let resp = ui.add(
            egui::Slider::new(
                &mut edited,
                metadata.min.into_degrees()..=metadata.max.into_degrees(),
            )
            .suffix("\u{b0}"),
        );

        if resp.changed() {
            // do not perform unnecessary updates,
            // otherwise we will be accumulating numerical errors in a loop.
            *value = Angle::from_degrees(edited);
        }

        resp
    }
}
