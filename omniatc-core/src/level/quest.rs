//! A quest is a completable task predefined in the save file.
//!
//! A quest is in one of the following states:
//!
//! - Active: The quest can be completed currently
//!   by fulfilling its conditions.
//! - Pending: The quest is not yet active
//!   due to having incomplete dependencies.
//! - Completed: The quest has been completed.
//!
//! All quests have the [`Quest`] component.

use std::mem;
use std::sync::Arc;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::{Entity, EntityHashSet};
use bevy::ecs::message::Message;
use bevy::ecs::query::{Has, QueryData};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Command, Commands, Query};
use bevy::ecs::world::World;

use crate::level::waypoint::Waypoint;
use crate::level::{SystemSets, object};
use crate::{WorldTryLog, load};

pub mod condition;
pub mod highlight;
pub mod loader;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_plugins(condition::Plug);
        app.add_message::<UiEvent>();
        app.add_systems(
            app::Update,
            manage_active_system
                .in_set(SystemSets::QuestCompletion)
                .after(condition::RemovalSystemSet),
        );
    }
}

/// Basic scalar metadata for a quest.
#[derive(Component)]
pub struct Quest {
    pub title:       String,
    pub description: String,
    pub class:       store::QuestClass,
    /// Sorting index among active quests.
    pub index:       usize,

    /// Actions to perform when the quest is completed.
    ///
    /// We deliberately do not convert the store type into a runtime type here,
    /// because completion hooks typically need to be re-serialized
    /// if the quest remains incomplete,
    /// and the execution logic typically involves invoking store conversion logic,
    /// such as loading an object from a stored template.
    pub completion_hooks: Vec<store::QuestCompletionHook>,
}

#[derive(Component, Default)]
pub struct Topology {
    /// List of quests that this quest depends on,
    /// including completed ones.
    pub dependencies: Vec<Entity>,
    /// List of quests that depend on this quest,
    pub dependents:   Vec<Entity>,
}

/// Marker component to indicate that a quest is currently active.
#[derive(Component)]
pub struct Active;

#[derive(QueryData)]
#[query_data(mutable)]
struct ActiveQuestQuery {
    entity:   Entity,
    quest:    &'static mut Quest,
    topology: &'static Topology,
    active:   Has<Active>,
    counter:  condition::Counter,
}

fn manage_active_system(mut commands: Commands, mut query: Query<ActiveQuestQuery>) {
    let mut completed_quests = EntityHashSet::new();
    for mut data in &mut query {
        if data.counter.count() == 0 {
            completed_quests.insert(data.entity);

            for hook in mem::take(&mut data.quest.completion_hooks) {
                commands.queue(ExecuteCompletionHook(hook));
            }
        }
    }

    let mut active_quests = EntityHashSet::new();
    let mut min_active_index = None;

    for data in &query {
        if data.counter.count() > 0
            && data.topology.dependencies.iter().all(|dep| completed_quests.contains(dep))
        {
            active_quests.insert(data.entity);

            if min_active_index.is_none_or(|(index, _)| index > data.quest.index) {
                min_active_index = Some((data.quest.index, data.entity));
            }
        }
    }

    for data in query {
        let is_active = active_quests.contains(&data.entity);
        match (data.active, is_active) {
            (true, false) => {
                commands.entity(data.entity).remove::<Active>();
            }
            (false, true) => {
                commands.entity(data.entity).insert(Active);
            }
            _ => {}
        }
    }
}

struct ExecuteCompletionHook(store::QuestCompletionHook);

impl Command for ExecuteCompletionHook {
    fn apply(self, world: &mut World) {
        match self.0 {
            store::QuestCompletionHook::SpawnObject { object } => {
                let contexts = world.resource::<load::SpawnContext>();
                let aerodromes = Arc::clone(&contexts.aerodromes);
                let waypoints = Arc::clone(&contexts.waypoints);
                let route_presets = Arc::clone(&contexts.route_presets);
                let mut next_standby_id = contexts.next_standby_id;

                if let Err(err) = object::loader::spawn(
                    world,
                    &aerodromes,
                    &waypoints,
                    &route_presets,
                    &mut next_standby_id,
                    &object,
                ) {
                    bevy::log::error!("Failed to spawn object: {err}");
                }

                world.resource_mut::<load::SpawnContext>().next_standby_id = next_standby_id;
            }
            store::QuestCompletionHook::RevealWaypoint { waypoint } => {
                let contexts = world.resource::<load::SpawnContext>();
                let waypoint_entity = match contexts.waypoints.resolve(&waypoint) {
                    Ok(entity) => entity,
                    Err(err) => {
                        bevy::log::error!("Unresolved waypoint to reveal: {err}");
                        return;
                    }
                };
                if let Some(mut waypoint) = world.log_get_mut::<Waypoint>(waypoint_entity) {
                    waypoint.hidden = false;
                }
            }
        }
    }
}

#[derive(Message)]
pub enum UiEvent {
    CameraDragged,
    CameraZoomed,
    CameraRotated,
    ObjectSelected,
}
