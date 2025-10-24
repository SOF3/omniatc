use std::cell::Cell;
use std::time::Duration;
use std::{fmt, iter, mem};

use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::Command;
use bevy::ecs::world::{EntityRef, World};
use itertools::Itertools;
use math::{Length, Speed};
use ordered_float::OrderedFloat;
use pathfinding::prelude::dijkstra;

use super::{Node, NodeKind, Route, RunNodeResult, trigger};
use crate::level::{ground, message, object, taxi};
use crate::util::TakeLast;
use crate::{EntityTryLog, WorldTryLog};

#[cfg(test)]
mod tests;

/// Finds the shortest path to a segment with the specified label.
///
/// # Completion condition
/// Completes when the object enters a segment with the specified label.
///
/// # Interaction with subsequent nodes
/// If immediately followed by other [`TaxiNode`]s,
/// the shortest path traversing all specified labels in order is used.
///
/// # Prerequisites
/// The object must be on ground.
#[derive(Clone)]
pub struct TaxiNode {
    /// Taxi via `label`.
    ///
    /// When multiple contiguous `TaxiNode`s are planned,
    /// the shortest possible path satisfying all labels is used.
    pub label:     ground::SegmentLabel,
    /// Only consider the segment as matched when approaching from this direction.
    ///
    /// This does not prevent the segment from being used on the opposite direction,
    /// but ensures that the shortest path that eventually uses the segment in this direction
    /// is selected.
    pub direction: Option<ground::SegmentDirection>,
    /// Where to stop at if this is the last `TaxiNode` in succession.
    ///
    /// No effect if the node immediately following this one is also a `TaxiNode`.
    pub stop:      TaxiStopMode,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TaxiStopMode {
    /// Hold before entering the intersection.
    HoldShort,
    /// Hold immediately after entering the intersection.
    LineUp,
    /// Hold right before leaving the intersection.
    Exhaust,
}

impl TaxiStopMode {
    pub fn message(&self, segment_name: impl fmt::Display) -> String {
        match self {
            Self::HoldShort => format!("Hold short of {segment_name}"),
            Self::LineUp => format!("Line up on {segment_name}"),
            Self::Exhaust => format!("Taxi to {segment_name}"),
        }
    }
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

            target.resolution = None;

            let next_segments = recompute_action(object.world(), object.as_readonly());
            let mut target = object
                .get_mut::<taxi::Target>()
                .expect("Target should still be present after an immutable world lend");
            if let Some(next_segments) = next_segments {
                target.action = taxi::TargetAction::Taxi {
                    options: next_segments.paths.iter().map(|path| path.first_segment).collect(),
                };
                object.insert((trigger::TaxiTargetResolution, next_segments));
                RunNodeResult::PendingTrigger
            } else {
                #[expect(clippy::match_same_arms, reason = "different explanations")]
                {
                    target.action = match self.stop {
                        // The end of the current segment intersects with the final HoldShort node,
                        // so just stop at the end of the current segment.
                        TaxiStopMode::HoldShort => {
                            taxi::TargetAction::Hold { kind: taxi::HoldKind::SegmentEnd }
                        }
                        // The current segment is the segment requested by the final LineUp node,
                        // so we can align and stop immediately.
                        TaxiStopMode::LineUp => {
                            taxi::TargetAction::Hold { kind: taxi::HoldKind::WhenAligned }
                        }
                        // The current segment is the segment requested by the final Exhaust node,
                        // so taxi to the end of this segment.
                        TaxiStopMode::Exhaust => {
                            taxi::TargetAction::Hold { kind: taxi::HoldKind::SegmentEnd }
                        }
                    };
                }
                RunNodeResult::NodeDone
            }
        } else {
            if let Some(next_segments) = recompute_action(object.world(), object.as_readonly()) {
                object.insert((
                    taxi::Target {
                        action:     taxi::TargetAction::Taxi {
                            options: next_segments
                                .paths
                                .iter()
                                .map(|path| path.first_segment)
                                .collect(),
                        },
                        resolution: None,
                    },
                    trigger::TimeDelay(Duration::from_secs(1)),
                ));
                object.insert((trigger::TaxiTargetResolution, next_segments));
                RunNodeResult::PendingTrigger
            } else {
                RunNodeResult::NodeDone
            }
        }
    }
}

#[derive(Component)]
pub struct PossiblePaths {
    pub paths:     Vec<PossiblePath>,
    pub stop_mode: TaxiStopMode,
}

