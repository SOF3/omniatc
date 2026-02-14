use math::{Accel, AccelRate, AngularAccel, AngularSpeed, Length, Speed};

#[must_use]
pub fn a359_taxi_limits() -> store::TaxiLimits {
    store::TaxiLimits {
        base_braking: Accel::from_knots_per_sec(3.0),
        accel:        Accel::from_knots_per_sec(5.0),
        max_speed:    Speed::from_knots(100.0),
        min_speed:    Speed::from_knots(-4.0),
        turn_rate:    AngularSpeed::from_degrees_per_sec(8.0),
        width:        Length::from_meters(50.0),
        half_length:  Length::from_meters(70.0),
    }
}

#[must_use]
pub fn a359_nav_limits() -> store::NavLimits {
    store::NavLimits {
        min_horiz_speed:   Speed::from_knots(120.),
        max_yaw_speed:     AngularSpeed::from_degrees_per_sec(3.),
        max_vert_accel:    Accel::from_fpm_per_sec(200.),
        exp_climb:         store::ClimbProfile {
            vert_rate: Speed::from_fpm(3000.),
            accel:     Accel::from_knots_per_sec(0.2),
            decel:     Accel::from_knots_per_sec(-1.8),
        },
        std_climb:         store::ClimbProfile {
            vert_rate: Speed::from_fpm(1500.),
            accel:     Accel::from_knots_per_sec(0.6),
            decel:     Accel::from_knots_per_sec(-1.4),
        },
        level:             store::ClimbProfile {
            vert_rate: Speed::from_fpm(0.),
            accel:     Accel::from_knots_per_sec(1.),
            decel:     Accel::from_knots_per_sec(-1.),
        },
        std_descent:       store::ClimbProfile {
            vert_rate: Speed::from_fpm(-1500.),
            accel:     Accel::from_knots_per_sec(1.4),
            decel:     Accel::from_knots_per_sec(-0.6),
        },
        exp_descent:       store::ClimbProfile {
            vert_rate: Speed::from_fpm(-3000.),
            accel:     Accel::from_knots_per_sec(1.8),
            decel:     Accel::from_knots_per_sec(-0.2),
        },
        weight:            1e5,
        accel_change_rate: AccelRate::from_knots_per_sec2(0.3),
        drag_coef:         3. / 500. / 500.,
        max_yaw_accel:     AngularAccel::from_degrees_per_sec2(1.),
        takeoff_speed:     Speed::from_knots(150.),
        short_final_dist:  Length::from_nm(4.),
        short_final_speed: Speed::from_knots(150.),
    }
}
