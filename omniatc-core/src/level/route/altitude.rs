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
    StartApproachAltitude(StartApproachAltitude),
    ApproachAltitudeBy(ApproachAltitudeBy),
    AlignGlidepath(AlignGlidepath),
}

impl ChannelType for Control {
    type PredictorForwardState = PredictorForwardState;

    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync {
        <Self as Node>::resync(self, world, object)
    }

    fn teardown(self, params: &mut World, object: Entity) {
        <Self as Node>::teardown(self, params, object)
    }
}

pub struct PredictorForwardState {
    last_altitude: Position<f32>,
}

#[portrait::make]
pub trait Node {
    /// Predicts an object state change if this node is executed to completion.
    fn predict(&self, world: &mut World, object: Entity) -> impl Predictor<Control>;

    /// Resyncs the node by ensuring required components are inserted into `object`.
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync;

    /// Cleans up the trigger components inserted by this node.
    fn teardown(self, world: &mut World, object: Entity);
}

/// Immediately start approaching `altitude` upon initiation.
///
/// Completes when the current altitude differs from `altitude` less than
/// `completion_condition`.
#[derive(Clone)]
pub struct StartApproachAltitude {
    pub altitude:             Position<f32>,
    pub expedite:             bool,
    pub completion_condition: CompletionCondition<Distance<f32>>,
}

impl Node for StartApproachAltitude {
    fn predict(&self, world: &mut World, object: Entity) -> impl Predictor<Control> {
        let &Self { altitude: target_altitude, expedite, .. } = self;
        let Some(nav::Limits { std_climb, exp_climb, std_descent, exp_descent, .. }) =
            world.get(object)
        else {
            return None;
        };
        let [climb_rate, descent_rate] =
            if expedite { [exp_climb, exp_descent] } else { [std_climb, std_descent] }
                .map(|profile| profile.vert_rate);

        Some(FnPredictor(
            move |_, state: &mut PredictorForwardState| {
                let start_altitude = mem::replace(&mut state.last_altitude, target_altitude);
                PredictForwardResult::SpontaneousCompletion(
                    move |state: &mut PredictState, time| match state.spatial {
                        PredictStateSpatial::OnGround { .. } => {
                            PredictBackwardResult::Done(Duration::ZERO)
                        }
                        PredictStateSpatial::Airborne { ref mut altitude, .. } => {
                            // TODO FIXME: should only start start changing altitude
                            // when node is almost starting.

                            let altitude_rate =
                                if start_altitude > *altitude { descent_rate } else { climb_rate };
                            let required_time = (*altitude - start_altitude) / altitude_rate;

                            if required_time > time {
                                *altitude -= altitude_rate * time;
                                PredictBackwardResult::Pending
                            } else {
                                *altitude = start_altitude;
                                PredictBackwardResult::Done(required_time)
                            }
                        }
                    },
                )
            },
            PhantomData,
        ))
    }

    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync {
        world
            .entity_mut(object)
            .insert(nav::TargetAltitude { altitude: self.altitude, expedite: self.expedite });

        let Object { position, .. } = world.get(object).expect("object must have Position");
        match self.completion_condition.satisfies((position.altitude() - self.altitude).abs()) {
            Ok(()) => NodeResync::Complete,
            Err(tolerance) => {
                world
                    .entity_mut(object)
                    .insert(trigger::NearAltitude { target: self.altitude, tolerance });
                NodeResync::Pending
            }
        }
    }

    fn teardown(self, world: &mut World, object: Entity) {
        world.entity_mut(object).remove::<trigger::NearAltitude>();
    }
}

/// Start approaching `altitude` at normal rate
/// such that the target altitude is reached approximately at the time `when` is notified.
///
/// Expedites automatically if `altitude` is insufficient.
/// Completes when the current altitude differs from `altitude` less than
/// `completion_condition`.
#[derive(Clone)]
pub struct ApproachAltitudeBy {
    pub altitude:             Position<f32>,
    pub completion_condition: CompletionCondition<Distance<f32>>,
    pub when:                 ConditionId,
}

impl Node for ApproachAltitudeBy {
    fn predict(&self, world: &mut World, object: Entity) -> impl Predictor<Control> {
        let &Self { altitude: target_altitude, completion_condition, when } = self;
        let Some(nav::Limits { std_climb, exp_climb, std_descent, exp_descent, .. }) =
            world.get(object)
        else {
            return None;
        };
        let [climb_rate, descent_rate] = [std_climb, std_descent].map(|profile| profile.vert_rate);

        Some(FnPredictor(
            move |conditions, state: &mut PredictorForwardState| {
                if conditions.next_pending_condition >= when {
                    // in terms of forward prediction, this node is as relevant as
                    // a `Wait { until: when }` node.
                    return PredictForwardResult::PendingCondition;
                }

                let start_altitude = mem::replace(&mut state.last_altitude, target_altitude);
                PredictForwardResult::SpontaneousCompletion(
                    move |state: &mut PredictState, time| match state.spatial {
                        PredictStateSpatial::OnGround { .. } => {
                            PredictBackwardResult::Done(Duration::ZERO)
                        }
                        PredictStateSpatial::Airborne { ref mut altitude, .. } => {
                            let altitude_rate =
                                if start_altitude > *altitude { descent_rate } else { climb_rate };
                            let required_time = (*altitude - start_altitude) / altitude_rate;

                            if required_time > time {
                                *altitude -= altitude_rate * time;
                                PredictBackwardResult::Pending
                            } else {
                                *altitude = start_altitude;
                                PredictBackwardResult::Done(required_time)
                            }
                        }
                    },
                )
            },
            PhantomData,
        ))
    }

    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync {

    }

    fn teardown(self, world: &mut World, object: Entity) {
        world.entity_mut(object).remove::<trigger::NearAltitude>();
    }
}

/// Immediately start aligning the altitude to the glidepath of `runway`.
///
/// Completes when the aircraft touches down.
#[derive(Clone)]
pub struct AlignGlidepath {
    pub runway:            Entity,
    pub goaround_template: Option<Entity>,
}

impl Node for AlignGlidepath {}
