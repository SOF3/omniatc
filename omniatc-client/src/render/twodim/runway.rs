use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, ParamSet, Query};
use bevy::render::view::Visibility;
use bevy::transform::components::Transform;
use bevy_mod_config::{AppExt, Config};
use math::{Length, LengthUnit};
use omniatc::QueryTryLog;
use omniatc::level::navaid::{self, Navaid};
use omniatc::level::runway::{self, Runway};
use omniatc::level::waypoint::Waypoint;
use ordered_float::OrderedFloat;

use crate::{ConfigManager, render};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("2d:runway");
        app.add_systems(app::Update, spawn_system.in_set(render::SystemSets::Spawn));
        app.add_systems(app::Update, update_system.in_set(render::SystemSets::Update));
    }
}

mod glide_point;
mod localizer;
mod strip;

fn spawn_system(
    mut events: EventReader<runway::SpawnEvent>,
    mut params: ParamSet<(Commands, localizer::SpawnParam, strip::SpawnParam)>,
) {
    for &runway::SpawnEvent(entity) in events.read() {
        params.p0().entity(entity).insert((Transform::IDENTITY, Visibility::Visible));
        params.p1().spawn(entity);
        params.p2().spawn(entity);
    }
}

fn update_system(
    runway_query: Query<(
        Entity,
        &Waypoint,
        &Runway,
        &navaid::ListAtWaypoint,
        &localizer::HasLocalizer,
        &strip::HasStrip,
        Option<&glide_point::PointList>,
    )>,
    navaid_query: Query<&Navaid>,
    mut params: ParamSet<(localizer::UpdateParam, strip::UpdateParam, glide_point::UpdateParam)>,
) {
    for (entity, waypoint, runway, navaids, localizer, strip, glide_point) in runway_query {
        let localizer_length = navaids
            .navaids()
            .iter()
            .filter_map(|&navaid| {
                let navaid = navaid_query.log_get(navaid)?;
                Some(navaid.max_dist_horizontal)
            })
            .max_by_key(|&f| OrderedFloat(f.0))
            .unwrap_or_else(|| {
                bevy::log::warn!("Every runway must have visual navaid");
                Length::from_nm(1.0)
            });

        params.p0().update(runway, localizer, localizer_length);
        params.p1().update(runway, strip);
        params.p2().update(entity, waypoint, runway, glide_point, localizer_length);
    }
}

#[derive(Config)]
struct Conf {
    /// Thickness of runway localizer display, in screen coordinates.
    #[config(default = 0.8, min = 0.0, max = 10.0)]
    localizer_thickness: f32,
    /// Color of runway localizer display.
    #[config(default = Color::WHITE)]
    localizer_color:     Color,
    /// Thickness of runway strip display, in screen coordinates.
    #[config(default = 5.0, min = 0.0, max = 10.0)]
    strip_thickness:     f32,
    /// Color of runway strip display.
    #[config(default = Color::srgb(0.5, 0.5, 0.5))]
    strip_color:         Color,
    /// Size of glidepath points, in screen coordinates.
    #[config(default = 3.0, min = 0.0, max = 5.0)]
    glide_point_size:    f32,
    /// Color of glidepath points.
    #[config(default = Color::WHITE)]
    glide_point_color:   Color,
    /// Glidepath points are rendered when they intersect multiples of this altitude AMSL.
    #[config(
        default = Length::from_feet(1000.0),
        min = Length::from_feet(100.0),
        max = Length::from_feet(10000.0),
        precision = Some(Length::from_feet(100.0)),
        unit = LengthUnit::Feet,
    )]
    glide_point_density: Length<f32>,
}
