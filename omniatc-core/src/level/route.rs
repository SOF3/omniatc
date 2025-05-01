use std::collections::VecDeque;
use std::mem;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::system::SystemState;
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{
    Commands, Component, Entity, EntityCommand, EntityRef, IntoScheduleConfigs, Query, Res, World,
};
use bevy::time::{self, Time};
use serde::{Deserialize, Serialize};

use super::object::{self, GroundSpeedCalculator, Object};
use super::runway::{self, Runway};
use super::waypoint::Waypoint;
use super::{nav, SystemSets};
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
            )
                .in_set(SystemSets::Action),
        );
    }
}

/// Stores the flight plan of the object.
///
/// Always manipulate through commands e.g. [`Push`], [`ClearAll`], etc.
#[derive(Component, Default)]
pub struct Route {
    current:    Option<Node>, // promoted to its own field to improve cache locality.
    next_queue: VecDeque<Node>,
}

impl Route {
    pub fn push(&mut self, node: Node) {
        if self.current.is_none() {
            self.current = Some(node);
        } else {
            self.next_queue.push_back(node);
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
        let mut route = entity.get_mut::<Route>().expect("just inserted");
        route.shift();

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

// TODO possible optimization: run this in systems with parallelization.
fn run_current_node(world: &mut World, entity: Entity) {
    loop {
        {
            let current_node =
                world.entity(entity).get::<Route>().and_then(|route| route.current());

            match current_node {
                None => break clear_all_triggers(world, entity),
                Some(node) => match node.run_as_current_node(world, entity) {
                    RunNodeResult::PendingTrigger => break,
                    RunNodeResult::NodeDone => {
                        let mut entity_ref = world.entity_mut(entity);
                        let mut route = entity_ref
                            .get_mut::<Route>()
                            .expect("route should not be shifted by run_current_node");
                        route.shift();
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
    let Some(&object::Airborne { airspeed: current_airspeed }) = entity_ref.get() else {
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
    world.entity_mut(entity).remove::<(FlyByTrigger, FlyOverTrigger)>();
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
                world
                    .entity_mut(entity)
                    .remove::<FlyByTrigger>()
                    .insert(FlyOverTrigger { waypoint, distance });
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
                world
                    .entity_mut(entity)
                    .remove::<FlyOverTrigger>()
                    .insert(FlyByTrigger { waypoint, completion_condition });
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

/// Set the speed to the desired value.
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
        let current_airspeed =
            SystemState::<object::GetAirspeed>::new(world).get(world).get_airspeed(entity);

        if self.error.is_none_or(|error| {
            current_airspeed
                .horizontal()
                .magnitude_cmp()
                .between_inclusive(&(self.speed - error), &(self.speed + error))
        }) {
            RunNodeResult::NodeDone
        } else {
            if let Some(mut airborne) = world.entity_mut(entity).get_mut::<nav::VelocityTarget>() {
                airborne.horiz_speed = self.speed;
            }
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

#[derive(Clone, Copy)]
pub struct AlignRunwayNode {
    /// The runway waypoint entity.
    pub runway:   Entity,
    /// Whether to allow descent expedition to align with the glidepath.
    pub expedite: bool,
}

impl NodeKind for AlignRunwayNode {
    fn run_as_current_node(self, world: &mut World, entity: Entity) -> RunNodeResult {
        let &Runway { glide_angle, .. } = try_log!(
            world.get::<Runway>(self.runway),
            expect "AlignRunwayNode references non-runway entity {:?}" (self.runway)
            or return RunNodeResult::PendingTrigger
        );
        let &runway::LocalizerWaypointRef { localizer_waypoint } = try_log!(
            world.get(self.runway),
            expect "Runway {:?} has no LocalizerWaypointRef" (self.runway)
            or return RunNodeResult::PendingTrigger
        );

        let mut entity_ref = world.entity_mut(entity);
        entity_ref.remove::<(nav::TargetWaypoint, nav::TargetAltitude)>().insert((
            nav::TargetAlignment {
                start_waypoint:   localizer_waypoint,
                end_waypoint:     self.runway,
                activation_range: ALIGN_RUNWAY_ACTIVATION_RANGE,
                lookahead:        ALIGN_RUNWAY_LOOKAHEAD,
            },
            nav::TargetGlide {
                target_waypoint: self.runway,
                glide_angle:     -glide_angle,
                // the actual minimum pitch is regulated by maximum descent rate.
                min_pitch:       -Angle::RIGHT,
                max_pitch:       Angle::ZERO,
                lookahead:       ALIGN_RUNWAY_LOOKAHEAD,
                expedite:        self.expedite,
            },
        ));

        RunNodeResult::NodeDone
    }

    fn configures_heading(self, world: &World) -> Option<ConfiguresHeading> {
        let runway = world.get::<Runway>(self.runway)?;
        Some(ConfiguresHeading::Heading(Heading::from_vec2(runway.landing_length.0)))
    }
}

/// An entry in the flight plan.
#[derive(Clone, Copy)]
#[portrait::derive(NodeKind with portrait::derive_delegate)]
pub enum Node {
    DirectWaypoint(DirectWaypointNode),
    SetAirspeed(SetAirspeedNode),
    StartSetAltitude(StartSetAltitudeNode),
    AlignRunway(AlignRunwayNode),
}

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
                        * (current_heading.closest_distance(next_heading).abs() / 2.).tan();

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
