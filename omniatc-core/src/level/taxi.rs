//! Controls ground object movement.


use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Query, Res};
use bevy::math::Vec2;
use bevy::time::{self, Time};
use serde::{Deserialize, Serialize};

use super::object::Object;
use super::{ground, object, SystemSets};
use crate::math::point_line_closest;
use crate::units::{
    Accel, AngularSpeed, Position,
};
use crate::try_log;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Update, maintain_dir.in_set(SystemSets::Aviate));
    }
}

#[derive(Component)]
pub struct Target {
    pub path: Vec<ground::SegmentLabel>,
}

#[derive(Component, Clone, Serialize, Deserialize)]
pub struct Limits {
    /// Maximum acceleration on ground.
    pub accel:        Accel<f32>,
    /// Braking deceleration under optimal conditions.
    /// Always positive.
    pub base_braking: Accel<f32>,

    /// Maximum absolute rotation speed during taxi. Always positive.
    pub turn_rate: AngularSpeed<f32>,
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
        let segment = try_log!(segment_query.get(ground.segment), expect "object::OnGround must reference valid segment {:?}" (ground.segment) or continue);
        let (other_endpoint_entity, target_endpoint_entity) = match ground.direction {
            ground::SegmentDirection::AlphaToBeta => (segment.alpha, segment.beta),
            ground::SegmentDirection::BetaToAlpha => (segment.beta, segment.alpha),
        };
        let other_endpoint = try_log!(
            endpoint_query.get(other_endpoint_entity),
            expect "ground::Segment must reference valid endpoint {other_endpoint_entity:?}" or continue
        );
        let target_endpoint = try_log!(
            endpoint_query.get(target_endpoint_entity),
            expect "ground::Segment must reference valid endpoint {target_endpoint_entity:?}" or continue
        );

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
    // First, maintain speed regardless of direction.
    let desired_eventual_speed = ground.target_speed;
    let current_speed =
        object.ground_speed.horizontal().project_onto_dir(ground.heading.into_dir2());
    let speed_deviation = desired_eventual_speed - current_speed;
    let accel_limit = match (current_speed.is_positive(), speed_deviation.is_positive()) {
        (true, true) | (false, false) => limits.accel,
        (true, false) | (false, true) => limits.base_braking,
    } * time.delta();
    let new_speed = current_speed + speed_deviation.clamp(-accel_limit, accel_limit);

    // new_speed is the magnitude of the finaly ground speed.
    // However we first consider `speed` used by direction calculation,
    // which reverses the speed if the target speed is negative.

    let reversed = ground.target_speed.is_negative();
    let speed = if reversed { -new_speed } else { new_speed };
    let current_heading = if reversed { ground.heading.opposite() } else { ground.heading };

    // In the following direction calculations, we ignore reversal by treating the backward
    // direction as the heading if reversal is desired.

    let target_heading = (target_endpoint - other_endpoint).heading();

    // Point-line distance from object.position to the line other_endpoint..target_endpoint.
    let closest_point =
        point_line_closest(object.position.horizontal(), other_endpoint, target_endpoint);
    // The vector from the object to the closest point on the line, orthogonal to the line.
    let object_to_line_ortho = closest_point - object.position.horizontal();
    let object_to_line_ortho_heading = object_to_line_ortho.heading();

    let turn_towards_target_dir = current_heading.closest_distance(target_heading);

    // Estimated change in orthogonal displacement of the object if we start turning towards
    // the desired eventual heading now, derived from
    // speed * int_0^{heading_deviation / turn_rate} sin(heading_deviation - turn_rate * t) dt.
    let convergence_dist = current_speed
        * (1.0 - turn_towards_target_dir.cos())
        * limits.turn_rate.duration_per_radian();

    let direct_heading = (target_endpoint - object.position.horizontal()).heading();

    let should_turn_towards_line =
        if direct_heading.is_between(current_heading, object_to_line_ortho_heading) {
            // We are facing away from the target and not moving towards the line
            true
        } else if object_to_line_ortho.magnitude_cmp() < convergence_dist {
            // We will cross the line even if we immediately turn towards the target heading,
            // so we have to turn as soon as possible.
            false
        } else {
            // Otherwise, turn towards the line to align closer first.
            true
        };

    let desired_heading = if should_turn_towards_line {
        // Turn towards the direction orthogonal to the line, facing the segment.
        object_to_line_ortho_heading
    } else {
        // Turn towards the target heading.
        target_heading
    };
    let max_turn = limits.turn_rate * time.delta();
    let new_heading = current_heading.restricted_turn(desired_heading, max_turn);

    let desired_velocity = speed * new_heading;
    object.ground_speed = desired_velocity.horizontally();
    ground.heading = if reversed { new_heading.opposite() } else { new_heading };
}
