use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::math::Vec2;
use bevy::prelude::{
    Commands, Component, Entity, EntityCommand, IntoScheduleConfigs, Query, Res, World,
};
use bevy::time::{self, Time};

use super::schedule;
use crate::level::object::Object;
use crate::level::waypoint::Waypoint;
use crate::level::SystemSets;
use crate::math::Between;
use crate::units::{Distance, Position, Speed};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            (altitude_diff_system, waypoint_distance_system).in_set(SystemSets::Action),
        );
    }
}

#[derive(Component)]
pub struct NearAltitude {
    pub target:    Position<f32>,
    pub tolerance: Distance<f32>,
}

fn altitude_diff_system(
    time: Res<Time<time::Virtual>>,
    object_query: Query<(Entity, &Object, &NearAltitude)>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query
        .iter()
        .filter(|(_, &Object { position, .. }, trigger)| {
            (position.altitude() - trigger.target).abs() <= trigger.tolerance
        })
        .for_each(|(object, _, _)| {
            commands.entity(object).queue(schedule::DoResync);
        });
}

#[derive(Component)]
pub struct NearWaypoint {
    pub target_waypoint: Entity,
    pub tolerance:       Distance<f32>,
}

fn waypoint_distance_system(
    time: Res<Time<time::Virtual>>,
    object_query: Query<(Entity, &Object, &NearWaypoint)>,
    waypoint_query: Query<&Waypoint>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query
        .iter()
        .filter(|(_, &Object { position, .. }, trigger)| {
            let Ok(&Waypoint { position: waypoint_position, .. }) =
                waypoint_query.get(trigger.target_waypoint)
            else {
                bevy::log::error!(
                    "Unknown target waypoint {:?} in distance trigger",
                    trigger.target_waypoint
                );
                return false;
            };
            position.horizontal().distance_cmp(waypoint_position.horizontal()) <= trigger.tolerance
        })
        .for_each(|(object, _, _)| {
            commands.entity(object).queue(schedule::DoResync);
        });
}