pub struct PossiblePath {
    /// The endpoint that the path starts from.
    /// The object is currently heading towards this endpoint.
    pub start_endpoint: Entity,
    /// The first segment in the path.
    /// This is the segment the object will enter after reaching `start_endpoint`.
    pub first_segment:  Entity,
    /// The remaining endpoints in the path, in order.
    pub next_endpoints: Vec<Entity>,
    /// The total length of the path.
    pub length:         Length<f32>,
}

impl PossiblePath {
    pub fn endpoints(&self) -> impl Iterator<Item = Entity> + '_ {
        iter::once(self.start_endpoint).chain(self.next_endpoints.iter().copied())
    }
}

/// Performs pathfinding to determine the priority of next segments to go to.
///
/// Returns `None` if the current node is completed and
/// the object should stop on the current segment.
/// Otherwise, returns a list of possible next segments to switch to
/// when the current segment is exhausted.
fn recompute_action(world: &World, object: EntityRef) -> Option<PossiblePaths> {
    let taxi_limits = object.log_get::<taxi::Limits>()?;
    let ground = object.log_get::<object::OnGround>()?;
    let current_segment = world.log_get::<ground::Segment>(ground.segment)?;
    let current_segment_label = world.log_get::<ground::SegmentLabel>(ground.segment)?;
    let (_, target_endpoint_id) = current_segment.by_direction(ground.direction);
    let target_endpoint = world.log_get::<ground::Endpoint>(target_endpoint_id)?;

    let route =
        object.get::<Route>().expect("run_as_current_node must be called from a route handler");
    let (subseq, TakeLast(stop_mode)): (Vec<_>, _) = route
        .iter()
        .map(|node| if let Node::Taxi(taxi) = node { Some(taxi) } else { None })
        .while_some()
        .map(|node| (SubseqItem { label: &node.label, direction: node.direction }, node.stop))
        .unzip();
    assert!(
        !subseq.is_empty(),
        "subseq is must contain at least the label from the current executing TaxiNode"
    );
    let stop_mode = stop_mode.expect("iterator was checked nonempty");

    if subseq[0].matches(current_segment_label, ground.direction) {
        // The current node requests the current segment label,
        // so this node is already completed.
        return None;
    }

    let mut next_segments = Vec::new();
    for &next_segment_id in &target_endpoint.adjacency {
        if next_segment_id == ground.segment {
            continue;
        }

        let next_segment = world.log_get::<ground::Segment>(next_segment_id)?;
        let next_segment_label = world.log_get::<ground::SegmentLabel>(next_segment_id)?;
        let Some(next_segment_direction) = next_segment.direction_from(target_endpoint_id) else {
            bevy::log::error!(
                "Segment {next_segment_id:?} is in the adjacency of endpoint \
                 {target_endpoint_id:?} but does not include it as an endpoint"
            );
            return None;
        };
        let next_endpoint_id = next_segment.other_endpoint(target_endpoint_id)?;
        let next_endpoint = world.log_get::<ground::Endpoint>(next_endpoint_id)?;

        if let [first_si] = subseq[..]
            && first_si.matches(next_segment_label, next_segment_direction)
        {
            // This segment already satisfies the subsequence requirement.
            if stop_mode == TaxiStopMode::HoldShort {
                // Just hold short at the current endpoint.
                return None;
            }
            next_segments.push((
                next_segment_id,
                Path {
                    endpoints: vec![next_endpoint_id],
                    cost:      target_endpoint.position.distance_exact(next_endpoint.position),
                },
            ));
        } else {
            let mut subseq = &subseq[..];
            if let Some(si) = subseq.first()
                && si.matches(next_segment_label, next_segment_direction)
            {
                subseq = &subseq[1..];
            }

            if let Some(path) = pathfind_through_subseq(
                world,
                next_segment_id,
                next_endpoint_id,
                subseq,
                if stop_mode == TaxiStopMode::Exhaust {
                    PathfindMode::SegmentEnd
                } else {
                    PathfindMode::SegmentStart
                },
                PathfindOptions { min_width: Some(taxi_limits.width), ..Default::default() },
            ) {
                next_segments.push((next_segment_id, path));
            }
        }
    }

    next_segments.sort_by_key(|(_, path)| OrderedFloat(path.cost.0));

    Some(PossiblePaths {
        paths: next_segments
            .into_iter()
            .map(|(first_segment, path)| PossiblePath {
                start_endpoint: target_endpoint_id,
                first_segment,
                next_endpoints: path.endpoints,
                length: path.cost,
            })
            .collect(),
        stop_mode,
    })
}

/// An item in the required subsequence
#[derive(Debug, Clone, Copy)]
pub struct SubseqItem<'a> {
    pub label:     &'a ground::SegmentLabel,
    pub direction: Option<ground::SegmentDirection>,
}

