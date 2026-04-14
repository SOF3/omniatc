use std::ops;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::color::{Color, Mix};
use bevy::ecs::query::{Has, QueryData};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res};
use bevy::time::{self, Time};
use bevy_mod_config::{Config, ConfigField, ReadConfig};
use math::{LengthUnit, Position, Speed, SpeedUnit, TROPOPAUSE_ALTITUDE};
use omniatc::level::object::Object;
use omniatc::level::{conflict, dest};
use omniatc::util::OptionalConfigField;
use serde::{Deserialize, Serialize};

use super::{ColorTheme, Conf, SetColorThemeSystemSet};
use crate::render;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            update_system
                .in_set(render::SystemSets::Update)
                .in_set(SetColorThemeSystemSet::BaseConfig),
        );
    }
}

#[derive(QueryData)]
struct UpdateData {
    object:   &'static Object,
    dest:     Option<&'static dest::Destination>,
    conflict: Has<conflict::ActiveObject>,
}

fn update_system(
    conf: ReadConfig<Conf>,
    object_query: Query<(&mut ColorTheme, UpdateData)>,
    time: Res<Time<time::Real>>,
) {
    let conf = conf.read();

    for (mut theme, data) in object_query {
        theme.body = select_color(&conf.plane.color_scheme, &time, &data);
        theme.label = select_color(&conf.plane.label_color_scheme, &time, &data);
        theme.ring = select_color(&conf.separation_ring.color_scheme, &time, &data);
        theme.vector = select_color(&conf.vector.color_scheme, &time, &data);
    }
}

fn select_color(scheme: &SchemeRead, time: &Time<time::Real>, data: &UpdateDataItem) -> Color {
    if data.conflict
        && let Some(scheme) = scheme.conflict.as_option()
    {
        let flash_cycle_duration = scheme.flash_on_duration + scheme.flash_off_duration;
        let flash_cycle_position = time.elapsed().as_millis() % flash_cycle_duration.as_millis();
        if flash_cycle_position < scheme.flash_on_duration.as_millis() {
            return scheme.color;
        }
    }

    match scheme.base {
        SchemeBaseRead::Destination { departure, arrival } => {
            if let Some(dest) = data.dest {
                match dest {
                    dest::Destination::VacateAnyRunway
                    | dest::Destination::Landing { .. }
                    | dest::Destination::Parking { .. } => arrival,
                    dest::Destination::Departure { .. } => departure,
                }
            } else {
                departure
            }
        }
        SchemeBaseRead::Altitude(lerp) => lerp.lerp(data.object.position.altitude()),
        SchemeBaseRead::Speed(lerp) => {
            lerp.lerp(data.object.ground_speed.horizontal().magnitude_exact())
        }
        SchemeBaseRead::VertRate(lerp) => lerp.lerp(data.object.ground_speed.vertical()),
    }
}

#[derive(Serialize, Deserialize, Config)]
#[config(expose(read))]
pub struct LerpColor<T: ConfigField> {
    /// Color when the value is at or below `bottom_value`.
    #[config(default = Color::oklch(0.647, 0.182, 0.0))]
    pub bottom_color: Color,
    /// Color when the value is at or above `top_value`.
    #[config(default = Color::oklch(0.773, 0.142, 180.0))]
    pub top_color:    Color,
    /// Lower value for interpolation.
    pub bottom_value: T,
    /// Upper value for interpolation.
    pub top_value:    T,
    /// Degree of interpolation polynomial `y = x^degree`,
    /// where `x` is the unlerped ratio of the value between `bottom_value` and `top_value`.
    pub degree:       f32,
}

impl<T> LerpColorRead<'_, T>
where
    T: ConfigField + Copy + ops::Sub,
    <T as ops::Sub>::Output: ops::Div<Output = f32>,
    for<'a> T: From<T::Reader<'a>>,
{
    fn lerp(&self, value: T) -> Color {
        let top_value = T::from(self.top_value);
        let bottom_value = T::from(self.bottom_value);
        let ratio = (value - bottom_value) / (top_value - bottom_value);
        self.bottom_color.mix(&self.top_color, ratio.clamp(0.0, 1.0).powf(self.degree))
    }
}

#[derive(Serialize, Deserialize, Config)]
#[config(expose(read))]
pub struct Scheme {
    pub base:     SchemeBase,
    pub conflict: OptionalConfigField<ConflictScheme>,
}

#[derive(Serialize, Deserialize, Config)]
#[config(expose(read, discrim))]
pub enum SchemeBase {
    Destination {
        #[config(default = Color::oklch(0.6484, 0.2805, 321.88))]
        departure: Color,
        #[config(default = Color::oklch(0.7735, 0.1973, 164.12))]
        arrival:   Color,
    },
    Altitude(
        #[config(
            bottom_value.default = Position::SEA_LEVEL,
            top_value.default = TROPOPAUSE_ALTITUDE,
            bottom_value.unit = LengthUnit::Feet,
            top_value.unit = LengthUnit::Feet,
            degree.default = 0.4,
        )]
        LerpColor<Position<f32>>,
    ),
    Speed(
        #[config(
            top_value.default = Speed::from_fpm(300.0),
            bottom_value.unit = SpeedUnit::Knots,
            top_value.unit = SpeedUnit::Knots,
            degree.default = 2.0,
        )]
        LerpColor<Speed<f32>>,
    ),
    VertRate(
        #[config(
            bottom_value.default = Speed::from_fpm(-3000.0),
            top_value.default = Speed::from_fpm(3000.0),
            bottom_value.unit = SpeedUnit::FeetPerMinute,
            top_value.unit = SpeedUnit::FeetPerMinute,
            degree.default = 1.0,
        )]
        LerpColor<Speed<f32>>,
    ),
}

#[derive(Serialize, Deserialize, Config)]
pub struct ConflictScheme {
    /// Color when object is in conflict.
    #[config(default = Color::oklch(0.628, 0.2317, 32.82))]
    pub color:              Color,
    /// Duration of displaying `flash_color` when an object enters conflict.
    #[config(default = Duration::from_millis(500))]
    pub flash_on_duration:  Duration,
    /// Duration between successive flashes when an object exits conflict.
    #[config(default = Duration::from_millis(500))]
    pub flash_off_duration: Duration,
}
