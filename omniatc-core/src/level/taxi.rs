//! Controls ground object movement.
//!
//! Levels of control:
//! - Higher-level systems (e.g. [`route::TaxiNode`](crate::level::route::TaxiNode))
//!   update the [`Target`] component to indicate the action for the next segment.
//! - `target_path_system` updates [`object::OnGround`] to determine the state
//!   "object should move along (which segment) at (what speed) in (which direction)"
//!   to smoothly transition to the target segment as required by [`Target`].
//! - `maintain_dir` executes the movement indicated by [`object::OnGround`]
//!   to move along the centerline at the required speed and heading,
//!   effecting its output on [`Object`] and [`object::TaxiStatus`].
//!
//! `maintain_dir` reduces the speed only when the object is expected to diverge
//! from the centerline beyond the overshoot tolerance.
//! Otherwise, `maintain_dir` always tries to attain the target speed,
//! and it is the responsibility of `target_path_system` to reduce the target speed
//! when approaching an intersection or holding short.

use std::ops;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::{Message, MessageWriter};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy::math::Vec2;
use bevy::time::{self, Time};
use math::{Angle, CanSqrt, Heading, Length, Position, Speed, point_line_closest};
use ordered_float::OrderedFloat;
use wordvec::WordVec;

use super::object::Object;
use super::{SystemSets, ground, object};
use crate::{QueryTryLog, try_log, try_log_return};

/// An object is considered stationary when slower than this speed.
///
/// This value is intended for comparison.
const NEGLIGIBLE_SPEED: Speed<f32> = Speed::from_knots(1.);

/// The default speed when an object must use a nonzero speed to move
/// but wants to be as slow as possible (especially due to turning).
const MIN_POSITIVE_SPEED: Speed<f32> = Speed::from_knots(2.);

/// If an object is within this distance from the centerline,
/// it is considered to be on the centerline,
/// and it will head directly towards the target endpoint
/// instead of pursuing the centerline.
const NEGLIGIBLE_DEVIATION_LENGTH: Length<f32> = Length::from_meters(1.0);

/// If an object is within this angle of deviation from the segment heading,
/// it is considered to be aligned with the segment.
/// This affects [`HoldKind::WhenAligned`] execution.
const NEGLIGIBLE_DEVIATION_ANGLE: Angle = Angle::from_degrees(5.0);

/// If the object is expected to diverge from the centerline beyond this distance,
/// the object will not accelerate beyond `MIN_POSITIVE_SPEED`.
const SLOW_TURN_OVERSHOOT_TOLERANCE: Length<f32> = Length::from_meters(3.0);

/// If the object exceeds this distance beyond the ideal turning point
/// even with maximum braking from now on,
/// it will consider this turn as missed.
const MISS_TURN_OVERSHOOT_TOLERANCE: Length<f32> = Length::from_meters(15.0);

/// Extra deceleration distance in case braking is less effective.
const DECEL_BUFFER: f32 = 1.2;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Update, maintain_dir_system.in_set(SystemSets::Aviate));
        app.add_systems(app::Update, target_path_system.in_set(SystemSets::Navigate));
        app.add_message::<TargetResolutionMessage>();
    }
}

#[derive(Component, Clone)]
pub struct Limits(pub store::TaxiLimits);

impl ops::Deref for Limits {
    type Target = store::TaxiLimits;
    fn deref(&self) -> &Self::Target { &self.0 }
}

