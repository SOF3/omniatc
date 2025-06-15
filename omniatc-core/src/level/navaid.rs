//! Navaid entities are linked to the waypoint representing the naviad.

use std::time::Duration;
use std::{mem, ops};

use bevy::app::{self, App, Plugin};
use bevy::ecs::event::EventWriter;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::Query;
use bevy::math::Vec3;
use bevy::prelude::{Component, Entity, Event};

use super::object::Object;
use super::waypoint::Waypoint;
use super::SystemSets;
use crate::try_log;
use crate::units::{Distance, Heading, Position, TurnDirection};
use crate::util::RateLimit;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<UsageChangeEvent>();
        app.add_systems(app::Update, maintain_usages_system.in_set(SystemSets::ReconcileForRead));
    }
}

/// Component on waypoints.
#[derive(Component)]
#[relationship_target(relationship = OwnerWaypoint, linked_spawn)]
pub struct ListAtWaypoint(Vec<Entity>);

impl ListAtWaypoint {
    #[must_use]
    pub fn navaids(&self) -> &[Entity] { &self.0 }
}

/// Reference to the waypoint owning this navaid.
#[derive(Component)]
#[relationship(relationship_target = ListAtWaypoint)]
pub struct OwnerWaypoint(pub Entity);

/// A spherical sector centered at the waypoint position
/// within which the navaid can be reliably received.
///
/// A navaid entity is always a child of a waypoint entity.
#[derive(Component)]
pub struct Navaid {
    pub kind: Kind,

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
    pub pitch_range_tan: ops::Range<f32>,

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

impl Navaid {
    #[must_use]
    pub fn is_usable_from(
        &self,
        user_position: Position<Vec3>,
        navaid_position: Position<Vec3>,
    ) -> bool {
        let to_user = user_position - navaid_position;
        let heading = to_user.horizontal().heading();
        if self
            .heading_range
            .start
            .nonzero_distance(self.heading_range.end, TurnDirection::Clockwise)
            < self.heading_range.start.nonzero_distance(heading, TurnDirection::Clockwise)
        {
            return false;
        }

        let dist_horizontal = to_user.horizontal().magnitude_squared();
        if !(self.min_dist_horizontal.squared() <= dist_horizontal
            && dist_horizontal <= self.max_dist_horizontal.squared())
        {
            return false;
        }

        let dist_vertical = to_user.vertical().abs();
        if !(self.min_dist_vertical <= dist_vertical && dist_vertical <= self.max_dist_vertical) {
            return false;
        }

        let ratio = dist_vertical / dist_horizontal.sqrt_or_zero();
        ratio.is_nan() || self.pitch_range_tan.start <= ratio && ratio <= self.pitch_range_tan.end
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Visual,
    Localizer,
    Vor,
    Dme,
    Gnss,
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
pub struct LandingAid; // TODO add system to control min range subject to interference

/// When a navaid connection is lost.
#[derive(Event)]
pub struct UsageChangeEvent {
    pub object: Entity,
}

/// Component on object entities, listing established navaid entities.
#[derive(Component, Default)]
pub struct ObjectUsageList(pub Vec<Entity>);

const MAINTAIN_USAGE_PERIOD: Duration = Duration::from_secs(1);

fn maintain_usages_system(
    mut rl: RateLimit,
    object_query: Query<(Entity, &Object, &mut ObjectUsageList)>,
    navaid_query: Query<(Entity, &OwnerWaypoint, &Navaid)>,
    waypoint_query: Query<&Waypoint>,
    mut usage_change_event_writer: EventWriter<UsageChangeEvent>,
) {
    if rl.should_run(MAINTAIN_USAGE_PERIOD).is_none() {
        return;
    }

    let mut prev = Vec::new();
    for (object_id, object, mut used) in object_query {
        mem::swap(&mut used.0, &mut prev);
        used.0.clear();
        used.0.extend(navaid_query.iter().filter(|(_, waypoint_ref, navaid)| {
            let waypoint = try_log!(waypoint_query.get(waypoint_ref.0), expect "navaid parent must be waypoint" or return false);
            navaid.is_usable_from(object.position, waypoint.position)
        }).map(|(navaid_id, _, _)| navaid_id));

        used.0.sort();
        if used.0 != prev {
            usage_change_event_writer.write(UsageChangeEvent { object: object_id });
        }
    }
}