impl SubseqItem<'_> {
    #[must_use]
    pub fn matches(
        &self,
        label: &ground::SegmentLabel,
        direction: ground::SegmentDirection,
    ) -> bool {
        self.label == label && self.direction.is_none_or(|target| target == direction)
    }
}

/// World getters required for pathfinding.
pub trait PathfindContext<'a>: Copy {
    /// Returns the specified endpoint, or log the failure and return `None`.
    fn endpoint(self, id: Entity) -> Option<&'a ground::Endpoint>;
    /// Returns the specified segment, or log the failure and return `None`.
    fn segment(self, id: Entity) -> Option<(&'a ground::Segment, &'a ground::SegmentLabel)>;
}

impl<'a> PathfindContext<'a> for &'a World {
    fn endpoint(self, id: Entity) -> Option<&'a ground::Endpoint> { self.log_get(id) }
    fn segment(self, id: Entity) -> Option<(&'a ground::Segment, &'a ground::SegmentLabel)> {
        self.log_get(id).zip(self.log_get(id))
    }
}

#[derive(Clone, Copy)]
pub struct ClosurePathfindContext<EndpointFn, SegmentFn> {
    pub endpoint_fn: EndpointFn,
    pub segment_fn:  SegmentFn,
}

impl<'a, EndpointFn, SegmentFn> PathfindContext<'a>
    for ClosurePathfindContext<EndpointFn, SegmentFn>
where
    EndpointFn: Fn(Entity) -> Option<&'a ground::Endpoint> + Copy,
    SegmentFn: Fn(Entity) -> Option<(&'a ground::Segment, &'a ground::SegmentLabel)> + Copy,
{
    fn endpoint(self, id: Entity) -> Option<&'a ground::Endpoint> { (self.endpoint_fn)(id) }
    fn segment(self, id: Entity) -> Option<(&'a ground::Segment, &'a ground::SegmentLabel)> {
        (self.segment_fn)(id)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct DijkstraVertex {
    endpoint_id:  Entity,
    label_offset: usize,
    from_segment: Entity,
}

struct PathfindState<'a, 'subseq, Ctx> {
    ctx:                      Ctx,
    failed:                   Cell<bool>,
    initial_dest_endpoint_id: Entity,
    subseq_labels:            &'a [SubseqItem<'subseq>],
    options:                  PathfindOptions,
}

impl<'ctx, Ctx: PathfindContext<'ctx>> PathfindState<'_, '_, Ctx> {
    fn endpoint(&self, entity: Entity) -> Option<&'ctx ground::Endpoint> {
        if let Some(value) = self.ctx.endpoint(entity) {
            Some(value)
        } else {
            self.failed.set(true);
            None
        }
    }

    fn segment(
        &self,
        entity: Entity,
    ) -> Option<(&'ctx ground::Segment, &'ctx ground::SegmentLabel)> {
        if let Some(value) = self.ctx.segment(entity) {
            Some(value)
        } else {
            self.failed.set(true);
            None
        }
    }

    fn successors(
        &self,
        source: DijkstraVertex,
    ) -> Option<Vec<(DijkstraVertex, OrderedFloat<f32>)>> {
        let source_endpoint = self.endpoint(source.endpoint_id)?;
        let successors =
            source_endpoint.adjacency.iter().copied().filter_map(move |succ_segment_id| {
                if succ_segment_id == source.from_segment {
                    // do not U-turn
                    return None;
                }

                let (succ_segment, succ_label) = self.segment(succ_segment_id)?;
                let Some(succ_direction) = succ_segment.direction_from(source.endpoint_id) else {
                    bevy::log::error!(
                        "Segment {succ_segment_id:?} is in the adjacency of endpoint {:?} but \
                         does not include it as an endpoint",
                        source.endpoint_id,
                    );
                    self.failed.set(true);
                    return None;
                };
                if source.endpoint_id == self.initial_dest_endpoint_id
                    && let Some(initial_speed) = self.options.initial_speed
                    && succ_segment.max_speed < initial_speed
                {
                    return None;
                }

                if let Some(min_width) = self.options.min_width
                    && succ_segment.width < min_width
                {
                    return None;
                }

                let dest_endpoint_id = succ_segment
                    .other_endpoint(source.endpoint_id)
                    .expect("adjacency segment of endpoint must contain itself");
                let dest_endpoint = self.endpoint(dest_endpoint_id)?;
                let distance = source_endpoint.position.distance_exact(dest_endpoint.position);
                let cost = OrderedFloat(distance.0);

                let next_label_offset = if let Some(si) =
                    self.subseq_labels.get(source.label_offset)
                    && si.matches(succ_label, succ_direction)
                {
                    source.label_offset + 1
                } else {
                    source.label_offset
                };

                Some((
                    DijkstraVertex {
                        endpoint_id:  dest_endpoint_id,
                        label_offset: next_label_offset,
                        from_segment: succ_segment_id,
                    },
                    cost,
                ))
            });
        Some(successors.collect())
    }

    fn is_terminal(&self, vertex: DijkstraVertex, mode: PathfindMode) -> bool {
        if self.failed.get() {
            return false;
        }

        match mode {
            PathfindMode::SegmentStart => {
                if vertex.label_offset + 1 == self.subseq_labels.len() {
                    let endpoint = self
                        .ctx
                        .endpoint(vertex.endpoint_id)
                        .expect("successors only generates checked endpoints");
                    let last_si = self.subseq_labels.last().expect("subseq_labels is non-empty");
                    endpoint.adjacency.iter().any(|&segment_id| {
                        if let Some((segment, label)) = self.ctx.segment(segment_id)
                            && let Some(dir) = segment.direction_from(vertex.endpoint_id)
                        {
                            last_si.matches(label, dir)
                        } else {
                            false
                        }
                    })
                } else {
                    false
                }
            }
            PathfindMode::SegmentEnd => vertex.label_offset == self.subseq_labels.len(),
            PathfindMode::Endpoint(dest) => {
                vertex.label_offset == self.subseq_labels.len() && dest == vertex.endpoint_id
            }
            PathfindMode::Segment(_) => {
                vertex.label_offset == self.subseq_labels.len()
                    && mode == PathfindMode::Segment(vertex.from_segment)
            }
        }
    }
}

