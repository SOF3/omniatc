use std::{convert, mem};

use bevy::ecs::system::SystemState;
use bevy::prelude::{
    Commands, Component, Entity, EntityCommand, EntityRef, IntoSystemConfigs, Query, Res, World,
};

use super::{nav, SystemSets, *};
use crate::level::object::{self, GroundSpeedCalculator, Object, RefAltitudeType};
use crate::level::runway::{self, Runway};
use crate::level::waypoint::Waypoint;
use crate::math::Between;
use crate::units::{Angle, Distance, Heading, Position, Speed};

#[portrait::derive(Node with portrait::derive_delegate)]
#[derive(Clone)]
pub enum Control {
    DirectWaypoint(DirectWaypoint),
    AlignLocalizer(AlignLocalizer),
}

/// Head towards the waypoint directly.
///
/// Completes when the current position is less than `completion_condition` away from `waypoint`.
#[derive(Clone)]
pub struct DirectWaypoint {
    pub waypoint:             Entity,
    pub completion_condition: CompletionCondition<Distance<f32>>,
}

impl Node for DirectWaypoint {
    fn predict(&self, world: &mut World, state: &mut PredictState, object: Entity) {
        let Some(waypoint) = world.get::<Waypoint>(self.waypoint) else {
            bevy::log::error!("Waypoint {:?} not found", self.waypoint);
            return;
        };
        let waypoint_position = waypoint.position;

        let duration = match state.spatial {
            PredictStateSpatial::OnGround { .. } => return,
            PredictStateSpatial::Airborne { ref mut position, ias, altitude } => {
                let old = mem::replace(position, waypoint.position);
                let dist = old.distance_exact(waypoint.position);
                let gs_calc = SystemState::<GroundSpeedCalculator>::new(world);
                gs_calc.get(world).get_ground_speed(position, ias);
            }
        };

        state.time += s
    }
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync { todo!() }
    fn teardown(self, params: &mut World, object: Entity) { todo!() }
}

/// Immediately start aligning the track to the localizer of `runway`.
///
/// Completes when the aircraft touches down.
///
/// Fails when the remaining runway distance is less than the required runway distance.
#[derive(Clone)]
pub struct AlignLocalizer {
    pub runway: Entity,
}

impl Node for AlignLocalizer {
    fn predict(&self, world: &mut World, state: &mut PredictState, object: Entity) {
        let Some(waypoint) = world.get::<Waypoint>(self.waypoint) else {
            bevy::log::error!("Waypoint {:?} not found", self.waypoint);
            return;
        };
        let waypoint_position = waypoint.position;

        let duration = match state.spatial {
            PredictStateSpatial::OnGround { .. } => return,
            PredictStateSpatial::Airborne { ref mut position, ias, altitude } => {
                let old = mem::replace(position, waypoint.position);
                let dist = old.distance_exact(waypoint.position);
                let gs_calc = SystemState::<GroundSpeedCalculator>::new(world);
                gs_calc.get(world).get_ground_speed(position, ias);
            }
        };

        state.time += s
    }
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync { todo!() }
    fn teardown(self, params: &mut World, object: Entity) { todo!() }
}
