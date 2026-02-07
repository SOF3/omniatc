use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::MessageReader;
use bevy::ecs::query::{Added, With};
use bevy::ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, Query, Res, ScheduleSystem, SystemParam};
use bevy::time::{self, Time};
use math::{Heading, Position, Speed};
use store::Score;

use crate::QueryTryLog;
use crate::level::instr::Instruction;
use crate::level::object::{self, Object};
use crate::level::score::Stats;
use crate::level::{SystemSets, ground, instr, quest};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            (
                make_ui_event_system::<UiActionDrag>(|event| {
                    matches!(event, quest::UiEvent::CameraDragged)
                }),
                make_ui_event_system::<UiActionZoom>(|event| {
                    matches!(event, quest::UiEvent::CameraZoomed)
                }),
                make_ui_event_system::<UiActionRotate>(|event| {
                    matches!(event, quest::UiEvent::CameraRotated)
                }),
                reach_altitude_system,
                reach_speed_system,
                reach_heading_system,
                reach_segment_system,
                make_instr_action_system::<InstrActionDirectWaypoint>(|instr| {
                    matches!(
                        instr,
                        Instruction::SetWaypoint(_)
                            | Instruction::AirborneVector(instr::AirborneVector {
                                directional: Some(instr::AirborneVectorDirectional::SetWaypoint(_)),
                                ..
                            })
                    )
                }),
                make_instr_action_system::<InstrActionClearIls>(|_instr| {
                    false // TODO implement ILS clearance
                }),
                make_instr_action_system::<InstrActionClearLineUp>(|instr| {
                    matches!(instr, Instruction::RemoveStandby(_)) // TODO specific to lineup
                }),
                make_instr_action_system::<InstrActionClearTakeoff>(|instr| {
                    matches!(instr, Instruction::RemoveStandby(_)) // TODO specific to takeoff
                }),
                make_instr_action_system::<InstrActionFollowRoute>(|instr| {
                    matches!(instr, Instruction::SelectRoute(_))
                }),
                make_min_stat_system(|cond: &MinScore| cond.score, |stats: &Stats| stats.total),
                make_min_stat_system(
                    |cond: &MinLanding| cond.landings,
                    |stats: &Stats| stats.num_runway_arrivals,
                ),
                make_min_stat_system(
                    |cond: &MinParking| cond.parkings,
                    |stats: &Stats| stats.num_apron_arrivals,
                ),
                make_min_stat_system(
                    |cond: &MinDeparture| cond.departures,
                    |stats: &Stats| stats.num_departures,
                ),
                max_conflicts_system,
                time_elapsed_system,
            )
                .in_set(RemovalSystemSet)
                .in_set(SystemSets::QuestCompletion),
        );
        app.configure_sets(app::Update, RemovalSystemSet.ambiguous_with(RemovalSystemSet));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct RemovalSystemSet;

#[expect(non_snake_case)]
mod all {
    use bevy::ecs::bundle::Bundle;
    use bevy::ecs::query::{Has, QueryData};

    macro_rules! decl_counter {
        ($($presence:ident,)*) => {
            paste::paste! {
                /// Counts the number of unfulfilled conditions of an entity.
                ///
                /// This `QueryData` does not filter entities.
                /// The result may be latent if
                /// commands from condition completion systems have not been flushed yet.
                #[derive(QueryData)]
                #[expect(clippy::struct_field_names)]
                pub struct Counter {
                    $(
                        [<has_ $presence>]: Has<super::$presence>,
                    )*
                }

                impl CounterItem<'_, '_> {
                    pub fn count(&self) -> u32 {
                        let mut count = 0;
                        $(
                            if self.[<has_ $presence>] {
                                count += 1;
                            }
                        )*
                        count
                    }
                }

                /// Bundle of all condition components.
                ///
                /// Used for `entity.remove()` only.
                #[derive(Bundle)]
                pub struct AllBundle($(super::$presence),*);
            }
        }
    }

