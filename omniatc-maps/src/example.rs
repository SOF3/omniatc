use bevy::math::Vec2;
use math::{
    Accel, AccelRate, Angle, AngularAccel, AngularSpeed, Distance, Heading, Position, Speed, Unit,
};
use omniatc::level::route::WaypointProximity;
use omniatc::level::{nav, taxi};
use omniatc::store;

pub fn default_plane_taxi_limits() -> taxi::Limits {
    taxi::Limits {
        base_braking: Accel::from_knots_per_sec(3.0),
        accel:        Accel::from_knots_per_sec(5.0),
        max_speed:    Speed::from_knots(100.0),
        min_speed:    Speed::from_knots(-4.0),
        turn_rate:    AngularSpeed::from_degrees_per_sec(8.0),
        width:        Distance::from_meters(60.0),
        half_length:  Distance::from_meters(70.0),
    }
}

pub fn default_plane_nav_limits() -> nav::Limits {
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
        accel_change_rate: AccelRate::from_knots_per_sec2(0.3),
        drag_coef:         3. / 500. / 500.,
        max_yaw_accel:     AngularAccel::from_degrees_per_sec2(1.),
        short_final_dist:  Distance::from_nm(4.),
        short_final_speed: Speed::from_knots(150.),
    }
}

fn route_retry_18r() -> Vec<store::RouteNode> {
    [
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("RETRY".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("REMRG".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNW".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome_code: "MAIN".into(),
                runway_name:    "18R".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
        store::RouteNode::Taxi {
            options: [
                store::SegmentRef::Taxiway("A3".into()),
                store::SegmentRef::Taxiway("A4".into()),
            ]
            .into(),
        },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("A".into())].into() },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("T".into())].into() },
    ]
    .into()
}

fn route_dwind_18l() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("DWIND".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("LONG".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNE".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome_code: "MAIN".into(),
                runway_name:    "18L".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
        store::RouteNode::Taxi {
            options: [
                store::SegmentRef::Taxiway("B3".into()),
                store::SegmentRef::Taxiway("B4".into()),
            ]
            .into(),
        },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("B".into())].into() },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("T".into())].into() },
    ]
    .into()
}

fn route_dwind_18r() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("DWIND".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("LONG".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNW".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome_code: "MAIN".into(),
                runway_name:    "18R".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
        store::RouteNode::Taxi {
            options: [
                store::SegmentRef::Taxiway("A3".into()),
                store::SegmentRef::Taxiway("A4".into()),
            ]
            .into(),
        },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("A".into())].into() },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("T".into())].into() },
    ]
    .into()
}

fn route_polar_18l() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("POLAR".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNE".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome_code: "MAIN".into(),
                runway_name:    "18L".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
        store::RouteNode::Taxi {
            options: [
                store::SegmentRef::Taxiway("B3".into()),
                store::SegmentRef::Taxiway("B4".into()),
            ]
            .into(),
        },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("B".into())].into() },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("T".into())].into() },
    ]
    .into()
}

fn route_polar_18r() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("POLAR".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNW".into()),
            distance:  Distance::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome_code: "MAIN".into(),
                runway_name:    "18R".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
        store::RouteNode::Taxi {
            options: [
                store::SegmentRef::Taxiway("A3".into()),
                store::SegmentRef::Taxiway("A4".into()),
            ]
            .into(),
        },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("A".into())].into() },
        store::RouteNode::Taxi { options: [store::SegmentRef::Taxiway("T".into())].into() },
    ]
    .into()
}

