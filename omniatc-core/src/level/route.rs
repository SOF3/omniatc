use std::collections::VecDeque;
use std::mem;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::event::EventReader;
use bevy::ecs::system::SystemState;
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Dir2, Vec2, Vec3};
use bevy::prelude::{
    Commands, Component, Entity, EntityCommand, EntityRef, IntoScheduleConfigs, Query, Res, World,
};
use bevy::time::{self, Time};
use serde::{Deserialize, Serialize};

use super::object::{self, GroundSpeedCalculator, Object};
use super::runway::{self, Runway};
use super::waypoint::Waypoint;
use super::{nav, navaid, SystemSets};
use crate::level::object::RefAltitudeType;
use crate::math::Between;
use crate::units::{Angle, Distance, Heading, Position, Speed};
use crate::{try_log, try_log_return};

/// Horizontal distance before the point at which
/// an object must start changing altitude at standard rate
/// in order to reach the required configured altitude set in the future.
const ALTITUDE_CHANGE_TRIGGER_WINDOW: Distance<f32> = Distance::from_nm(1.);

/// Frequency of re-executing the route plan for each object.
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);

/// [Activation range](nav::TargetAlignment::activation_range) for `AlignRunway` nodes.
///
/// This constant has relatively longer activation range
/// compared to the default one triggered by explicit user command,
/// because the object is expected to immediately start aligning
/// by the time the `AlignRunway` node becomes active.
const ALIGN_RUNWAY_ACTIVATION_RANGE: Distance<f32> = Distance::from_nm(0.5);

/// [Lookahead duration](nav::TargetAlignment::lookahead) for `AlignRunway` nodes.
const ALIGN_RUNWAY_LOOKAHEAD: Duration = Duration::from_secs(10);

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            (
                fly_over_trigger_system,
                fly_by_trigger_system,
                time_trigger_system,
                distance_trigger_system,
                navaid_trigger_system,
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
/// Always manipulate through commands e.g. [`Push`], [`ClearAll`], etc.
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

pub struct SetStandby;

impl EntityCommand for SetStandby {
    fn apply(self, mut entity: EntityWorldMut) {
        let mut route =
            entity.insert_if_new(Route::default()).get_mut::<Route>().expect("just inserted");

        if matches!(route.current(), Some(Node::Standby(..))) {
            return; // already standby
        }

        route.prepend(StandbyNode.into());

        let entity_id = entity.id();
        entity.world_scope(|world| run_current_node(world, entity_id));
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

// TODO possible optimization: run this in systems with parallelization.
fn run_current_node(world: &mut World, entity: Entity) {
    loop {
        {
            // TODO revisit whether we can optimize away unnecessary remove-reinserts.
            clear_all_triggers(world, entity);

            let current_node =
                world.entity(entity).get::<Route>().and_then(|route| route.current());

            match current_node {
                None => break,
                Some(node) => match node.run_as_current_node(world, entity) {
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
                            let preset = try_log!(world.get::<Preset>(preset_id), expect "invalid route preset reference" or return Vec::new());
                            preset.nodes.clone()
                        });

                        let mut entity_ref = world.entity_mut(entity);
                        let mut route = entity_ref
                            .get_mut::<Route>()
                            .expect("route should not be removed by run_current_node");
                        route.clear();
                        route.extend(new_nodes);
                    }
                },
            }
        };
    }

    update_altitude(world, entity);

    let time_elapsed = world.resource::<Time<time::Virtual>>().elapsed();
    let mut entity_ref = world.entity_mut(entity);
    let mut trigger = entity_ref
        .insert_if_new(TimeTrigger(time_elapsed))
        .get_mut::<TimeTrigger>()
        .expect("inserted if missing");
    trigger.0 = time_elapsed + REFRESH_INTERVAL;
}

