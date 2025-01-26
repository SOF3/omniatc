use bevy::app::{self, App, Plugin};
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{
    Children, Component, Entity, EntityCommand, Event, IntoSystemConfigs, Query, Without, World,
};

use super::waypoint::{self, Waypoint};
use super::SystemSets;
use crate::units::{Angle, Distance, Position};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnEvent>();
        app.add_systems(
            app::Update,
            maintain_localizer_waypoint_system.in_set(SystemSets::PrepareEnviron),
        );
    }
}

/// A runway entity is always a waypoint entity.
///
/// The waypoint location is the default touchdown point of the runway.
/// The display type for a runway waypoint should be `DisplayType::Localizer`.
///
/// Runway waypoints must have a child Navaid entity
/// with the [`waypoint::Visual`] component for final approach.
#[derive(Component)]
pub struct Runway {
    /// The aerodrome that this runway belongs to.
    pub aerodrome: Entity,

    /// Usable runway length.
    ///
    /// Only used to determine takeoff/landing feasibility.
    /// Does not affect display.
    ///
    /// A plane must initiate takeoff at `waypoint.position + k * usable_length`
    /// for some `0 <= k < 1`
    /// such that `(1 - k) * usable_length.length()` exceeds the minimum takeoff distance.
    ///
    /// A plane must touch down at `waypoint.position + k * usable_length`
    /// for some `0 <= k < 1`
    /// such that braking over `(1 - k) * usable_length.length()`
    /// allows the plane to reduce to taxi speed.
    pub usable_length: Distance<Vec2>,

    /// Starting point of the rendered runway.
    pub display_start: Position<Vec3>,
    /// Ending point of the rendered runway.
    pub display_end:   Position<Vec3>,
    /// The displayed width for the runway.
    pub display_width: Distance<f32>,

    /// Standard angle of depression for the glide path.
    ///
    /// Always positive.
    pub glide_angle: Angle<f32>,
}

pub struct SpawnCommand {
    pub runway:   Runway,
    pub waypoint: Waypoint,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, entity: Entity, world: &mut World) {
        waypoint::SpawnCommand { waypoint: self.waypoint }.apply(entity, world);

        world.entity_mut(entity).insert(self.runway);
        world.send_event(SpawnEvent(entity));
    }
}

/// Sent when a runway entity is spawned.
#[derive(Event)]
pub struct SpawnEvent(pub Entity);

/// Marks that a waypoint entity should translate along the extended approach centerline
/// such that the segment between the waypoint and the runway
/// is the range an approach can be established on.
#[derive(Component)]
pub struct LocalizerWaypoint {
    /// The runway that the waypoint is associated with.
    pub runway_ref: Entity,
}

/// Component on Runway entities referencing the [`LocalizerWaypoint`].
#[derive(Component)]
pub struct LocalizerWaypointRef {
    /// The localizer waypoint entity.
    pub localizer_waypoint: Entity,
}

fn maintain_localizer_waypoint_system(
    mut waypoint_query: Query<(Entity, &mut Waypoint, &LocalizerWaypoint), Without<Runway>>,
    runway_query: Query<(&Waypoint, &Runway, &Children)>,
    navaid_query: Query<&waypoint::Navaid>,
) {
    waypoint_query.iter_mut().for_each(
        |(waypoint_entity, mut waypoint, &LocalizerWaypoint { runway_ref })| {
            let Ok((
                &Waypoint { position: runway_position, .. },
                &Runway { usable_length, glide_angle, .. },
                children,
            )) = runway_query.get(runway_ref)
            else {
                bevy::log::error!(
                    "Runway {runway_ref:?} referenced from waypoint {waypoint_entity:?} is not a \
                     runway entity"
                );
                return;
            };

            let mut range = Distance(0f32);
            for &navaid_ref in children {
                if let Ok(navaid) = navaid_query.get(navaid_ref) {
                    range = range.max(navaid.max_dist_horizontal);
                }
            }

            waypoint.position = runway_position
                + usable_length
                    .normalize_to_magnitude(-range)
                    .projected_from_elevation_angle(glide_angle);
        },
    );
}
