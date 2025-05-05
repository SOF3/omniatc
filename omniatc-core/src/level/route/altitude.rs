use bevy::ecs::entity::Entity;
use bevy::ecs::world::{EntityWorldMut, World};

use super::{predict, trigger, CompletionCondition, ConditionId, NodeResyncResult, Route};
use crate::level::nav;
use crate::level::object::Object;
use crate::level::waypoint::Waypoint;
use crate::try_log_return;
use crate::units::{Distance, Position};

#[derive(Clone, derive_more::From)]
#[portrait::derive(NodeKind with portrait::derive_delegate)]
pub enum Node {
    ApproachBy(ApproachBy),
    AlignGlidePath(AlignGlidePath),
}

impl super::Node for Node {
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult {
        NodeKind::resync(self, object)
    }

    fn teardown(&mut self, object: &mut EntityWorldMut) { NodeKind::teardown(self, object) }

    fn set_eventual(&self, world: &World, object: Entity, state: &mut predict::State) {
        NodeKind::set_eventual(self, world, object, &mut state.altitude);
    }
}

#[portrait::make]
trait NodeKind {
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult;

    fn teardown(&mut self, object: &mut EntityWorldMut);

    fn set_eventual(&self, world: &World, object: Entity, altitude: &mut Position<f32>);
}

/// Maintain current altitude,
/// then start climbing/descending to `altitude` at the appropriate moment
/// such that the object is exactly at `altitude` when `deadline` is notified.
///
/// Completes when the difference between current altitude and `altitude` satisfies `completion_condition`.
#[derive(Clone)]
pub struct ApproachBy {
    pub altitude:             Position<f32>,
    pub deadline:             ConditionId,
    pub completion_condition: CompletionCondition<Distance<f32>>,
}

impl NodeKind for ApproachBy {
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult {
        let schedule =
            &object.get::<Route>().expect("cannot resync entity without schedule").schedule;
        if schedule.is_condition_notified(self.deadline) {
            return self.start_approach_now(object, true);
        }

        // A: maintain a1                     -> climb a2           -> exactly c2
        // S: maintain s1        -> wait c1   -> set s2
        // H: goto p1 -> goto p2 -> notify c1 -> goto p3 -> goto p4 -> notify c2

        schedule.channels.slice_until_condition(self.deadline);

        NodeResyncResult::Pending
    }

    fn teardown(&mut self, object: &mut EntityWorldMut) { object.remove::<nav::TargetAltitude>(); }

    fn set_eventual(&self, _world: &World, _object: Entity, altitude: &mut Position<f32>) {
        *altitude = self.altitude;
    }
}

impl ApproachBy {
    fn start_approach_now(&self, entity: &mut EntityWorldMut, expedite: bool) -> NodeResyncResult {
        entity.insert(nav::TargetAltitude { altitude: self.altitude, expedite });

        let altitude =
            entity.get::<Object>().expect("resync must be used on object").position.altitude();
        match self.completion_condition.satisfies(altitude - self.altitude) {
            Ok(()) => NodeResyncResult::Completed,
            Err(tolerance) => {
                entity.insert(trigger::NearAltitude { target: self.altitude, tolerance });
                NodeResyncResult::Pending
            }
        }
    }
}

#[derive(Clone)]
pub struct AlignGlidePath {
    /// The runway entity to align to.
    pub runway: Entity,
}

impl NodeKind for AlignGlidePath {
    fn resync(&mut self, entity: &mut EntityWorldMut) -> NodeResyncResult { todo!() }

    fn teardown(&mut self, entity: &mut EntityWorldMut) { todo!() }

    fn set_eventual(&self, world: &World, _object: Entity, altitude: &mut Position<f32>) {
        let waypoint = try_log_return!(
            world.get::<Waypoint>(self.runway),
            expect "AlignGlidePath node must reference a runway with a waypoint component"
        );
        *altitude = waypoint.position.altitude();
    }
}
