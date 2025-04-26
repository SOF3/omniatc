use bevy_egui::egui;
use omniatc_core::units::{Angle, Distance, DistanceUnit, Position, Speed};
use serde::Serialize;

use super::{Field, FieldEguiContext, FieldMeta};

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

    fn show_egui(
        &mut self,
        meta: FieldMeta<F32Opts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        let mut active = false;

        ui.horizontal(|ui| {
            let resp = ui.label(meta.id);
            active = active || resp.hovered() || resp.has_focus();

            if let (Some(min), Some(max)) = (meta.opts.min, meta.opts.max) {
                let mut tmp = *self;
                let resp = ui.add(
                    egui::Slider::new(&mut tmp, min..=max)
                        .prefix(meta.opts.prefix.unwrap_or_default())
                        .suffix(meta.opts.suffix.unwrap_or_default())
                        .step_by(meta.opts.precision.unwrap_or_default().into()),
                );
                active = active || resp.hovered() || resp.has_focus();

                #[expect(clippy::float_cmp)] // Exact equality if user did not touch.
                if tmp != *self {
                    *self = tmp;
                    *ctx.changed = true;
                }
            } else {
                let mut value = *self;
                if let Some(precision) = meta.opts.precision {
                    value = (value / precision).round() * precision;
                }
                let mut s = value.to_string();
                if !s.contains('.') {
                    s.push('.');
                }

                if let Some(prefix) = meta.opts.prefix {
                    ui.small(prefix);
                }

                #[expect(clippy::cast_precision_loss)] // s.len() is expected to be small
                let s_len = s.len() as f32;
                let resp = ui.add(egui::TextEdit::singleline(&mut s).desired_width(s_len * 8.));
                active = active || resp.hovered() || resp.has_focus();

                if let Some(suffix) = meta.opts.suffix {
                    ui.small(suffix);
                }

                if let Ok(v) = s.parse() {
                    #[expect(clippy::float_cmp)] // Exact equality if user did not touch.
                    if v != *self {
                        *self = v;
                        *ctx.changed = true;
                    }
                }
            }
        });

        if active {
            meta.doc.clone_into(ctx.doc);
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
    pub unit:      Option<DistanceUnit>,
}

impl Field for Distance<f32> {
    type Opts = DistanceOpts;

    fn show_egui(
        &mut self,
        meta: FieldMeta<DistanceOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        let unit = meta.opts.unit.unwrap_or(DistanceUnit::Nautical);

        let mut value = unit.from_distance()(*self);
        value.show_egui(
            FieldMeta {
                group: meta.group,
                id:    meta.id,
                doc:   meta.doc,
                opts:  F32Opts {
                    min:       meta.opts.min.map(unit.from_distance()),
                    max:       meta.opts.max.map(unit.from_distance()),
                    prefix:    None,
                    suffix:    Some(unit.to_str()),
                    precision: Some(meta.opts.precision.map_or(0.1, unit.from_distance())),
                },
            },
            ui,
            ctx,
        );
        *self = unit.into_distance()(value);
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}

#[derive(Default)]
pub struct PositionOpts {
    pub min:       Option<Position<f32>>,
    pub max:       Option<Position<f32>>,
    pub precision: Option<Distance<f32>>,
}

impl Field for Position<f32> {
    type Opts = PositionOpts;

    fn show_egui(
        &mut self,
        meta: FieldMeta<PositionOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        let mut feet = self.amsl().into_feet();
        feet.show_egui(
            FieldMeta {
                group: meta.group,
                id:    meta.id,
                doc:   meta.doc,
                opts:  F32Opts {
                    min:       meta.opts.min.map(Position::get),
                    max:       meta.opts.max.map(Position::get),
                    prefix:    None,
                    suffix:    Some("ft"),
                    precision: Some(
                        meta.opts.precision.unwrap_or_else(|| Distance::from_feet(1000.)).0,
                    ),
                },
            },
            ui,
            ctx,
        );
        *self = Self::from_amsl_feet(feet);
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}

#[derive(Default)]
pub struct SpeedOpts {
    pub min:       Option<Speed<f32>>,
    pub max:       Option<Speed<f32>>,
    pub precision: Option<Speed<f32>>,
}

impl Field for Speed<f32> {
    type Opts = SpeedOpts;

    fn show_egui(
        &mut self,
        meta: FieldMeta<SpeedOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
        let mut knots = self.into_knots();
        knots.show_egui(
            FieldMeta {
                group: meta.group,
                id:    meta.id,
                doc:   meta.doc,
                opts:  F32Opts {
                    min:       meta.opts.min.map(|d| d.0),
                    max:       meta.opts.max.map(|d| d.0),
                    prefix:    None,
                    suffix:    Some("kt"),
                    precision: Some(
                        meta.opts.precision.unwrap_or_else(|| Speed::from_knots(0.1)).0,
                    ),
                },
            },
            ui,
            ctx,
        );
        *self = Self::from_knots(knots);
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

    fn show_egui(
        &mut self,
        meta: FieldMeta<AngleOpts>,
        ui: &mut egui::Ui,
        ctx: &mut FieldEguiContext,
    ) {
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
            ctx,
        );
    }

    fn as_serialize(&self) -> impl Serialize + '_ { self }

    type Deserialize = Self;
    fn from_deserialize(de: Self::Deserialize) -> Self { de }
}
