//! Controls ground object movement.

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::{Event, EventWriter};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res, SystemParam};
use bevy::math::Vec2;
use bevy::time::{self, Time};
use math::{point_line_closest, Accel, AngularSpeed, CanSqrt, Length, Position, Speed};
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use smallvec::SmallVec;

use super::object::Object;
use super::{ground, object, SystemSets};
use crate::{try_log, try_log_return, QueryTryLog};

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
const NEGLIGIBLE_DEVIATION: Length<f32> = Length::from_meters(20.0);

/// If the object is expected to diverge from the centerline beyond this distance,
/// the object will not accelerate beyond `MIN_POSITIVE_SPEED`.
const OVERSHOOT_TOLERANCE: Length<f32> = Length::from_meters(10.0);

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Update, maintain_dir.in_set(SystemSets::Aviate));
        app.add_systems(app::Update, target_path_system.in_set(SystemSets::Navigate));
        app.add_event::<TargetResolutionEvent>();
    }
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct Limits {
    /// Maximum acceleration on ground.
    pub accel:        Accel<f32>,
    /// Braking deceleration under optimal conditions.
    /// Always positive.
    pub base_braking: Accel<f32>,

    /// Maximum speed during taxi.
    pub max_speed: Speed<f32>,
    /// Fastest pushback/reversal speed.
    ///
    /// Should be negative if the object can reverse,
    /// zero otherwise.
    pub min_speed: Speed<f32>,

    /// Maximum absolute rotation speed during taxi. Always positive.
    pub turn_rate: AngularSpeed,

    /// Minimum width of segments this object can taxi on.
    ///
    /// For planes, this is the wingspan.
    /// For helicopters, this is the rotor diameter.
    pub width:       Length<f32>,
    /// The distance between two objects on the same segment
    /// must be at least the sum of their half-lengths.
    ///
    /// This value could include extra padding to represent safety distance.
    pub half_length: Length<f32>,
}