    decl_counter! {
        UiActionDrag,
        UiActionZoom,
        UiActionRotate,
        ReachAltitude,
        ReachSpeed,
        ReachHeading,
        ReachSegment,
        InstrActionDirectWaypoint,
        InstrActionClearIls,
        InstrActionClearLineUp,
        InstrActionClearTakeoff,
        InstrActionFollowRoute,
        MinLanding,
        MinParking,
        MinDeparture,
        MinScore,
        MaxConflicts,
        TimeElapsed,
    }
}
pub use all::{AllBundle, Counter, CounterItem};

/// Completes when the client reports camera dragging.
#[derive(Component)]
pub struct UiActionDrag;

/// Completes when the client reports camera zooming.
#[derive(Component)]
pub struct UiActionZoom;

/// Completes when the client reports camera rotation.
#[derive(Component)]
pub struct UiActionRotate;

#[derive(SystemParam)]
struct UiEventSystemParams<'w, 's, Cond: Component> {
    ui_event_reader: MessageReader<'w, 's, quest::UiEvent>,
    quest_query:     Query<'w, 's, Entity, (With<Cond>, With<quest::Active>)>,
    commands:        Commands<'w, 's>,
}

fn make_ui_event_system<Cond: Component>(
    match_event: impl Fn(&quest::UiEvent) -> bool + Send + Sync + 'static,
) -> ScheduleConfigs<ScheduleSystem> {
    (move |mut params: UiEventSystemParams<Cond>| {
        let has_match = params.ui_event_reader.read().any(&match_event);
        if has_match {
            for entity in params.quest_query.iter() {
                params.commands.entity(entity).remove::<Cond>();
            }
        }
    })
    .into_configs()
}

/// Completes when any object is within the altitude range.
#[derive(Component)]
pub struct ReachAltitude {
    pub min: Position<f32>,
    pub max: Position<f32>,
}

pub(super) fn reach_altitude_system(
    object_query: Query<&Object>,
    quest_query: Query<(Entity, &ReachAltitude), With<quest::Active>>,
    mut commands: Commands,
) {
    for (quest_entity, cond) in quest_query {
        if object_query
            .iter()
            .any(|object| (cond.min..=cond.max).contains(&object.position.altitude()))
        {
            commands.entity(quest_entity).remove::<ReachAltitude>();
        }
    }
}

/// Completes when any object is within the indicated airspeed range.
#[derive(Component)]
pub struct ReachSpeed {
    pub min: Speed<f32>,
    pub max: Speed<f32>,
}

pub(super) fn reach_speed_system(
    object_query: Query<&object::Airborne>,
    quest_query: Query<(Entity, &ReachSpeed), With<quest::Active>>,
    mut commands: Commands,
) {
    for (quest_entity, cond) in quest_query {
        if object_query.iter().any(|object| {
            let airspeed = object.airspeed.horizontal().magnitude_cmp();
            airspeed >= cond.min && airspeed <= cond.max
        }) {
            commands.entity(quest_entity).remove::<ReachSpeed>();
        }
    }
}

/// Completes when any object is within the heading range.
#[derive(Component)]
pub struct ReachHeading {
    pub min: Heading,
    pub max: Heading,
}

pub(super) fn reach_heading_system(
    object_query: Query<&object::Airborne>,
    quest_query: Query<(Entity, &ReachHeading), With<quest::Active>>,
    mut commands: Commands,
) {
    for (quest_entity, cond) in quest_query {
        if object_query
            .iter()
            .any(|object| object.airspeed.horizontal().heading().is_between(cond.min, cond.max))
        {
            commands.entity(quest_entity).remove::<ReachHeading>();
        }
    }
}

/// Completes when any object is on a segment with the given label.
#[derive(Component)]
pub struct ReachSegment {
    pub label: ground::SegmentLabel,
}

pub(super) fn reach_segment_system(
    object_query: Query<&object::OnGround>,
    quest_query: Query<(Entity, &ReachSegment), With<quest::Active>>,
    segment_query: Query<&ground::SegmentLabel>,
    mut commands: Commands,
) {
    for (quest_entity, cond) in quest_query {
        if object_query
            .iter()
            .any(|object| segment_query.log_get(object.segment) == Some(&cond.label))
        {
            commands.entity(quest_entity).remove::<ReachSegment>();
        }
    }
}

