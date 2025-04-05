use std::collections::VecDeque;
use std::convert;
use std::marker::PhantomData;
use std::time::Duration;

use bevy::app::{App, Plugin};
use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, EntityCommand, World};

use super::{nav, SystemSets};
use crate::units::{Distance, Position, Speed};

/// Horizontal distance before the point at which
/// an object must start changing altitude at standard rate
/// in order to reach the required configured altitude set in the future.
const ALTITUDE_CHANGE_TRIGGER_WINDOW: Distance<f32> = Distance::from_nm(1.);

/// Frequency of re-executing the route plan for each object.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

/// [Activation range](nav::TargetAlignment::activation_range) for `AlignRunway` nodes.
///
/// This constant has relatively longer activation range
/// compared to the default one triggered by explicit user command,
/// because the object is expected to immediately start aligning
/// by the time the `AlignRunway` node becomes active.
const ALIGN_RUNWAY_ACTIVATION_RANGE: Distance<f32> = Distance::from_nm(0.5);

/// [Lookahead duration](nav::TargetAlignment::lookahead) for `AlignRunway` nodes.
const ALIGN_RUNWAY_LOOKAHEAD: Duration = Duration::from_secs(10);

mod altitude;
// mod heading;
// mod passive;
// mod speed;
mod trigger;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_plugins(trigger::Plug); }
}

/// A predefined template of schedule nodes.
///
/// Each template is an individual entity.
#[derive(Component)]
pub struct RouteTemplate {
    altitude_control: Channel<altitude::Control>,
    // speed_control:    Channel<speed::Control>,
    // heading_control:  Channel<heading::Control>,
    // passive:          Channel<passive::Passive>,
}

pub struct ResyncTrigger;

impl EntityCommand for ResyncTrigger {
    fn apply(self, entity: Entity, world: &mut World) {
        'retry_all: loop {
            'next_channel: for f in Schedule::channels() {
                'retry_channel: loop {
                    match f.resync_channel(world, entity) {
                        ResyncChannelResult::Done => continue 'next_channel,
                        ResyncChannelResult::RetryThisChannel => continue 'retry_channel,
                        ResyncChannelResult::RetryAllChannels => continue 'retry_all,
                    }
                }
            }

            return;
        }
    }
}

fn resync_channel<T: ChannelType>(
    world: &mut World,
    entity: Entity,
    field: impl Fn(&mut Schedule) -> &mut Channel<T>,
) -> ResyncChannelResult {
    let pending_condition;
    let current_node = {
        let mut object = world.entity_mut(entity);
        let Some(mut schedule) = object.get_mut::<Schedule>() else {
            bevy::log::error!("ResyncTrigger invoked on an unschedulable object");
            return ResyncChannelResult::Done;
        };

        pending_condition = schedule.pending_condition;

        let channel = field(&mut *schedule);
        channel.nodes.pop_front()
    };

    let Some(mut current_node) = current_node else { return ResyncChannelResult::Done };

    let resync = match current_node {
        NodeOrFlow::Node(ref mut node) => node.resync(world, entity),
        NodeOrFlow::Wait { until } => {
            if pending_condition <= until {
                NodeResync::Pending
            } else {
                NodeResync::Complete
            }
        }
        NodeOrFlow::Notify { which } => {
            if pending_condition >= which {
                NodeResync::Complete
            } else {
                let mut object = world.entity_mut(entity);
                object.get_mut::<Schedule>().expect("checked above").pending_condition = which;
                return ResyncChannelResult::RetryAllChannels;
            }
        }
    };

    match resync {
        NodeResync::Pending => {
            let mut object = world.entity_mut(entity);
            let mut schedule = object.get_mut::<Schedule>().expect("checked above");
            let channel = field(&mut *schedule);
            channel.nodes.push_front(current_node);
            ResyncChannelResult::Done
        }
        NodeResync::Complete => {
            if let NodeOrFlow::Node(node) = current_node {
                node.teardown(world, entity);
            }
            ResyncChannelResult::RetryThisChannel
        }
        NodeResync::Fail { replacement } => {
            if let NodeOrFlow::Node(node) = current_node {
                node.teardown(world, entity);
            }

            if let Some(replacement) = replacement {
                let [mut object, template] = world.entity_mut([entity, replacement]);

                let mut schedule = object.get_mut::<Schedule>().expect("checked above");

                let Some(template) = template.get::<RouteTemplate>() else {
                    bevy::log::error!("Invalid RouteTemplate entity {replacement:?}");
                    return ResyncChannelResult::Done;
                };

                Schedule::channels().into_iter().for_each(|field| {
                    field.clear(&mut *schedule);
                    field.push_template(&mut *schedule, template);
                });
            } else {
                let mut schedule = world.get_mut::<Schedule>(entity).expect("checked above");
                Schedule::channels().into_iter().for_each(|field| {
                    field.clear(&mut *schedule);
                });
            }

            ResyncChannelResult::RetryAllChannels
        }
    }
}

