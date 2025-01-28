use omniatc_core::level::nav;
use omniatc_core::level::route::WaypointProximity;
use omniatc_core::store;
use omniatc_core::units::{
    Accel, AccelRate, Angle, AngularAccel, AngularSpeed, Distance, Heading, Position, Speed,
};

pub fn default_plane_limits() -> nav::Limits {
    nav::Limits {
        min_horiz_speed:   Speed::from_knots(120.),
        max_yaw_speed:     AngularSpeed::from_degrees_per_sec(3.),
        max_vert_accel:    Accel::from_fpm_per_sec(200.),
        exp_climb:         nav::ClimbProfile {
            vert_rate: Speed::from_fpm(3000.),
            accel:     Accel::from_knots_per_sec(0.2),
            decel:     Accel::from_knots_per_sec(-1.8),
        },
        std_climb:         nav::ClimbProfile {
            vert_rate: Speed::from_fpm(1500.),
            accel:     Accel::from_knots_per_sec(0.6),
            decel:     Accel::from_knots_per_sec(-1.4),
        },
        level:             nav::ClimbProfile {
            vert_rate: Speed::from_fpm(0.),
            accel:     Accel::from_knots_per_sec(1.),
            decel:     Accel::from_knots_per_sec(-1.),
        },
        std_descent:       nav::ClimbProfile {
            vert_rate: Speed::from_fpm(-1500.),
            accel:     Accel::from_knots_per_sec(1.4),
            decel:     Accel::from_knots_per_sec(-0.6),
        },
        exp_descent:       nav::ClimbProfile {
            vert_rate: Speed::from_fpm(-3000.),
            accel:     Accel::from_knots_per_sec(1.8),
            decel:     Accel::from_knots_per_sec(-0.2),
        },
        drag_coef:         3. / 500. / 500.,
        accel_change_rate: AccelRate::from_knots_per_sec2(0.3),
        max_yaw_accel:     AngularAccel::from_degrees_per_sec2(1.),
    }
}

