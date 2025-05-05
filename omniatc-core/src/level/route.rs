use std::collections::VecDeque;

use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::{EntityWorldMut, World};

pub mod altitude;
pub mod heading;
mod predict;
mod schedule;
pub mod trigger;

pub use schedule::{ConditionId, DoResync, Entry as ScheduleEntry, Schedule};

#[cfg(test)]
mod tests;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_plugins(trigger::Plug); }
}

/// A template entity has the `Template` component providing the schedule to be copied from
/// when an object is instructed to follow this template.
#[derive(Component)]
pub struct Template {
    pub schedule: Schedule,
}

/// The current schedule of an object.
#[derive(Component)]
pub struct Route {
    pub schedule: Schedule,
}

pub trait Node {
    /// Resync triggers and nav targets for this object.
    fn resync(&mut self, object: &mut EntityWorldMut) -> NodeResyncResult;

    /// Remove triggers set up in `resync`;
    /// remove nav targets if no longer relevant.
    fn teardown(&mut self, object: &mut EntityWorldMut);

    /// Sets the prediction state to the eventual state after this node completes.
    fn set_eventual(&self, world: &World, object: Entity, state: &mut predict::State);
}

#[derive(Debug, Clone, Copy)]
pub enum NodeResyncResult {
    /// Will need to wait for a [`DoResync`] trigger managed by the node.
    Pending,
    /// The node has been completed. Should be immediately torn down.
    Completed,
    /// The schedule has been overwritten by the node.
    /// A full resync is necessary.
    Interrupt,
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
