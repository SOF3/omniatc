use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, ParamSet, Query};
use bevy::render::view::Visibility;
use bevy::transform::components::Transform;
use omniatc::level::navaid::{self, Navaid};
use omniatc::level::runway::{self, Runway};
use omniatc::level::waypoint::Waypoint;
use omniatc::try_log;
use omniatc::units::{Distance, DistanceUnit};
use omniatc_macros::Config;
use ordered_float::OrderedFloat;

use crate::config::AppExt;
use crate::render;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<Conf>();
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
        let localizer_length = navaids.navaids().iter().filter_map(|&navaid| {
            let navaid = try_log!(navaid_query.get(navaid), expect "navaid referenced from runway must be a navaid" or return None);
            Some(navaid.max_dist_horizontal)
        }).max_by_key(|&f| OrderedFloat(f.0))
        .unwrap_or_else(|| {
                bevy::log::warn!("Every runway must have visual navaid");
                Distance::from_nm(1.)
            });

        params.p0().update(runway, localizer, localizer_length);
        params.p1().update(runway, strip);
        params.p2().update(entity, waypoint, runway, glide_point, localizer_length);
    }
}

#[derive(Resource, Config)]
#[config(id = "runway", name = "Runways")]
struct Conf {
    /// Thickness of runway localizer display, in screen coordinates.
    #[config(min = 0., max = 10.)]
    localizer_thickness: f32,
    /// Color of runway localizer display.
    localizer_color:     Color,
    /// Thickness of runway strip display, in screen coordinates.
    #[config(min = 0., max = 10.)]
    strip_thickness:     f32,
    /// Color of runway strip display.
    strip_color:         Color,
    /// Size of glidepath points, in screen coordinates.
    #[config(min = 0., max = 5.)]
    glide_point_size:    f32,
    /// Color of glidepath points.
    glide_point_color:   Color,
    /// Glidepath points are rendered when they intersect multiples of this altitude AMSL.
    #[config(min = Distance::from_feet(100.), max = Distance::from_feet(10000.), precision = Distance::from_feet(100.), unit = DistanceUnit::Feet)]
    glide_point_density: Distance<f32>,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            localizer_thickness: 0.8,
            localizer_color:     Color::WHITE,
            strip_thickness:     5.,
            strip_color:         Color::WHITE,
            glide_point_size:    3.,
            glide_point_color:   Color::WHITE,
            glide_point_density: Distance::from_feet(1000.),
        }
    }
}
