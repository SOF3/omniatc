use std::ops;

use bevy::app::{App, Plugin};
use bevy::math::Vec3;
use bevy::prelude::{Component, Entity, EntityCommand, Event, World};

use crate::math::Heading;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_event::<SpawnEvent>(); }
}

#[derive(Component)]
pub struct Waypoint {
    /// Display name of the waypoint.
    pub name:         String,
    /// Type of the waypoint as displayed.
    pub display_type: DisplayType,
    /// Position of the waypoint.
    ///
    /// The altitude component is only used to compute the navaid range.
    pub position:     Vec3,
    /// Signal range, if the waypoint is a navaid.
    pub navaid_range: Vec<NavaidRange>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayType {
    /// A normal point on the map.
    Waypoint,
    /// A VHF Omnidirectional Range station.
    Vor,
    /// The waypoint should not be displayed on the map.
    /// Used for virtual waypoints such as localizer limits.
    None,
}

/// A spherical sector centered at the waypoint position
/// within which the navaid can be reliably received..
pub struct NavaidRange {
    /// Horizontal radial directions of the sector boundary.
    ///
    /// The range is taken in clockwise direction. That is,
    /// A receiver at heading `r` from the navaid is within this range
    /// if and only if sweeping from `start` to `r` in clockwise direction
    /// does not cross `end`.
    pub heading_range: ops::Range<Heading>,

    /// Minimum angle of the receiver relative to the navaid, in radians.
    pub min_pitch: f32,

    /// Maximum 3D distance of the receiver from the navaid, in nm.
    pub max_range: f32,
}

pub struct SpawnCommand {
    pub waypoint: Waypoint,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity).insert(self.waypoint);
        world.send_event(SpawnEvent(entity));
    }
}

#[derive(Event)]
pub struct SpawnEvent(pub Entity);
