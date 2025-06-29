use bevy::app::{self, App, Plugin};
use bevy::color::{Color, Mix};
use bevy::ecs::query::QueryData;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::Query;
use omniatc::level::object::Object;
use omniatc::math::TROPOPAUSE_ALTITUDE;
use omniatc::units::{Position, Speed};
use omniatc_macros::FieldEnum;
use serde::{Deserialize, Serialize};

use super::{ColorTheme, Conf, SetColorThemeSystemSet};
use crate::{config, render};

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

fn update_system(conf: config::Read<Conf>, object_query: Query<(&mut ColorTheme, UpdateData)>) {
    for (mut theme, data) in object_query {
        theme.body = select_color(&conf.base_color_scheme, &data);
        theme.ring = select_color(&conf.ring_color_scheme, &data);
        theme.vector = select_color(&conf.vector_color_scheme, &data);
    }
}

fn select_color(scheme: &Scheme, data: &UpdateDataItem) -> Color {
    match *scheme {
        Scheme::Altitude { base_color, base_altitude, top_color, top_altitude } => base_color.mix(
            &top_color,
            data.object.position.altitude().ratio_between(base_altitude, top_altitude),
        ),
        Scheme::Speed { base_color, base_speed, top_color, top_speed } => base_color.mix(
            &top_color,
            data.object
                .ground_speed
                .horizontal()
                .magnitude_exact()
                .ratio_between(base_speed, top_speed),
        ),
        Scheme::VertRate { base_color, base_speed, top_color, top_speed } => base_color.mix(
            &top_color,
            data.object.ground_speed.vertical().ratio_between(base_speed, top_speed),
        ),
    }
}

#[derive(FieldEnum, Serialize, Deserialize)]
pub(super) enum Scheme {
    #[field_default]
    Altitude {
        #[field_default(Color::srgb(0.8, 0.4, 0.6))]
        base_color:    Color,
        #[field_default(Position::SEA_LEVEL)]
        base_altitude: Position<f32>,
        #[field_default(Color::srgb(0.4, 0.8, 0.6))]
        top_color:     Color,
        #[field_default(TROPOPAUSE_ALTITUDE)]
        top_altitude:  Position<f32>,
    },
    Speed {
        #[field_default(Color::srgb(0.8, 0.4, 0.6))]
        base_color: Color,
        #[field_default(Speed::ZERO)]
        base_speed: Speed<f32>,
        #[field_default(Color::srgb(0.4, 0.8, 0.6))]
        top_color:  Color,
        #[field_default(Speed::from_knots(500.))]
        top_speed:  Speed<f32>,
    },
    VertRate {
        #[field_default(Color::srgb(0.8, 0.4, 0.6))]
        base_color: Color,
        #[field_default(Speed::from_fpm(-3000.))]
        base_speed: Speed<f32>,
        #[field_default(Color::srgb(0.4, 0.8, 0.6))]
        top_color:  Color,
        #[field_default(Speed::from_fpm(3000.))]
        top_speed:  Speed<f32>,
    },
}