#[derive(PartialEq)]
enum ResyncChannelResult {
    Done,
    RetryThisChannel,
    RetryAllChannels,
}

/// Conditions work like condvars:
/// channels can wait for the condition or notify the waiters of a condition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct ConditionId(u64);

/// Describes the planned route of an object.
///
/// A schedule is resynced when a [`ResyncTrigger`] for the object is received.
/// At each resync, the first node in each channel (if any) executes
/// by setting up its required components (mainly triggers)
/// and executing its initiation actions.
/// returns a [`NodeResync`] that determines the subsequent action.
#[derive(Component)]
pub struct Schedule {
    altitude_control: Channel<altitude::Control>,

    // speed_control:    Channel<speed::Control>,
    // heading_control:  Channel<heading::Control>,
    // passive:          Channel<passive::Passive>,
    /// The next `ConditionId` to be notified.
    pending_condition:    ConditionId,
    /// The next `ConditionId` to be returned for a new allocation.
    next_alloc_condition: ConditionId,
}

impl Schedule {
    fn channels() -> [&'static dyn ChannelField; 1] {
        [
            &ChannelFieldVtable(
                |schedule: &mut Schedule| &mut schedule.altitude_control,
                |template: &RouteTemplate| &template.altitude_control,
            ) as &dyn ChannelField,
            // (|schedule| &mut schedule.speed_control, ),
            // (|schedule| &mut schedule.heading_control, ),
            // (|schedule| &mut schedule.passive, ),
        ]
    }
}

struct Channel<T> {
    nodes: VecDeque<NodeOrFlow<T>>,
}

trait ChannelField {
    fn clear(&self, schedule: &mut Schedule);
    fn push_template(&self, schedule: &mut Schedule, template: &RouteTemplate);
    fn resync_channel(&self, world: &mut World, entity: Entity) -> ResyncChannelResult;
}

struct ChannelFieldVtable<T, F1, F2>(F1, F2)
where
    F1: for<'a> Fn(&'a mut Schedule) -> &'a mut Channel<T> + Copy,
    F2: for<'a> Fn(&'a RouteTemplate) -> &'a Channel<T> + Copy;
impl<T: ChannelType, F1, F2> ChannelField for ChannelFieldVtable<T, F1, F2>
where
    F1: for<'a> Fn(&'a mut Schedule) -> &'a mut Channel<T> + Copy,
    F2: for<'a> Fn(&'a RouteTemplate) -> &'a Channel<T> + Copy,
{
    fn clear(&self, schedule: &mut Schedule) { self.0(schedule).nodes.clear(); }

    fn push_template(&self, schedule: &mut Schedule, template: &RouteTemplate) {
        self.0(schedule).nodes.extend(self.1(template).nodes.iter().cloned());
    }
    fn resync_channel(&self, world: &mut World, entity: Entity) -> ResyncChannelResult {
        resync_channel(world, entity, self.0)
    }
}

#[derive(Clone)]
enum NodeOrFlow<T> {
    Node(T),
    /// Notify completion of `NotifyId` upon execution.
    /// Completes immediately.
    Notify {
        which: ConditionId,
    },
    /// Completes when `until` is notified.
    Wait {
        until: ConditionId,
    },
}

pub trait ChannelType: Clone {
    type PredictorForwardState;

    /// Resyncs the node by ensuring required components are inserted into `object`.
    fn resync(&mut self, world: &mut World, object: Entity) -> NodeResync;

    /// Cleans up the trigger components inserted by this node.
    fn teardown(self, params: &mut World, object: Entity);
}

pub trait Predictor<T: ChannelType> {
    fn scan_forward(
        &mut self,
        conditions: &mut PredictForwardConditions,
        channel_state: &mut T::PredictorForwardState,
    ) -> PredictForwardResult<impl PredictorRunBackward>;
}

