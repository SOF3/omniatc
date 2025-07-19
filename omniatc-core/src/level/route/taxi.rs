use std::collections::BinaryHeap;
use std::num::NonZeroUsize;
use std::time::Duration;
use std::{iter, slice};

use bevy::ecs::entity::{Entity, EntityHashMap, EntityHashSet};
use bevy::ecs::system::Command;
use bevy::ecs::world::{EntityRef, World};
use bevy::math::Vec2;
use itertools::Itertools;
use math::{Length, Position};
use ordered_float::OrderedFloat;
use smallvec::SmallVec;

use super::{trigger, Node, NodeKind, Route, RunNodeResult};
use crate::level::{ground, message, object, taxi};
use crate::try_log;

#[derive(Clone)]
pub struct TaxiNode {
    /// Taxi to the first segment with one of these labels.
    /// If there is no subsequent `TaxiNode` in the route,
    /// the object would hold short at the first intersection after entry.
    ///
    /// When there are multiple labels,
    /// the highest priority path is the shortest path through the first candidate segment
    /// of each subsequent contiguous `TaxiNode` in the route that reaches the last one.
    /// The second segment is only used when no path through the first segment is reachable.
    pub labels:     SmallVec<[ground::SegmentLabel; 1]>,
    /// If `true`, the object will no enter the intersection reaching the segments in this node.
    /// If `false`, the object will enter the intersection and taxi to the first segment matching
    /// this node.
    ///
    /// No effect if the node immediately following this one is also a `TaxiNode`.
    pub hold_short: bool,
}

impl NodeKind for TaxiNode {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult {
        let mut object = world.entity_mut(entity);
        if let Some(mut target) = object.get_mut::<taxi::Target>() {
            if let Some(taxi::TargetResolution::Inoperable) = target.resolution {
                message::SendExpiring {
                    content:  "Unable to find path to the taxi target, skipping node.".into(),
                    class:    message::Class::NeedAck,
                    source:   entity,
                    duration: Duration::from_secs(5),
                }
                .apply(world);
                return RunNodeResult::NodeDone;
            }

            // Secondary completion basically means we need to recompute
            // since an unideal segment is selected..
            target.resolution = None;

            let new_action = recompute_action(object.world(), object.as_readonly());
            let mut target = object
                .get_mut::<taxi::Target>()
                .expect("Target should still be present after an immutable world lend");
            if let Some(action) = new_action {
                target.action = action;
                object.insert(trigger::TaxiTargetResolution);
                RunNodeResult::PendingTrigger
            } else {
                target.action = taxi::TargetAction::HoldShort;
                RunNodeResult::NodeDone
            }
        } else {
            if let Some(action) = recompute_action(object.world(), object.as_readonly()) {
                object.insert((
                    taxi::Target { action, resolution: None },
                    trigger::TimeDelay(Duration::from_secs(1)),
                ));
                object.insert(trigger::TaxiTargetResolution);
                RunNodeResult::PendingTrigger
            } else {
                RunNodeResult::NodeDone
            }
        }
    }
}

fn recompute_action(world: &World, object: EntityRef) -> Option<taxi::TargetAction> {
    let ground = try_log!(
        object.get::<object::OnGround>(),
        expect "taxi node must be used on ground objects" or return None
    );
    let segment = try_log!(
        world.get::<ground::Segment>(ground.segment),
        expect "object::OnGround must refer to a valid segment" or return None
    );
    let segment_label = try_log!(
        world.get::<ground::SegmentLabel>(ground.segment),
        expect "object::OnGround must refer to a valid labeled segment" or return None
    );
    let (_, target_endpoint_id) = segment.by_direction(ground.direction);
    let target_endpoint = try_log!(
        world.get::<ground::Endpoint>(target_endpoint_id),
        expect "segment must refer to a valid endpoint" or return None
    );

    let route =
        object.get::<Route>().expect("run_as_current_node must be called from a route handler");
    let mut hold_short = false;
    let next_label_sets: Vec<_> = iter::once(slice::from_ref(segment_label))
        .chain(
            route
                .iter()
                .map(|node| if let Node::Taxi(taxi) = node { Some(taxi) } else { None })
                .while_some()
                .map(|node| {
                    hold_short = node.hold_short;
                    &node.labels[..]
                }),
        )
        .collect();
    assert!(
        next_label_sets.len() >= 2,
        "next_label_sets is a chain of [segment_label] and route nodes containing at least the \
         current executing TaxiNode"
    );

    if next_label_sets[1].contains(segment_label) {
        // The current node requests the current segment label,
        // so this node is already completed.
        return None;
    }

    let mut next_segments: Vec<_> = target_endpoint
        .adjacency
        .iter()
        .copied()
        .filter(|&next_segment| next_segment != ground.segment)
        .filter_map(|next_segment| {
            pathfind_min_distance_segments(
                world,
                target_endpoint_id,
                next_segment,
                &next_label_sets,
                hold_short,
            )
            .map(|cost| (next_segment, cost))
        })
        .collect();
    next_segments.sort_by_key(|(_, cost)| cost.clone());

    Some(taxi::TargetAction::Taxi {
        options: next_segments.into_iter().map(|(segment, _)| segment).collect(),
    })
}

