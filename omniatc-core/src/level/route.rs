use std::collections::VecDeque;
use std::mem;
use std::num::NonZero;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{EntityCommand, SystemState};
use bevy::ecs::world::{EntityRef, EntityWorldMut, World};
use bevy::math::{Vec2, Vec3};
use bevy::time::{self, Time};
use math::{Heading, Length, Position, Speed};

use crate::level::dest::Destination;
use crate::level::object::{self, GroundSpeedCalculator, Object, RefAltitudeType};
use crate::level::{SystemSets, nav};
use crate::{EntityMutTryLog, WorldTryLog};

mod landing;
pub use landing::*;
mod navigation;
pub use navigation::*;
mod takeoff;
pub use takeoff::*;
mod taxi;
mod trigger;
pub use taxi::*;

pub mod loader;

/// Horizontal distance before the point at which
/// an object must start changing altitude at standard rate
/// in order to reach the required configured altitude set in the future.
const ALTITUDE_CHANGE_TRIGGER_WINDOW: Length<f32> = Length::from_nm(1.);

/// Frequency of re-executing the route plan for each object.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            (
                trigger::fly_over_system,
                trigger::fly_by_system,
                trigger::time_system,
                trigger::distance_system,
                trigger::navaid_system,
            )
                .in_set(SystemSets::Action),
        );
    }
}

/// The preset ID that the current [`Route`] was loaded from.
///
/// This is to track the origin of routes to allow easier switching.
#[derive(Component)]
pub struct Id(pub Option<String>);

/// Stores the flight plan of the object.
///
/// Always manipulate through commands e.g. [`PushNode`], [`ClearAllNodes`], etc.
#[derive(Component, Default)]
pub struct Route {
    current:    Option<Node>, // promoted to its own field to improve cache locality.
    next_queue: VecDeque<Node>,
}

impl Route {
    pub fn clear(&mut self) {
        self.current = None;
        self.next_queue.clear();
    }

    pub fn push(&mut self, node: Node) {
        if self.current.is_none() {
            self.current = Some(node);
        } else {
            self.next_queue.push_back(node);
        }
    }

    pub fn extend(&mut self, nodes: impl IntoIterator<Item = Node>) {
        for node in nodes {
            self.push(node);
        }
    }

    pub fn prepend(&mut self, node: Node) {
        let next = self.current.replace(node);
        if let Some(next) = next {
            self.next_queue.push_front(next);
        }
    }

    #[must_use]
    pub fn current(&self) -> Option<&Node> { self.current.as_ref() }

    #[must_use]
    pub fn next(&self) -> Option<&Node> { self.next_queue.front() }

    #[must_use]
    pub fn last(&self) -> Option<&Node> { self.next_queue.back().or(self.current.as_ref()) }

    #[must_use]
    pub fn last_mut(&mut self) -> Option<&mut Node> {
        self.next_queue.back_mut().or(self.current.as_mut())
    }

    pub fn shift(&mut self) -> Option<Node> {
        let ret = self.current.take();
        self.current = self.next_queue.pop_front();
        ret
    }

    #[must_use]
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &Node> + Clone {
        self.current.iter().chain(self.next_queue.iter())
    }

    #[must_use]
    pub fn get(&self, index: usize) -> Option<&Node> {
        match index.checked_sub(1) {
            Some(queue_offset) => self.next_queue.get(queue_offset),
            None => self.current.as_ref(),
        }
    }
}

impl FromIterator<Node> for Route {
    fn from_iter<I: IntoIterator<Item = Node>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        Self { current: iter.next(), next_queue: iter.collect() }
    }
}

pub struct PushNode(pub Node);

impl EntityCommand for PushNode {
    fn apply(self, mut entity: EntityWorldMut) {
        let mut route =
            entity.insert_if_new(Route::default()).get_mut::<Route>().expect("just inserted");
        route.push(self.0);

        let entity_id = entity.id();
        entity.world_scope(|world| run_current_node(world, entity_id));
    }
}

pub struct NextNode;

