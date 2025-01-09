use std::collections::VecDeque;

use bevy::app::{self, App, Plugin};
use bevy::prelude::{
    Component, Entity, Event, EventWriter, Events, IntoSystemConfigs, Query, With,
};

use super::object::Object;
use super::waypoint::Waypoint;
use super::{nav, SystemSets};
use crate::units::Distance;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<StepRouteEvent>();
        app.allow_ambiguous_resource::<Events<StepRouteEvent>>();

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
    pub distance:  Distance<f32>,
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
    distance: Distance<f32>,
}

fn fly_over_trigger_system(
    waypoint_query: Query<&Waypoint>,
    object_query: Query<(Entity, &Object, &Route, &FlyOverTrigger)>,
    mut step_route_events: EventWriter<StepRouteEvent>,
) {
    object_query.iter().for_each(
        |(object_entity, &Object { position: current_pos, .. }, route, trigger)| {
            let Some(node) = route.current() else {
                bevy::log::error!("FlyOverTrigger applied on object with an empty Route");
                return;
            };

            let Ok(&Waypoint { position: current_target, .. }) = waypoint_query.get(node.waypoint)
            else {
                bevy::log::error!("Invalid waypoint referenced in route");
                return;
            };

            if current_pos.distance_cmp(current_target) <= trigger.distance {
                step_route_events.send(StepRouteEvent(object_entity));
            }
        },
    );
}

#[derive(Component)]
struct FlyByTrigger;

fn fly_by_trigger_system(
    waypoint_query: Query<&Waypoint>,
    object_query: Query<(Entity, &Object, &nav::Limits, &Route), With<FlyByTrigger>>,
    mut step_route_events: EventWriter<StepRouteEvent>,
) {
    object_query.iter().for_each(
        |(
            object_entity,
            &Object { position: current_pos, ground_speed: speed },
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
            let current_target = current_target.horizontal();

            match route.next() {
                Some(next_node) => {
                    let Ok(&Waypoint { position: next_target, .. }) =
                        waypoint_query.get(next_node.waypoint)
                    else {
                        bevy::log::error!("Invalid waypoint referenced in next node in route");
                        return;
                    };
                    let next_target = next_target.horizontal();

                    let current_heading = (current_target - current_pos.horizontal()).heading();
                    let next_heading = (next_target - current_target).heading();
                    let turn_radius = Distance(
                        speed.horizontal().magnitude_exact().0 / nav_limits.max_yaw_speed.0,
                    ); // (L/T) / (1/T) = L
                    let turn_distance = turn_radius
                        * (current_heading.closest_distance(next_heading).abs() / 2.).tan();

                    if current_pos.horizontal().distance_cmp(current_target) <= turn_distance {
                        step_route_events.send(StepRouteEvent(object_entity));
                    }
                }
                None => {
                    if current_pos.horizontal().distance_cmp(current_target)
                        <= current_node.distance
                    {
                        step_route_events.send(StepRouteEvent(object_entity));
                    }
                }
            }
        },
    );
}

#[derive(Event)]
struct StepRouteEvent(Entity);