/// A simple map featuring different mechanisms for testing.
pub fn file() -> store::File {
    store::File {
        meta:  store::Meta {
            title:       "Example".into(),
            description: "Demo map".into(),
            authors:     vec!["omniatc demo".into()],
            tags:        vec!["demo".into(), "fictional".into()],
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
                winds:      vec![store::Wind {
                    start:        Position::from_origin_nm(-1000., -1000.),
                    end:          Position::from_origin_nm(1000., 1000.),
                    top:          Position::from_amsl_feet(40000.),
                    bottom:       Position::from_amsl_feet(40000.),
                    top_speed:    Speed::from_knots(15.).with_heading(Heading::from_degrees(300.)),
                    bottom_speed: Speed::from_knots(15.).with_heading(Heading::from_degrees(300.)),
                }],
            },
            aerodromes:  vec![store::Aerodrome {
                code:      "MAIN".into(),
                full_name: "Main Airport".into(),
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
                        visual_range:     Distance::from_meters(200.),
                        decision_height:  Distance::from_feet(100.),
                    }),
                }],
            }],
            waypoints:   vec![
                store::Waypoint {
                    name:      "EXITS".into(),
                    position:  Position::from_origin_nm(15., 1.),
                    elevation: Some(Position::from_amsl_feet(0.)),
                    visual:    None,
                    navaids:   vec![
                        store::Navaid {
                            ty:                  store::NavaidType::Vor,
                            heading_start:       Heading::NORTH,
                            heading_end:         Heading::NORTH,
                            min_pitch:           Angle::ZERO,
                            max_dist_horizontal: Distance::from_nm(199.),
                            max_dist_vertical:   Distance::from_feet(40000.),
                        },
                        store::Navaid {
                            ty:                  store::NavaidType::Dme,
                            heading_start:       Heading::NORTH,
                            heading_end:         Heading::NORTH,
                            min_pitch:           Angle::ZERO,
                            max_dist_horizontal: Distance::from_nm(199.),
                            max_dist_vertical:   Distance::from_feet(40000.),
                        },
                    ],
                },
                store::Waypoint {
                    name:      "DWIND".into(),
                    position:  Position::from_origin_nm(8., 0.),
                    elevation: None,
                    visual:    None,
                    navaids:   vec![],
                },
                store::Waypoint {
                    name:      "TBASE".into(),
                    position:  Position::from_origin_nm(6., 22.),
                    elevation: None,
                    visual:    None,
                    navaids:   vec![],
                },
                store::Waypoint {
                    name:      "APPCH".into(),
                    position:  Position::from_origin_nm(0., 16.),
                    elevation: None,
                    visual:    None,
                    navaids:   vec![],
                },
            ],
            objects:     vec![
                store::Object::Plane(store::Plane {
                    aircraft:   store::BaseAircraft {
                        name:         "ABC123".into(),
                        dest:         store::Destination::Landing { aerodrome_code: "MAIN".into() },
                        position:     Position::from_origin_nm(2., -16.),
                        altitude:     Position::from_amsl_feet(12000.),
                        ground_speed: Speed::from_knots(280.),
                        ground_dir:   Heading::from_degrees(250.),
                        vert_rate:    Speed::ZERO,
                    },
                    control:    store::PlaneControl {
                        heading:     Heading::from_degrees(80.),
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    limits:     default_plane_limits(),
                    nav_target: store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                        yaw:              nav::YawTarget::Heading(Heading::from_degrees(80.)),
                        horiz_speed:      Speed::from_knots(280.),
                        vert_rate:        Speed::from_fpm(0.),
                        expedite:         false,
                        target_altitude:  None,
                        target_glide:     None,
                        target_waypoint:  None,
                        target_alignment: None,
                    })),
                    route:      store::Route {
                        nodes: vec![
                            store::RouteNode::DirectWaypoint {
                                waypoint:  store::WaypointRef::Named("DWIND".into()),
                                distance:  Distance::from_nm(1.),
                                proximity: WaypointProximity::FlyBy,
                                altitude:  None,
                            },
                            store::RouteNode::DirectWaypoint {
                                waypoint:  store::WaypointRef::Named("TBASE".into()),
                                distance:  Distance::from_nm(1.),
                                proximity: WaypointProximity::FlyBy,
                                altitude:  Some(Position::from_amsl_feet(4000.)),
                            },
                            store::RouteNode::DirectWaypoint {
                                waypoint:  store::WaypointRef::Named("APPCH".into()),
                                distance:  Distance::from_nm(1.),
                                proximity: WaypointProximity::FlyBy,
                                altitude:  None,
                            },
                            store::RouteNode::AlignRunway {
                                runway:   store::RunwayRef {
                                    aerodrome_code: "MAIN".into(),
                                    runway_name:    "18".into(),
                                },
                                expedite: true,
                            },
                        ],
                    },
                }),
                store::Object::Plane(store::Plane {
                    aircraft:   store::BaseAircraft {
                        name:         "ADE127".into(),
                        dest:         store::Destination::ReachWaypoint {
                            min_altitude:       Some(Position::from_amsl_feet(18000.)),
                            waypoint_proximity: Some((
                                store::WaypointRef::Named("EXITS".into()),
                                Distance::from_nm(1.),
                            )),
                        },
                        position:     Position::from_origin_nm(10., -1.),
                        altitude:     Position::from_amsl_feet(8000.),
                        ground_speed: Speed::from_knots(250.),
                        ground_dir:   Heading::EAST,
                        vert_rate:    Speed::ZERO,
                    },
                    control:    store::PlaneControl {
                        heading:     Heading::EAST,
                        yaw_speed:   default_plane_limits().max_yaw_speed,
                        horiz_accel: Accel::ZERO,
                    },
                    limits:     default_plane_limits(),
                    nav_target: store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                        yaw:              nav::YawTarget::Heading(Heading::NORTH),
                        horiz_speed:      Speed::from_knots(250.),
                        vert_rate:        Speed::from_fpm(1000.),
                        expedite:         false,
                        target_altitude:  Some(store::TargetAltitude {
                            altitude: Position::from_amsl_feet(30000.),
                            expedite: false,
                        }),
                        target_glide:     None,
                        target_waypoint:  Some(store::TargetWaypoint {
                            waypoint: store::WaypointRef::Named("EXITS".into()),
                        }),
                        target_alignment: None,
                    })),
                    route:      store::Route { nodes: vec![] },
                }),
            ],
        },
        ui:    store::Ui {
            camera: store::Camera::TwoDimension(store::Camera2d {
                center:       Position::from_origin_nm(0., 0.),
                up:           Heading::NORTH,
                scale_axis:   store::AxisDirection::X,
                scale_length: Distance::from_nm(50.),
            }),
        },
    }
}
