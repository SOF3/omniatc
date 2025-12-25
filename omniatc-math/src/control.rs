use std::time::Duration;

use crate::{Accel, CanSqrt, Length, Speed, Squared};

#[cfg(test)]
mod tests;

pub struct LinearSpeedSetpoint {
    /// Position of the object relative to the target.
    pub deviation: Length<f32>,

    /// Current rate of deviation change.
    pub current_speed: Speed<f32>,

    /// Maximum increase in speed per time when the speed is positive.
    /// Should be a positive value for the deviation to converge.
    pub max_forward_accel: Accel<f32>,
    /// Maximum decrease in speed per time when speed is positive.
    /// Should be a positive value for the deviation to converge.
    pub max_forward_brake: Accel<f32>,

    /// Maximum increase in absolute speed per time when the speed is negative.
    /// Should be a positive value for the deviation to converge.
    pub max_backward_accel: Accel<f32>,
    /// Maximum decrease in absolute speed per time when speed is negative.
    /// Should be a positive value for the deviation to converge.
    pub max_backward_brake: Accel<f32>,

    /// Maximum possible speed.
    ///
    /// The return value will never exceed this speed.
    pub max_speed: Speed<f32>,
    /// Most negative possible speed.
    ///
    /// The return value will never be more negative than this speed.
    pub min_speed: Speed<f32>,

    /// Duration for which the result is to be scaled for.
    ///
    /// This parameter ensures that the returned speed
    /// will not unnecessarily overshoot the target because of large time steps.
    pub dt: Duration,
}

/// Computes the speed setpoint for a moving object
/// such that `deviation` eventually becomes zero.
#[must_use]
pub fn linear_speed_setpoint(args: LinearSpeedSetpoint) -> Speed<f32> {
    let desired_displace = -args.deviation;

    if args.current_speed.is_positive() {
        // Moving towards positive direction.
        let brake_distance = args.current_speed.squared() / (args.max_forward_brake * 2.0);

        if desired_displace < brake_distance {
            // overshooting if desired disp is negative or closer than brake distance
            // brake_and_negate returns negative speed,
            // and we want to brake and accelerate backward, so the sign is correct.
            brake_and_negate(
                args.current_speed,
                args.max_forward_brake,
                args.max_backward_accel,
                args.max_backward_brake,
                desired_displace,
                args.dt,
                -args.min_speed,
            )
        } else {
            // accelerate_and_brake returns positive speed, which is what we want.
            accelerate_and_brake(
                args.current_speed,
                args.max_forward_accel,
                args.max_forward_brake,
                desired_displace,
                args.dt,
                args.max_speed,
            )
        }
    } else {
        // Moving towards negative direction.
        let brake_distance = args.current_speed.squared() / (args.max_backward_brake * 2.0);

        if desired_displace > -brake_distance {
            // overshooting if desired disp is positive (forward) or closer than brake distance
            // brake_and_negate returns negative speed, but we want to brake and accelerate forward
            -brake_and_negate(
                -args.current_speed,
                args.max_backward_brake,
                args.max_forward_accel,
                args.max_forward_brake,
                -desired_displace,
                args.dt,
                args.max_speed,
            )
        } else {
            // accelerate_and_brake returns positive speed, but we will accelerate backwards.
            -accelerate_and_brake(
                -args.current_speed,
                args.max_backward_accel,
                args.max_backward_brake,
                -desired_displace,
                args.dt,
                -args.min_speed,
            )
        }
    }
}

/// Given that the desired displacement must be reached by overshooting first,
/// reversing to some speed and braking again,
/// compute the most negative speed during the reverse.
///
/// All parameters are expected to be positive,
/// except `desired_displace`, which may be either positive or negative.
///
/// Returns a negative value if all non-displacement parameters are positive.
fn brake_and_negate(
    initial_speed: Speed<f32>,
    max_initial_brake: Accel<f32>,
    max_backward_accel: Accel<f32>,
    max_backward_brake: Accel<f32>,
    desired_displace: Length<f32>,
    dt: Duration,
    backward_speed_limit: Speed<f32>,
) -> Speed<f32> {
    // s_initial_brake = u^2 / (2 * a_initial_brake)
    // s_backward_accel = v^2 / (2 * a_backward_accel)
    // s_backward_brake = v^2 / (2 * a_backward_brake)
    // We want to find v such that s_initial_brake + s_backward_accel + s_backward_brake =
    // desired_displace,
    // so v^2 = 2 * (a_backward_accel * a_backward_brake) / (a_backward_accel + a_backward_brake)
    // * (u^2 / (2 * a_initial_brake) - desired_displace)

    let speed_squared = Squared::<Speed<f32>>::new(
        2.0 * (max_backward_accel.0 * max_backward_brake.0)
            / (max_backward_accel + max_backward_brake).0
            * (initial_speed.squared().0 / (2.0 * max_initial_brake.0) - desired_displace.0),
    );
    // u^2 / 2a is theoretically greater than desired_displace since we would be overshooting.
    let desired_backward_speed = speed_squared.sqrt_or_zero();
    let mut max_backward_speed = desired_backward_speed.min(backward_speed_limit);

    // must not overshoot in one time step, i.e. result * dt <= desired_displace
    if dt != Duration::ZERO {
        let time_limited_speed = desired_displace.abs() / dt;
        max_backward_speed = max_backward_speed.min(time_limited_speed);
    }
    -max_backward_speed
}

/// Find the max speed to accelerate to before braking at the same rate.
///
/// All parameters should be positive. Returns a positive speed.
fn accelerate_and_brake(
    initial_speed: Speed<f32>,
    max_accel: Accel<f32>,
    max_decel: Accel<f32>,
    desired_displace: Length<f32>,
    dt: Duration,
    speed_limit: Speed<f32>,
) -> Speed<f32> {
    // s_accel = (v^2 - u^2) / (2 * a_accel)
    // s_brake = v^2 / (2 * a_brake)
    // We want to find v such that s_accel + s_brake = desired_displace,
    // so v^2 = (2 accel * decel * s + decel * u^2) / (accel + decel)
    let speed_squared = Squared::<Speed<f32>>::new(
        (max_accel.0 * max_decel.0 * desired_displace.0 * 2.0
            + max_decel.0 * initial_speed.squared().0)
            / (max_accel + max_decel).0,
    );
    // speed_squared is theoretically positive since all inputs are of the same sign.
    let desired_max_speed = speed_squared.sqrt_or_zero();
    let mut max_speed = desired_max_speed.min(speed_limit);

    // must not overshoot in one time step, i.e. result * dt <= desired_displace
    if dt != Duration::ZERO {
        let time_limited_speed = desired_displace.abs() / dt;
        max_speed = max_speed.min(time_limited_speed);
    }
    max_speed
}
