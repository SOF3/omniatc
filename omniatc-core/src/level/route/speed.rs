use std::collections::VecDeque;
use std::time::Duration;
use std::{convert, mem};

use bevy::app::{self, App, Plugin};
use bevy::ecs::system::SystemState;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{
    Commands, Component, Entity, EntityCommand, EntityRef, IntoSystemConfigs, Query, Res, World,
};
use bevy::time::{self, Time};
use serde::{Deserialize, Serialize};

use super::{nav, SystemSets, *};
use crate::level::object::{self, GroundSpeedCalculator, Object, RefAltitudeType};
use crate::level::runway::{self, Runway};
use crate::level::waypoint::Waypoint;
use crate::math::Between;
use crate::units::{Angle, Distance, Heading, Position, Speed};

#[derive(Clone)]
#[portrait::derive(Node with portrait::derive_delegate)]
pub enum Control {
    StartApproachSpeed(StartApproachSpeed),
}

/// Immediately start approaching `speed` upon initiaiton.
#[derive(Clone)]
pub struct StartApproachSpeed {
    pub speed:                Speed<f32>,
    pub completion_condition: CompletionCondition<Speed<f32>>,
}

impl Node for StartApproachSpeed {
    fn predict(&self, world: &mut World, state: &mut PredictState, object: Entity) { todo!() }
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync {
        let Some(v_target) = world.entity_mut(object).get_mut::<nav::VelocityTarget>() else {
            bevy::log::error!("cannot set airspeed target on object without nav::T");
            return NodeResync::Complete
        };
        v_target.horiz_speed = self.altitude

        let current_airspeed = SystemState::<object::GetAirspeed>::new(world).get(world).get_airspeed(object);
    }
    fn teardown(self, params: &mut World, object: Entity) { todo!() }
}
