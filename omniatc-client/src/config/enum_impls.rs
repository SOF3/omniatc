use std::fmt;

use bevy::math::Vec2;
use bevy::sprite::Anchor;
use bevy_egui::egui;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use strum::VariantArray;

use super::{Field, FieldEguiContext, FieldMeta};

macro_rules! anchor_serde {
    ($($variants:ident,)*) => {
        #[derive(Clone, Copy, Serialize, Deserialize)]
        pub enum AnchorSerde {
            $($variants,)*
            Custom([f32; 2]),
        }

        impl From<Anchor> for AnchorSerde {
            fn from(anchor: Anchor) -> Self {
                match anchor {
                    $(Anchor::$variants => AnchorSerde::$variants,)*
                    Anchor::Custom(v) => AnchorSerde::Custom([v.x, v.y]),
                }
            }
        }

        impl From<AnchorSerde> for Anchor {
            fn from(anchor: AnchorSerde) -> Self {
                match anchor {
                    $(AnchorSerde::$variants => Anchor::$variants,)*
                    AnchorSerde::Custom([x, y]) => Anchor::Custom(Vec2::new(x, y)),
                }
            }
        }

        #[derive(Clone, Copy, PartialEq, Eq, strum::VariantArray, strum::Display)]
        enum AnchorVariants {
            $($variants,)*
            Custom,
        }

        impl AnchorVariants {
            fn from_anchor(anchor: Anchor) -> Self {
                match anchor {
                    $(Anchor::$variants => Self::$variants,)*
                    Anchor::Custom(..) => Self::Custom,
                }
            }

            fn to_anchor_or_custom(self) -> Option<Anchor> {
                match self {
                    $(Self::$variants => Some(Anchor::$variants),)*
                    Self::Custom => None,
                }
            }
        }
    }
}

anchor_serde! {
    Center,
    BottomLeft,
    BottomCenter,
    BottomRight,
    CenterLeft,
    CenterRight,
    TopLeft,
    TopCenter,
    TopRight,
}

#[derive(Default)]
pub struct AnchorOpts {}

impl Field for Anchor {
    type Opts = AnchorOpts;

    fn show_egui(
        &mut self,
        meta: FieldMeta<AnchorOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        let mut select = AnchorVariants::from_anchor(*self);

        let resp = egui::ComboBox::new(format!("{}.{}", meta.group, meta.id), meta.id)
            .selected_text(format!("{select}"))
            .show_ui(ui, |ui| {
                for &variant in AnchorVariants::VARIANTS {
                    ui.selectable_value(&mut select, variant, format!("{variant}"));
                }
            });
        if resp.response.hovered() || resp.response.has_focus() {
            meta.doc.clone_into(ctx.doc);
        }

        let out = if let Some(v) = select.to_anchor_or_custom() {
            v
        } else {
            let mut v = self.as_vec();
            ui.horizontal_wrapped(|ui| {
                ui.add(egui::Slider::new(&mut v.x, 0. ..=1.).text("X"));
                ui.add(egui::Slider::new(&mut v.y, 0. ..=1.).text("Y"));
            });
            Anchor::Custom(v)
        };

        if out != *self {
            *self = out;
            *ctx.changed = true;
        }
    }

    fn as_serialize(&self) -> impl Serialize + '_ { AnchorSerde::from(*self) }

    type Deserialize = AnchorSerde;
    fn from_deserialize(de: Self::Deserialize) -> Self { de.into() }
}

/// Marks an enum as a field enum that can be used in dropdowns based on the strum enum message.
pub trait EnumField:
    strum::IntoEnumIterator + Copy + PartialEq + fmt::Display + Serialize + DeserializeOwned
{
}

#[derive(Default)]
pub struct EnumOpts {}

impl<T: EnumField> Field for T {
    type Opts = EnumOpts;

    fn show_egui(
        &mut self,
        meta: FieldMeta<EnumOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        let resp = egui::ComboBox::new(format!("{}.{}", meta.group, meta.id), meta.id)
            .selected_text(format!("{self}"))
            .show_ui(ui, |ui| {
                let mut tmp = *self;
                for variant in T::iter() {
                    ui.selectable_value(&mut tmp, variant, format!("{variant}"));
                }

                if tmp != *self {
                    *self = tmp;
                    *ctx.changed = true;
                }
            });
        if resp.response.hovered() || resp.response.has_focus() {
            meta.doc.clone_into(ctx.doc);
        }
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}