fn maintain_dir_system(
    time: Res<Time<time::Virtual>>,
    object_query: Query<(Entity, &mut Object, &object::OnGround, &mut object::TaxiStatus, &Limits)>,
    segment_query: Query<&ground::Segment>,
    endpoint_query: Query<&ground::Endpoint>,
) {
    if time.is_paused() {
        return;
    }

    for (object_id, mut object, ground, mut taxi_status, limits) in object_query {
        let Some(segment) = segment_query.log_get(ground.segment) else { continue };
        let (other_endpoint_entity, target_endpoint_entity) = match ground.direction {
            ground::SegmentDirection::AlphaToBeta => (segment.alpha, segment.beta),
            ground::SegmentDirection::BetaToAlpha => (segment.beta, segment.alpha),
        };
        let Some(other_endpoint) = endpoint_query.log_get(other_endpoint_entity) else { continue };
        let Some(target_endpoint) = endpoint_query.log_get(target_endpoint_entity) else {
            continue;
        };

        maintain_dir_for_object(
            &time,
            object_id,
            &mut object,
            ground,
            &mut taxi_status,
            limits,
            [other_endpoint, target_endpoint].map(|e| e.position),
        );
    }
}

#[derive(Debug)]
enum TurnTowards {
    /// Turn towards the target endpoint.
    TargetEndpoint,
    /// Turn towards the start endpoint.
    StartEndpoint,
    /// Turn towards the segment centerline,
    /// targeting the pure pursuit position based on time delta.
    Centerline,
}

