use bevy::app::{self, App, Plugin};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{Component, Entity, EntityCommand, Event, IntoScheduleConfigs, Query, Without};
use math::{Angle, Length, Position};
use smallvec::SmallVec;

use super::navaid::Navaid;
use super::waypoint::{self, Waypoint};
use super::{navaid, SystemSets};
use crate::QueryTryLog;

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

    /// Usable runway length for landings.
    ///
    /// Only used to determine landing feasibility.
    /// Does not affect display.
    ///
    /// A plane must touch down at `waypoint.position + k * landing_length`
    /// for some `0 <= k < 1`
    /// such that braking over `(1 - k) * landing_length.length()`
    /// allows the plane to reduce to taxi speed.
    pub landing_length: Length<Vec2>,

    /// Starting point of the rendered runway.
    pub display_start: Position<Vec3>,
    /// Ending point of the rendered runway.
    pub display_end:   Position<Vec3>,

    /// The usable width for the runway.
    pub width: Length<f32>,

    /// Standard angle of depression for the glide path.
    ///
    /// Always positive.
    pub glide_descent: Angle,
}

/// Runway conditions due to environmental factors.
#[derive(Component, Clone)]
pub struct Condition {
    /// A multiplier to the base braking rate of an object.
    pub friction_factor: f32,
}

pub struct SpawnCommand {
    pub runway:   Runway,
    pub waypoint: Waypoint,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        let entity_id = entity.id();

        entity.world_scope(|world| {
            waypoint::SpawnCommand { waypoint: self.waypoint }.apply(world.entity_mut(entity_id));
        });

        entity.insert((self.runway, Condition { friction_factor: 1. }));
        entity.world_scope(|world| world.send_event(SpawnEvent(entity_id)));
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
    mut waypoint_query: Query<(&mut Waypoint, &LocalizerWaypoint), Without<Runway>>,
    runway_query: Query<(&Waypoint, &Runway, &navaid::ListAtWaypoint)>,
    navaid_query: Query<&Navaid>,
) {
    waypoint_query.iter_mut().for_each(|(mut waypoint, &LocalizerWaypoint { runway_ref })| {
        let Some((
            &Waypoint { position: runway_position, .. },
            &Runway { landing_length, glide_descent: glide_angle, .. },
            navaids,
        )) = runway_query.log_get(runway_ref)
        else {
            return;
        };

        let mut range = Length::from_meters(1.); // visibility is never zero.
        for &navaid_ref in navaids.navaids() {
            if let Ok(navaid) = navaid_query.get(navaid_ref) {
                range = range.max(navaid.max_dist_horizontal);
            }
        }

        waypoint.position = runway_position
            + landing_length
                .normalize_to_magnitude(-range)
                .projected_from_elevation_angle(glide_angle);
    });
}

/// List of ground segment entities that make up the ground structure of this runway.
#[derive(Component)]
pub struct GroundSegmentList {
    pub segments: SmallVec<[Entity; 8]>,
}