impl EntityCommand for NextNode {
    fn apply(self, mut entity: EntityWorldMut) {
        let mut route =
            entity.get_mut::<Route>().expect("NextNode must be used on object entity with a route");
        route.shift();

        let entity_id = entity.id();
        entity.world_scope(|world| run_current_node(world, entity_id));
    }
}

pub struct PrependStandby;

impl EntityCommand for PrependStandby {
    fn apply(self, mut entity: EntityWorldMut) {
        let mut route =
            entity.insert_if_new(Route::default()).get_mut::<Route>().expect("just inserted");

        if matches!(route.current(), Some(Node::Standby(..))) {
            return; // already standby
        }

        route.prepend(StandbyNode { preset_id: None }.into());

        let entity_id = entity.id();
        entity.world_scope(|world| run_current_node(world, entity_id));
    }
}

pub struct RemoveStandby {
    pub preset_id: Option<NonZero<u32>>,
}

impl EntityCommand for RemoveStandby {
    fn apply(self, mut entity: EntityWorldMut) {
        let Some(mut route) = entity.log_get_mut::<Route>() else { return };

        if let Some(Node::Standby(standby)) = route.current() {
            if standby.preset_id == self.preset_id {
                route.shift();

                let entity_id = entity.id();
                entity.world_scope(|world| run_current_node(world, entity_id));
            }
        } else {
            for (index, node) in route.next_queue.iter().enumerate() {
                if let Node::Standby(standby) = node
                    && standby.preset_id == self.preset_id
                {
                    route.next_queue.remove(index);
                    break;
                }
            }
        }
    }
}

/// Recompute the triggers for the route, used after the entire route got replaced.
pub struct RunCurrentNode;

impl EntityCommand for RunCurrentNode {
    fn apply(self, mut entity: EntityWorldMut) {
        let entity_id = entity.id();
        entity.world_scope(|world| run_current_node(world, entity_id));
    }
}

pub struct ClearAllNodes;

impl EntityCommand for ClearAllNodes {
    fn apply(self, mut entity: EntityWorldMut) {
        if let Some(mut route) = entity.get_mut::<Route>() {
            route.current = None;
            route.next_queue.clear();

            let entity_id = entity.id();
            entity.world_scope(|world| run_current_node(world, entity_id));
        }
    }
}

pub struct ReplaceNodes(pub Vec<Node>);

impl EntityCommand for ReplaceNodes {
    fn apply(self, mut entity: EntityWorldMut) {
        let mut route =
            entity.insert_if_new(Route::default()).get_mut::<Route>().expect("just inserted");

        route.clear();
        route.extend(self.0);

        let entity_id = entity.id();
        entity.world_scope(|world| run_current_node(world, entity_id));
    }
}

fn run_current_node(world: &mut World, entity: Entity) {
    fn replace_route(world: &mut World, entity: Entity, new_nodes: Vec<Node>) {
        let mut entity_ref = world.entity_mut(entity);
        let mut route =
            entity_ref.get_mut::<Route>().expect("route should not be removed by run_current_node");
        route.clear();
        route.extend(new_nodes);
    }

    loop {
        {
            // TODO revisit whether we can optimize away unnecessary remove-reinserts.
            clear_all_triggers(world, entity);

            let current_node =
                world.entity(entity).get::<Route>().and_then(|route| route.current());

            match current_node {
                None => break,
                Some(node) => match node.clone().run_as_current_node(world, entity) {
                    RunNodeResult::PendingTrigger => break,
                    RunNodeResult::NodeDone => {
                        let mut entity_ref = world.entity_mut(entity);
                        let mut route = entity_ref
                            .get_mut::<Route>()
                            .expect("route should not be removed by run_current_node");
                        route.shift();
                    }
                    RunNodeResult::ReplaceWithPreset(preset_id) => {
                        let new_nodes = preset_id.map_or_else(Vec::new, |preset_id| {
                            let Some(preset) = world.log_get::<Preset>(preset_id) else {
                                return Vec::new();
                            };
                            preset.nodes.clone()
                        });

                        replace_route(world, entity, new_nodes);
                    }
                    RunNodeResult::ReplaceWithNodes(new_nodes) => {
                        replace_route(world, entity, new_nodes);
                    }
                },
            }
        };
    }

    update_altitude(world, entity);

    let time_elapsed = world.resource::<Time<time::Virtual>>().elapsed();
    let mut entity_ref = world.entity_mut(entity);
    let mut trigger = entity_ref
        .insert_if_new(trigger::TimeDelay(time_elapsed))
        .get_mut::<trigger::TimeDelay>()
        .expect("inserted if missing");
    trigger.0 = time_elapsed + REFRESH_INTERVAL;
}

