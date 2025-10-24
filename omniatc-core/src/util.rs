use bevy::app::{App, Plugin};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::relationship::{Relationship, RelationshipTarget};
use bevy::ecs::system::Commands;

mod async_task;
pub use async_task::{
    AsyncPollList, AsyncResult, AsyncResultTrigger, RunAsync, run_async, run_async_local,
};
mod eq_any;
pub use eq_any::EqAny;
mod iter;
pub use iter::TakeLast;
mod rate_limit;
pub use rate_limit::RateLimit;
mod query;
pub use query::{MapQuery, QueryWith};
mod schedule;
pub use schedule::{EnumScheduleConfig, configure_ordered_system_sets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_plugins(async_task::Plug); }
}

// TODO deprecate this function. This style is bad for performance.
pub fn manage_entity_vec<C, X, NB>(
    list_entity: Entity,
    list: Option<&C>,
    ctx: &mut X,
    mut spawn_fn: impl FnMut(usize, &mut X) -> Option<NB>,
    mut update_fn: impl FnMut(usize, &mut X, Entity) -> Result<(), ()>,
    commands: &mut Commands,
) where
    C: Component + RelationshipTarget,
    NB: Bundle,
{
    let mut entities = list.into_iter().flat_map(C::iter).peekable();

    let mut index = 0;
    let mut fused = false;
    while !fused {
        if let Some(&entity) = entities.peek() {
            if update_fn(index, ctx, entity).is_ok() {
                entities.next();
            } else {
                fused = true;
            }
        } else {
            match spawn_fn(index, ctx) {
                Some(bundle) => {
                    commands.spawn((bundle, <C::Relationship as Relationship>::from(list_entity)));
                }
                _ => {
                    fused = true;
                }
            }
        }

        index += 1;
    }

    for entity in entities {
        commands.entity(entity).despawn();
    }
}
