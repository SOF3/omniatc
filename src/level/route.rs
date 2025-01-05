use std::collections::VecDeque;

use bevy::app::{self, App, Plugin};
use bevy::math::Vec3Swizzles;
use bevy::prelude::{Component, Entity, Event, EventWriter, IntoSystemConfigs, Query, With};

use super::waypoint::Waypoint;
use super::{nav, object, SystemSets};
use crate::math::Heading;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<StepRouteEvent>();
        app.add_systems(
            app::Update,
            (fly_over_trigger_system, fly_by_trigger_system).in_set(SystemSets::Action),
        );
    }
}

/// Stores the planned route of the object.
/// No effect alone.
#[derive(Component)]
pub struct Route {
    current: Option<Node>, // promoted to its own field to improve cache locality.
    nodes:   VecDeque<Node>,
}

impl Route {
    pub fn push(&mut self, node: Node) {
        if self.current.is_none() {
            self.current = Some(node);
        } else {
            self.nodes.push_back(node);
        }
    }

    pub fn current(&self) -> Option<&Node> { self.current.as_ref() }

    pub fn next(&self) -> Option<&Node> { self.nodes.front() }

    pub fn shift(&mut self) -> Option<Node> {
        let ret = self.current.take();
        self.current = self.nodes.pop_front();
        ret
    }

    pub fn iter(&self) -> impl Iterator<Item = &Node> {
        self.current.iter().chain(self.nodes.iter())
    }
}

pub struct Node {
    pub waypoint:  Entity,
    pub altitude:  Option<f32>,
    pub proximity: WaypointProximity,
    pub distance:  f32,
}

pub enum WaypointProximity {
    /// Turn to the next waypoint before arriving at the waypoint,
    /// such that the position after the turn is exactly between the two waypoints.
    ///
    /// The step is always completed when the proximity range is entered,
    /// allowing smooth tranistion when the next waypoint hsa the same heading.
    FlyBy,
    /// Enter the horizontal distance range of the waypoint before turning to the next one.
    FlyOver,
}

#[derive(Component)]
struct FlyOverTrigger {
    distance: f32,
}

fn fly_over_trigger_system(
    waypoint_query: Query<&Waypoint>,
    object_query: Query<(Entity, &object::Position, &Route, &FlyOverTrigger)>,
    mut step_route_events: EventWriter<StepRouteEvent>,
) {
    object_query.iter().for_each(
        |(object_entity, &object::Position(current_pos), route, trigger)| {
            let Some(node) = route.current() else {
                bevy::log::error!("FlyOverTrigger applied on object with an empty Route");
                return;
            };

            let Ok(&Waypoint { position: current_target, .. }) = waypoint_query.get(node.waypoint)
            else {
                bevy::log::error!("Invalid waypoint referenced in route");
                return;
            };

            if current_pos.distance_squared(current_target.into()) <= trigger.distance {
                step_route_events.send(StepRouteEvent(object_entity));
            }
        },
    );
}

#[derive(Component)]
struct FlyByTrigger;

fn fly_by_trigger_system(
    waypoint_query: Query<&Waypoint>,
    object_query: Query<
        (Entity, &object::Position, &object::GroundSpeed, &nav::Limits, &Route),
        With<FlyByTrigger>,
    >,
    mut step_route_events: EventWriter<StepRouteEvent>,
) {
    object_query.iter().for_each(
        |(
            object_entity,
            &object::Position(current_pos),
            &object::GroundSpeed(speed),
            nav_limits,
            route,
        )| {
            let Some(current_node) = route.current() else {
                bevy::log::error!("FlyOverTrigger applied on object with an empty Route");
                return;
            };

            let Ok(&Waypoint { position: current_target, .. }) =
                waypoint_query.get(current_node.waypoint)
            else {
                bevy::log::error!("Invalid waypoint referenced in route");
                return;
            };
            let current_target = current_target.xy();

            match route.next() {
                Some(next_node) => {
                    let Ok(&Waypoint { position: next_target, .. }) =
                        waypoint_query.get(next_node.waypoint)
                    else {
                        bevy::log::error!("Invalid waypoint referenced in next node in route");
                        return;
                    };
                    let next_target = next_target.xy();

                    let current_heading = Heading::from_vec2(current_target - current_pos.xy());
                    let next_heading = Heading::from_vec2(next_target - current_target);
                    let turn_radius = speed.xy().length() / nav_limits.max_yaw_speed;
                    let turn_distance = (current_heading.closest_distance(next_heading).abs() / 2.)
                        .tan()
                        * turn_radius;

                    if current_pos.xy().distance_squared(current_target) <= turn_distance.powi(2) {
                        step_route_events.send(StepRouteEvent(object_entity));
                    }
                }
                None => {
                    if current_pos.xy().distance_squared(current_target) <= current_node.distance {
                        step_route_events.send(StepRouteEvent(object_entity));
                    }
                }
            }
        },
    );
}

#[derive(Event)]
struct StepRouteEvent(Entity);
