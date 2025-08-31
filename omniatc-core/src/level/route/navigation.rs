use bevy::ecs::system::SystemState;
use bevy::math::Vec2;
use bevy::prelude::{Entity, World};
use math::{Between, Length, Position, Speed};

use super::{
    DesiredAltitude, HorizontalTarget, NodeKind, Route, RunNodeResult, WaypointProximity, trigger,
};
use crate::level::nav;
use crate::level::object::{self, Object};
use crate::level::waypoint::Waypoint;

/// Stay in this node until explicitly completed by user command.
#[derive(Clone, Copy)]
pub struct StandbyNode;

impl NodeKind for StandbyNode {
    fn run_as_current_node(&self, _: &mut World, _: Entity) -> RunNodeResult {
        RunNodeResult::PendingTrigger
    }

    fn desired_altitude(&self, _: &World) -> DesiredAltitude { DesiredAltitude::NotRequired }
}

/// Head towards a waypoint.
///
/// This node completes when `distance` OR `proximity` is satisfied.
#[derive(Clone, Copy)]
pub struct DirectWaypointNode {
    /// Waypoint to fly towards.
    pub waypoint:  Entity,
    /// The node is considered complete when
    /// the horizontal distance between the object and the waypoint is less than this value.
    pub distance:  Length<f32>,
    /// Whether the object is allowed to complete this node early when in proximity.
    pub proximity: WaypointProximity,
    /// Start pitching at standard rate *during or before* this node,
    /// approximately reaching this altitude by the time the specified waypoint is reached.
    pub altitude:  Option<Position<f32>>,
}

impl NodeKind for DirectWaypointNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        let Self { waypoint, distance, .. } = *self;

        world
            .entity_mut(entity)
            .remove::<(nav::TargetAlignment, nav::TargetAlignmentStatus)>()
            .insert(nav::TargetWaypoint { waypoint_entity: waypoint });

        match self.proximity {
            WaypointProximity::FlyOver => {
                world.entity_mut(entity).insert(trigger::FlyOver { waypoint, distance });
                RunNodeResult::PendingTrigger
            }
            WaypointProximity::FlyBy => {
                let next_node = world.entity(entity).get::<Route>().and_then(|route| {
                    route.next_queue.iter().find_map(|node| node.configures_heading(world))
                });

                let completion_condition = match next_node {
                    None => trigger::FlyByCompletionCondition::Distance(distance),
                    Some(next) => trigger::FlyByCompletionCondition::Heading(next),
                };
                world.entity_mut(entity).insert(trigger::FlyBy { waypoint, completion_condition });
                RunNodeResult::PendingTrigger
            }
        }
    }

    fn configures_heading(&self, _world: &World) -> Option<HorizontalTarget> {
        Some(HorizontalTarget::Waypoint(self.waypoint))
    }

    fn desired_altitude(&self, world: &World) -> DesiredAltitude {
        match self.altitude {
            Some(altitude) => {
                if let Some(waypoint) = world.entity(self.waypoint).get::<Waypoint>() {
                    DesiredAltitude::Desired(waypoint.position.horizontal().with_altitude(altitude))
                } else {
                    bevy::log::error!(
                        "Invalid waypoint entity {:?} referenced from route node",
                        self.waypoint
                    );
                    DesiredAltitude::NotRequired
                }
            }
            None => DesiredAltitude::Inconclusive,
        }
    }

    fn configures_position(&self, world: &World) -> Option<Position<Vec2>> {
        world.get::<Waypoint>(self.waypoint).map(|waypoint| waypoint.position.horizontal())
    }
}

/// Increase/reduce the speed to the desired value.
///
/// When the object is not yet airborne, this would control the expected airspeed
/// if the object is immediately airborne.
#[derive(Clone, Copy)]
pub struct SetAirspeedNode {
    /// Desired speed to set.
    pub speed: Speed<f32>,
    /// The node completes immediately if `error` is `None`,
    /// or when the difference between `speed` and the indicated airspeed of the object
    /// is less than `error` if it is `Some`.
    pub error: Option<Speed<f32>>,
}

impl NodeKind for SetAirspeedNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        if let Some(mut airborne) = world.entity_mut(entity).get_mut::<nav::VelocityTarget>() {
            airborne.horiz_speed = self.speed;
            // TODO: what about ground objects?
        }

        let current_airspeed = move |world: &mut World| {
            let mut state = SystemState::<object::GetAirspeed>::new(world);
            state.get(world).get_airspeed(entity)
        };

        if self.error.is_none_or(|error| {
            current_airspeed(world)
                .horizontal()
                .magnitude_cmp()
                .between_inclusive(&(self.speed - error), &(self.speed + error))
        }) {
            RunNodeResult::NodeDone
        } else {
            RunNodeResult::PendingTrigger
        }
    }

    fn configures_airspeed(&self, _world: &World) -> Option<Speed<f32>> { Some(self.speed) }
}

/// Start pitching to reach the given altitude.
#[derive(Clone, Copy)]
pub struct StartSetAltitudeNode {
    /// The target altitude to reach.
    pub altitude: Position<f32>,
    /// The node completes immediately if `error` is `None`,
    /// or when the difference between `speed` and the real altitude of the object
    /// is less than `error` if it is `Some`.
    pub error:    Option<Length<f32>>,
    pub expedite: bool,
    // TODO control pressure altitude instead?
}

impl NodeKind for StartSetAltitudeNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        let mut entity_ref = world.entity_mut(entity);
        let current_altitude =
            entity_ref.get::<Object>().expect("entity must be an Object").position.altitude();
        if self.error.is_none_or(|error| {
            current_altitude.between_inclusive(&(self.altitude - error), &(self.altitude + error))
        }) {
            RunNodeResult::NodeDone
        } else {
            entity_ref
                .insert(nav::TargetAltitude { altitude: self.altitude, expedite: self.expedite });
            RunNodeResult::PendingTrigger
        }
    }

    fn desired_altitude(&self, _world: &World) -> DesiredAltitude { DesiredAltitude::NotRequired }
}
