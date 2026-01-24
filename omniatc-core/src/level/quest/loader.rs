use std::collections::HashMap;

use bevy::ecs::bundle::Bundle;
use bevy::ecs::entity::Entity;
use bevy::ecs::name::Name;
use bevy::ecs::world::{EntityWorldMut, World};

use crate::level::aerodrome::loader::AerodromeMap;
use crate::level::quest::{self, Quest, condition};
use crate::load::{self, StoredEntity};

/// Spawns quest entities from stored data.
///
/// # Errors
/// If any quest dependency cannot be resolved.
pub fn spawn(
    world: &mut World,
    tree: &store::QuestTree,
    aerodromes: &AerodromeMap,
) -> load::Result {
    let quests: HashMap<_, _> = tree
        .quests
        .iter()
        .enumerate()
        .map(|(index, quest)| {
            let mut entity = world.spawn(spawn_quest(quest, index));

            for condition in &quest.conditions {
                insert_condition(&mut entity, condition, aerodromes)?;
            }

            for highlight in &quest.ui_highlight {
                insert_highlight(&mut entity, highlight);
            }

            Ok((&quest.id, (entity.id(), quest)))
        })
        .collect::<load::Result<_>>()?;

    populate_deps(world, &quests)?;

    Ok(())
}

fn spawn_quest(quest: &store::Quest, index: usize) -> impl Bundle {
    (
        Quest {
            title: quest.title.clone(),
            description: quest.description.clone(),
            class: quest.class,
            index,
        },
        quest::Topology::default(),
        Name::new("Quest"),
        StoredEntity,
    )
}

fn insert_condition(
    entity: &mut EntityWorldMut,
    condition: &store::QuestCompletionCondition,
    segments: &AerodromeMap,
) -> load::Result {
    match condition {
        store::QuestCompletionCondition::Camera(condition) => match condition {
            store::CameraQuestCompletionCondition::Drag => {
                entity.insert(condition::UiActionDrag);
            }
            store::CameraQuestCompletionCondition::Zoom => {
                entity.insert(condition::UiActionZoom);
            }
            store::CameraQuestCompletionCondition::Rotate => {
                entity.insert(condition::UiActionRotate);
            }
        },
        store::QuestCompletionCondition::ObjectControl(condition) => match condition {
            store::ObjectControlQuestCompletionCondition::ReachAltitude(range) => {
                entity.insert(condition::ReachAltitude { min: range.min, max: range.max });
            }
            store::ObjectControlQuestCompletionCondition::ReachSpeed(range) => {
                entity.insert(condition::ReachSpeed { min: range.min, max: range.max });
            }
            store::ObjectControlQuestCompletionCondition::ReachHeading(range) => {
                entity.insert(condition::ReachHeading { min: range.min, max: range.max });
            }
            store::ObjectControlQuestCompletionCondition::DirectToWaypoint => {
                entity.insert(condition::InstrActionDirectWaypoint);
            }
            store::ObjectControlQuestCompletionCondition::ClearIls => {
                entity.insert(condition::InstrActionClearIls);
            }
            store::ObjectControlQuestCompletionCondition::TaxiSegment(segment) => {
                let label = segments.resolve_segment(segment)?;
                entity.insert(condition::ReachSegment { label });
            }
            store::ObjectControlQuestCompletionCondition::ClearLineUp => {
                entity.insert(condition::InstrActionClearLineUp);
            }
            store::ObjectControlQuestCompletionCondition::ClearTakeoff => {
                entity.insert(condition::InstrActionClearTakeoff);
            }
            store::ObjectControlQuestCompletionCondition::FollowRoute => {
                entity.insert(condition::InstrActionFollowRoute);
            }
        },
        store::QuestCompletionCondition::Statistic(condition) => match *condition {
            store::StatisticQuestCompletionCondition::MinLanding(landings) => {
                entity.insert(condition::MinLanding { landings });
            }
            store::StatisticQuestCompletionCondition::MinParking(parkings) => {
                entity.insert(condition::MinParking { parkings });
            }
            store::StatisticQuestCompletionCondition::MinDeparture(departures) => {
                entity.insert(condition::MinDeparture { departures });
            }
            store::StatisticQuestCompletionCondition::MinScore(score) => {
                entity.insert(condition::MinScore { score });
            }
            store::StatisticQuestCompletionCondition::MaxConflicts(conflicts) => {
                entity.insert(condition::MaxConflicts { conflicts });
            }
            store::StatisticQuestCompletionCondition::TimeElapsed(time) => {
                entity.insert(condition::TimeElapsed { time });
            }
        },
    }

    Ok(())
}

fn insert_highlight(entity: &mut EntityWorldMut, highlight: &store::HighlightableUiElement) {
    match highlight {
        store::HighlightableUiElement::RadarView => {
            entity.insert(quest::highlight::RadarView);
        }
        store::HighlightableUiElement::SetCameraRotation => {
            entity.insert(quest::highlight::SetCameraRotation);
        }
        store::HighlightableUiElement::SetCameraZoom => {
            entity.insert(quest::highlight::SetCameraZoom);
        }
        store::HighlightableUiElement::SetAltitude => {
            entity.insert(quest::highlight::SetAltitude);
        }
        store::HighlightableUiElement::SetSpeed => {
            entity.insert(quest::highlight::SetSpeed);
        }
        store::HighlightableUiElement::SetHeading => {
            entity.insert(quest::highlight::SetHeading);
        }
    }
}

fn populate_deps(
    world: &mut World,
    quests: &HashMap<&store::QuestRef, (Entity, &store::Quest)>,
) -> load::Result {
    for &(entity, quest) in quests.values() {
        for dep in &quest.dependencies {
            let Some(&(dep_entity, _)) = quests.get(dep) else {
                return Err(load::Error::UnresolvedQuest(dep.0.clone()));
            };

            world
                .entity_mut(entity)
                .get_mut::<quest::Topology>()
                .expect("inserted during HashMap construction")
                .dependencies
                .push(dep_entity);
            world
                .entity_mut(dep_entity)
                .get_mut::<quest::Topology>()
                .expect("inserted during HashMap construction")
                .dependents
                .push(entity);
        }
    }
    Ok(())
}