/// A simple map featuring different mechanisms for testing.
pub fn file() -> store::File {
    store::File {
        meta:  store::Meta {
            id:          "omniatc.example".into(),
            title:       "Example".into(),
            description: "Demo map".into(),
            authors:     ["omniatc demo".into()].into(),
            tags:        [("region", "fictional"), ("source", "builtin"), ("type", "demo")]
                .into_iter()
                .map(|(k, v)| (String::from(k), String::from(v)))
                .collect(),
        },
        level: store::Level {
            environment:   store::Environment {
                heightmap:  store::HeatMap2 {
                    aligned: store::AlignedHeatMap2::constant(Position::from_amsl_feet(0.)),
                    sparse:  store::SparseHeatMap2 { functions: [].into() },
                },
                visibility: store::HeatMap2 {
                    aligned: store::AlignedHeatMap2::constant(Distance::from_nm(1000.)),
                    sparse:  store::SparseHeatMap2 { functions: [].into() },
                },
                winds:      [store::Wind {
                    start:        Position::from_origin_nm(-1000., -1000.),
                    end:          Position::from_origin_nm(1000., 1000.),
                    top:          Position::from_amsl_feet(40000.),
                    bottom:       Position::from_amsl_feet(0.),
                    top_speed:    Speed::from_knots(85.).with_heading(Heading::from_degrees(330.)),
                    bottom_speed: Speed::from_knots(25.).with_heading(Heading::from_degrees(300.)),
                }]
                .into(),
            },
            aerodromes:    [store::Aerodrome {
                code:           "MAIN".into(),
                full_name:      "Main Airport".into(),
                elevation:      Position::from_amsl_feet(300.),
                ground_network: store::GroundNetwork {
                    taxiways:    [
                        store::Taxiway {
                            name:      "A".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., 0.)),
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., -3000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "A1".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.),
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., 0.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "A2".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(0., -1000.)),
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., -600.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "A3".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(0., -2000.)),
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., -2400.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "A4".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(0., -3000.)),
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., -3000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "B".into(),
                            endpoints: [
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., 0.)),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., -3000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "B1".into(),
                            endpoints: [
                                Position::from_origin_nm(1., 0.),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., 0.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "B2".into(),
                            endpoints: [
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(0., -1000.)),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., -600.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "B3".into(),
                            endpoints: [
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(0., -2000.)),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., -2400.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "B4".into(),
                            endpoints: [
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(0., -3000.)),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., -3000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "J".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(350., -1000.)),
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(350., -2000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "K".into(),
                            endpoints: [
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-350., -1000.)),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-350., -2000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "T".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., -1000.)),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., -1000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                        store::Taxiway {
                            name:      "U".into(),
                            endpoints: [
                                Position::from_origin_nm(0., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(200., -2000.)),
                                Position::from_origin_nm(1., 0.)
                                    + Distance::vec2_from_meters(Vec2::new(-200., -2000.)),
                            ]
                            .into(),
                            width:     Distance::from_meters(80.),
                        },
                    ]
                    .into(),
                    aprons:      [
                        ('N', Heading::NORTH, Distance::from_meters(-800.)),
                        ('S', Heading::SOUTH, Distance::from_meters(-1200.)),
                        ('N', Heading::NORTH, Distance::from_meters(-1800.)),
                        ('S', Heading::SOUTH, Distance::from_meters(-2200.)),
                    ]
                    .into_iter()
                    .enumerate()
                    .flat_map(|(row, (prefix, heading, y))| {
                        (-2..=2)
                            .map(move |x_offset: i16| {
                                let x = Distance::from_meters(200.) * f32::from(x_offset);
                                (
                                    prefix,
                                    heading,
                                    Position::from_origin_nm(0.5, 0.) + Distance::from((x, y)),
                                )
                            })
                            .enumerate()
                            .map(move |(index, (prefix, heading, position))| store::Apron {
                                name: format!("{prefix}{:02}", row * 5 + index + 1),
                                position,
                                forward_heading: heading,
                                width: Distance::from_meters(80.),
                            })
                    })
                    .collect(),
                    taxi_speed:  Speed::from_knots(25.0),
                    apron_speed: Speed::from_meter_per_sec(5.0),
                },
                runways:        [
                    store::RunwayPair {
                        width:          Distance::from_meters(100.),
                        forward_start:  Position::from_origin_nm(0., 0.),
                        forward:        store::Runway {
                            name:                   "18R".into(),
                            touchdown_displacement: Distance::from_meters(160.),
                            stopway:                Distance::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Distance::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Distance::from_nm(20.),
                                vertical_range:   Distance::from_feet(6000.),
                                visual_range:     Distance::from_meters(200.),
                                decision_height:  Distance::from_feet(100.),
                            }),
                        },
                        backward_start: Position::from_origin_nm(0., 0.)
                            + Distance::vec2_from_meters(Vec2::new(0., -3000.)),
                        backward:       store::Runway {
                            name:                   "36L".into(),
                            touchdown_displacement: Distance::from_meters(160.),
                            stopway:                Distance::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Distance::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Distance::from_nm(20.),
                                vertical_range:   Distance::from_feet(6000.),
                                visual_range:     Distance::from_meters(200.),
                                decision_height:  Distance::from_feet(100.),
                            }),
                        },
                    },
                    store::RunwayPair {
                        width:          Distance::from_meters(100.),
                        forward_start:  Position::from_origin_nm(1., 0.),
                        forward:        store::Runway {
                            name:                   "18L".into(),
                            touchdown_displacement: Distance::from_meters(160.),
                            stopway:                Distance::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Distance::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Distance::from_nm(20.),
                                vertical_range:   Distance::from_feet(6000.),
                                visual_range:     Distance::from_meters(200.),
                                decision_height:  Distance::from_feet(100.),
                            }),
                        },
                        backward_start: Position::from_origin_nm(1., 0.)
                            + Distance::vec2_from_meters(Vec2::new(0., -3000.)),
                        backward:       store::Runway {
                            name:                   "36R".into(),
                            touchdown_displacement: Distance::from_meters(160.),
                            stopway:                Distance::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Distance::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Distance::from_nm(20.),
                                vertical_range:   Distance::from_feet(6000.),
                                visual_range:     Distance::from_meters(200.),
                                decision_height:  Distance::from_feet(100.),
                            }),
                        },
                    },
                ]
                .into(),
            }]
            .into(),
            waypoints:     [
                store::Waypoint {
                    name:      "EXITS".into(),
                    position:  Position::from_origin_nm(15., 1.),
                    elevation: Some(Position::from_amsl_feet(0.)),
                    visual:    None,
                    navaids:   [
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
                    ]
                    .into(),
                },
                store::Waypoint {
                    name:      "DWIND".into(),
                    position:  Position::from_origin_nm(8., 0.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "POLAR".into(),
                    position:  Position::from_origin_nm(8., 24.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "LONG".into(),
                    position:  Position::from_origin_nm(8., 16.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "SHORT".into(),
                    position:  Position::from_origin_nm(6., 18.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "RETRY".into(),
                    position:  Position::from_origin_nm(-6., 0.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "REMRG".into(),
                    position:  Position::from_origin_nm(-6., 16.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "APPNW".into(),
                    position:  Position::from_origin_nm(0., 16.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "APPNE".into(),
                    position:  Position::from_origin_nm(1., 16.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
            ]
            .into(),
            route_presets: [
                store::route_presets_at_waypoints("DWIND18L", "DWIND 18L", route_dwind_18l()),
                store::route_presets_at_waypoints("DWIND18R", "DWIND 18R", route_dwind_18r()),
                store::route_presets_at_waypoints("POLAR18L", "POLAR 18L", route_polar_18l()),
                store::route_presets_at_waypoints("POLAR18R", "POLAR 18R", route_polar_18r()),
                [store::RoutePreset {
                    trigger: store::RoutePresetTrigger::Waypoint(store::WaypointRef::Named(
                        "RETRY".into(),
                    )),
                    id:      "RETRY18R".into(),
                    ref_id:  Some("RETRY.RETRY18R".into()),
                    title:   "Missed approach 18R".into(),
                    nodes:   route_retry_18r(),
                }]
                .into(),
            ]
            .into_iter()
            .flatten()
            .collect(),
            objects:       [
                store::Object::Plane(store::Plane {
                    aircraft:    store::BaseAircraft {
                        name:         "ABC123".into(),
                        dest:         store::Destination::Landing { aerodrome_code: "MAIN".into() },
                        position:     Position::from_origin_nm(2., -14.),
                        altitude:     Position::from_amsl_feet(12000.),
                        ground_speed: Speed::from_knots(280.),
                        ground_dir:   Heading::from_degrees(250.),
                        vert_rate:    Speed::ZERO,
                        weight:       1e5,
                        wingspan:     Distance::from_meters(50.),
                    },
                    control:     store::PlaneControl {
                        heading:     Heading::from_degrees(80.),
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    taxi_limits: default_plane_taxi_limits(),
                    nav_limits:  default_plane_nav_limits(),
                    nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                        yaw:              nav::YawTarget::Heading(Heading::from_degrees(80.)),
                        horiz_speed:      Speed::from_knots(280.),
                        vert_rate:        Speed::from_fpm(0.),
                        expedite:         false,
                        target_altitude:  None,
                        target_glide:     None,
                        target_waypoint:  None,
                        target_alignment: None,
                    })),
                    route:       store::Route {
                        id:    Some("DWIND18L".into()),
                        nodes: route_dwind_18l(),
                    },
                }),
                store::Object::Plane(store::Plane {
                    aircraft:    store::BaseAircraft {
                        name:         "DEF789".into(),
                        dest:         store::Destination::Landing { aerodrome_code: "MAIN".into() },
                        position:     Position::from_origin_nm(2., -18.),
                        altitude:     Position::from_amsl_feet(12000.),
                        ground_speed: Speed::from_knots(280.),
                        ground_dir:   Heading::from_degrees(250.),
                        vert_rate:    Speed::ZERO,
                        weight:       1e5,
                        wingspan:     Distance::from_meters(50.),
                    },
                    control:     store::PlaneControl {
                        heading:     Heading::from_degrees(80.),
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    taxi_limits: default_plane_taxi_limits(),
                    nav_limits:  default_plane_nav_limits(),
                    nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                        yaw:              nav::YawTarget::Heading(Heading::from_degrees(80.)),
                        horiz_speed:      Speed::from_knots(280.),
                        vert_rate:        Speed::from_fpm(0.),
                        expedite:         false,
                        target_altitude:  None,
                        target_glide:     None,
                        target_waypoint:  None,
                        target_alignment: None,
                    })),
                    route:       store::Route {
                        id:    Some("DWIND18L".into()),
                        nodes: route_dwind_18l(),
                    },
                }),
                store::Object::Plane(store::Plane {
                    aircraft:    store::BaseAircraft {
                        name:         "ARC512".into(),
                        dest:         store::Destination::Landing { aerodrome_code: "MAIN".into() },
                        position:     Position::from_origin_nm(8., 28.),
                        altitude:     Position::from_amsl_feet(7000.),
                        ground_speed: Speed::from_knots(220.),
                        ground_dir:   Heading::from_degrees(250.),
                        vert_rate:    Speed::ZERO,
                        weight:       1e5,
                        wingspan:     Distance::from_meters(50.),
                    },
                    control:     store::PlaneControl {
                        heading:     Heading::from_degrees(200.),
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    taxi_limits: default_plane_taxi_limits(),
                    nav_limits:  default_plane_nav_limits(),
                    nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                        yaw:              nav::YawTarget::Heading(Heading::from_degrees(80.)),
                        horiz_speed:      Speed::from_knots(220.),
                        vert_rate:        Speed::from_fpm(0.),
                        expedite:         false,
                        target_altitude:  None,
                        target_glide:     None,
                        target_waypoint:  None,
                        target_alignment: None,
                    })),
                    route:       store::Route {
                        id:    Some("POLAR18L".into()),
                        nodes: route_polar_18l(),
                    },
                }),
                store::Object::Plane(store::Plane {
                    aircraft:    store::BaseAircraft {
                        name:         "ADE127".into(),
                        weight:       1e5,
                        wingspan:     Distance::from_meters(50.),
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
                    control:     store::PlaneControl {
                        heading:     Heading::EAST,
                        yaw_speed:   default_plane_nav_limits().max_yaw_speed,
                        horiz_accel: Accel::ZERO,
                    },
                    taxi_limits: default_plane_taxi_limits(),
                    nav_limits:  default_plane_nav_limits(),
                    nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
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
                    route:       store::Route { id: None, nodes: [].into() },
                }),
            ]
            .into(),
        },
        ui:    store::Ui {
            camera: store::Camera::TwoDimension(store::Camera2d {
                center:       Position::from_origin_nm(0., 0.),
                up:           Heading::NORTH,
                scale_axis:   store::AxisDirection::X,
                scale_length: Distance::from_nm(100.),
            }),
        },
    }
}
