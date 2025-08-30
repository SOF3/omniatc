use std::cell::Cell;
use std::iter;
use std::time::Duration;

use bevy::ecs::entity::Entity;
use bevy::ecs::system::Command;
use bevy::ecs::world::{EntityRef, World};
use itertools::Itertools;
use math::{Length, Speed};
use ordered_float::OrderedFloat;
use pathfinding::prelude::dijkstra;

use super::{trigger, Node, NodeKind, Route, RunNodeResult};
use crate::level::{ground, message, object, taxi};
use crate::{EntityTryLog, WorldTryLog};

#[derive(Clone)]
pub struct TaxiNode {
    /// Taxi through `label`.
    ///
    /// When multiple contiguous `TaxiNode`s are planned,
    /// the shortest possible path satisfying all labels is used.
    pub label:      ground::SegmentLabel,
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
    let taxi_limits = object.log_get::<taxi::Limits>()?;
    let ground = object.log_get::<object::OnGround>()?;
    let segment = world.log_get::<ground::Segment>(ground.segment)?;
    let segment_label = world.log_get::<ground::SegmentLabel>(ground.segment)?;
    let (_, target_endpoint_id) = segment.by_direction(ground.direction);
    let target_endpoint = world.log_get::<ground::Endpoint>(target_endpoint_id)?;

    let route =
        object.get::<Route>().expect("run_as_current_node must be called from a route handler");
    let mut hold_short = false;
    let subseq_labels: Vec<_> = iter::once(segment_label)
        .chain(
            route
                .iter()
                .map(|node| if let Node::Taxi(taxi) = node { Some(taxi) } else { None })
                .while_some()
                .map(|node| {
                    hold_short = node.hold_short;
                    &node.label
                }),
        )
        .collect();
    assert!(
        subseq_labels.len() >= 2,
        "subseq_labels is a chain of segment_label and route nodes containing at least the \
         current executing TaxiNode"
    );

    if subseq_labels[1] == segment_label {
        // The current node requests the current segment label,
        // so this node is already completed.
        return None;
    }

    let mut next_segments: Vec<_> = target_endpoint
        .adjacency
        .iter()
        .copied()
        .filter(|&next_segment| next_segment != ground.segment)
        .filter_map(|next_segment_id| {
            let next_segment = world.log_get::<ground::Segment>(next_segment_id)?;
            let next_endpoint_id = next_segment.other_endpoint(target_endpoint_id)?;

            pathfind_through_subseq(
                world,
                next_segment_id,
                next_endpoint_id,
                &subseq_labels,
                if hold_short { PathfindMode::SegmentStart } else { PathfindMode::SegmentEnd },
                PathfindOptions { min_width: Some(taxi_limits.width), ..Default::default() },
            )
            .map(|cost| (next_segment_id, cost))
        })
        .collect();
    next_segments.sort_by_key(|(_, cost)| OrderedFloat(cost.cost.0));

    Some(taxi::TargetAction::Taxi {
        options: next_segments.into_iter().map(|(segment, _)| segment).collect(),
    })
}

