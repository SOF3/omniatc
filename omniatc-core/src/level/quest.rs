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

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::{Entity, EntityHashSet};
use bevy::ecs::message::Message;
use bevy::ecs::query::{Has, QueryData};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query};

use crate::level::SystemSets;

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
}

#[derive(Component, Default)]
pub struct Topology {
    /// List of quests that this quest depend on,
    /// including completed ones.
    pub dependencies: Vec<Entity>,
    /// List of quests that depend on this quest,
    pub dependents:   Vec<Entity>,
}

/// Marker component to indicate that a quest is currently active.
#[derive(Component)]
pub struct Active;

#[derive(QueryData)]
struct ActiveQuestQuery {
    entity:   Entity,
    quest:    &'static Quest,
    topology: &'static Topology,
    active:   Has<Active>,
    counter:  condition::Counter,
}

fn manage_active_system(mut commands: Commands, query: Query<ActiveQuestQuery>) {
    let completed_quests: EntityHashSet = query
        .iter()
        .filter_map(|data| (data.counter.count() == 0).then_some(data.entity))
        .collect();

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

#[derive(Message)]
pub enum UiEvent {
    CameraDragged,
    CameraZoomed,
    CameraRotated,
}