/// Update the heading and speed of `object`
/// to maintain the path along the segment from `other_endpoint` to `target_endpoint`.
///
/// - Always try to attain the target speed, even if the direction is not parallel.
/// - If rotating from the current heading to the target heading at the maximum turn rate
///   would still result in the trajectory intersecting the segment,
///   rotate towards the target heading at the maximum turn rate.
/// - Otherwise, rotate towards the direction orthogonal to and facing the segment
///   at maximum turn rate, such that the object faces the segment.
/// - Respect reversal of the target speed, i.e. if the target speed is negative,
///   all speeds and headings interpreted regarding the object should be negated.
///   (This process is only relevant for initial reading and final writing)
fn maintain_dir_for_object(
    time: &Time<time::Virtual>,
    object_id: Entity,
    object: &mut Object,
    ground: &object::OnGround,
    taxi_status: &mut object::TaxiStatus,
    limits: &Limits,
    [start_endpoint, target_endpoint]: [Position<Vec2>; 2],
) {
    let reversed = match ground.target_speed {
        object::OnGroundTargetSpeed::Exact(speed) => speed.is_negative(),
        object::OnGroundTargetSpeed::TakeoffRoll => false,
    };

    let current_speed =
        object.ground_speed.horizontal().project_onto_dir(taxi_status.heading.into_dir2());
    let current_corrected_speed = if reversed { -current_speed } else { current_speed };

    let current_heading =
        if reversed { taxi_status.heading.opposite() } else { taxi_status.heading };

    // In the following direction calculations, we ignore reversal by treating the backward
    // direction as the heading if reversal is desired.

    let target_heading = (target_endpoint - start_endpoint).heading();

    // Point-line distance from object.position to the line other_endpoint..target_endpoint.
    let closest_point =
        point_line_closest(object.position.horizontal(), start_endpoint, target_endpoint);
    // The vector from the object to the closest point on the line, orthogonal to the line.
    let object_to_line_ortho = closest_point - object.position.horizontal();

    let is_ahead_segment =
        (closest_point - target_endpoint).dot(start_endpoint - target_endpoint) <= 0.0;
    if is_ahead_segment {
        bevy::log::warn!("Object {object_id:?} overshot segment, need recovery");
        // TODO recover to the nearest segment
    }

    let is_behind_segment =
        (closest_point - start_endpoint).dot(target_endpoint - start_endpoint) <= 0.0;

    // Whether the current heading is facing towards the centerline.
    // Always true if the object is negligibly near the centerline.
    let is_towards_centerline = object_to_line_ortho.magnitude_cmp() < NEGLIGIBLE_DEVIATION_LENGTH
        || current_heading.into_dir2().dot(object_to_line_ortho.0) >= 0.0;

    // Amount of turn required to face the target heading, signed.
    let turn_towards_target_dir = current_heading.closest_distance(target_heading);

    // Estimated change in orthogonal displacement of the object
    // if we start turning towards the segment heading now, derived from
    // speed * int_0^{heading_deviation / turn_rate} sin(heading_deviation - turn_rate * t) dt.
    // This value always is positive as long as the ground speed is in the direction of the target
    // speed.
    let convergence_dist = current_corrected_speed
        * (1.0 - turn_towards_target_dir.cos())
        * limits.turn_rate.duration_per_radian();

    // Direct heading from object to target endpoint.
    let direct_heading = (target_endpoint - object.position.horizontal()).heading();

    let turn_towards = if object_to_line_ortho.magnitude_cmp() < NEGLIGIBLE_DEVIATION_LENGTH {
        // Do not overcorrect if the deviation from the centerline is negligible.
        TurnTowards::TargetEndpoint
    } else if direct_heading.is_between(current_heading, object_to_line_ortho.heading()) {
        // We have non-negligible deviation, facing away from the target and not moving towards the line,
        // need to turn towards the centerline first.
        TurnTowards::Centerline
    } else {
        // The current heading will end up somewhere on the centerline before the target endpoint,
        // but we might be able to converge towards the centerline earlier.

        if object_to_line_ortho.magnitude_cmp() < convergence_dist {
            // We will cross the line even if we immediately turn towards the target heading,
            // so we have to turn as soon as possible.
            //
            // This condition is always false if the object is not moving in the target speed
            // direction.
            TurnTowards::TargetEndpoint
        } else if is_behind_segment {
            // behind segment start, turn towards segment start directly since
            // there is no "centerline" to pursue.
            TurnTowards::StartEndpoint
        } else {
            // Otherwise, turn towards the centerline to align closer first.
            TurnTowards::Centerline
        }
    };

    let desired_heading = match turn_towards {
        TurnTowards::TargetEndpoint => target_heading,
        TurnTowards::StartEndpoint => (start_endpoint - object.position.horizontal()).heading(),
        TurnTowards::Centerline => {
            // If we are very close to the centerline,
            // we only want to turn to a point on the centerline
            // such that it would not overshoot the centerline
            // even after time.delta() at the current speed.
            //
            // sqrt_or_zero() sends the object to turn as much as possible towards the centerline
            // if it cannot overshoot it within time.delta().
            let mut forward_offset = ((current_corrected_speed * time.delta()).squared()
                - object_to_line_ortho.magnitude_squared())
            .sqrt_or_zero();
            if current_corrected_speed.is_negative() {
                // If the object is moving backwards,
                // the pure pursuit point should be backwards instead of forward;
                // otherwise the object would rotate opposite to the target.
                forward_offset = -forward_offset;
            }
            (object_to_line_ortho + forward_offset * target_heading).heading()
        }
    };
    let max_turn = limits.turn_rate * time.delta();
    let new_heading = current_heading.restricted_turn(desired_heading, max_turn);

    let should_brake = if current_corrected_speed.is_positive() {
        // desired is always positive anyway
        // cross centerline and diverge beyond threshold
        let crossing_diverge =
            object_to_line_ortho.magnitude_cmp() < convergence_dist - SLOW_TURN_OVERSHOOT_TOLERANCE;
        // diverging from centerline and will continue to diverge beyond threshold
        let continue_diverge = !is_towards_centerline
            && object_to_line_ortho.magnitude_cmp()
                > SLOW_TURN_OVERSHOOT_TOLERANCE - convergence_dist;

        // In either case, the object will cross the centerline significantly
        // before it can turn towards the target heading,
        // so slow down further.
        crossing_diverge || continue_diverge
    } else {
        false
    };

    let new_speed = match ground.target_speed {
        _ if should_brake => {
            limited_taxi_speed(reversed, MIN_POSITIVE_SPEED, current_speed, limits, time)
        }

        object::OnGroundTargetSpeed::Exact(target_speed) => {
            limited_taxi_speed(reversed, target_speed.abs(), current_speed, limits, time)
        }
        object::OnGroundTargetSpeed::TakeoffRoll => {
            let speed_change = limits.accel * time.delta();
            current_speed + speed_change
        }
    };

    let desired_velocity = new_speed * new_heading;
    object.ground_speed = desired_velocity.horizontally();
    taxi_status.heading = if reversed { new_heading.opposite() } else { new_heading };

    // TODO check for other objects on the segment.
    // Control speed such that the braking distance is shorter than the separation between objects.
}

