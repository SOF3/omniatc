use std::time::Duration;

use bevy::ecs::entity::{Entity, EntityHashSet};
use bevy::ecs::query::{Has, With};
use bevy::ecs::schedule::{IntoScheduleConfigs, ScheduleConfigs};
use bevy::ecs::system::{Commands, ParamSet, Query, Res, ResMut, ScheduleSystem, SystemParam};
use bevy::time::{self, Time};
use bevy_mod_config::ReadConfig;
use math::Length;
use store::Score;

use super::{SystemSets, message, object, score};
use crate::level::conflict::{ActiveObject, PairState, Record};
use crate::level::index::OctreeIndex;

// Wraps `system_impl` to suppress private interface compile error.
pub(super) fn system() -> ScheduleConfigs<ScheduleSystem> {
    system_impl.in_set(SystemSets::Statistics).in_set(score::Writer)
}

fn system_impl(
    mut params: ParamSet<(
        CollectPairsParams,
        UpdatePairsParams,
        CleanupInactivePairsParams,
        UpdateActiveMarkersParams,
    )>,
) {
    let pairs = collect_pairs(params.p0());
    let active_pairs = update_pairs(&pairs, params.p1());
    let active_object_set = cleanup_inactive_pairs(&active_pairs, params.p2());
    update_active_object_markers(&active_object_set, params.p3());
}

