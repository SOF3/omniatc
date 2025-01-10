use std::time::Duration;

use omniatc_core::level::{nav, plane};
use omniatc_core::store;
use omniatc_core::units::{
    Accel, AccelRate, Angle, AngularAccel, AngularSpeed, Distance, Heading, Position, Speed,
};

pub fn default_plane_limits() -> plane::Limits {
    plane::Limits {
        max_vert_accel:    Accel::from_fpm_per_sec(1.),
        exp_climb:         plane::ClimbProfile {
            vert_rate: Speed::from_knots(30.),
            accel:     Accel::from_knots_per_sec(0.2),
            decel:     Accel::from_knots_per_sec(-1.8),
        },
        std_climb:         plane::ClimbProfile {
            vert_rate: Speed::from_knots(15.),
            accel:     Accel::from_knots_per_sec(0.6),
            decel:     Accel::from_knots_per_sec(-1.4),
        },
        level:             plane::ClimbProfile {
            vert_rate: Speed::from_knots(0.),
            accel:     Accel::from_knots_per_sec(1.),
            decel:     Accel::from_knots_per_sec(-1.),
        },
        exp_descent:       plane::ClimbProfile {
            vert_rate: Speed::from_knots(-15.),
            accel:     Accel::from_knots_per_sec(1.4),
            decel:     Accel::from_knots_per_sec(-0.6),
        },
        std_descent:       plane::ClimbProfile {
            vert_rate: Speed::from_knots(-30.),
            accel:     Accel::from_knots_per_sec(1.8),
            decel:     Accel::from_knots_per_sec(-0.2),
        },
        drag_coef:         3. / 500. / 500.,
        accel_change_rate: AccelRate::from_knots_per_sec2(0.3),
        max_yaw_accel:     AngularAccel::from_degrees_per_sec2(0.3),
    }
}

pub fn default_nav_limits() -> nav::Limits {
    nav::Limits {
        min_horiz_speed: Speed::from_knots(120.),
        max_yaw_speed:   AngularSpeed::from_degrees_per_sec(3.),
    }
}

/// A simple map featuring different mechanisms for testing.
pub fn file() -> store::File {
    store::File {
        ui:    store::Ui {
            camera: store::Camera {
                center:       Position::from_origin_nm(0., 0.),
                up:           Heading::NORTH,
                scale_axis:   store::AxisDirection::X,
                scale_length: Distance::from_nm(50.),
            },
        },
        level: store::Level {
            environment: store::Environment {
                heightmap:  store::HeatMap2 {
                    aligned: store::AlignedHeatMap2::constant(Position::from_amsl_feet(0.)),
                    sparse:  store::SparseHeatMap2 { functions: vec![] },
                },
                visibility: store::HeatMap2 {
                    aligned: store::AlignedHeatMap2::constant(Distance::from_nm(1000.)),
                    sparse:  store::SparseHeatMap2 { functions: vec![] },
                },
                wind:       vec![store::Wind {
                    start:        Position::from_origin_nm(-1000., -1000.),
                    end:          Position::from_origin_nm(1000., 1000.),
                    top:          Position::from_amsl_feet(40000.),
                    bottom:       Position::from_amsl_feet(40000.),
                    top_speed:    Speed::from_knots(15.).with_heading(Heading::from_degrees(300.)),
                    bottom_speed: Speed::from_knots(15.).with_heading(Heading::from_degrees(300.)),
                }],
            },
            aerodromes:  vec![store::Aerodrome {
                name:      "MAIN".into(),
                full_name: "Demo Main Airport".into(),
                runways:   vec![store::Runway {
                    name:                       "18".into(),
                    elevation:                  Position::from_amsl_feet(0.),
                    touchdown_position:         Position::from_origin_nm(0., 0.),
                    heading:                    Heading::SOUTH,
                    landing_distance_available: Distance::from_meters(3000.),
                    touchdown_displacement:     Distance::from_meters(160.),
                    glide_angle:                Angle::from_degrees(3.),
                    width:                      Distance::from_feet(60.),
                    max_visual_distance:        Distance::from_nm(3.),
                    ils:                        Some(store::Localizer {
                        half_width:       Angle::from_degrees(3.),
                        min_pitch:        Angle::ZERO,
                        max_pitch:        Angle::RIGHT,
                        horizontal_range: Distance::from_nm(20.),
                        vertical_range:   Distance::from_feet(6000.),
                    }),
                }],
            }],
            waypoints:   vec![store::Waypoint {
                name:      "V".into(),
                position:  Position::from_origin_nm(8., 1.),
                elevation: Some(Position::from_amsl_feet(0.)),
                visual:    None,
                navaids:   vec![
                    store::Navaid { ty: store::NavaidType::Vor },
                    store::Navaid { ty: store::NavaidType::Dme },
                ],
            }],
            objects:     vec![
                store::Object::Plane(store::Plane {
                    aircraft:     store::BaseAircraft {
                        name:         "ABC123".into(),
                        position:     Position::from_origin_nm(1., 15.),
                        altitude:     Position::from_amsl_feet(5000.),
                        ground_speed: Speed::from_knots(180.),
                        ground_dir:   Heading::from_degrees(200.),
                    },
                    control:      store::PlaneControl {
                        heading:     Heading::from_degrees(210.),
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    plane_limits: default_plane_limits(),
                    nav_limits:   default_nav_limits(),
                    airborne:     Some(store::Airborne {
                        velocity:         nav::VelocityTarget {
                            yaw:         nav::YawTarget::Heading(Heading::from_degrees(200.)),
                            horiz_speed: Speed::from_knots(160.),
                            vert_rate:   Speed::from_fpm(0.),
                            expedite:    false,
                        },
                        target_altitude:  Some(Position::from_amsl_feet(5000.)),
                        target_waypoint:  None,
                        target_alignment: None,
                    }),
                }),
                store::Object::Plane(store::Plane {
                    aircraft:     store::BaseAircraft {
                        name:         "ADE127".into(),
                        position:     Position::from_origin_nm(10., -1.),
                        altitude:     Position::from_amsl_feet(8000.),
                        ground_speed: Speed::from_knots(250.),
                        ground_dir:   Heading::EAST,
                    },
                    control:      store::PlaneControl {
                        heading:     Heading::EAST,
                        yaw_speed:   default_nav_limits().max_yaw_speed,
                        horiz_accel: Accel::ZERO,
                    },
                    plane_limits: default_plane_limits(),
                    nav_limits:   default_nav_limits(),
                    airborne:     Some(store::Airborne {
                        velocity:         nav::VelocityTarget {
                            yaw:         nav::YawTarget::Heading(Heading::NORTH),
                            horiz_speed: Speed::from_knots(250.),
                            vert_rate:   Speed::from_fpm(1000.),
                            expedite:    false,
                        },
                        target_altitude:  Some(Position::from_amsl_feet(30000.)),
                        target_waypoint:  None,
                        target_alignment: Some(store::TargetAlignment {
                            start_waypoint:   "MAIN/18/ILS".into(),
                            end_waypoint:     "MAIN/18".into(),
                            lookahead:        Duration::from_secs(20),
                            activation_range: Distance::from_nm(0.2),
                        }),
                    }),
                }),
            ],
        },
    }
}