fn update_altitude(world: &mut World, entity: Entity) {
    let mut gs_calc = SystemState::<GroundSpeedCalculator>::new(world);

    let entity_ref = world.entity(entity);
    if let Some(route) = entity_ref.get::<Route>() {
        match plan_altitude(world, &entity_ref, route, &mut gs_calc) {
            PlanAltitudeResult::None => {
                world.entity_mut(entity).remove::<trigger::ByDistance>();
            }
            PlanAltitudeResult::Immediate { altitude, expedite } => {
                world
                    .entity_mut(entity)
                    .remove::<(trigger::ByDistance, nav::TargetGlide, nav::TargetGlideStatus)>()
                    .insert(nav::TargetAltitude { altitude, expedite });
            }
            PlanAltitudeResult::DelayedTrigger { distance, eventual_target_altitude } => {
                let pos = entity_ref
                    .get::<Object>()
                    .expect("checked numerous times at this stage")
                    .position;

                if let Some(&nav::TargetAltitude { altitude: current_target, expedite }) =
                    entity_ref.get()
                {
                    #[expect(clippy::float_cmp)] // comparison of constant signums is fine
                    if (current_target - pos.altitude()).signum()
                        == (eventual_target_altitude - pos.altitude()).signum()
                    {
                        // No need to wait since we are already moving towards that direction.
                        // Just disable expedite if necessary since we have plenty of time there.
                        if expedite {
                            world
                                .entity_mut(entity)
                                .get_mut::<nav::TargetAltitude>()
                                .expect("checked above")
                                .expedite = false;
                        }
                        return;
                    }
                }

                world.entity_mut(entity).insert(trigger::ByDistance {
                    remaining_distance: distance,
                    last_observed_pos:  pos.horizontal(),
                });
            }
        }
    }
}

#[derive(Debug)]
enum PlanAltitudeResult {
    None,
    Immediate {
        altitude: Position<f32>,
        expedite: bool,
    },
    DelayedTrigger {
        distance:                 Length<f32>,
        eventual_target_altitude: Position<f32>,
    },
}

