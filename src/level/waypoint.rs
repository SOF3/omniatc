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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayType {
    /// A normal point on the map.
    Waypoint,
    /// A VHF Omnidirectional Range station.
    Vor,
    /// The waypoint should not be displayed on the map.
    /// Used for virtual waypoints.
    None,
    /// The waypoint is a runway.
    /// This will prevent the waypoint UI from rendering any icons,
    /// but unlike `None`, the label is still rendered.
    Runway,
}

/// A spherical sector centered at the waypoint position
/// within which the navaid can be reliably received.
///
/// A navaid entity is always a child of a waypoint entity.
#[derive(Component)]
pub struct Navaid {
    /// Horizontal radial directions of the sector boundary.
    ///
    /// The range is taken in clockwise direction. That is,
    /// A receiver at heading `r` from the navaid is within this range
    /// if and only if sweeping from `start` to `r` in clockwise direction
    /// does not cross `end`.
    ///
    /// `Heading::NORTH..Heading::NORTH` is explicitly defined to cover all directions.
    pub heading_range: ops::Range<Heading>,

    /// Minimum angle of the receiver relative to the navaid, in radians.
    pub min_pitch: f32,
    /// Maximum angle of the receiver relative to the navaid, in radians.
    pub max_pitch: f32,

    /// Minimum horizontal distance of the receiver from the navaid, in nm.
    ///
    /// This is used to represent the runway visual range for ILS approach.
    /// For example, Cat I localizers should have a longer min range than Cat III localizers.
    ///
    /// This value may fluctuate during ILS ground interference.
    pub min_dist_horizontal: f32,
    /// Minimum vertical distance of the receiver from the navaid, in nm.
    ///
    /// This is used to represent the decision height for ILS approach.
    /// For example, Cat I localizers should have a longer min range than Cat III localizers.
    ///
    /// This value may fluctuate during ILS ground interference.
    pub min_dist_vertical:   f32,

    /// Maximum horizontal distance of the receiver from the navaid, in nm.
    pub max_dist_horizontal: f32,
    /// Maximum vertical distance of the receiver from the navaid, in nm.
    pub max_dist_vertical:   f32,
}

/// Marks the navaid entity as a visual reference.
///
/// Visual navaids should have zero min distance and full heading/pitch range,
/// and the max range depends entirely on cloud level and visibility.
///
/// Runways should always have a visual navaid.
#[derive(Component)]
pub struct Visual; // TODO add system to control max range based on visibility

/// Marks that the navaid entity has an ILS critical region subject to ground interference.
#[derive(Component)]
pub struct HasCriticalRegion {} // TODO add system to control min range subject to interference

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