fn maintain_dir(
    time: Res<Time<time::Virtual>>,
    object_query: Query<(&mut Object, &mut object::OnGround, &Limits)>,
    segment_query: Query<&ground::Segment>,
    endpoint_query: Query<&ground::Endpoint>,
) {
    if time.is_paused() {
        return;
    }

    for (mut object, mut ground, limits) in object_query {
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
            &mut object,
            &mut ground,
            limits,
            other_endpoint.position,
            target_endpoint.position,
        );
    }
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
    object: &mut Object,
    ground: &mut object::OnGround,
    limits: &Limits,
    other_endpoint: Position<Vec2>,
    target_endpoint: Position<Vec2>,
) {
    enum TurnTowards {
        /// Turn towards the target endpoint.
        TargetEndpoint,
        /// Turn towards the segment centerline,
        /// targetting the pure pursuit position based on time delta.
        CenterLine,
    }

    let reversed = ground.target_speed.is_negative();

    let current_speed =
        object.ground_speed.horizontal().project_onto_dir(ground.heading.into_dir2());
    let current_corrected_speed = if reversed { -current_speed } else { current_speed };

    let current_heading = if reversed { ground.heading.opposite() } else { ground.heading };

    // In the following direction calculations, we ignore reversal by treating the backward
    // direction as the heading if reversal is desired.

    let target_heading = (target_endpoint - other_endpoint).heading();

    // Point-line distance from object.position to the line other_endpoint..target_endpoint.
    let closest_point =
        point_line_closest(object.position.horizontal(), other_endpoint, target_endpoint);
    // The vector from the object to the closest point on the line, orthogonal to the line.
    let object_to_line_ortho = closest_point - object.position.horizontal();

    let turn_towards_target_dir = current_heading.closest_distance(target_heading);

    // Estimated change in orthogonal displacement of the object if we start turning towards
    // the desired eventual heading now, derived from
    // speed * int_0^{heading_deviation / turn_rate} sin(heading_deviation - turn_rate * t) dt.
    // This value always is positive as long as the ground speed is in the direction of the target
    // speed.
    let convergence_dist = current_corrected_speed
        * (1.0 - turn_towards_target_dir.cos())
        * limits.turn_rate.duration_per_radian();

    // Direct heading from object to target endpoint.
    let direct_heading = (target_endpoint - object.position.horizontal()).heading();

    let turn_towards = if object_to_line_ortho.magnitude_cmp() < NEGLIGIBLE_DEVIATION {
        TurnTowards::TargetEndpoint
    } else if direct_heading.is_between(current_heading, object_to_line_ortho.heading()) {
        // We are facing away from the target and not moving towards the line
        TurnTowards::CenterLine
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
        } else {
            // Otherwise, turn towards the centerline to align closer first.
            TurnTowards::CenterLine
        }
    };

    let desired_heading = match turn_towards {
        TurnTowards::TargetEndpoint => target_heading,
        TurnTowards::CenterLine => {
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

    let mut desired_corrected_speed = ground.target_speed.abs();
    if current_corrected_speed.is_positive() {
        // desired is always positive anyway
        // cross centerline and diverge beyond threshold
        let crossing_diverge =
            object_to_line_ortho.magnitude_cmp() < convergence_dist - OVERSHOOT_TOLERANCE;
        // diverging from centerline and will continue to diverge beyond threshold
        let continue_diverge =
            object_to_line_ortho.magnitude_cmp() > OVERSHOOT_TOLERANCE - convergence_dist;

        if crossing_diverge || continue_diverge {
            // In either case, the object will cross the centerline significantly
            // before it can turn towards the target heading,
            // so slow down further.
            desired_corrected_speed = MIN_POSITIVE_SPEED;
        }
    }
    let desired_speed = if reversed { -desired_corrected_speed } else { desired_corrected_speed };
    let speed_deviation = desired_speed - current_speed;
    let accel_limit = match (current_speed.is_positive(), speed_deviation.is_positive()) {
        (true, true) | (false, false) => limits.accel,
        (true, false) | (false, true) => limits.base_braking,
    } * time.delta();
    let new_speed = (current_speed + speed_deviation.clamp(-accel_limit, accel_limit))
        .clamp(limits.min_speed, limits.max_speed);

    let desired_velocity = new_speed * new_heading;
    object.ground_speed = desired_velocity.horizontally();
    ground.heading = if reversed { new_heading.opposite() } else { new_heading };

    // TODO check for other objects on the segment.
    // Control speed such that the braking distance is shorter than the separation between objects.
}

/// The next planned segment for an object.
///
/// If this component is absent, the object will hold at the end of the current segment.
#[derive(Component)]
pub struct Target {
    /// The step to execute.
    pub action:     TargetAction,
    /// Updated by the taxi plugin during the [`SystemSets::Navigate`][SystemSets::Navigate]
    /// stage to indicate that this target has been resolved.
    ///
    /// `None` means the target is still pending.
    /// `Some` means the target has been resolved and the next target can be assigned.
    pub resolution: Option<TargetResolution>,
}

#[derive(Clone)]
pub enum TargetAction {
    /// Taxi to the first segment if available,
    /// otherwise to the next available segment.
    /// If all segments are unavailable, the object will hold at the end of the current segment.
    Taxi { options: SmallVec<[Entity; 2]> },
    /// Hold at the end of the current segment.
    HoldShort,
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
    segment_query:        Query<'w, 's, &'static ground::Segment>,
    endpoint_query:       Query<'w, 's, &'static ground::Endpoint>,
    resolve_event_writer: EventWriter<'w, TargetResolutionEvent>,
}

/// An event sent when the target resolution of an object changes.
#[derive(Event)]
pub struct TargetResolutionEvent {
    /// The object whose target resolution has changed.
    pub object: Entity,
}

fn target_path_system(
    object_query: Query<(Entity, &Object, &Limits, Option<&mut Target>, &mut object::OnGround)>,
    mut params: TargetPathParams<'_, '_>,
) {
    for (object_id, object, limits, mut target, mut ground) in object_query {
        params.update_target_path_once(
            object_id,
            object,
            limits,
            &mut ground,
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
        target: Option<&mut Target>,
    ) {
        let (action, resolution_mut) = match target {
            Some(Target { action, resolution: resolution @ None }) => (&*action, Some(resolution)),
            None | Some(Target { action: _, resolution: Some(_) }) => {
                (&TargetAction::HoldShort, None)
            }
        };

        let resolution = match action {
            TargetAction::Taxi { options } => self.action_taxi(object, limits, ground, options),
            TargetAction::HoldShort => self.action_hold_short(object, limits, ground),
        };
        if let Some(resolution_mut) = resolution_mut {
            if resolution.is_some() != resolution_mut.is_some() {
                self.resolve_event_writer.write(TargetResolutionEvent { object: object_id });
            }
            *resolution_mut = resolution;
        }
    }

    fn action_taxi(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        options: &[Entity],
    ) -> Option<TargetResolution> {
        if let Some(position) = options.iter().position(|&target| target == ground.segment) {
            return Some(TargetResolution::Completed(position));
        }

        let current_segment = self.segment_query.log_get(ground.segment)?;

        let intersection_endpoint = current_segment.by_direction(ground.direction).1;
        for (option_index, &target_segment) in options.iter().enumerate() {
            #[expect(clippy::match_same_arms)] // different explanations
            match self.turn_to_segment(
                object,
                limits,
                ground,
                intersection_endpoint,
                target_segment,
            ) {
                TurnResult::Error => {
                    // Error has been logged, just return.
                    return None;
                }
                TurnResult::TooFast | TurnResult::TooNarrow => {}
                TurnResult::Later => {
                    // We can turn to the target segment later,
                    // no need to fall through to the next target yet.
                    return None;
                }
                TurnResult::Completed => return Some(TargetResolution::Completed(option_index)),
            }
        }

        // All options are inoperable, just hold.
        self.hold_before_endpoint(object, limits, ground, intersection_endpoint);
        if object.ground_speed.magnitude_cmp() < NEGLIGIBLE_SPEED {
            Some(TargetResolution::Inoperable)
        } else {
            None
        }
    }

    fn action_hold_short(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
    ) -> Option<TargetResolution> {
        let current_segment = self.segment_query.log_get(ground.segment)?;

        self.hold_before_endpoint(
            object,
            limits,
            ground,
            current_segment.by_direction(ground.direction).1,
        );
        if object.ground_speed.magnitude_cmp() < NEGLIGIBLE_SPEED {
            Some(TargetResolution::Completed(0))
        } else {
            None
        }
    }

    fn turn_to_segment(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        intersect_endpoint: Entity,
        next_segment_id: Entity,
    ) -> TurnResult {
        let linear_speed = object.ground_speed.horizontal().magnitude_exact();

        let Some(&ground::Endpoint { position: intersect_pos, .. }) =
            self.endpoint_query.log_get(intersect_endpoint)
        else {
            return TurnResult::Error;
        };

        let Some(current_segment) = self.segment_query.log_get(ground.segment) else {
            return TurnResult::Error;
        };
        let Some(next_segment) = self.segment_query.log_get(next_segment_id) else {
            return TurnResult::Error;
        };
        if next_segment.width < limits.width {
            return TurnResult::TooNarrow;
        }

        let next_target_endpoint = try_log!(
            next_segment.other_endpoint(intersect_endpoint),
            expect "adjacent segment {next_segment_id:?} must back-reference endpoint {intersect_endpoint:?}"
            or return TurnResult::Error
        );
        let Some(&ground::Endpoint { position: next_target_pos, .. }) =
            self.endpoint_query.log_get(next_target_endpoint)
        else {
            return TurnResult::Error;
        };
        let next_segment_heading = (next_target_pos - intersect_pos).heading();
        let abs_turn = ground.heading.closest_distance(next_segment_heading).abs();

        let intersection_width = try_log!(
            self.endpoint_width(intersect_endpoint),
            expect "endpoint {intersect_endpoint:?} adjacency list must not be empty"
            or return TurnResult::Error
        );

        // linear_speed = turn_radius * limits.turn_rate
        // turn_sagitta = turn_radius * (1.0 - abs_turn.cos())
        // thus, the max speed for turn_sagitta <= intersection_width is
        // linear_speed / limits.turn_rate * (1.0 - abs_turn.cos()) <= intersection_width
        // i.e. linear_speed <= intersection_width * limits.turn_rate / (1.0 - abs_turn.cos())
        let max_turn_speed =
            intersection_width.radius_to_arc(limits.turn_rate) / (1.0 - abs_turn.cos());

        let object_dist =
            object.position.horizontal().distance_exact(intersect_pos) - intersection_width;

        // We want to try to reduce below `max_turn_speed` by the time `object_dist` turns zero.
        // The required distance to reduce speed from `linear_speed` to `max_turn_speed`:
        if linear_speed > max_turn_speed {
            // max_turn_speed^2 = linear_speed^2 - 2 * limits.base_braking * decel_distance =>
            let decel_distance =
                (linear_speed.squared() - max_turn_speed.squared()) / (limits.base_braking * 2.0);
            if object_dist > decel_distance {
                // We can continue at the current speed on the original segment
                // until decel_distance from the intersection threshold.
                ground.target_speed = current_segment.max_speed;
                return TurnResult::Later;
            }

            // How much extra distance behind the threshold before we are slow enough to turn?
            let deficit = decel_distance - object_dist;
            if deficit > intersection_width {
                // Even if we start braking now, we are already past the intersection
                // by the time we are slow enough to turn,
                // so just skip this turn.
                return TurnResult::TooFast;
            }

            // We are too fast to turn, so we have to reduce speed.
            // We still haven't accepted nor rejected this segment yet.
            ground.target_speed = max_turn_speed;
            return TurnResult::Later;
        }

        // We are slow enough to turn, but are we close enough to the intersection point yet?

        // Estimated distance required to turn from ground.heading to next_segment_heading,
        // measured parallel to ground.heading from the intersection center,
        // is given by turn_radius * tan(abs_turn / 2).
        // (Consider the triangle between the intersection point, the turning center and starting
        // point)
        let turn_radius = linear_speed * limits.turn_rate.duration_per_radian();
        let turn_distance = turn_radius * (abs_turn * 0.5).acute_signed_tan();

        if object_dist > turn_distance {
            // We can continue on the original segment until turn_distance from the intersection
            // point.
            ground.target_speed = current_segment.max_speed;
            return TurnResult::Later;
        }

        ground.segment = next_segment_id;
        ground.target_speed = next_segment.max_speed;
        ground.direction =
            next_segment.direction_from(intersect_endpoint).expect("checked in other_endpoint");
        TurnResult::Completed
    }

    fn hold_before_endpoint(
        &self,
        object: &Object,
        limits: &Limits,
        ground: &mut object::OnGround,
        endpoint: Entity,
    ) {
        let Some(endpoint) = self.endpoint_query.log_get(endpoint) else { return };

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

        ground.target_speed = Speed::ZERO;
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
    /// A [`try_log`]ed error occurred, just return.
    Error,
    /// Unable to turn because the object is too fast
    /// to complete turning within the endpoint width.
    TooFast,
    /// Unable to turn because the next segment is too narrow for the object.
    TooNarrow,
    /// The object can turn to the next segment,
    /// but it is not yet close enough to the intersection point.
    Later,
    /// The object has successfully turned to the next segment.
    Completed,
}
