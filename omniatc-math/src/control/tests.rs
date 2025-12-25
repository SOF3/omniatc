use std::time::Duration;

use crate::{Accel, Length, LinearSpeedSetpoint, Speed, linear_speed_setpoint};

#[test]
fn test_linear_speed_identity() {
    linear_speed_setpoint(LinearSpeedSetpoint {
        current_speed:      Speed::from_knots(0.0),
        deviation:          Length::from_nm(0.0),
        max_forward_accel:  Accel::from_knots_per_sec(10.0),
        max_forward_brake:  Accel::from_knots_per_sec(10.0),
        max_backward_accel: Accel::from_knots_per_sec(10.0),
        max_backward_brake: Accel::from_knots_per_sec(10.0),
        min_speed:          Speed::from_knots(-5.0),
        max_speed:          Speed::from_knots(5.0),
        dt:                 Duration::from_secs(1),
    })
    .assert_approx(Speed::ZERO, Speed::from_knots(1.0))
    .unwrap();
}

#[test]
fn test_linear_speed_accel_forward() {
    linear_speed_setpoint(LinearSpeedSetpoint {
        current_speed:      Speed::from_knots(0.0),
        deviation:          Length::from_nm(-5.0),
        max_forward_accel:  Accel::from_knots_per_sec(10.0),
        max_forward_brake:  Accel::from_knots_per_sec(10.0),
        max_backward_accel: Accel::from_knots_per_sec(10.0),
        max_backward_brake: Accel::from_knots_per_sec(10.0),
        min_speed:          Speed::from_knots(-5.0),
        max_speed:          Speed::from_knots(5.0),
        dt:                 Duration::from_secs(1),
    })
    .assert_approx(Speed::from_knots(5.0), Speed::from_knots(1.0))
    .unwrap();
}

#[test]
fn test_linear_speed_accel_backward() {
    linear_speed_setpoint(LinearSpeedSetpoint {
        current_speed:      Speed::from_knots(0.0),
        deviation:          Length::from_nm(5.0),
        max_forward_accel:  Accel::from_knots_per_sec(10.0),
        max_forward_brake:  Accel::from_knots_per_sec(10.0),
        max_backward_accel: Accel::from_knots_per_sec(10.0),
        max_backward_brake: Accel::from_knots_per_sec(10.0),
        min_speed:          Speed::from_knots(-5.0),
        max_speed:          Speed::from_knots(5.0),
        dt:                 Duration::from_secs(1),
    })
    .assert_approx(Speed::from_knots(-5.0), Speed::from_knots(1.0))
    .unwrap();
}

#[test]
fn test_linear_speed_continue_accel_forward() {
    linear_speed_setpoint(LinearSpeedSetpoint {
        current_speed:      Speed::from_knots(1.0),
        deviation:          Length::from_nm(-5.0),
        max_forward_accel:  Accel::from_knots_per_sec(10.0),
        max_forward_brake:  Accel::from_knots_per_sec(10.0),
        max_backward_accel: Accel::from_knots_per_sec(10.0),
        max_backward_brake: Accel::from_knots_per_sec(10.0),
        min_speed:          Speed::from_knots(-5.0),
        max_speed:          Speed::from_knots(5.0),
        dt:                 Duration::from_secs(1),
    })
    .assert_approx(Speed::from_knots(5.0), Speed::from_knots(1.0))
    .unwrap();
}

#[test]
fn test_linear_speed_continue_accel_backward() {
    linear_speed_setpoint(LinearSpeedSetpoint {
        current_speed:      Speed::from_knots(-1.0),
        deviation:          Length::from_nm(5.0),
        max_forward_accel:  Accel::from_knots_per_sec(10.0),
        max_forward_brake:  Accel::from_knots_per_sec(10.0),
        max_backward_accel: Accel::from_knots_per_sec(10.0),
        max_backward_brake: Accel::from_knots_per_sec(10.0),
        min_speed:          Speed::from_knots(-5.0),
        max_speed:          Speed::from_knots(5.0),
        dt:                 Duration::from_secs(1),
    })
    .assert_approx(Speed::from_knots(-5.0), Speed::from_knots(1.0))
    .unwrap();
}

#[test]
fn test_linear_speed_continue_brake_forward() {
    linear_speed_setpoint(LinearSpeedSetpoint {
        current_speed:      Speed::from_knots(100.0),
        deviation:          Length::from_nm(-5.0),
        max_forward_accel:  Accel::from_knots_per_sec(10.0),
        max_forward_brake:  Accel::from_knots_per_sec(10.0),
        max_backward_accel: Accel::from_knots_per_sec(10.0),
        max_backward_brake: Accel::from_knots_per_sec(10.0),
        min_speed:          Speed::from_knots(-5.0),
        max_speed:          Speed::from_knots(5.0),
        dt:                 Duration::from_secs(1),
    })
    .assert_approx(Speed::from_knots(5.0), Speed::from_knots(1.0))
    .unwrap();
}

#[test]
fn test_linear_speed_continue_brake_backward() {
    linear_speed_setpoint(LinearSpeedSetpoint {
        current_speed:      Speed::from_knots(-100.0),
        deviation:          Length::from_nm(5.0),
        max_forward_accel:  Accel::from_knots_per_sec(10.0),
        max_forward_brake:  Accel::from_knots_per_sec(10.0),
        max_backward_accel: Accel::from_knots_per_sec(10.0),
        max_backward_brake: Accel::from_knots_per_sec(10.0),
        min_speed:          Speed::from_knots(-5.0),
        max_speed:          Speed::from_knots(5.0),
        dt:                 Duration::from_secs(1),
    })
    .assert_approx(Speed::from_knots(-5.0), Speed::from_knots(1.0))
    .unwrap();
}
