use bevy::ecs::entity::Entity;
use bevy::ecs::world::{EntityWorldMut, World};
use bevy::math::Vec2;

use super::{predict, trigger, CompletionCondition, ConditionId, NodeResyncResult};
use crate::level::nav;
use crate::level::object::Object;
use crate::level::waypoint::Waypoint;
use crate::units::{Distance, Position};
use crate::{try_log, try_log_return};

#[derive(Clone, derive_more::From)]
#[portrait::derive(NodeKind with portrait::derive_delegate)]
pub enum Node {
    DirectWaypoint(DirectWaypoint),
    AlignLocalizer(AlignLocalizer),
}

impl super::Node for Node {
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult {
        NodeKind::resync(self, object)
    }

    fn teardown(&mut self, object: &mut EntityWorldMut) { NodeKind::teardown(self, object) }

    fn set_eventual(&self, world: &World, object: Entity, state: &mut predict::State) {
        NodeKind::set_eventual(self, world, object, &mut state.ground_position);
    }
}

#[portrait::make]
trait NodeKind {
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult;

    fn teardown(&mut self, object: &mut EntityWorldMut);

    fn set_eventual(&self, world: &World, object: Entity, state: &mut Position<Vec2>);
}

#[derive(Clone)]
pub struct DirectWaypoint {
    pub waypoint:             Entity,
    pub completion_condition: CompletionCondition<Distance<f32>>,
}

impl NodeKind for DirectWaypoint {
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult {
        let &Waypoint { position: target_position, .. } = try_log!(
            object.world().get(self.waypoint),
            expect "DirectWaypoint node must reference a valid waypoint entity"
            or return NodeResyncResult::Pending
        );

        let Object { position: current_position, .. } =
            try_log!(object.get(), expect "invalid object" or return NodeResyncResult::Pending);

        match self.completion_condition.satisfies(current_position.distance_cmp(target_position)) {
            Ok(()) => NodeResyncResult::Completed,
            Err(tolerance) => {
                object.insert((
                    nav::TargetWaypoint { waypoint_entity: self.waypoint },
                    trigger::NearWaypoint { target_waypoint: self.waypoint, tolerance },
                ));
                NodeResyncResult::Pending
            }
        }
    }

    fn teardown(&mut self, object: &mut EntityWorldMut) {
        object.remove::<(trigger::NearWaypoint, nav::TargetWaypoint)>();
    }

    fn set_eventual(&self, world: &World, _object: Entity, state: &mut Position<Vec2>) {
        let waypoint = try_log_return!(
            world.get::<Waypoint>(self.waypoint),
            expect "DirectWaypoint node must reference a waypoint"
        );
        *state = waypoint.position.horizontal();
    }
}

#[derive(Clone)]
pub struct AlignLocalizer {
    /// The runway entity to align to.
    pub runway: Entity,
}

impl NodeKind for AlignLocalizer {
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult { todo!() }

    fn teardown(&mut self, object: &mut EntityWorldMut) { todo!() }

    fn set_eventual(&self, world: &World, _object: Entity, state: &mut Position<Vec2>) {
        let waypoint = try_log_return!(
            world.get::<Waypoint>(self.runway),
            expect "AlignLocalizer node must reference a runway with a waypoint component"
        );
        *state = waypoint.position.horizontal();
    }
}