fn update_altitude(world: &mut World, entity: Entity) {
    let mut gs_calc = SystemState::<GroundSpeedCalculator>::new(world);

    let entity_ref = world.entity(entity);
    if let Some(route) = entity_ref.get::<Route>() {
        match plan_altitude(world, &entity_ref, route, &mut gs_calc) {
            PlanAltitudeResult::None => {
                world.entity_mut(entity).remove::<DistanceTrigger>();
            }
            PlanAltitudeResult::Immediate { altitude, expedite } => {
                world
                    .entity_mut(entity)
                    .remove::<DistanceTrigger>()
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

                world.entity_mut(entity).insert(DistanceTrigger {
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
        distance:                 Distance<f32>,
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
        const SAMPLE_DENSITY: Distance<f32> = Distance::from_nm(1.);

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
        FlyByTrigger,
        FlyOverTrigger,
        DistanceTrigger,
        TimeTrigger,
        NavaidChangeTrigger,
    )>();
}

#[portrait::make]
trait NodeKind: Copy {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult;

    /// Whether the node configures the object heading.
    fn configures_heading(self, _world: &World) -> Option<ConfiguresHeading> { None }

    /// Whether the node expects an altitude to be reached.
    fn desired_altitude(self, _world: &World) -> DesiredAltitude { DesiredAltitude::Inconclusive }

    /// Whether the node configures the object airspeed.
    fn configures_airspeed(self, _world: &World) -> Option<Speed<f32>> { None }

    /// Whether the node expects to lead an object to a position.
    ///
    /// This is similar to `configures_heading`, but used for different purposes:
    /// `configures_heading` indicates the directional information to orient the object
    /// while `configures_position` indicates the positional information to locate the object.
    fn configures_position(self, _world: &World) -> Option<Position<Vec2>> { None }
}

enum RunNodeResult {
    /// Pending triggers to activate, nothing more to do.
    PendingTrigger,
    /// Current node is done, skip to the next node.
    NodeDone,
    /// The entire route should be aborted and replaced with the specified preset.
    ReplaceWithPreset(Option<Entity>),
}

enum ConfiguresHeading {
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

/// Stay in this node until explicitly completed by user command.
#[derive(Clone, Copy)]
pub struct StandbyNode;

impl NodeKind for StandbyNode {
    fn run_as_current_node(self, _: &mut World, _: Entity) -> RunNodeResult {
        RunNodeResult::PendingTrigger
    }

    fn configures_heading(self, _: &World) -> Option<ConfiguresHeading> { None }

    fn desired_altitude(self, _: &World) -> DesiredAltitude { DesiredAltitude::NotRequired }

    fn configures_position(self, _: &World) -> Option<Position<Vec2>> { None }
}

/// Head towards a waypoint.
///
/// This node completes when `distance` OR `proximity` is satisfied.
#[derive(Clone, Copy)]
pub struct DirectWaypointNode {
    /// Waypoint to fly towards.
    pub waypoint:  Entity,
    /// The node is considered complete when
    /// the horizontal distance between the object and the waypoint is less than this value.
    pub distance:  Distance<f32>,
    /// Whether the object is allowed to complete this node early when in proximity.
    pub proximity: WaypointProximity,
    /// Start pitching at standard rate *during or before* this node,
    /// approximately reaching this altitude by the time the specified waypoint is reached.
    pub altitude:  Option<Position<f32>>,
}

impl NodeKind for DirectWaypointNode {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult {
        let Self { waypoint, distance, .. } = self;

        world.entity_mut(entity).insert(nav::TargetWaypoint { waypoint_entity: waypoint });

        match self.proximity {
            WaypointProximity::FlyOver => {
                world.entity_mut(entity).insert(FlyOverTrigger { waypoint, distance });
                RunNodeResult::PendingTrigger
            }
            WaypointProximity::FlyBy => {
                let next_node = world.entity(entity).get::<Route>().and_then(|route| {
                    route.next_queue.iter().find_map(|node| node.configures_heading(world))
                });

                let completion_condition = match next_node {
                    None => FlyByCompletionCondition::Distance(distance),
                    Some(next) => FlyByCompletionCondition::Heading(next),
                };
                world.entity_mut(entity).insert(FlyByTrigger { waypoint, completion_condition });
                RunNodeResult::PendingTrigger
            }
        }
    }

    fn configures_heading(self, _world: &World) -> Option<ConfiguresHeading> {
        Some(ConfiguresHeading::Waypoint(self.waypoint))
    }

    fn desired_altitude(self, world: &World) -> DesiredAltitude {
        match self.altitude {
            Some(altitude) => {
                if let Some(waypoint) = world.entity(self.waypoint).get::<Waypoint>() {
                    DesiredAltitude::Desired(waypoint.position.horizontal().with_altitude(altitude))
                } else {
                    bevy::log::error!(
                        "Invalid waypoint entity {:?} referenced from route node",
                        self.waypoint
                    );
                    DesiredAltitude::NotRequired
                }
            }
            None => DesiredAltitude::Inconclusive,
        }
    }

    fn configures_position(self, world: &World) -> Option<Position<Vec2>> {
        world.get::<Waypoint>(self.waypoint).map(|waypoint| waypoint.position.horizontal())
    }
}

/// Increase/reduce the speed to the desired value.
///
/// When the object is not yet airborne, this would control the expected airspeed
/// if the object is immediately airborne.
#[derive(Clone, Copy)]
pub struct SetAirspeedNode {
    /// Desired speed to set.
    pub speed: Speed<f32>,
    /// The node completes immediately if `error` is `None`,
    /// or when the difference between `speed` and the indicated airspeed of the object
    /// is less than `error` if it is `Some`.
    pub error: Option<Speed<f32>>,
}

impl NodeKind for SetAirspeedNode {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult {
        if let Some(mut airborne) = world.entity_mut(entity).get_mut::<nav::VelocityTarget>() {
            airborne.horiz_speed = self.speed;
            // TODO: what about ground objects?
        }

        let current_airspeed = move |world: &mut World| {
            let mut state = SystemState::<object::GetAirspeed>::new(world);
            state.get(world).get_airspeed(entity)
        };

        if self.error.is_none_or(|error| {
            current_airspeed(world)
                .horizontal()
                .magnitude_cmp()
                .between_inclusive(&(self.speed - error), &(self.speed + error))
        }) {
            RunNodeResult::NodeDone
        } else {
            RunNodeResult::PendingTrigger
        }
    }

    fn configures_airspeed(self, _world: &World) -> Option<Speed<f32>> { Some(self.speed) }
}

/// Start pitching to reach the given altitude.
#[derive(Clone, Copy)]
pub struct StartSetAltitudeNode {
    /// The target altitude to reach.
    pub altitude: Position<f32>,
    /// The node completes immediately if `error` is `None`,
    /// or when the difference between `speed` and the real altitude of the object
    /// is less than `error` if it is `Some`.
    pub error:    Option<Distance<f32>>,
    pub expedite: bool,
    // TODO control pressure altitude instead?
}

impl NodeKind for StartSetAltitudeNode {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult {
        let mut entity_ref = world.entity_mut(entity);
        let current_altitude =
            entity_ref.get::<Object>().expect("entity must be an Object").position.altitude();
        if self.error.is_none_or(|error| {
            current_altitude.between_inclusive(&(self.altitude - error), &(self.altitude + error))
        }) {
            RunNodeResult::NodeDone
        } else {
            entity_ref
                .insert(nav::TargetAltitude { altitude: self.altitude, expedite: self.expedite });
            RunNodeResult::PendingTrigger
        }
    }

    fn desired_altitude(self, _world: &World) -> DesiredAltitude { DesiredAltitude::NotRequired }
}

fn align_runway(object: &mut EntityWorldMut, runway: Entity, expedite: bool) -> Result<(), ()> {
    let Some((glide_descent, localizer_waypoint)) = object.world_scope(|world| {
        Some((
            try_log!(
                world.get::<Runway>(runway),
                expect "AlignRunwayNode references non-runway entity {runway:?}"
                or return None
            )
            .glide_descent,
            try_log!(
                world.get::<runway::LocalizerWaypointRef>(runway),
                expect "Runway {runway:?} has no LocalizerWaypointRef"
                or return None
            )
            .localizer_waypoint,
        ))
    }) else {
        return Err(());
    };

    object.remove::<(nav::TargetWaypoint, nav::TargetAltitude)>().insert((
        nav::TargetAlignment {
            start_waypoint:   localizer_waypoint,
            end_waypoint:     runway,
            activation_range: ALIGN_RUNWAY_ACTIVATION_RANGE,
            lookahead:        ALIGN_RUNWAY_LOOKAHEAD,
        },
        nav::TargetGlide {
            target_waypoint: runway,
            glide_angle: -glide_descent,
            // the actual minimum pitch is regulated by maximum descent rate.
            min_pitch: -Angle::RIGHT,
            max_pitch: Angle::ZERO,
            lookahead: ALIGN_RUNWAY_LOOKAHEAD,
            expedite,
        },
    ));

    Ok(())
}

/// Aligns the object with a runway localizer during the final leg,
/// before switching to short final.
///
/// Short final here is defined as the point at which
/// the object must start reducing to threshold crossing speed.
///
/// Must be followed by [`ShortFinalNode`].
/// Completes when distance from runway is less than
/// [`nav::Limits::short_final_dist`].
#[derive(Clone, Copy)]
pub struct AlignRunwayNode {
    /// The runway waypoint entity.
    pub runway:          Entity,
    /// Whether to allow descent expedition to align with the glidepath.
    pub expedite:        bool,
    /// The preset to switch to in case of missed approach.
    pub goaround_preset: Option<Entity>,
}

impl NodeKind for AlignRunwayNode {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult {
        let &Waypoint { position: runway_position, .. } = try_log!(
            world.get::<Waypoint>(self.runway),
            expect "Runway {:?} must have a corresponding waypoint" (self.runway)
            or return RunNodeResult::PendingTrigger
        );

        let mut object = world.entity_mut(entity);
        if align_runway(&mut object, self.runway, self.expedite).is_err() {
            return RunNodeResult::PendingTrigger;
        }

        let position = object.get::<Object>().expect("entity must be an Object").position;
        let limits = try_log!(
            object.get::<nav::Limits>(),
            expect "Landing aircraft must have nav limits"
            or return RunNodeResult::PendingTrigger
        );
        let dist = position.horizontal_distance_exact(runway_position);
        if dist < limits.short_final_dist {
            RunNodeResult::NodeDone
        } else {
            let dist_before_short = dist - limits.short_final_dist;
            object.insert(DistanceTrigger {
                last_observed_pos:  position.horizontal(),
                remaining_distance: dist_before_short,
            });
            RunNodeResult::PendingTrigger
        }
    }

    fn configures_heading(self, world: &World) -> Option<ConfiguresHeading> {
        let runway = world.get::<Runway>(self.runway)?;
        Some(ConfiguresHeading::Heading(Heading::from_vec2(runway.landing_length.0)))
    }
}

/// Enforces final approach speed and wait for visual contact with runway.
///
/// Completes when visual contact is established with the runway.
/// Switches to goaround preset if ILS is lost before visual contact is established,
/// e.g. due to ILS interference or low visibility
/// (no visual contact within minimum runway visual range).
///
/// The main goal of this node is to ensure allow ILS-only approach before visual contact;
/// ILS is no longer used after this node completes.
///
/// Must be followed by [`VisualLandingNode`].
#[derive(Clone, Copy)]
pub struct ShortFinalNode {
    /// The runway waypoint entity.
    pub runway:          Entity,
    /// The preset to switch to in case of missed approach.
    pub goaround_preset: Option<Entity>,
}

impl NodeKind for ShortFinalNode {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult {
        fn classify_navaid(
            runway: Entity,
            navaid: Entity,
            world: &World,
            has_visual: &mut bool,
            has_ils: &mut bool,
        ) {
            let owner = try_log_return!(world.get::<navaid::OwnerWaypoint>(navaid), expect "navaid must have an owner waypoint");
            if owner.0 == runway {
                if world.entity(navaid).contains::<navaid::Visual>() {
                    *has_visual = true;
                }
                if world.entity(navaid).contains::<navaid::LandingAid>() {
                    *has_ils = true;
                }
            }
        }

        let mut object = world.entity_mut(entity);
        if align_runway(&mut object, self.runway, true).is_err() {
            return RunNodeResult::PendingTrigger;
        }

        let &nav::Limits { short_final_speed, .. } = try_log!(
            object.get(),
            expect "Landing aircraft must have nav limits"
            or return RunNodeResult::PendingTrigger
        );

        let mut vel_target = try_log!(
            object.get_mut::<nav::VelocityTarget>(),
            expect "Landing aircraft must have navigation target"
            or return RunNodeResult::PendingTrigger
        );
        vel_target.horiz_speed = short_final_speed;

        let navaids =
            object.get::<navaid::ObjectUsageList>().expect("dependency of VelocityTarget");

        let mut has_visual = false;
        let mut has_ils = false;
        for &navaid in &navaids.0 {
            classify_navaid(self.runway, navaid, object.world(), &mut has_visual, &mut has_ils);
        }

        if has_visual {
            RunNodeResult::NodeDone
        } else if has_ils {
            object.insert(NavaidChangeTrigger);
            RunNodeResult::PendingTrigger
        } else {
            RunNodeResult::ReplaceWithPreset(self.goaround_preset)
        }
    }
}

/// Maintains final approach configuration until touchdown.
///
/// Completes when the altitude is below or runway elevation.
/// Switches to goaround preset if:
/// - runway is not clear
/// - runway length is shorter than full deceleration distance to zero speed
/// - unsafe crosswind
/// - intolerable wake
/// - too high (above runway elevation but beyond runway length)
/// - not aligned (beyond runway threshold but not within runway width)
#[derive(Clone, Copy)]
pub struct VisualLandingNode {
    /// The runway waypoint entity.
    pub runway:          Entity,
    /// The preset to switch to in case of missed approach.
    pub goaround_preset: Option<Entity>,
}

impl NodeKind for VisualLandingNode {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult {
        let Ok(state) = determine_landing_state(&world.entity(entity), &world.entity(self.runway)) else { return RunNodeResult::PendingTrigger };

        let virtual_time_now = world.resource::<Time<time::Virtual>>().elapsed();

        let mut object = world.entity_mut(entity);
        if align_runway(&mut object, self.runway, true).is_err() {
            return RunNodeResult::PendingTrigger;
        }

        match state {
            LandingState::Approaching { remaining_time } => {
                object.insert(TimeTrigger ( virtual_time_now + remaining_time ));
                RunNodeResult::PendingTrigger
            }
            LandingState::TooHigh => {
                // TODO send message
                RunNodeResult::ReplaceWithPreset(self.goaround_preset)
            }
            LandingState::NotAligned => {
                // TODO send message
                RunNodeResult::ReplaceWithPreset(self.goaround_preset)
            }
            LandingState::MayLand { track_deviation } => {
                // TODO check track deviation
                // TODO set to ground
                RunNodeResult::NodeDone
            }
        }
    }
}

#[derive(Debug)]
enum LandingState {
    Approaching { remaining_time: Duration },
    MayLand { track_deviation: Angle<f32> },
    TooHigh,
    NotAligned,
}

fn determine_landing_state(object: &EntityRef, runway: &EntityRef) -> Result<LandingState, ()> {
    let &Object { position: object_position, ground_speed } =
        object.get().expect("entity must be an Object");
    let &Waypoint { position: runway_position, .. } = try_log!(
        runway.get(), expect "runway must be a waypoint" or return Err(())
    );
    let &Runway { landing_length, width: runway_width, .. } = try_log!(
        runway.get(), expect "runway must be valid" or return Err(())
    );
    let &runway::Condition { friction_factor } = try_log!(
        runway.get(), expect "runway must have condition" or return Err(())
    );

    let runway_dir = try_log!(
        Dir2::new(landing_length.0),
        expect "runway must have nonzero landing length" or return Err(())
    );

    let projected_speed = ground_speed.horizontal().project_onto_dir(runway_dir);

    let threshold_dist = runway_position - object_position;
    let height = -threshold_dist.vertical();
    let projected_threshold_dist = threshold_dist.horizontal().project_onto_dir(runway_dir);

    if height.is_positive() && projected_threshold_dist.is_positive() {
        let remaining_time = height
            .try_div(-ground_speed.vertical())
            .unwrap_or(Duration::ZERO)
            .min(projected_threshold_dist.try_div(projected_speed).unwrap_or(Duration::ZERO));
        Ok(LandingState::Approaching { remaining_time })
    } else {
        let centerline_dist =
            threshold_dist.horizontal().magnitude_squared() - projected_threshold_dist.squared();
        if centerline_dist > runway_width.squared() {
            return Ok(LandingState::NotAligned);
        }

        // if height is non-positive but threshold distance is positive,
        // it basically ditched into terrain before reaching the runway...
        // but for simplicity we just assume it is an extended runway for now.
        // TODO handle aircraft crash
        let remaining_runway_dist = projected_threshold_dist + landing_length.magnitude_exact();

        // TODO compute from nav::Limits based on deceleration, current ground speed and runway
        // condition
        let required_landing_dist = Distance::from_meters(1000.);

        if remaining_runway_dist < required_landing_dist {
            Ok(LandingState::TooHigh)
        } else {
            let runway_heading = landing_length.heading();
            let track_heading = ground_speed.horizontal().heading();
            Ok(LandingState::MayLand {
                track_deviation: track_heading.closest_distance(runway_heading),
            })
        }
    }
}

/// An entry in the flight plan.
#[derive(Clone, Copy, derive_more::From)]
#[portrait::derive(NodeKind with portrait::derive_delegate)]
pub enum Node {
    Standby(StandbyNode),
    DirectWaypoint(DirectWaypointNode),
    SetAirSpeed(SetAirspeedNode),
    StartSetAltitude(StartSetAltitudeNode),
    AlignRunway(AlignRunwayNode),
    ShortFinal(ShortFinalNode),
    VisualLanding(VisualLandingNode),
}

pub fn node_vec(node: impl Into<Node>) -> Vec<Node> { Vec::from([node.into()]) }

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum WaypointProximity {
    /// Turn to the next waypoint before arriving at the waypoint,
    /// such that the position after the turn is exactly between the two waypoints.
    ///
    /// The step is always completed when the proximity range is entered,
    /// allowing smooth transition when the next waypoint has the same heading.
    FlyBy,
    /// Enter the horizontal [distance](Node::distance) range of the waypoint before turning to the next one.
    FlyOver,
}

#[derive(Component)]
struct FlyOverTrigger {
    waypoint: Entity,
    distance: Distance<f32>,
}

fn fly_over_trigger_system(
    time: Res<Time<time::Virtual>>,
    waypoint_query: Query<&Waypoint>,
    object_query: Query<(Entity, &Object, &FlyOverTrigger)>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query.iter().for_each(
        |(object_entity, &Object { position: current_pos, .. }, trigger)| {
            let &Waypoint { position: current_target, .. } = try_log_return!(
                waypoint_query.get(trigger.waypoint),
                expect "Invalid waypoint referenced in route"
            );

            if current_pos.distance_cmp(current_target) <= trigger.distance {
                commands.entity(object_entity).queue(NextNode);
            }
        },
    );
}

#[derive(Component)]
struct FlyByTrigger {
    waypoint:             Entity,
    completion_condition: FlyByCompletionCondition,
}

enum FlyByCompletionCondition {
    Heading(ConfiguresHeading),
    Distance(Distance<f32>),
}

fn fly_by_trigger_system(
    time: Res<Time<time::Virtual>>,
    waypoint_query: Query<&Waypoint>,
    object_query: Query<(Entity, &Object, &nav::Limits, &FlyByTrigger)>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query.iter().for_each(
        |(
            object_entity,
            &Object { position: current_pos, ground_speed: speed },
            nav_limits,
            trigger,
        )| {
            let &Waypoint { position: current_target, .. } = try_log_return!(
                waypoint_query.get(trigger.waypoint),
                expect "Invalid waypoint referenced in route"
            );
            let current_target = current_target.horizontal();

            match trigger.completion_condition {
                FlyByCompletionCondition::Heading(ref heading_config) => {
                    let next_heading = match *heading_config {
                        ConfiguresHeading::Position(next_target) => {
                            (next_target - current_target).heading()
                        }
                        ConfiguresHeading::Waypoint(next_waypoint) => {
                            let &Waypoint { position: next_target, .. } = try_log_return!(
                                waypoint_query.get(next_waypoint),
                                expect "Invalid waypoint referenced in next node in route"
                            );
                            let next_target = next_target.horizontal();

                            (next_target - current_target).heading()
                        }
                        ConfiguresHeading::Heading(heading) => heading,
                    };

                    let current_heading = (current_target - current_pos.horizontal()).heading();
                    let turn_radius = Distance(
                        speed.horizontal().magnitude_exact().0 / nav_limits.max_yaw_speed.0,
                    ); // (L/T) / (1/T) = L
                    let turn_distance = turn_radius
                        * (current_heading.closest_distance(next_heading).abs() / 2.)
                            .acute_signed_tan();

                    if current_pos.horizontal().distance_cmp(current_target) <= turn_distance {
                        commands.entity(object_entity).queue(NextNode);
                    }
                }
                FlyByCompletionCondition::Distance(max_distance) => {
                    if current_pos.horizontal().distance_cmp(current_target) <= max_distance {
                        commands.entity(object_entity).queue(NextNode);
                    }
                }
            }
        },
    );
}

#[derive(Component)]
struct TimeTrigger(Duration);

fn time_trigger_system(
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(Entity, &TimeTrigger)>,
    mut commands: Commands,
) {
    object_query.iter_mut().for_each(|(object_entity, trigger)| {
        if trigger.0 >= time.elapsed() {
            commands.entity(object_entity).queue(RunCurrentNode);
        }
    });
}

#[derive(Component)]
struct DistanceTrigger {
    remaining_distance: Distance<f32>,
    last_observed_pos:  Position<Vec2>,
}

fn distance_trigger_system(
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(Entity, &Object, &mut DistanceTrigger)>,
    mut commands: Commands,
) {
    if time.is_paused() {
        return;
    }

    object_query.iter_mut().for_each(|(object_entity, object, mut trigger)| {
        let last_pos = mem::replace(&mut trigger.last_observed_pos, object.position.horizontal());
        trigger.remaining_distance -= last_pos.distance_exact(object.position.horizontal());

        if !trigger.remaining_distance.is_positive() {
            commands.entity(object_entity).queue(RunCurrentNode);
        }
    });
}

#[derive(Component)]
struct NavaidChangeTrigger;

fn navaid_trigger_system(
    mut event_reader: EventReader<navaid::UsageChangeEvent>,
    mut commands: Commands,
) {
    for event in event_reader.read() {
        commands.entity(event.object).queue(RunCurrentNode);
    }
}

#[derive(Component)]
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
