use std::fmt;

use bevy::math::Vec2;
use bevy::sprite::Anchor;
use bevy_egui::egui;
use omniatc_core::units::{Angle, Distance};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use strum::VariantArray;

use super::{Field, FieldMeta};

#[derive(Default)]
pub struct F32Opts {
    pub min:       Option<f32>,
    pub max:       Option<f32>,
    pub prefix:    Option<&'static str>,
    pub suffix:    Option<&'static str>,
    pub precision: Option<f32>,
}

impl Field for f32 {
    type Opts = F32Opts;

    fn show_egui(&mut self, meta: FieldMeta<F32Opts>, ui: &mut egui::Ui, changed: &mut bool) {
        if let (Some(min), Some(max)) = (meta.opts.min, meta.opts.max) {
            ui.horizontal(|ui| {
                ui.label(meta.id);

                let mut tmp = *self;
                ui.add(
                    egui::Slider::new(&mut tmp, min..=max)
                        .prefix(meta.opts.prefix.unwrap_or_default())
                        .suffix(meta.opts.suffix.unwrap_or_default())
                        .step_by(meta.opts.precision.unwrap_or_default().into()),
                );

                #[expect(clippy::float_cmp)] // Exact equality if user did not touch.
                if tmp != *self {
                    *self = tmp;
                    *changed = true;
                }
            });
        } else {
            let mut value = *self;
            if let Some(precision) = meta.opts.precision {
                value = (value / precision).round() * precision;
            }
            let mut s = value.to_string();
            if !s.contains('.') {
                s.push('.');
            }

            ui.horizontal(|ui| {
                ui.label(meta.id);

                if let Some(prefix) = meta.opts.prefix {
                    ui.small(prefix);
                }

                #[expect(clippy::cast_precision_loss)] // s.len() is expected to be small
                let s_len = s.len() as f32;
                ui.add(egui::TextEdit::singleline(&mut s).desired_width(s_len * 8.));

                if let Some(suffix) = meta.opts.suffix {
                    ui.small(suffix);
                }
            });

            if let Ok(v) = s.parse() {
                #[expect(clippy::float_cmp)] // Exact equality if user did not touch.
                if v != *self {
                    *self = v;
                    *changed = true;
                }
            }
        }
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}

#[derive(Default)]
pub struct DistanceOpts {
    pub min:       Option<Distance<f32>>,
    pub max:       Option<Distance<f32>>,
    pub precision: Option<Distance<f32>>,
}

impl Field for Distance<f32> {
    type Opts = DistanceOpts;

    fn show_egui(&mut self, meta: FieldMeta<DistanceOpts>, ui: &mut egui::Ui, changed: &mut bool) {
        self.0.show_egui(
            FieldMeta {
                group: meta.group,
                id:    meta.id,
                doc:   meta.doc,
                opts:  F32Opts {
                    min:       meta.opts.min.map(|d| d.0),
                    max:       meta.opts.max.map(|d| d.0),
                    prefix:    None,
                    suffix:    Some("nm"),
                    precision: Some(
                        meta.opts.precision.unwrap_or_else(|| Distance::from_nm(0.1)).0,
                    ),
                },
            },
            ui,
            changed,
        );
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}

#[derive(Default)]
pub struct AngleOpts {
    pub min:       Option<Angle<f32>>,
    pub max:       Option<Angle<f32>>,
    pub precision: Option<Angle<f32>>,
}

impl Field for Angle<f32> {
    type Opts = AngleOpts;

    fn show_egui(&mut self, meta: FieldMeta<AngleOpts>, ui: &mut egui::Ui, changed: &mut bool) {
        let mut degrees = self.into_degrees();
        degrees.show_egui(
            FieldMeta {
                group: meta.group,
                id:    meta.id,
                doc:   meta.doc,
                opts:  F32Opts {
                    min:       meta.opts.min.map(|d| d.0),
                    max:       meta.opts.max.map(|d| d.0),
                    prefix:    None,
                    suffix:    Some("\u{b0}"),
                    precision: Some(
                        meta.opts
                            .precision
                            .unwrap_or_else(|| Angle::from_degrees(1.))
                            .into_degrees(),
                    ),
                },
            },
            ui,
            changed,
        );
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}

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

    fn show_egui(&mut self, meta: FieldMeta<AnchorOpts>, ui: &mut egui::Ui, changed: &mut bool) {
        let mut select = AnchorVariants::from_anchor(*self);

        egui::ComboBox::new(format!("{}.{}", meta.group, meta.id), meta.id)
            .selected_text(format!("{select}"))
            .show_ui(ui, |ui| {
                for &variant in AnchorVariants::VARIANTS {
                    ui.selectable_value(&mut select, variant, format!("{variant}"));
                }
            });

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
            *changed = true;
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

    fn show_egui(&mut self, meta: FieldMeta<EnumOpts>, ui: &mut egui::Ui, changed: &mut bool) {
        egui::ComboBox::new(format!("{}.{}", meta.group, meta.id), meta.id)
            .selected_text(format!("{self}"))
            .show_ui(ui, |ui| {
                let mut tmp = *self;
                for variant in T::iter() {
                    ui.selectable_value(&mut tmp, variant, format!("{variant}"));
                }

                if tmp != *self {
                    *self = tmp;
                    *changed = true;
                }
            });
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}