fn plan_altitude(
    world: &World,
    entity_ref: &EntityRef,
    route: &Route,
    gs_calc: &mut SystemState<GroundSpeedCalculator>,
) -> PlanAltitudeResult {
    struct PathSegment {
        start:    Position<Vec2>,
        end:      Position<Vec2>,
        airspeed: Speed<f32>, // TODO take airspeed reduction time into account
    }

    let current_position = entity_ref.get::<Object>().expect("entity must be an Object").position;
    let Some(&object::Airborne { airspeed: current_airspeed, .. }) = entity_ref.get() else {
        // no need to plan altitude if we are not airborne yet
        return PlanAltitudeResult::None;
    };

    let Some((target_node_index, DesiredAltitude::Desired(target_position))) =
        route.iter().enumerate().map(|(index, node)| (index, node.desired_altitude(world))).find(
            |(_, desired)| {
                matches!(desired, DesiredAltitude::Desired(_) | DesiredAltitude::NotRequired)
            },
        )
    else {
        return PlanAltitudeResult::None;
    };

    let mut segments = Vec::new();

    let mut next_segment_speed = current_airspeed.magnitude_exact();
    let mut next_segment_start = current_position.horizontal();

    for node in route.iter().take(target_node_index + 1) {
        if let Some(speed) = node.configures_airspeed(world) {
            next_segment_speed = speed;
        }

        if let Some(pos) = node.configures_position(world) {
            let start = mem::replace(&mut next_segment_start, pos);
            segments.push(PathSegment { start, end: pos, airspeed: next_segment_speed });
        }
    }

    let Some(limits) = entity_ref.get::<nav::Limits>() else {
        bevy::log::warn_once!("Cannot plan altitude for object {} without limits", entity_ref.id());
        return if target_node_index == 0 {
            PlanAltitudeResult::Immediate { altitude: target_position.altitude(), expedite: false }
        } else {
            PlanAltitudeResult::None
        };
    };

    let std_rate = if target_position.altitude() > current_position.altitude() {
        limits.std_climb.vert_rate
    } else {
        limits.std_descent.vert_rate
    };

    let mut segment_altitude = target_position.altitude();
    for (segment_index, segment) in segments.iter().enumerate().rev() {
        const SAMPLE_DENSITY: Length<f32> = Length::from_nm(1.);

        let new_altitude = gs_calc.get(world).estimate_altitude_change(
            [segment.start, segment.end],
            std_rate,
            segment.airspeed,
            segment_altitude,
            RefAltitudeType::End,
            SAMPLE_DENSITY,
        );

        // assume more or less constant vertical:horizontal speed ratio.
        let ratio = current_position.altitude().ratio_between(new_altitude, segment_altitude);
        if ratio >= 0. {
            // we have found the segment where the altitude change should begin
            return if segment_index == 0 {
                let distance = segment.start.distance_exact(segment.end) * ratio;
                if distance < ALTITUDE_CHANGE_TRIGGER_WINDOW {
                    // start changing altitude as we are almost at the starting point
                    PlanAltitudeResult::Immediate {
                        altitude: target_position.altitude(),
                        expedite: false,
                    }
                } else {
                    PlanAltitudeResult::DelayedTrigger {
                        distance:                 distance - ALTITUDE_CHANGE_TRIGGER_WINDOW,
                        eventual_target_altitude: target_position.altitude(),
                    }
                }
            } else {
                // Object not yet at the trigger segment,
                // wait for replan after route nodes shift
                PlanAltitudeResult::None
            };
        }

        // else, expected trigger point is before this segment
        segment_altitude = new_altitude;
    }

    // expedite altitude change since we are already past expected trigger point
    PlanAltitudeResult::Immediate { altitude: target_position.altitude(), expedite: true }
}

fn clear_all_triggers(world: &mut World, entity: Entity) {
    world.entity_mut(entity).remove::<(
        trigger::FlyBy,
        trigger::FlyOver,
        trigger::ByDistance,
        trigger::TimeDelay,
        trigger::NavaidChange,
        trigger::TaxiTargetResolution,
    )>();
}

#[portrait::make]
trait NodeKind: Sized {
    fn run_as_current_node(&self, world: &mut World, entity: Entity) -> RunNodeResult;

    /// Whether the node configures the object heading.
    fn configures_heading(&self, _world: &World) -> Option<HorizontalTarget> { None }

    /// Whether the node expects an altitude to be reached.
    fn desired_altitude(&self, _world: &World) -> DesiredAltitude { DesiredAltitude::Inconclusive }

    /// Whether the node configures the object airspeed.
    fn configures_airspeed(&self, _world: &World) -> Option<Speed<f32>> { None }

    /// Whether the node expects to lead an object to a position.
    ///
    /// This is similar to `configures_heading`, but used for different purposes:
    /// `configures_heading` indicates the directional information to orient the object
    /// while `configures_position` indicates the positional information to locate the object.
    fn configures_position(&self, _world: &World) -> Option<Position<Vec2>> { None }
}

enum RunNodeResult {
    /// Pending triggers to activate, nothing more to do.
    PendingTrigger,
    /// Current node is done, skip to the next node.
    NodeDone,
    /// The entire route should be aborted and replaced with the specified preset.
    ReplaceWithPreset(Option<Entity>),
    /// The entire route should be aborted and replaced with the specified nodes.
    ReplaceWithNodes(Vec<Node>),
}

/// The horizontal direction to navigate towards.
enum HorizontalTarget {
    /// The heading after this node should point towards a position.
    Position(Position<Vec2>),
    /// The heading after this node should point towards a waypoint.
    Waypoint(Entity),
    /// The heading after this node should be a constant.
    Heading(Heading),
}