fn limited_taxi_speed(
    reversed: bool,
    mut desired_speed: Speed<f32>,
    current_speed: Speed<f32>,
    limits: &Limits,
    time: &Time<time::Virtual>,
) -> Speed<f32> {
    if reversed {
        desired_speed = -desired_speed;
    }
    let speed_deviation = desired_speed - current_speed;

    let accel_limit = match (current_speed.is_positive(), speed_deviation.is_positive()) {
        (true, true) | (false, false) => limits.accel,
        (true, false) | (false, true) => limits.base_braking,
    } * time.delta();

    let speed_change = speed_deviation.clamp(-accel_limit, accel_limit);
    let unlimited_speed = current_speed + speed_change;
    unlimited_speed.clamp(limits.min_speed, limits.max_speed)
}

/// The next planned segment for an object.
///
/// If this component is absent, the object will hold at the end of the current segment.
#[derive(Component)]
pub struct Target {
    /// The step to execute.
    pub action:     TargetAction,
    /// Updated by the taxi plugin during the [`SystemSets::Navigate`]
    /// stage to indicate that this target has been resolved.
    ///
    /// `None` means the target is still pending.
    /// `Some` means the target has been resolved and the next target can be assigned.
    pub resolution: Option<TargetResolution>,
}

#[derive(Clone)]
pub enum TargetAction {
    /// Taxi along the runway with maximum acceleration.
    Takeoff { runway: Entity },
    /// Taxi to the first segment if available,
    /// otherwise to the next available segment.
    /// If all segments are unavailable, the object will hold at the end of the current segment.
    Taxi { options: WordVec<Entity, 1> },
    /// Hold at the end of the current segment.
    Hold { kind: HoldKind },
}

/// Whether to hold as soon as possible or until the end.
#[derive(Clone, Copy)]
pub enum HoldKind {
    /// Hold as long as the object is aligned with the segment,
    /// used for lining up before takeoff.
    WhenAligned,
    /// Hold before the end of the current segment,
    /// effectively holding short of the intersection.
    SegmentEnd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetResolution {
    /// The nth target is accepted.
    Completed(usize),
    /// All targets are inoperable, e.g. the object is too fast or wide to enter all targets.
    Inoperable,
}

#[derive(SystemParam)]
struct TargetPathParams<'w, 's> {
    segment_query:      Query<'w, 's, &'static ground::Segment>,
    endpoint_query:     Query<'w, 's, &'static ground::Endpoint>,
    resolve_msg_writer: MessageWriter<'w, TargetResolutionMessage>,
}

/// An event sent when the target resolution of an object changes.
#[derive(Message)]
pub struct TargetResolutionMessage {
    /// The object whose target resolution has changed.
    pub object: Entity,
}

/// Executes [`Target`] actions to determine if
/// the object should accelerate, decelerate or switch to another segment.
fn target_path_system(
    time: Res<Time<time::Virtual>>,
    object_query: Query<(
        Entity,
        &Object,
        &Limits,
        Option<&mut Target>,
        &mut object::OnGround,
        &object::TaxiStatus,
    )>,
    mut params: TargetPathParams<'_, '_>,
) {
    if time.is_paused() {
        return;
    }

    for (object_id, object, limits, mut target, mut ground, taxi_status) in object_query {
        params.update_target_path_once(
            object_id,
            object,
            limits,
            &mut ground,
            taxi_status,
            target.as_deref_mut(),
        );
    }
}

impl TargetPathParams<'_, '_> {
    fn update_target_path_once(
        &mut self,
        object_id: Entity,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        taxi_status: &object::TaxiStatus,
        target: Option<&mut Target>,
    ) {
        let (action, resolution_mut) = match target {
            Some(Target { action, resolution: resolution @ None }) => (&*action, Some(resolution)),
            None | Some(Target { action: _, resolution: Some(_) }) => {
                (&TargetAction::Hold { kind: HoldKind::SegmentEnd }, None)
            }
        };

        let resolution = match *action {
            TargetAction::Takeoff { runway: _ } => {
                ground.target_speed = object::OnGroundTargetSpeed::TakeoffRoll;
                None // no resolution from the taxi plugin
            }
            TargetAction::Taxi { ref options } => {
                self.action_taxi(object, limits, ground, taxi_status, options)
            }
            TargetAction::Hold { kind } => {
                self.action_hold_short(object, limits, ground, taxi_status, kind)
            }
        };
        if let Some(resolution_mut) = resolution_mut {
            if resolution.is_some() != resolution_mut.is_some() {
                self.resolve_msg_writer.write(TargetResolutionMessage { object: object_id });
            }
            *resolution_mut = resolution;
        }
    }

