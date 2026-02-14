use bevy::app::{self, App, Plugin};
use bevy::color::{Color, Mix};
use bevy::ecs::query::QueryData;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::Query;
use bevy_mod_config::{Config, ReadConfig};
use math::{Position, Speed, SpeedUnit, TROPOPAUSE_ALTITUDE};
use omniatc::level::object::Object;
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
    object: &'static Object,
}

fn update_system(conf: ReadConfig<Conf>, object_query: Query<(&mut ColorTheme, UpdateData)>) {
    let conf = conf.read();

    for (mut theme, data) in object_query {
        theme.body = select_color(&conf.plane.color_scheme, &data);
        theme.label = select_color(&conf.plane.label_color_scheme, &data);
        theme.ring = select_color(&conf.separation_ring.color_scheme, &data);
        theme.vector = select_color(&conf.vector.color_scheme, &data);
    }
}

fn select_color(scheme: &SchemeRead, data: &UpdateDataItem) -> Color {
    match *scheme {
        SchemeRead::Altitude { base_color, base_altitude, top_color, top_altitude } => base_color
            .mix(
                &top_color,
                data.object.position.altitude().ratio_between(base_altitude, top_altitude),
            ),
        SchemeRead::Speed { base_color, base_speed, top_color, top_speed } => base_color.mix(
            &top_color,
            data.object
                .ground_speed
                .horizontal()
                .magnitude_exact()
                .ratio_between(base_speed, top_speed),
        ),
        SchemeRead::VertRate { base_color, base_speed, top_color, top_speed } => base_color.mix(
            &top_color,
            data.object.ground_speed.vertical().ratio_between(base_speed, top_speed),
        ),
    }
}

#[derive(Serialize, Deserialize, Config)]
#[config(expose(read))]
pub(super) enum Scheme {
    Altitude {
        #[config(default = Color::srgb(0.8, 0.4, 0.6))]
        base_color:    Color,
        #[config(default = Position::SEA_LEVEL)]
        base_altitude: Position<f32>,
        #[config(default = Color::srgb(0.4, 0.8, 0.6))]
        top_color:     Color,
        #[config(default = TROPOPAUSE_ALTITUDE)]
        top_altitude:  Position<f32>,
    },
    Speed {
        #[config(default = Color::srgb(0.8, 0.4, 0.6))]
        base_color: Color,
        #[config(default = Speed::ZERO)]
        base_speed: Speed<f32>,
        #[config(default = Color::srgb(0.4, 0.8, 0.6))]
        top_color:  Color,
        #[config(default = Speed::from_knots(500.))]
        top_speed:  Speed<f32>,
    },
    VertRate {
        #[config(default = Color::srgb(0.8, 0.4, 0.6))]
        base_color: Color,
        #[config(default = Speed::from_fpm(-3000.0), unit = SpeedUnit::FeetPerMinute)]
        base_speed: Speed<f32>,
        #[config(default = Color::srgb(0.4, 0.8, 0.6))]
        top_color:  Color,
        #[config(default = Speed::from_fpm(3000.0), unit = SpeedUnit::FeetPerMinute)]
        top_speed:  Speed<f32>,
    },
}