enum DesiredAltitude {
    /// No preference on altitude so far.
    Inconclusive,
    /// Desired altitude to reach.
    Desired(Position<Vec3>),
    /// No need to plan altitude ahead.
    NotRequired,
}

/// An entry in the flight plan.
#[derive(Clone, derive_more::From)]
#[portrait::derive(NodeKind with portrait::derive_delegate)]
pub enum Node {
    Standby(StandbyNode),
    DirectWaypoint(DirectWaypointNode),
    SetAirSpeed(SetAirspeedNode),
    StartSetAltitude(StartSetAltitudeNode),
    AlignRunway(AlignRunwayNode),
    ShortFinal(ShortFinalNode),
    VisualLanding(VisualLandingNode),
    Takeoff(TakeoffNode),
    Taxi(TaxiNode),
}

/// Stay in this node until explicitly completed by user command.
///
/// # Completion condition
/// This node never completes on its own.
/// It must be explicitly ended by user command.
#[derive(Clone, Copy)]
pub struct StandbyNode {
    /// Identifies this node during transmission.
    ///
    /// This ID is *not* globally unique.
    /// It only identifies which route preset step it comes from.
    ///
    /// If `None`, this is used to represent the state when
    /// an object is instructed to deviate from its current route.
    /// Thus a `preset_id == None` should only appear
    /// in the first node of an object route,
    /// and should never exist in a preset route.
    pub preset_id: Option<NonZero<u32>>,
}

impl NodeKind for StandbyNode {
    fn run_as_current_node(&self, _: &mut World, _: Entity) -> RunNodeResult {
        RunNodeResult::PendingTrigger
    }

    fn desired_altitude(&self, _: &World) -> DesiredAltitude { DesiredAltitude::NotRequired }
}

#[derive(Component, Clone)]
pub struct Preset {
    pub id:    String,
    pub title: String,
    pub nodes: Vec<Node>,
}

#[derive(Component)]
#[relationship(relationship_target = WaypointPresetList)]
pub struct PresetFromWaypoint(pub Entity);

#[derive(Component)]
#[relationship_target(relationship = PresetFromWaypoint)]
pub struct WaypointPresetList(Vec<Entity>);

impl WaypointPresetList {
    pub fn iter(&self) -> impl Iterator<Item = Entity> + use<'_> { self.0.iter().copied() }
}

#[derive(Component)]
pub struct DestinationMatcher {
    pub items: Vec<DestinationMatcherItem>,
}

impl DestinationMatcher {
    #[must_use]
    pub fn matches(&self, dest: &Destination) -> bool {
        self.items.iter().any(|item| item.matches(dest))
    }
}

pub enum DestinationMatcherItem {
    Arrival { aerodrome: Entity },
    AnyArrival,
    Departure { waypoint: Entity },
    AnyDeparture,
}

impl DestinationMatcherItem {
    #[must_use]
    pub fn matches(&self, dest: &Destination) -> bool {
        #[expect(clippy::match_same_arms, reason = "simple value")]
        match (self, dest) {
            (
                Self::Arrival { aerodrome: a1 },
                Destination::Landing { aerodrome: a2 } | Destination::Parking { aerodrome: a2 },
            ) => a1 == a2,
            (Self::Arrival { .. }, Destination::VacateAnyRunway) => true,
            (Self::Arrival { .. }, Destination::Departure { .. }) => false,
            (
                Self::AnyArrival,
                Destination::Landing { .. }
                | Destination::Parking { .. }
                | Destination::VacateAnyRunway,
            ) => true,
            (Self::AnyArrival, Destination::Departure { .. }) => false,
            (
                Self::Departure { waypoint: w1 },
                Destination::Departure { waypoint_proximity: Some((w2, ..)), .. },
            ) => w1 == w2,
            (Self::Departure { .. }, Destination::Departure { waypoint_proximity: None, .. }) => {
                false
            }
            (Self::AnyDeparture, Destination::Departure { .. }) => true,
            _ => false,
        }
    }
}
