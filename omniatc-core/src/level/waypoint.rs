use std::ops;

use bevy::app::{App, Plugin};
use bevy::math::Vec3;
use bevy::prelude::{Component, Entity, EntityCommand, Event, World};

use crate::units::{Angle, Distance, Heading, Position};

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
    pub position:     Position<Vec3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayType {
    /// A normal point on the map.
    Waypoint,
    /// A VHF Omnidirectional Range station with Distance Measuring Equipment.
    VorDme,
    /// A VHF Omnidirectional Range station.
    Vor,
    /// A Distance Measuring Equipment station.
    Dme,
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

    /// Minimum and maximum angles of the receiver relative to the navaid.
    pub pitch_range: ops::Range<Angle<f32>>,

    /// Minimum horizontal distance of the receiver from the navaid.
    ///
    /// This is used to represent the runway visual range for ILS approach.
    /// For example, Cat I localizers should have a longer min range than Cat III localizers.
    ///
    /// This value may fluctuate during ILS ground interference.
    ///
    /// For non-ILS navaids, this value should always be 0.
    pub min_dist_horizontal: Distance<f32>,
    /// Minimum vertical distance of the receiver from the navaid.
    ///
    /// This is used to represent the decision height for ILS approach.
    /// For example, Cat I localizers should have a longer min range than Cat III localizers.
    ///
    /// This value may fluctuate during ILS ground interference.
    ///
    /// For non-ILS navaids, this value should always be 0.
    pub min_dist_vertical:   Distance<f32>,

    /// Maximum horizontal distance of the receiver from the navaid.
    pub max_dist_horizontal: Distance<f32>,
    /// Maximum vertical distance of the receiver from the navaid.
    pub max_dist_vertical:   Distance<f32>,
}

/// Marks the navaid entity as a visual reference.
///
/// Visual navaids should have zero min distance and full heading/pitch range,
/// and the max range depends entirely on cloud level and visibility.
///
/// Runways should always have a visual navaid.
#[derive(Component)]
pub struct Visual {
    // TODO add system to update Navaid horizontal range based on visibility
    /// Maximum visual range to see the runway.
    ///
    /// The actual visual range is the minimum of this value and the actual visibility.
    pub max_range: Distance<f32>,
}

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