/// Finds the shortest path starting from `initial_segment_id` through `initial_dest_endpoint_id`,
/// such that `subseq_labels` is an ordered subsequence of the labels of the segments in the path.
///
/// Returns `None` if no valid path can be found.
pub fn pathfind_through_subseq<'a>(
    ctx: impl PathfindContext<'a>,
    initial_segment_id: Entity,
    initial_dest_endpoint_id: Entity,
    subseq_labels: &[SubseqItem],
    mode: PathfindMode,
    options: PathfindOptions,
) -> Option<Path> {
    let state = &PathfindState {
        ctx,
        failed: Cell::new(false),
        initial_dest_endpoint_id,
        subseq_labels,
        options,
    };

    let (nodes, cost) = dijkstra(
        &DijkstraVertex {
            endpoint_id:  initial_dest_endpoint_id,
            label_offset: 0,
            from_segment: initial_segment_id,
        },
        move |&vertex| state.successors(vertex).into_iter().flatten(),
        |&vertex| state.is_terminal(vertex, mode),
    )?;
    bevy::log::trace!(
        "request {subseq_labels:?}, found path with cost {:?}: {}",
        Length::new(cost.0).into_meters(),
        DisplayPath { nodes: &nodes, ctx: &ctx }
    );

    Some(Path {
        endpoints: nodes.into_iter().map(|vertex| vertex.endpoint_id).collect(),
        cost:      Length::new(cost.0),
    })
}

struct DisplayPath<'a, Ctx> {
    nodes: &'a [DijkstraVertex],
    ctx:   &'a Ctx,
}

impl<'b, Ctx: PathfindContext<'b>> fmt::Display for DisplayPath<'_, Ctx> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut label_offset = 0;
        write!(f, "START")?;
        for node in self.nodes {
            write!(
                f,
                " -- {:?} --> to endpoint {:?}",
                self.ctx.segment(node.from_segment).map(|(_, lbl)| lbl),
                node.endpoint_id,
            )?;
            if mem::replace(&mut label_offset, node.label_offset) != node.label_offset {
                write!(f, " [label] ")?;
            }
        }
        Ok(())
    }
}

/// Destination mode for [`pathfind_through_subseq`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    /// The path must end on this segment.
    ///
    /// This segment may be the only segment matching the last label in `subseq_labels`.
    Segment(Entity),
}

/// Optional limits for [`pathfind_through_subseq`].
#[derive(Default, Clone, Copy)]
pub struct PathfindOptions {
    /// - If `Some`, the *first* segment after `initial_dest_endpoint_id`
    ///   must have a `max_speed` greater than or equal to `current_speed`.
    ///   This limit is not checked for subsequent segments.
    pub initial_speed: Option<Speed<f32>>,
    /// - If `Some`, *all* segments in the path must have a width greater than or equal to `width`.
    pub min_width:     Option<Length<f32>>,
}

/// A pathfinding result.
#[derive(Debug)]
pub struct Path {
    pub endpoints: Vec<Entity>,
    pub cost:      Length<f32>,
}