pub trait PredictorRunBackward {
    fn run_backward(
        &mut self,
        state: &mut PredictState,
        duration: Duration,
    ) -> PredictBackwardResult;
}

pub struct FnPredictor<S, F, FB>(F, PhantomData<(S, fn() -> FB)>)
where
    F: FnMut(&mut PredictForwardConditions, &mut S) -> PredictForwardResult<FB>,
    FB: PredictorRunBackward;

impl<T, F, FB> Predictor<T> for FnPredictor<T::PredictorForwardState, F, FB>
where
    T: ChannelType,
    F: FnMut(
        &mut PredictForwardConditions,
        &mut T::PredictorForwardState,
    ) -> PredictForwardResult<FB>,
    FB: PredictorRunBackward,
{
    fn scan_forward(
        &mut self,
        conditions: &mut PredictForwardConditions,
        channel_state: &mut T::PredictorForwardState,
    ) -> PredictForwardResult<impl PredictorRunBackward> {
        (self.0)(conditions, channel_state)
    }
}

impl<FB> PredictorRunBackward for FB
where
    FB: FnMut(&mut PredictState, Duration) -> PredictBackwardResult,
{
    fn run_backward(
        &mut self,
        state: &mut PredictState,
        duration: Duration,
    ) -> PredictBackwardResult {
        self(state, duration)
    }
}

impl<T: ChannelType, P: Predictor<T>> Predictor<T> for Option<P> {
    fn scan_forward(
        &mut self,
        conditions: &mut PredictForwardConditions,
        channel_state: &mut T::PredictorForwardState,
    ) -> PredictForwardResult<impl PredictorRunBackward> {
        if let Some(this) = self {
            match this.scan_forward(conditions, channel_state) {
                PredictForwardResult::PendingCondition => PredictForwardResult::PendingCondition,
                PredictForwardResult::SpontaneousCompletion(run) => {
                    PredictForwardResult::SpontaneousCompletion(Some(run))
                }
            }
        } else {
            PredictForwardResult::SpontaneousCompletion(None)
        }
    }
}

impl<P: PredictorRunBackward> PredictorRunBackward for Option<P> {
    fn run_backward(
        &mut self,
        state: &mut PredictState,
        duration: Duration,
    ) -> PredictBackwardResult {
        if let Some(this) = self {
            this.run_backward(state, duration)
        } else {
            PredictBackwardResult::Done(Duration::ZERO)
        }
    }
}

pub struct PredictForwardConditions {
    pub next_pending_condition: ConditionId,
}

pub enum PredictForwardResult<RunBackward> {
    PendingCondition,
    SpontaneousCompletion(RunBackward),
}

pub struct PredictState {
    pub spatial: PredictStateSpatial,
    pub time:    Duration,
}

pub enum PredictStateSpatial {
    OnGround { segment: Entity },
    Airborne { altitude: Position<f32>, ias: Speed<f32>, position: Position<Vec2> },
}

pub enum PredictBackwardResult {
    Pending,
    Done(Duration),
}

pub enum NodeResync {
    /// The node is not complete yet.
    /// To be polled again in the next resync.
    Pending,
    /// The node is complete.
    ///
    /// The node will be torn down and popped from the channel.
    Complete,
    /// The node encountered an exceptional condition.
    ///
    /// The node will be torn down, and the entire schedule will be cleared.
    ///
    /// Replaces the entire schedule with the contents of
    /// the [`RouteTemplate`] from `replacement` if set.
    Fail { replacement: Option<Entity> },
}

#[derive(Clone, Copy)]
pub enum CompletionCondition<D: Copy> {
    Unconditional,
    Tolerance(D),
}

impl<D: Copy + PartialOrd> CompletionCondition<D> {
    /// Tests whether `error` is within the requirements.
    ///
    /// Returns `Err` with the maximum absolute tolerance for `error` on failure.
    pub fn satisfies(&self, error: impl PartialOrd<D>) -> Result<(), D> {
        match *self {
            CompletionCondition::Unconditional => Ok(()),
            CompletionCondition::Tolerance(tolerance) if error <= tolerance => Ok(()),
            CompletionCondition::Tolerance(tolerance) => Err(tolerance),
        }
    }
}