/// Finds the shortest path starting from `initial_segment_id` through `initial_dest_endpoint_id`,
/// such that `subseq_labels` is an ordered subsequence of the labels of the segments in the path.
///
/// Returns `None` if no valid path can be found.
#[expect(clippy::missing_panics_doc)]
pub fn pathfind_through_subseq(
    world: &World,
    initial_segment_id: Entity,
    initial_dest_endpoint_id: Entity,
    subseq_labels: &[impl AsRef<ground::SegmentLabel>],
    mode: PathfindMode,
    options: PathfindOptions,
) -> Option<Path> {
    macro_rules! get_or_fail {
        ($world:ident, $failed:ident, $entity:expr $(, $comp:ty)?) => {
            match $world.log_get $(::<$comp>)? ($entity) {
                Some(value) => value,
                None => {
                    $failed.set(true);
                    return None;
                }
            }
        }
    }

    let failed = &Cell::new(false);

    let successors = |source_endpoint_id: Entity, label_offset: usize| {
        let source = get_or_fail!(world, failed, source_endpoint_id, ground::Endpoint);
        let successors = source.adjacency.iter().copied().filter_map(move |segment_id| {
            if source_endpoint_id == initial_dest_endpoint_id && segment_id != initial_segment_id {
                // cannot turn back
                return None;
            }

            let segment = get_or_fail!(world, failed, segment_id, ground::Segment);
            if source_endpoint_id == initial_dest_endpoint_id {
                if let Some(current_speed) = options.initial_speed {
                    if segment.max_speed < current_speed {
                        return None;
                    }
                }
            }

            if let Some(min_width) = options.min_width {
                if segment.width < min_width {
                    return None;
                }
            }

            let dest_endpoint_id = segment
                .other_endpoint(source_endpoint_id)
                .expect("adjacency segment of enndpoint must contain itself");
            let dest_endpoint = get_or_fail!(world, failed, dest_endpoint_id, ground::Endpoint);
            let distance = source.position.distance_exact(dest_endpoint.position);
            let distance = OrderedFloat(distance.0);

            let label = get_or_fail!(world, failed, segment_id, ground::SegmentLabel);
            let next_label_offset =
                if subseq_labels.get(label_offset).map(AsRef::as_ref) == Some(label) {
                    label_offset + 1
                } else {
                    label_offset
                };

            Some(((dest_endpoint_id, next_label_offset), distance))
        });
        Some(successors)
    };

    let (nodes, cost) = dijkstra(
        &(initial_dest_endpoint_id, 0),
        move |&(endpoint_id, required_labels)| {
            successors(endpoint_id, required_labels).into_iter().flatten()
        },
        |&(endpoint_id, label_offset)| {
            if failed.get() {
                return false;
            }

            match mode {
                PathfindMode::SegmentStart => {
                    if label_offset == subseq_labels.len() - 1 {
                        let endpoint = world
                            .get::<ground::Endpoint>(endpoint_id)
                            .expect("successors only generates checked endpoints");
                        let last_label =
                            subseq_labels.last().expect("subseq_labels is non-empty").as_ref();
                        endpoint.adjacency.iter().any(|&segment_id| {
                            world.log_get::<ground::SegmentLabel>(segment_id) == Some(last_label)
                        })
                    } else {
                        false
                    }
                }
                PathfindMode::SegmentEnd => label_offset == subseq_labels.len(),
                PathfindMode::Endpoint(dest) => {
                    label_offset == subseq_labels.len() && dest == endpoint_id
                }
            }
        },
    )?;

    Some(Path {
        endpoints: nodes.into_iter().map(|(endpoint_id, _)| endpoint_id).collect(),
        cost:      Length::new(cost.0),
    })
}

/// Destination mode for [`pathfind_through_subseq`].
pub enum PathfindMode {
    /// The path ends at the start of a segment with the last `subseq_labels` label.
    ///
    /// The returned path always ends with the last endpoint adjacent to `subseq_labels.last()`,
    /// but the segment between the last two endpoints is not equal to `subseq_labels.last()`.
    /// The length of the segment matching `subseq_labels.last()`
    /// is not considered for selecting the shortest path,
    /// and is not included in the returned cost.
    SegmentStart,
    /// The path ends at the end of a segment with the last `subseq_labels` label.
    /// However, the path does not traverse to the end of other subsequent segments with the same
    /// label.
    ///
    /// The returned cost includes the length of this last segment.
    ///
    /// The returned path always ends with the last two endpoints connected by `subseq_labels.last()`.
    /// The length of such segment is part of the returned cost,
    /// which is considered for selecting the shortest path.
    ///
    /// This may result in a completely different path than `SegmentStart`.
    /// For example, consider the following graph:
    /// ```text
    /// A --- p(3) --- B
    /// |              |
    /// |            r(10)
    /// |              |
    /// p(4)           C
    /// |              |
    /// |            r(1)
    /// |              |
    /// D --- q(4) --- E
    /// ```
    ///
    /// For initial endpoint `A` and subsequence `[p, r]`,
    /// `SegmentStart` would consider the following paths:
    /// - `A-B` with cost `3` (`p(3)`)
    /// - `A-D-E` with cost 8 (`p(4) + q(4)`)
    ///
    /// On the other hand, `SegmentEnd` would consider the following paths:
    /// - `A-D-E-C` with cost `9` (`p(4) + q(4) + r(1)`)
    /// - `A-B-C` with cost `13` (`p(3) + r(10)`)
    ///
    /// Note that `SegmentEnd` does not return `A-B-C-E`/`A-D-E-C-B`
    /// because the path stops at the first segment with the last label.
    SegmentEnd,
    /// The path must end at this endpoint.
    Endpoint(Entity),
}

/// Optional limits for [`pathfind_through_subseq`].
#[derive(Default)]
pub struct PathfindOptions {
    /// - If `Some`, the *first* segment after `initial_dest_endpoint_id`
    ///   must have a `max_speed` greater than or equal to `current_speed`.
    ///   This limit is not checked for subsequent segments.
    pub initial_speed: Option<Speed<f32>>,
    /// - If `Some`, *all* segments in the path must have a width greater than or equal to `width`.
    pub min_width:     Option<Length<f32>>,
}

pub struct Path {
    pub endpoints: Vec<Entity>,
    pub cost:      Length<f32>,
}
