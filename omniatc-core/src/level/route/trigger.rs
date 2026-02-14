use std::mem;
use std::time::Duration;

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageReader;
use bevy::ecs::system::{Commands, Query, Res};
use bevy::math::Vec2;
use bevy::time::{self, Time};
use math::{Length, Position};

use super::{HorizontalTarget, NextNode, RunCurrentNode};
use crate::QueryTryLog;
use crate::level::object::Object;
use crate::level::waypoint::Waypoint;
use crate::level::{nav, navaid, taxi};

#[derive(Component)]
pub(super) struct FlyOver {
    pub(super) waypoint: Entity,
    pub(super) distance: Length<f32>,
}

pub(super) fn fly_over_system(
    time: Res<Time<time::Virtual>>,
    waypoint_query: Query<&Waypoint>,
    object_query: Query<(Entity, &Object, &FlyOver)>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query.iter().for_each(
        |(object_entity, &Object { position: current_pos, .. }, trigger)| {
            let Some(&Waypoint { position: current_target, .. }) =
                waypoint_query.log_get(trigger.waypoint)
            else {
                return;
            };

            if current_pos.distance_cmp(current_target) <= trigger.distance {
                commands.entity(object_entity).queue(NextNode);
            }
        },
    );
}

#[derive(Component)]
pub(super) struct FlyBy {
    pub(super) waypoint:             Entity,
    pub(super) completion_condition: FlyByCompletionCondition,
}

pub(super) enum FlyByCompletionCondition {
    Heading(HorizontalTarget),
    Distance(Length<f32>),
}

pub(super) fn fly_by_system(
    time: Res<Time<time::Virtual>>,
    waypoint_query: Query<&Waypoint>,
    object_query: Query<(Entity, &Object, &nav::Limits, &FlyBy)>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query.iter().for_each(
        |(
            object_entity,
            &Object { position: current_pos, ground_speed: speed },
            nav_limits,
            trigger,
        )| {
            let Some(&Waypoint { position: current_target, .. }) =
                waypoint_query.log_get(trigger.waypoint)
            else {
                return;
            };
            let current_target = current_target.horizontal();

            match trigger.completion_condition {
                FlyByCompletionCondition::Heading(ref heading_config) => {
                    let next_heading = match *heading_config {
                        HorizontalTarget::Waypoint(next_waypoint) => {
                            let Some(&Waypoint { position: next_target, .. }) =
                                waypoint_query.log_get(next_waypoint)
                            else {
                                return;
                            };
                            let next_target = next_target.horizontal();

                            (next_target - current_target).heading()
                        }
                        HorizontalTarget::Heading(heading) => heading,
                    };

                    let current_heading = (current_target - current_pos.horizontal()).heading();
                    let turn_radius = speed
                        .horizontal()
                        .magnitude_exact()
                        .arc_to_radius(nav_limits.max_yaw_speed);
                    let turn_distance = turn_radius
                        * (current_heading.closest_distance(next_heading).abs() / 2.)
                            .acute_signed_tan();

                    if current_pos.horizontal().distance_cmp(current_target) <= turn_distance {
                        commands.entity(object_entity).queue(NextNode);
                    }
                }
                FlyByCompletionCondition::Distance(max_distance) => {
                    if current_pos.horizontal().distance_cmp(current_target) <= max_distance {
                        commands.entity(object_entity).queue(NextNode);
                    }
                }
            }
        },
    );
}

#[derive(Component)]
pub(super) struct TimeDelay(pub(super) Duration);

pub(super) fn time_system(
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(Entity, &TimeDelay)>,
    mut commands: Commands,
) {
    object_query.iter_mut().for_each(|(object_entity, trigger)| {
        if trigger.0 < time.elapsed() {
            bevy::log::trace!("Trigger {object_entity:?} resync due to delayed resync");
            commands.entity(object_entity).queue(RunCurrentNode);
        }
    });
}

#[derive(Component)]
pub(super) struct ByDistance {
    pub(super) remaining_distance: Length<f32>,
    pub(super) last_observed_pos:  Position<Vec2>,
}

pub(super) fn distance_system(
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(Entity, &Object, &mut ByDistance)>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query.iter_mut().for_each(|(object_entity, object, mut trigger)| {
        let last_pos = mem::replace(&mut trigger.last_observed_pos, object.position.horizontal());
        trigger.remaining_distance -= last_pos.distance_exact(object.position.horizontal());

        if !trigger.remaining_distance.is_positive() {
            bevy::log::trace!("Trigger {object_entity:?} resync due to distance traveled");
            commands.entity(object_entity).queue(RunCurrentNode);
        }
    });
}

#[derive(Component)]
pub(super) struct NavaidChange;

pub(super) fn navaid_system(
    mut msg_reader: MessageReader<navaid::UsageChangeMessage>,
    mut commands: Commands,
) {
    for event in msg_reader.read() {
        bevy::log::trace!("Trigger {:?} resync due to navaid change", event.object);
        commands.entity(event.object).queue(RunCurrentNode);
    }
}

#[derive(Component)]
pub(super) struct TaxiTargetResolution;

pub(super) fn taxi_target_resolution_system(
    mut msg_reader: MessageReader<taxi::TargetResolutionMessage>,
    mut commands: Commands,
) {
    for event in msg_reader.read() {
        bevy::log::trace!("Trigger {:?} resync due to taxi resolution", event.object);
        commands.entity(event.object).queue(RunCurrentNode);
    }
}