/// Find the shortest path starting from `initial_source_endpoint_id` through `initial_segment_id`,
/// passing through one or more segments matching each label in `next_label_sets` in order.
///
/// `next_label_sets[0]` must be a singleton slice containing the label of the previous segment before
/// `initial_segment_id`.
/// `next_label_sets.len()` must be at least 2.
///
/// Returns `None` if no valid path can be found through `initial_segment_id`.
#[expect(clippy::too_many_lines)] // no point in splitting up a pathfinding algorithm
fn pathfind_min_distance_segments(
    world: &World,
    initial_source_endpoint_id: Entity,
    initial_segment_id: Entity,
    next_label_sets: &[&[ground::SegmentLabel]],
    include_last_segment_cost: bool,
) -> Option<PathCost> {
    struct VisitedEndpoint<'a> {
        /// Position of the visited endpoint.
        position:                    Position<Vec2>,
        /// The cost of the path to this endpoint.
        cost:                        PathCost,
        /// The segment that entered this endpoint.
        entry_segment:               Entity,
        /// The index in `next_label_sets` that the segment entering this endpoint matched.
        entry_next_label_sets_index: usize,
        /// The label of the entry segment.
        entry_segment_label:         &'a ground::SegmentLabel,
    }

    let mut heap = BinaryHeap::new();
    let mut visited_endpoints = EntityHashMap::new();

    {
        let initial_segment = try_log!(
            world.get::<ground::Segment>(initial_segment_id),
            expect "initial segment must be a valid segment" or return None
        );

        let initial_segment_label = try_log!(
            world.get::<ground::SegmentLabel>(initial_segment_id),
            expect "initial segment must be a valid labeled segment" or return None
        );

        let initial_source_endpoint = try_log!(
            world.get::<ground::Endpoint>(initial_source_endpoint_id),
            expect "initial source endpoint must be a valid endpoint" or return None
        );
        let initial_dest_endpoint_id = try_log!(
            initial_segment.other_endpoint(initial_source_endpoint_id),
            expect "initial_segment_id should contain initial_source_endpoint_id as one endpoint" or return None
        );
        let initial_dest_endpoint = try_log!(
            world.get::<ground::Endpoint>(initial_dest_endpoint_id),
            expect "initial destination endpoint must be a valid endpoint" or return None
        );
        let distance =
            initial_source_endpoint.position.distance_exact(initial_dest_endpoint.position);

        // if label is not one of the first two, this segment is invalid and no path can be found.
        let (next_label_sets_index, alt) = if next_label_sets[0][0] == *initial_segment_label {
            (0, 0)
        } else {
            if let Some(entry_label_alt) =
                next_label_sets[1].iter().position(|label| label == initial_segment_label)
            {
                (1, entry_label_alt)
            } else {
                return None; // no valid path through this segment
            }
        };
        let mut alts = Alts::default();
        alts.push(next_label_sets_index, alt);
        let cost = PathCost { alts, distance };

        heap.push((cost.clone(), initial_dest_endpoint_id));
        visited_endpoints.insert(
            initial_dest_endpoint_id,
            VisitedEndpoint {
                position: initial_dest_endpoint.position,
                cost,
                entry_segment: initial_segment_id,
                entry_next_label_sets_index: next_label_sets_index,
                entry_segment_label: initial_segment_label,
            },
        );
    }

    let mut exclude_heap = EntityHashSet::new();

    while let Some((cost, source_endpoint_id)) = heap.pop() {
        if exclude_heap.contains(&source_endpoint_id) {
            // This endpoint was already visited with a shorter path.
            continue;
        }

        let source_info = visited_endpoints
            .get(&source_endpoint_id)
            .expect("visited_endpoints must contain endpoints pushed to heap");
        let Some(&next_label_set) =
            next_label_sets.get(source_info.entry_next_label_sets_index + 1)
        else {
            let mut result = cost;
            if !include_last_segment_cost {
                let entry_segment = world
                    .get::<ground::Segment>(source_info.entry_segment)
                    .expect("visited segment must be a checked segment");
                let prev_endpoint_id = entry_segment
                    .other_endpoint(source_endpoint_id)
                    .expect("visited endpoint has checked segment consistency");
                let prev_endpoint = world
                    .get::<ground::Endpoint>(prev_endpoint_id)
                    .expect("visited segment must be a checked endpoint");
                result.distance -= prev_endpoint.position.distance_exact(source_info.position);
            }

            // Last label reached.
            return Some(result);
        };

        let source_endpoint = world
            .get::<ground::Endpoint>(source_endpoint_id)
            .expect("visited endpoint must be a checked endpoint");
        for &adj_segment_id in &source_endpoint.adjacency {
            // Reborrow source_info every time since visited_endpoints is inserted
            // by the end of each iteration.
            let source_info = visited_endpoints.get(&source_endpoint_id).expect("checked above");

            if adj_segment_id == source_info.entry_segment {
                continue;
            }

            let adj_segment_label = try_log!(
                world.get::<ground::SegmentLabel>(adj_segment_id),
                expect "segment adjacency must refer to a valid labeled segment" or continue
            );
            let (next_label_sets_index, alt_index) =
                if adj_segment_label == source_info.entry_segment_label {
                    (source_info.entry_next_label_sets_index, None)
                } else if let Some(alt_index) =
                    next_label_set.iter().position(|l| l == adj_segment_label)
                {
                    (source_info.entry_next_label_sets_index + 1, Some(alt_index))
                } else {
                    continue; // cannot use this segment
                };

            let adj_segment = try_log!(
                world.get::<ground::Segment>(adj_segment_id),
                expect "segment adjacency must refer to a valid segment" or continue
            );
            let adj_dest_endpoint_id = try_log!(
                adj_segment.other_endpoint(source_endpoint_id),
                expect "segment adjacency must contain source endpoint as one endpoint" or continue
            );
            let adj_dest_endpoint = try_log!(
                world.get::<ground::Endpoint>(adj_dest_endpoint_id),
                expect "segment adjacency must refer to a valid endpoint" or continue
            );
            let distance = source_endpoint.position.distance_exact(adj_dest_endpoint.position);

            let mut alts = source_info.cost.alts.clone();
            if let Some(alt_index) = alt_index {
                alts.push(next_label_sets_index, alt_index);
            }

            let cost = PathCost { alts, distance: source_info.cost.distance + distance };

            if let Some(prev_visit) = visited_endpoints.get(&adj_segment_id) {
                if prev_visit.cost <= cost {
                    continue;
                }

                // We found a shorter path to this endpoint.
                // Since source_info.cost.distance + distance < prev_visit.cost.distance,
                // prev_visit must still be in the heap,
                // so we just insert this endpoint into the exclusion set.
                exclude_heap.insert(adj_dest_endpoint_id);
            }

            let visit = VisitedEndpoint {
                position:                    adj_dest_endpoint.position,
                cost:                        cost.clone(),
                entry_segment:               adj_segment_id,
                entry_next_label_sets_index: next_label_sets_index,
                entry_segment_label:         adj_segment_label,
            };
            visited_endpoints.insert(adj_dest_endpoint_id, visit);
            heap.push((cost, adj_dest_endpoint_id));
        }
    }

    // All endpoints have been visited, but no path consuming the last label was found.
    None
}

#[derive(Clone)]
struct PathCost {
    alts:     Alts,
    distance: Length<f32>,
}

impl PartialEq for PathCost {
    fn eq(&self, other: &Self) -> bool {
        self.alts == other.alts && OrderedFloat(self.distance.0) == OrderedFloat(other.distance.0)
    }
}
impl Eq for PathCost {}
impl Ord for PathCost {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.alts
            .cmp(&other.alts)
            .then_with(|| OrderedFloat(self.distance.0).cmp(&OrderedFloat(other.distance.0)))
    }
}
impl PartialOrd for PathCost {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

#[derive(Default, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Alts(SmallVec<[(usize, NonZeroUsize); 1]>);

impl Alts {
    fn push(&mut self, next_label_sets_index: usize, label_alt: usize) {
        if let Some(label_alt) = NonZeroUsize::new(label_alt) {
            let item = (next_label_sets_index, label_alt);
            if self.0.last() != Some(&item) {
                self.0.push(item);
            }
        }
    }
}