    /// Attempt to turn to one of the options,
    /// or hold before the end of the current segment if all are currently unavailable.
    ///
    /// `segment_options` must be a slice of segment entities.
    fn action_taxi(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        taxi_status: &object::TaxiStatus,
        segment_options: &[Entity],
    ) -> Option<TargetResolution> {
        if let Some(position) = segment_options.iter().position(|&target| target == ground.segment)
        {
            return Some(TargetResolution::Completed(position));
        }

        let current_segment = self.segment_query.log_get(ground.segment)?;

        let intersection_endpoint = current_segment.by_direction(ground.direction).1;
        for (option_index, &target_segment) in segment_options.iter().enumerate() {
            match self.turn_to_segment(
                object,
                limits,
                ground,
                taxi_status,
                intersection_endpoint,
                target_segment,
            )? {
                TurnResult::TooFast | TurnResult::TooNarrow | TurnResult::Occupied => {}
                TurnResult::Later => {
                    // We can turn to the target segment later,
                    // no need to fall through to the next target yet.
                    return None;
                }
                TurnResult::Completed => return Some(TargetResolution::Completed(option_index)),
            }
        }

        // All options are inoperable, just hold.
        self.hold_before_endpoint(
            object,
            limits,
            ground,
            self.endpoint_query.log_get(intersection_endpoint)?,
        );
        if object.ground_speed.magnitude_cmp() < NEGLIGIBLE_SPEED {
            Some(TargetResolution::Inoperable)
        } else {
            None
        }
    }

    /// Hold before the end of the current segment.
    fn action_hold_short(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        taxi_status: &object::TaxiStatus,
        kind: HoldKind,
    ) -> Option<TargetResolution> {
        let current_segment = self.segment_query.log_get(ground.segment)?;
        let (from_endpoint_id, to_endpoint_id) = current_segment.by_direction(ground.direction);
        let [from_endpoint, to_endpoint] =
            self.endpoint_query.log_get_many([from_endpoint_id, to_endpoint_id])?;

        match kind {
            HoldKind::WhenAligned => Self::hold_when_aligned(
                ground,
                object,
                taxi_status,
                from_endpoint.position,
                to_endpoint.position,
            ),
            HoldKind::SegmentEnd => self.hold_before_endpoint(object, limits, ground, to_endpoint),
        }
        if object.ground_speed.magnitude_cmp() < NEGLIGIBLE_SPEED {
            Some(TargetResolution::Completed(0))
        } else {
            None
        }
    }

