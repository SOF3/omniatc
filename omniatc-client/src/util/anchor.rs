use std::hash::Hash;

use bevy::sprite::Anchor;
use bevy_egui::egui;
use bevy_mod_config::manager::egui::{DefaultStyle, Editable};
use serde::{Deserialize, Serialize};

bevy_mod_config::impl_scalar_config_field!(
    AnchorConf,
    AnchorConfMetadata,
    |metadata: &AnchorConfMetadata| AnchorConf(metadata.default),
    'a => Anchor,
    |anchor: &AnchorConf| anchor.0,
);

pub struct AnchorConf(pub Anchor);

#[derive(Default, Clone)]
pub struct AnchorConfMetadata {
    pub default: Anchor,
}

const VARIANTS: &[(Anchor, &str)] = &[
    (Anchor::CENTER, "Center"),
    (Anchor::BOTTOM_LEFT, "Bottom Left"),
    (Anchor::BOTTOM_CENTER, "Bottom Center"),
    (Anchor::BOTTOM_RIGHT, "Bottom Right"),
    (Anchor::CENTER_LEFT, "Center Left"),
    (Anchor::CENTER_RIGHT, "Center Right"),
    (Anchor::TOP_LEFT, "Top Left"),
    (Anchor::TOP_CENTER, "Top Center"),
    (Anchor::TOP_RIGHT, "Top Right"),
];

fn anchor_to_str(value: Anchor) -> &'static str {
    VARIANTS.iter().find(|&&(anchor, _)| anchor == value).map_or("Custom", |&(_, label)| label)
}

impl Editable<DefaultStyle> for AnchorConf {
    type TempData = ();

    fn show(
        ui: &mut egui::Ui,
        value: &mut Self,
        _: &Self::Metadata,
        _: &mut Option<Self::TempData>,
        id_salt: impl Hash,
        _: &DefaultStyle,
    ) -> egui::Response {
        egui::ComboBox::from_id_salt(id_salt)
            .selected_text(anchor_to_str(value.0))
            .show_ui(ui, |ui| {
                for &(anchor, label) in VARIANTS {
                    ui.selectable_value(&mut value.0, anchor, label);
                }
            })
            .response
    }
}

impl Serialize for AnchorConf {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(anchor_to_str(self.0))
    }
}

impl<'de> Deserialize<'de> for AnchorConf {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct AnchorVisitor;

        impl serde::de::Visitor<'_> for AnchorVisitor {
            type Value = AnchorConf;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid Anchor value")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match VARIANTS.iter().find(|&&(_, label)| label == value) {
                    Some(&(anchor, _)) => Ok(AnchorConf(anchor)),
                    None => Err(E::custom(format!("Unknown anchor variant {value}"))),
                }
            }
        }

        deserializer.deserialize_str(AnchorVisitor)
    }
}
