use bevy::app::{App, Plugin};
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{Component, Entity, EntityCommand, Event, World};

use super::waypoint::{self, Waypoint};
use crate::units::{Angle, Distance, Position};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_event::<SpawnEvent>(); }
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