#[derive(SystemParam)]
struct CollectPairsParams<'w, 's> {
    octree:       Res<'w, OctreeIndex<object::Object>>,
    /// Filters to airborne objects only; ground objects are never conflict candidates.
    object_query: Query<'w, 's, (Entity, &'static object::Object), With<object::Airborne>>,
    conf:         ReadConfig<'w, 's, super::Conf>,
}

/// Returns all pairs of airborne objects currently violating both horizontal and vertical
/// separation minima, together with their normalised squared distance `norm_dist_sq`.
///
/// `entity_a < entity_b` for every returned tuple.
fn collect_pairs(params: CollectPairsParams) -> Vec<CollectedPair> {
    let conf = params.conf.read();
    let horiz_thres_sq = conf.horiz_sep.magnitude_squared();
    let vert_thres_sq = conf.vert_sep.magnitude_squared();

    let aabb_half_size = Length::from([conf.horiz_sep; 2]).with_vertical(conf.vert_sep);

    let mut pairs = Vec::new();
    for (entity_a, object_a) in &params.object_query {
        for entity_b in params.octree.entities_in_bounds([
            object_a.position - aabb_half_size,
            object_a.position + aabb_half_size,
        ]) {
            // Process each unordered pair exactly once.
            if entity_b <= entity_a {
                continue;
            }
            let Ok((_, object_b)) = params.object_query.get(entity_b) else { continue };

            let distance = object_a.position - object_b.position;
            let horiz_dist_sq = distance.horizontal().magnitude_squared();
            let vert_dist_sq = distance.vertical().squared();

            if horiz_dist_sq < horiz_thres_sq && vert_dist_sq < vert_thres_sq {
                let norm_dist_sq = horiz_dist_sq / horiz_thres_sq + vert_dist_sq / vert_thres_sq;
                pairs.push(CollectedPair { entities: [entity_a, entity_b], norm_dist_sq });
            }
        }
    }

    pairs
}

struct CollectedPair {
    entities:     [Entity; 2],
    /// `(h/h_sep)^2 + (v/v_sep)^2`, in `[0, 2)` when both separations are violated.
    norm_dist_sq: f32,
}

#[derive(SystemParam)]
struct UpdatePairsParams<'w, 's> {
    record_query:  Query<'w, 's, &'static mut Record>,
    pair_query:    Query<'w, 's, (Has<message::Message>, &'static mut PairState)>,
    display_query: Query<'w, 's, &'static object::Display>,
    time:          Res<'w, Time<time::Virtual>>,
    conf:          ReadConfig<'w, 's, super::Conf>,
    score:         ResMut<'w, score::Stats>,
    commands:      Commands<'w, 's>,
}

/// Creates or updates pair entities for every active violation.
///
/// Returns the set of pair entity IDs that are in violation this tick.
fn update_pairs(pairs: &[CollectedPair], mut params: UpdatePairsParams) -> EntityHashSet {
    let conf = params.conf.read();
    let mut active_pair_entities = EntityHashSet::default();

    for pair in pairs {
        let [entity_a, entity_b] = pair.entities;

        let pair_entity =
            params.record_query.get(entity_a).ok().and_then(|rec| rec.pair_for(entity_b));

        let pair_entity = if let Some(pair_entity) = pair_entity {
            let Ok((has_message, mut pair_state)) = params.pair_query.get_mut(pair_entity) else {
                continue;
            };

            apply_penalty(
                &mut pair_state,
                pair.norm_dist_sq,
                params.time.delta_secs(),
                conf.score_multiplier,
                &mut params.score,
            );

            pair_state.is_active = true;
            if !has_message {
                params.commands.entity(pair_entity).insert(build_conflict_message(
                    entity_a,
                    entity_b,
                    &params.display_query,
                    params.time.elapsed(),
                ));
            }

            pair_entity
        } else {
            let Ok([mut record_a, mut record_b]) =
                params.record_query.get_many_mut([entity_a, entity_b])
            else {
                continue;
            };

            // First violation for this pair.
            params.score.num_conflicts += 1;

            // PairState.cumul is initialised to zero because we don't know how much of
            // the current frame the conflict has already been active.
            let new_state = PairState {
                entity_a,
                entity_b,
                cumul: Duration::ZERO,
                is_active: true,
                uncommitted_score: 0.0,
            };

            let pair_entity = params
                .commands
                .spawn((
                    new_state,
                    build_conflict_message(
                        entity_a,
                        entity_b,
                        &params.display_query,
                        params.time.elapsed(),
                    ),
                ))
                .id();

            record_a.peers.insert(entity_b, pair_entity);
            record_b.peers.insert(entity_a, pair_entity);

            pair_entity
        };
        active_pair_entities.insert(pair_entity);

        params.score.total_conflict_time += params.time.delta();
    }

    active_pair_entities
}

#[derive(SystemParam)]
struct CleanupInactivePairsParams<'w, 's> {
    pair_query: Query<'w, 's, (Entity, Has<message::Message>, &'static mut PairState)>,
    commands:   Commands<'w, 's>,
}

/// Marks pairs absent from `active_pair_entities` as inactive and removes their conflict
/// message.
///
/// Returns the set of object entities that still have at least one active pair.
fn cleanup_inactive_pairs(
    active_pair_entities: &EntityHashSet,
    mut params: CleanupInactivePairsParams,
) -> EntityHashSet {
    let mut active_objects = EntityHashSet::default();

    for (pair_entity, has_message, mut pair_state) in &mut params.pair_query {
        if active_pair_entities.contains(&pair_entity) {
            active_objects.insert(pair_state.entity_a);
            active_objects.insert(pair_state.entity_b);
        } else {
            pair_state.is_active = false;
            if has_message {
                params.commands.entity(pair_entity).remove::<message::Message>();
            }
        }
    }

    active_objects
}

#[derive(SystemParam)]
struct UpdateActiveMarkersParams<'w, 's> {
    object_query: Query<'w, 's, (Entity, Has<ActiveObject>), With<object::Object>>,
    commands:     Commands<'w, 's>,
}

/// Inserts or removes [`ActiveObject`] on every aircraft depending on `active_objects`.
fn update_active_object_markers(
    active_objects: &EntityHashSet,
    mut params: UpdateActiveMarkersParams,
) {
    for (entity, has_active) in params.object_query.iter() {
        match (active_objects.contains(&entity), has_active) {
            (true, false) => {
                params.commands.entity(entity).insert(ActiveObject);
            }
            (false, true) => {
                params.commands.entity(entity).remove::<ActiveObject>();
            }
            _ => {}
        }
    }
}

/// Computes the incremental conflict score penalty for one frame.
///
/// - `norm_dist_sq` is `(h/h_sep)^2 + (v/v_sep)^2`, in `0..2` when both separations are violated.
/// - `cumul_secs` is the total cumulative conflict time already updated for this frame.
/// - The formula yields a cumulative penalty that grows as `t^(1/3)` over time and
///   scales with proximity within the separation bubble.
pub(super) fn compute_penalty(
    norm_dist_sq: f32,
    cumul_secs: f32,
    dt_secs: f32,
    multiplier: f32,
) -> f32 {
    if cumul_secs <= 0.0 || dt_secs <= 0.0 {
        return 0.0;
    }

    (2.0 - norm_dist_sq) * cumul_secs.powf(-2.0 / 3.0) * dt_secs * multiplier
}

/// Advances `pair_state.cumul` by `dt_secs`, computes the penalty, and commits the integer
/// floor to `stats.total` (remainder stays in `pair_state.uncommitted_score` for future frames).
fn apply_penalty(
    pair_state: &mut PairState,
    norm_dist_sq: f32,
    dt_secs: f32,
    multiplier: f32,
    stats: &mut score::Stats,
) {
    pair_state.cumul += Duration::from_secs_f32(dt_secs);
    let penalty =
        compute_penalty(norm_dist_sq, pair_state.cumul.as_secs_f32(), dt_secs, multiplier);
    pair_state.uncommitted_score += penalty;
    stats.total -= Score(pair_state.take_uncommitted_score());
}

fn build_conflict_message(
    entity_a: Entity,
    entity_b: Entity,
    display_query: &Query<&object::Display>,
    created: Duration,
) -> message::Message {
    let name_a = display_query.get(entity_a).map_or("?", |d| d.name.as_str());
    let name_b = display_query.get(entity_b).map_or("?", |d| d.name.as_str());
    message::Message {
        source: entity_a,
        created,
        content: format!("Separation conflict: {name_a}, {name_b}"),
        class: message::Class::Urgent,
    }
}
