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

#[portrait::derive(Node with portrait::derive_delegate)]
#[derive(Clone)]
pub enum Passive {
    RequireMinimumNavaids(RequireMinimumNavaids),
    RequireSpecificNavaid(RequireSpecificNavaid),
    WaitSpecificNavaid(WaitSpecificNavaid),
}

/// Fails when there are too few reachable navaids.
#[derive(Clone)]
pub struct RequireMinimumNavaids {
    /// Minimum number of navaids required.
    pub minimum_count: u32,
    /// Route template to replace with when there are insufficient navaids.
    pub otherwise:     Option<Entity>,
}

impl Node for RequireMinimumNavaids {
    fn predict(&self, _: &mut World, _: &mut PredictState, _: Entity) {}
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync { todo!() }
    fn teardown(self, params: &mut World, object: Entity) { todo!() }
}

/// Fails when `navaid` is not reachable.
#[derive(Clone)]
pub struct RequireSpecificNavaid {
    /// The navaid required.
    pub navaid:    Entity,
    /// Route template to replace with when there are insufficient navaids.
    pub otherwise: Option<Entity>,
}

impl Node for RequireSpecificNavaid {
    fn predict(&self, _: &mut World, _: &mut PredictState, _: Entity) {}
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync { todo!() }
    fn teardown(self, params: &mut World, object: Entity) { todo!() }
}

/// Completes when `navaid` is reachable.
#[derive(Clone)]
pub struct WaitSpecificNavaid {
    /// The navaid to wait for.
    pub navaid: Entity,
}

impl Node for WaitSpecificNavaid {
    fn predict(&self, _: &mut World, _: &mut PredictState, _: Entity) {}
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync { todo!() }
    fn teardown(self, params: &mut World, object: Entity) { todo!() }
}