    /// Attempt to turn to `next_segment_id` as the next segment.
    ///
    /// Returns `TooNarrow` if the segment is too narrow for the object.
    ///
    /// Attempt to decelerate such that the object is slow enough
    /// to turn to the next heading within the size of the intersection,
    /// i.e. the object starts turning upon entering intersection width
    /// and completely turns to the next heading upon exit.
    /// This speed must also be within the speed limit of the next segment.
    ///
    /// Returns `TooFast` if the braking distance to slow down to the required speed
    /// would exceed the overshoot tolerance beyond entering the intersection.
    ///
    /// Returns `Later` if the object may turn to the next segment later,
    /// but should not commence the turn yet.
    fn turn_to_segment(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        taxi_status: &object::TaxiStatus,
        intersect_endpoint: Entity,
        next_segment_id: Entity,
    ) -> Option<TurnResult> {
        let linear_speed = object.ground_speed.horizontal().magnitude_exact();

        let &ground::Endpoint { position: intersect_pos, .. } =
            self.endpoint_query.log_get(intersect_endpoint)?;

        let current_segment = self.segment_query.log_get(ground.segment)?;
        let next_segment = self.segment_query.log_get(next_segment_id)?;
        if next_segment.width < limits.width {
            return Some(TurnResult::TooNarrow);
        }

        let next_target_endpoint = try_log!(
            next_segment.other_endpoint(intersect_endpoint),
            expect "adjacent segment {next_segment_id:?} must back-reference endpoint {intersect_endpoint:?}"
            or return None
        );
        let &ground::Endpoint { position: next_target_pos, .. } =
            self.endpoint_query.log_get(next_target_endpoint)?;
        let next_segment_heading = (next_target_pos - intersect_pos).heading();
        let abs_turn = taxi_status.heading.closest_distance(next_segment_heading).abs();

        let intersection_width = try_log!(
            self.endpoint_width(intersect_endpoint),
            expect "endpoint {intersect_endpoint:?} adjacency list must not be empty"
            or return None
        );

        // Expect `intersection_width * 0.5 / turn_radius >= tan(abs_turn / 2)`
        // such that the turn fits exactly within the intersection circle.
        // Since `turn_speed = turn_radius * turn_rate`, we have
        // `turn_speed <= turn_rate * intersection_width * 0.5 / tan(abs_turn / 2)`.
        let max_turn_speed = (intersection_width * 0.5).radius_to_arc(limits.turn_rate)
            / (abs_turn * 0.5).acute_signed_tan().abs();

        let object_dist =
            object.position.horizontal().distance_exact(intersect_pos) - intersection_width;

        // We want to try to reduce below `max_turn_speed` by the time `object_dist` turns zero.
        // The required distance to reduce speed from `linear_speed` to `max_turn_speed`:
        if linear_speed > max_turn_speed {
            // max_turn_speed^2 = linear_speed^2 - 2 * limits.base_braking * decel_distance =>
            let decel_distance =
                (linear_speed.squared() - max_turn_speed.squared()) / (limits.base_braking * 2.0);
            if object_dist > decel_distance * DECEL_BUFFER {
                // We can continue at the current speed on the original segment
                // until decel_distance from the intersection threshold.
                ground.target_speed = object::OnGroundTargetSpeed::Exact(current_segment.max_speed);
                return Some(TurnResult::Later);
            }

            // How much extra distance behind the threshold before we are slow enough to turn?
            let deficit = decel_distance - object_dist;
            if deficit > MISS_TURN_OVERSHOOT_TOLERANCE {
                // Even if we start braking now, we are already past the intersection
                // by the time we are slow enough to turn,
                // so just skip this turn.
                return Some(TurnResult::TooFast);
            }

            // We are too fast to turn, so we have to reduce speed.
            // We still haven't accepted nor rejected this segment yet.
            ground.target_speed = object::OnGroundTargetSpeed::Exact(max_turn_speed);
            return Some(TurnResult::Later);
        }

        // We are slow enough to turn, but are we close enough to the intersection point yet?

        // Estimated distance required to turn from taxi_status.heading to next_segment_heading,
        // measured parallel to taxi_status.heading from the intersection center,
        // is given by turn_radius * tan(abs_turn / 2).
        // (Consider the triangle between the intersection point, the turning center and starting
        // point)
        let turn_radius = linear_speed * limits.turn_rate.duration_per_radian();
        let turn_distance = turn_radius * (abs_turn * 0.5).acute_signed_tan();

        if object_dist > turn_distance {
            // We can continue on the original segment until turn_distance from the intersection
            // point.
            ground.target_speed = object::OnGroundTargetSpeed::Exact(current_segment.max_speed);
            return Some(TurnResult::Later);
        }

        ground.segment = next_segment_id;
        ground.target_speed = object::OnGroundTargetSpeed::Exact(next_segment.max_speed);
        ground.direction =
            next_segment.direction_from(intersect_endpoint).expect("checked in other_endpoint");
        Some(TurnResult::Completed)
    }

