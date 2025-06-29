use std::mem;
use std::time::Duration;

use bevy::ecs::event::EventReader;
use bevy::math::Vec2;
use bevy::prelude::{Commands, Component, Entity, Query, Res};
use bevy::time::{self, Time};

use super::{HorizontalTarget, NextNode, RunCurrentNode};
use crate::level::object::Object;
use crate::level::waypoint::Waypoint;
use crate::level::{nav, navaid, taxi};
use crate::try_log_return;
use crate::units::{Distance, Position};

#[derive(Component)]
pub(super) struct FlyOver {
    pub(super) waypoint: Entity,
    pub(super) distance: Distance<f32>,
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
            let &Waypoint { position: current_target, .. } = try_log_return!(
                waypoint_query.get(trigger.waypoint),
                expect "Invalid waypoint referenced in route"
            );

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
    Distance(Distance<f32>),
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
            let &Waypoint { position: current_target, .. } = try_log_return!(
                waypoint_query.get(trigger.waypoint),
                expect "Invalid waypoint referenced in route"
            );
            let current_target = current_target.horizontal();

            match trigger.completion_condition {
                FlyByCompletionCondition::Heading(ref heading_config) => {
                    let next_heading = match *heading_config {
                        HorizontalTarget::Position(next_target) => {
                            (next_target - current_target).heading()
                        }
                        HorizontalTarget::Waypoint(next_waypoint) => {
                            let &Waypoint { position: next_target, .. } = try_log_return!(
                                waypoint_query.get(next_waypoint),
                                expect "Invalid waypoint referenced in next node in route"
                            );
                            let next_target = next_target.horizontal();

                            (next_target - current_target).heading()
                        }
                        HorizontalTarget::Heading(heading) => heading,
                    };

                    let current_heading = (current_target - current_pos.horizontal()).heading();
                    let turn_radius = Distance(
                        speed.horizontal().magnitude_exact().0 / nav_limits.max_yaw_speed.0,
                    ); // (L/T) / (1/T) = L
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
        if trigger.0 >= time.elapsed() {
            commands.entity(object_entity).queue(RunCurrentNode);
        }
    });
}

#[derive(Component)]
pub(super) struct ByDistance {
    pub(super) remaining_distance: Distance<f32>,
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
            commands.entity(object_entity).queue(RunCurrentNode);
        }
    });
}

#[derive(Component)]
pub(super) struct NavaidChange;

pub(super) fn navaid_system(
    mut event_reader: EventReader<navaid::UsageChangeEvent>,
    mut commands: Commands,
) {
    for event in event_reader.read() {
        commands.entity(event.object).queue(RunCurrentNode);
    }
}

#[derive(Component)]
pub(super) struct TaxiTargetResolution;

pub(super) fn taxi_target_resolution_system(
    mut event_reader: EventReader<taxi::TargetResolutionEvent>,
    mut commands: Commands,
) {
    for event in event_reader.read() {
        commands.entity(event.object).queue(RunCurrentNode);
    }
}
