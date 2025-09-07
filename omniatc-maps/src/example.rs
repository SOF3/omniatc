use bevy::math::Vec2;
use math::{Accel, AccelRate, Angle, AngularAccel, AngularSpeed, Heading, Length, Position, Speed};
use omniatc::level::route::WaypointProximity;
use omniatc::level::{nav, score, taxi};
use omniatc::store;

pub fn default_plane_taxi_limits() -> taxi::Limits {
    taxi::Limits {
        base_braking: Accel::from_knots_per_sec(3.0),
        accel:        Accel::from_knots_per_sec(5.0),
        max_speed:    Speed::from_knots(100.0),
        min_speed:    Speed::from_knots(-4.0),
        turn_rate:    AngularSpeed::from_degrees_per_sec(8.0),
        width:        Length::from_meters(60.0),
        half_length:  Length::from_meters(70.0),
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
        short_final_dist:  Length::from_nm(4.),
        short_final_speed: Speed::from_knots(150.),
    }
}

fn route_retry_18r() -> Vec<store::RouteNode> {
    [
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("RETRY".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("REMRG".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNW".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome:   "MAIN".into(),
                runway_name: "18R".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
    ]
    .into_iter()
    .chain(route_taxi_runway_west_to_tango())
    .collect()
}

fn route_dwind_18l() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("DWIND".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("LONG".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNE".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome:   "MAIN".into(),
                runway_name: "18L".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
    ]
    .into_iter()
    .chain(route_taxi_runway_east_to_tango())
    .collect()
}

fn route_dwind_18r() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("DWIND".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("LONG".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNW".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome:   "MAIN".into(),
                runway_name: "18R".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
    ]
    .into_iter()
    .chain(route_taxi_runway_west_to_tango())
    .collect()
}

fn route_polar_18l() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("POLAR".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNE".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome:   "MAIN".into(),
                runway_name: "18L".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
    ]
    .into_iter()
    .chain(route_taxi_runway_east_to_tango())
    .collect()
}

fn route_polar_18r() -> Vec<store::RouteNode> {
    [
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("POLAR".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHORT".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(200.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("APPNW".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(180.), error: None },
        store::RouteNode::RunwayLanding {
            runway:          store::RunwayRef {
                aerodrome:   "MAIN".into(),
                runway_name: "18R".into(),
            },
            goaround_preset: Some("RETRY.RETRY18R".into()),
        },
    ]
    .into_iter()
    .chain(route_taxi_runway_west_to_tango())
    .collect()
}

fn route_taxi_runway_east_to_tango() -> Vec<store::RouteNode> {
    [
        store::RouteNode::Taxi {
            segment: store::SegmentRef {
                aerodrome: "MAIN".into(),
                label:     store::SegmentLabel::Taxiway("B".into()),
            },
        },
        store::RouteNode::Taxi {
            segment: store::SegmentRef {
                aerodrome: "MAIN".into(),
                label:     store::SegmentLabel::Taxiway("T".into()),
            },
        },
    ]
    .into()
}

fn route_taxi_runway_west_to_tango() -> Vec<store::RouteNode> {
    [
        store::RouteNode::Taxi {
            segment: store::SegmentRef {
                aerodrome: "MAIN".into(),
                label:     store::SegmentLabel::Taxiway("A".into()),
            },
        },
        store::RouteNode::Taxi {
            segment: store::SegmentRef {
                aerodrome: "MAIN".into(),
                label:     store::SegmentLabel::Taxiway("T".into()),
            },
        },
    ]
    .into()
}

const RIGHT_RUNWAY_OFFSET: Length<f32> = Length::from_nm(1.0);
const RUNWAY_LENGTH: Length<f32> = Length::from_meters(3000.0);
const RUNWAY_WIDTH: Length<f32> = Length::from_meters(100.0);
const TAXIWAY_WIDTH: Length<f32> = Length::from_meters(80.0);
const FIRST_TAXIWAY_OFFSET: Length<f32> = Length::from_meters(200.0);
const SECOND_TAXIWAY_OFFSET: Length<f32> = Length::from_meters(350.0);
const HORIZONTAL_TAXIWAY_OFFSET: Length<f32> = Length::from_meters(1100.0);
const RAPID_EXIT_TAXIWAY_ANGLE: Angle = Angle::from_degrees(60.0);
const APRON_LENGTH: Length<f32> = Length::from_meters(150.0);
const APRON_INTERVAL: Length<f32> = Length::from_meters(120.0);
const TOP_LEFT_ORIGIN: Position<Vec2> = Position::from_origin_nm(0.0, 0.0);
const TOP_RIGHT_ORIGIN: Position<Vec2> =
    Position::from_origin_nm(RIGHT_RUNWAY_OFFSET.into_nm(), 0.0);
const BOTTOM_LEFT_ORIGIN: Position<Vec2> = Position::from_origin_nm(0.0, -RUNWAY_LENGTH.into_nm());
const BOTTOM_RIGHT_ORIGIN: Position<Vec2> =
    Position::from_origin_nm(RIGHT_RUNWAY_OFFSET.into_nm(), -RUNWAY_LENGTH.into_nm());

fn rapid_exit_taxiways() -> impl Iterator<Item = store::Taxiway> {
    let exits = [
        (Length::from_meters(0.0), Angle::ZERO),
        (Length::from_meters(200.0), Angle::ZERO),
        (Length::from_meters(800.0), RAPID_EXIT_TAXIWAY_ANGLE),
        (Length::from_meters(1250.0), RAPID_EXIT_TAXIWAY_ANGLE),
    ];
    [
        ("A", TOP_LEFT_ORIGIN, Heading::SOUTH, Heading::EAST, -1.0),
        ("B", TOP_RIGHT_ORIGIN, Heading::SOUTH, Heading::WEST, 1.0),
    ]
    .into_iter()
    .flat_map(move |(prefix, origin, runway_dir, runway_to_taxiway, angle_dir)| {
        let rev_exits = exits
            .into_iter()
            .rev()
            .map(|(offset, angle)| (Length::from_meters(3000.0) - offset, -angle));
        exits.into_iter().chain(rev_exits).enumerate().map(move |(index, (offset, angle))| {
            let runway_endpoint = origin + offset * runway_dir;
            let taxiway_endpoint = runway_endpoint
                + FIRST_TAXIWAY_OFFSET * (runway_to_taxiway + angle * angle_dir) / angle.cos();
            store::Taxiway {
                name:      format!("{prefix}{}", index + 1),
                endpoints: [
                    runway_endpoint,
                    taxiway_endpoint,
                    taxiway_endpoint
                        + (SECOND_TAXIWAY_OFFSET - FIRST_TAXIWAY_OFFSET) * runway_to_taxiway,
                ]
                .into(),
                width:     TAXIWAY_WIDTH,
            }
        })
    })
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
            stats:         store::Stats::default(),
            environment:   store::Environment {
                heightmap:  store::HeatMap2 {
                    aligned: store::AlignedHeatMap2::constant(Position::from_amsl_feet(0.)),
                    sparse:  store::SparseHeatMap2 { functions: [].into() },
                },
                visibility: store::HeatMap2 {
                    aligned: store::AlignedHeatMap2::constant(Length::from_nm(1000.)),
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
                                TOP_LEFT_ORIGIN
                                    + Length::from_components(FIRST_TAXIWAY_OFFSET, Length::ZERO),
                                BOTTOM_LEFT_ORIGIN
                                    + Length::from_components(FIRST_TAXIWAY_OFFSET, Length::ZERO),
                            ]
                            .into(),
                            width:     TAXIWAY_WIDTH,
                        },
                        store::Taxiway {
                            name:      "B".into(),
                            endpoints: [
                                TOP_RIGHT_ORIGIN
                                    + Length::from_components(-FIRST_TAXIWAY_OFFSET, Length::ZERO),
                                BOTTOM_RIGHT_ORIGIN
                                    + Length::from_components(-FIRST_TAXIWAY_OFFSET, Length::ZERO),
                            ]
                            .into(),
                            width:     TAXIWAY_WIDTH,
                        },
                        store::Taxiway {
                            name:      "J".into(),
                            endpoints: [
                                TOP_LEFT_ORIGIN
                                    + Length::from_components(SECOND_TAXIWAY_OFFSET, Length::ZERO),
                                BOTTOM_LEFT_ORIGIN
                                    + Length::from_components(SECOND_TAXIWAY_OFFSET, Length::ZERO),
                            ]
                            .into(),
                            width:     TAXIWAY_WIDTH,
                        },
                        store::Taxiway {
                            name:      "K".into(),
                            endpoints: [
                                TOP_RIGHT_ORIGIN
                                    + Length::from_components(-SECOND_TAXIWAY_OFFSET, Length::ZERO),
                                BOTTOM_RIGHT_ORIGIN
                                    + Length::from_components(-SECOND_TAXIWAY_OFFSET, Length::ZERO),
                            ]
                            .into(),
                            width:     TAXIWAY_WIDTH,
                        },
                        store::Taxiway {
                            name:      "T".into(),
                            endpoints: [
                                TOP_LEFT_ORIGIN
                                    + Length::from_components(
                                        FIRST_TAXIWAY_OFFSET,
                                        -HORIZONTAL_TAXIWAY_OFFSET,
                                    ),
                                TOP_RIGHT_ORIGIN
                                    + Length::from_components(
                                        -FIRST_TAXIWAY_OFFSET,
                                        -HORIZONTAL_TAXIWAY_OFFSET,
                                    ),
                            ]
                            .into(),
                            width:     TAXIWAY_WIDTH,
                        },
                        store::Taxiway {
                            name:      "U".into(),
                            endpoints: [
                                BOTTOM_LEFT_ORIGIN
                                    + Length::from_components(
                                        FIRST_TAXIWAY_OFFSET,
                                        HORIZONTAL_TAXIWAY_OFFSET,
                                    ),
                                BOTTOM_RIGHT_ORIGIN
                                    + Length::from_components(
                                        -FIRST_TAXIWAY_OFFSET,
                                        HORIZONTAL_TAXIWAY_OFFSET,
                                    ),
                            ]
                            .into(),
                            width:     TAXIWAY_WIDTH,
                        },
                    ]
                    .into_iter()
                    .chain(rapid_exit_taxiways())
                    .collect(),
                    aprons:      [
                        ('N', Heading::NORTH, -HORIZONTAL_TAXIWAY_OFFSET + APRON_LENGTH),
                        ('S', Heading::SOUTH, -HORIZONTAL_TAXIWAY_OFFSET - APRON_LENGTH),
                        (
                            'N',
                            Heading::NORTH,
                            -RUNWAY_LENGTH + HORIZONTAL_TAXIWAY_OFFSET + APRON_LENGTH,
                        ),
                        (
                            'S',
                            Heading::SOUTH,
                            -RUNWAY_LENGTH + HORIZONTAL_TAXIWAY_OFFSET - APRON_LENGTH,
                        ),
                    ]
                    .into_iter()
                    .flat_map(|(prefix, heading, y)| {
                        (-3..=3)
                            .map(move |x_offset: i16| {
                                let x = APRON_INTERVAL * f32::from(x_offset);
                                (
                                    prefix,
                                    heading,
                                    TOP_LEFT_ORIGIN.lerp(TOP_RIGHT_ORIGIN, 0.5)
                                        + Length::from((x, y)),
                                )
                            })
                            .map(move |(prefix, heading, position)| {
                                move |index| store::Apron {
                                    name: format!("{prefix}{index:02}"),
                                    position,
                                    forward_heading: heading,
                                    width: TAXIWAY_WIDTH,
                                }
                            })
                    })
                    .enumerate()
                    .map(|(index, f)| f(index + 1))
                    .collect(),
                    taxi_speed:  Speed::from_knots(30.0),
                    apron_speed: Speed::from_meter_per_sec(5.0),
                },
                runways:        [
                    store::RunwayPair {
                        width:          RUNWAY_WIDTH,
                        forward_start:  TOP_LEFT_ORIGIN,
                        forward:        store::Runway {
                            name:                   "18R".into(),
                            touchdown_displacement: Length::from_meters(160.),
                            stopway:                Length::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Length::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Length::from_nm(20.),
                                vertical_range:   Length::from_feet(6000.),
                                visual_range:     Length::from_meters(200.),
                                decision_height:  Length::from_feet(100.),
                            }),
                        },
                        backward_start: BOTTOM_LEFT_ORIGIN,
                        backward:       store::Runway {
                            name:                   "36L".into(),
                            touchdown_displacement: Length::from_meters(160.),
                            stopway:                Length::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Length::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Length::from_nm(20.),
                                vertical_range:   Length::from_feet(6000.),
                                visual_range:     Length::from_meters(200.),
                                decision_height:  Length::from_feet(100.),
                            }),
                        },
                    },
                    store::RunwayPair {
                        width:          RUNWAY_WIDTH,
                        forward_start:  TOP_RIGHT_ORIGIN,
                        forward:        store::Runway {
                            name:                   "18L".into(),
                            touchdown_displacement: Length::from_meters(160.),
                            stopway:                Length::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Length::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Length::from_nm(20.),
                                vertical_range:   Length::from_feet(6000.),
                                visual_range:     Length::from_meters(200.),
                                decision_height:  Length::from_feet(100.),
                            }),
                        },
                        backward_start: BOTTOM_RIGHT_ORIGIN,
                        backward:       store::Runway {
                            name:                   "36R".into(),
                            touchdown_displacement: Length::from_meters(160.),
                            stopway:                Length::ZERO,
                            glide_angle:            Angle::from_degrees(3.),
                            max_visual_distance:    Length::from_nm(3.),
                            ils:                    Some(store::Localizer {
                                half_width:       Angle::from_degrees(3.),
                                min_pitch:        Angle::ZERO,
                                max_pitch:        Angle::RIGHT,
                                horizontal_range: Length::from_nm(20.),
                                vertical_range:   Length::from_feet(6000.),
                                visual_range:     Length::from_meters(200.),
                                decision_height:  Length::from_feet(100.),
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
                            max_dist_horizontal: Length::from_nm(199.),
                            max_dist_vertical:   Length::from_feet(40000.),
                        },
                        store::Navaid {
                            ty:                  store::NavaidType::Dme,
                            heading_start:       Heading::NORTH,
                            heading_end:         Heading::NORTH,
                            min_pitch:           Angle::ZERO,
                            max_dist_horizontal: Length::from_nm(199.),
                            max_dist_vertical:   Length::from_feet(40000.),
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
                        name:             "ABC123".into(),
                        dest:             store::Destination::Landing { aerodrome: "MAIN".into() },
                        completion_score: score::Unit(10),
                        position:         Position::from_origin_nm(2., -14.),
                        altitude:         Position::from_amsl_feet(12000.),
                        ground_speed:     Speed::from_knots(280.),
                        ground_dir:       Heading::from_degrees(250.),
                        vert_rate:        Speed::ZERO,
                        weight:           1e5,
                        wingspan:         Length::from_meters(50.),
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
                        name:             "DEF789".into(),
                        dest:             store::Destination::Landing { aerodrome: "MAIN".into() },
                        completion_score: score::Unit(10),
                        position:         Position::from_origin_nm(2., -18.),
                        altitude:         Position::from_amsl_feet(12000.),
                        ground_speed:     Speed::from_knots(280.),
                        ground_dir:       Heading::from_degrees(250.),
                        vert_rate:        Speed::ZERO,
                        weight:           1e5,
                        wingspan:         Length::from_meters(50.),
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
                        name:             "ARC512".into(),
                        dest:             store::Destination::Landing { aerodrome: "MAIN".into() },
                        completion_score: score::Unit(10),
                        position:         Position::from_origin_nm(8., 28.),
                        altitude:         Position::from_amsl_feet(7000.),
                        ground_speed:     Speed::from_knots(220.),
                        ground_dir:       Heading::from_degrees(250.),
                        vert_rate:        Speed::ZERO,
                        weight:           1e5,
                        wingspan:         Length::from_meters(50.),
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
                        name:             "ADE127".into(),
                        weight:           1e5,
                        wingspan:         Length::from_meters(50.),
                        dest:             store::Destination::Departure {
                            min_altitude:       Some(Position::from_amsl_feet(18000.)),
                            waypoint_proximity: Some((
                                store::WaypointRef::Named("EXITS".into()),
                                Length::from_nm(1.),
                            )),
                        },
                        completion_score: score::Unit(1),
                        position:         Position::from_origin_nm(10., -1.),
                        altitude:         Position::from_amsl_feet(8000.),
                        ground_speed:     Speed::from_knots(250.),
                        ground_dir:       Heading::EAST,
                        vert_rate:        Speed::ZERO,
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
                store::Object::Plane(store::Plane {
                    aircraft:    store::BaseAircraft {
                        name:             "LND456".into(),
                        dest:             store::Destination::Parking { aerodrome: "MAIN".into() },
                        completion_score: score::Unit(5),
                        position:         Position::from_origin_nm(1., 0.),
                        altitude:         Position::from_amsl_feet(300.),
                        ground_speed:     Speed::from_knots(140.),
                        ground_dir:       Heading::SOUTH,
                        vert_rate:        Speed::ZERO,
                        weight:           1e5,
                        wingspan:         Length::from_meters(50.),
                    },
                    control:     store::PlaneControl {
                        heading:     Heading::SOUTH,
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    },
                    taxi_limits: default_plane_taxi_limits(),
                    nav_limits:  default_plane_nav_limits(),
                    nav_target:  store::NavTarget::Ground(store::GroundNavTarget {
                        segment: store::SegmentRef {
                            aerodrome: "MAIN".into(),
                            label:     store::SegmentLabel::Runway("36R".into()),
                        },
                    }),
                    route:       store::Route {
                        id:    None,
                        nodes: route_taxi_runway_east_to_tango(),
                    },
                }),
            ]
            .into(),
        },
        ui:    store::Ui {
            camera: store::Camera::TwoDimension(store::Camera2d {
                center:       Position::from_origin_nm(0., 0.),
                up:           Heading::NORTH,
                scale_axis:   store::AxisDirection::X,
                scale_length: Length::from_nm(100.),
            }),
        },
    }
}