    fn hold_when_aligned(
        ground: &mut object::OnGround,
        object: &Object,
        taxi_status: &object::TaxiStatus,
        from_position: Position<Vec2>,
        to_position: Position<Vec2>,
    ) {
        let segment_heading = Heading::from_vec2((to_position - from_position).0);

        let heading_deviation = taxi_status.heading.closest_distance(segment_heading).abs();
        if heading_deviation < NEGLIGIBLE_DEVIATION_ANGLE {
            let object_position = object.position.horizontal();
            let closest_on_segment =
                point_line_closest(object_position, from_position, to_position);
            if object_position.distance_cmp(closest_on_segment) < NEGLIGIBLE_DEVIATION_LENGTH {
                ground.target_speed = object::OnGroundTargetSpeed::Exact(Speed::ZERO);
            }
        }
    }

    /// Decelerate to stop before the width of the endpoint intersection.
    fn hold_before_endpoint(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        endpoint: &ground::Endpoint,
    ) {
        // Use endpoint width as the required distance from the endpoint.
        let intersection_width = endpoint
            .adjacency
            .iter()
            .filter_map(|&segment_id| {
                let segment = self.segment_query.log_get(segment_id)?;
                Some(segment.width)
            })
            .max_by_key(|width| OrderedFloat(width.0));
        let intersection_width =
            try_log_return!(intersection_width, expect "adjacency list must not be empty");

        let current_speed_squared = object.ground_speed.horizontal().magnitude_squared();
        // 0^2 = current_speed^2 - 2 * braking_decel * decel
        // => decel = current_speed^2 / (2 * braking_decel)
        let decel_distance = current_speed_squared * 0.5 / limits.base_braking;

        let distance_to_intersection = object.position.horizontal().distance_cmp(endpoint.position);

        if distance_to_intersection > decel_distance + intersection_width {
            // Intersection is far enough, no need to brake yet.
            return;
        }

        ground.target_speed = object::OnGroundTargetSpeed::Exact(Speed::ZERO);
    }

    fn endpoint_width(&self, endpoint: Entity) -> Option<Length<f32>> {
        let endpoint = self.endpoint_query.log_get(endpoint)?;
        let intersection_width = endpoint
            .adjacency
            .iter()
            .filter_map(|&segment_id| {
                let segment = self.segment_query.log_get(segment_id)?;
                Some(segment.width)
            })
            .max_by_key(|width| OrderedFloat(width.0));
        intersection_width.map(|width| width * 0.5)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnResult {
    /// Unable to turn because the object is too fast
    /// to complete turning within the endpoint width.
    TooFast,
    /// Unable to turn because the next segment is too narrow for the object.
    TooNarrow,
    /// Another object is blocking the intersection,
    /// within the feasible braking distance.
    Occupied,
    /// The object can turn to the next segment,
    /// but it is not yet close enough to the intersection point.
    Later,
    /// The object has successfully turned to the next segment.
    Completed,
}