/// Completes when the client sends a direct-to-waypoint instruction.
#[derive(Component)]
pub struct InstrActionDirectWaypoint;

/// Completes when the client sends an ILS clearance instruction.
#[derive(Component)]
pub struct InstrActionClearIls;

/// Completes when the client sends a runway lineup instruction.
#[derive(Component)]
pub struct InstrActionClearLineUp;

/// Completes when the client sends a runway takeoff instruction.
#[derive(Component)]
pub struct InstrActionClearTakeoff;

/// Completes when the client sends a route selection instruction.
#[derive(Component)]
pub struct InstrActionFollowRoute;

#[derive(SystemParam)]
struct InstrActionSystemParams<'w, 's, Cond: Component> {
    instr_query: Query<'w, 's, &'static Instruction, Added<Instruction>>,
    quest_query: Query<'w, 's, Entity, (With<Cond>, With<quest::Active>)>,
    commands:    Commands<'w, 's>,
}

fn make_instr_action_system<Cond: Component>(
    match_instr: impl Fn(&Instruction) -> bool + Send + Sync + 'static,
) -> ScheduleConfigs<ScheduleSystem> {
    (move |mut params: InstrActionSystemParams<Cond>| {
        let has_match = params.instr_query.iter().any(&match_instr);
        if has_match {
            for entity in params.quest_query.iter() {
                params.commands.entity(entity).remove::<Cond>();
            }
        }
    })
    .into_configs()
}

/// Completes when the number of landings reaches the given minimum.
#[derive(Component)]
pub struct MinLanding {
    pub landings: u32,
}

/// Completes when the number of parking arrivals reaches the given minimum.
#[derive(Component)]
pub struct MinParking {
    pub parkings: u32,
}

/// Completes when the number of departures reaches the given minimum.
#[derive(Component)]
pub struct MinDeparture {
    pub departures: u32,
}

/// Completes when the score reaches the given minimum.
#[derive(Component)]
pub struct MinScore {
    pub score: Score,
}

#[derive(SystemParam)]
struct MinScoreSystemParams<'w, 's, Cond: Component> {
    query:    Query<'w, 's, (Entity, &'static Cond), With<quest::Active>>,
    scores:   Res<'w, Stats>,
    commands: Commands<'w, 's>,
}

fn make_min_stat_system<Cond: Component, T: Ord>(
    extract_cond: impl Fn(&Cond) -> T + Send + Sync + 'static,
    extract_score: impl Fn(&Stats) -> T + Send + Sync + 'static,
) -> ScheduleConfigs<ScheduleSystem> {
    (move |mut params: MinScoreSystemParams<Cond>| {
        for (entity, cond) in params.query {
            if extract_score(&params.scores) >= extract_cond(cond) {
                params.commands.entity(entity).remove::<Cond>();
            }
        }
    })
    .into_configs()
}

/// Completes immediately when the number of conflicts is less than the given maximum.
/// Never completes otherwise.
#[derive(Component)]
pub struct MaxConflicts {
    pub conflicts: u32,
}

fn max_conflicts_system(
    query: Query<(Entity, &MaxConflicts), With<quest::Active>>,
    stats: Res<Stats>,
    mut commands: Commands,
) {
    for (entity, cond) in query {
        if stats.num_conflicts <= cond.conflicts {
            commands.entity(entity).remove::<MaxConflicts>();
        }
    }
}

/// Completes when the elapsed time exceeds the given duration.
#[derive(Component)]
pub struct TimeElapsed {
    pub time: Duration,
}

fn time_elapsed_system(
    query: Query<(Entity, &TimeElapsed), With<quest::Active>>,
    time: Res<Time<time::Virtual>>,
    mut commands: Commands,
) {
    for (entity, cond) in query {
        if time.elapsed() >= cond.time {
            commands.entity(entity).remove::<TimeElapsed>();
        }
    }
}

#[cfg(test)]
mod tests;
