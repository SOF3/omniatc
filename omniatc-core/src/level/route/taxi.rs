use std::time::Duration;

use bevy::ecs::entity::Entity;
use bevy::ecs::world::{EntityRef, World};
use smallvec::SmallVec;

use super::{trigger, NodeKind, RunNodeResult};
use crate::level::{ground, taxi};

#[derive(Clone)]
pub struct TaxiNode {
    pub step: TaxiStep,
}

#[derive(Clone)]
pub enum TaxiStep {
    /// Taxi through an adjacent segment with one of these labels if available,
    /// otherwise continue taxiing on segments with the same label as the current segment.
    Taxi(SmallVec<[ground::SegmentLabel; 1]>),
    /// Hold *before* an endpoint adjacent to a segment with this label,
    /// otherwise continue taxiing on segments with the same label as the current segment.
    ///
    /// Control loop will be stuck at this step until explicitly removed by
    /// an [`SystemSets::Communicate`]-level system.
    HoldShort(SmallVec<[ground::SegmentLabel; 1]>),
}

impl NodeKind for TaxiNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        let mut object = world.entity_mut(entity);
        if let Some(mut target) = object.get_mut::<taxi::Target>() {
            match target.resolution {
                Some(taxi::TargetResolution::PrimaryCompleted) => {
                    object.remove::<taxi::Target>();
                    RunNodeResult::NodeDone
                }
                None | Some(taxi::TargetResolution::SecondaryCompleted) => {
                    // Secondary completion basically means we need to recompute
                    // since an unideal segment is selected..
                    target.resolution = None;

                    let action = self.recompute_action(object.as_readonly());
                    object
                        .get_mut::<taxi::Target>()
                        .expect("Target should still be present after an immutable world lend")
                        .action = action;
                    // TODO add trigggers
                    RunNodeResult::PendingTrigger
                }
                Some(taxi::TargetResolution::Inoperable) => {
                    // TODO send message
                    RunNodeResult::PendingTrigger
                }
            }
        } else {
            let action = self.recompute_action(object.as_readonly());
            object.insert((
                taxi::Target { action, resolution: None },
                trigger::TimeTrigger(Duration::from_secs(1)),
            ));
            // TODO add trigggers
            RunNodeResult::PendingTrigger
        }
    }
}

impl TaxiNode {
    fn recompute_action(&self, object: EntityRef) -> taxi::TargetAction { todo!() }
}
