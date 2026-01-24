use std::time::Duration;

use bevy_math::Vec2;
use math::{Accel, AccelRate, Angle, AngularAccel, AngularSpeed, Heading, Length, Position, Speed};
use store::{Score, WaypointProximity, WeightedList};

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
            current_phase:   store::LandingPhase::Align,
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
            current_phase:   store::LandingPhase::Align,
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
            current_phase:   store::LandingPhase::Align,
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
            current_phase:   store::LandingPhase::Align,
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
            current_phase:   store::LandingPhase::Align,
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

fn route_takeoff_18r() -> Vec<store::RouteNode> {
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
                label:     store::SegmentLabel::Taxiway("A1".into()),
            },
        },
        store::RouteNode::HoldShort {
            segment: store::SegmentRef {
                aerodrome: "MAIN".into(),
                label:     store::SegmentLabel::Runway("18R".into()),
            },
        },
        store::RouteNode::WaitForClearance,
        store::RouteNode::RunwayLineup {
            runway: store::RunwayRef { aerodrome: "MAIN".into(), runway_name: "18R".into() },
        },
        store::RouteNode::WaitForClearance,
        store::RouteNode::RunwayTakeoff {
            runway:          store::RunwayRef {
                aerodrome:   "MAIN".into(),
                runway_name: "18R".into(),
            },
            target_altitude: Position::from_amsl_feet(4000.),
        },
    ]
    .into_iter()
    .collect()
}

fn route_sid_exits_18r() -> Vec<store::RouteNode> {
    let mut route = route_takeoff_18r();
    route.extend([
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("CLIFF".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyOver,
            altitude:  None,
        },
        store::RouteNode::SetAirSpeed { goal: Speed::from_knots(250.), error: None },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("SHADE".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(3000.)),
        },
        store::RouteNode::DirectWaypoint {
            waypoint:  store::WaypointRef::Named("EXITS".into()),
            distance:  Length::from_nm(1.),
            proximity: WaypointProximity::FlyBy,
            altitude:  Some(Position::from_amsl_feet(4000.)),
        },
    ]);
    route
}

const MAIN_AERODROME_ELEVATION: Position<f32> = Position::from_amsl_feet(300.0);
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
            id:          "omniatc.tutorial".into(),
            title:       "Tutorial".into(),
            description: "Tutorial map".into(),
            authors:     ["omniatc".into()].into(),
            tags:        [
                ("region", "fictional"),
                ("source", "builtin"),
                ("type", "scenario"),
                ("tutorial", "true"),
            ]
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
                weather:    [].into(),
            },
            object_types:  [(
                "A359",
                store::ObjectType {
                    full_name:   "Airbus A350-900".into(),
                    taxi_limits: a359_taxi_limits(),
                    class:       store::ObjectClassSpec::Plane { nav_limits: a359_nav_limits() },
                },
            )]
            .into_iter()
            .map(|(k, v)| (store::ObjectTypeRef(k.into()), v))
            .collect(),
            aerodromes:    [store::Aerodrome {
                code:           "MAIN".into(),
                full_name:      "Main Airport".into(),
                elevation:      MAIN_AERODROME_ELEVATION,
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
                    name:      "CLIFF".into(),
                    position:  Position::from_origin_nm(0., -6.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "SHADE".into(),
                    position:  Position::from_origin_nm(7., -8.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "DWIND".into(),
                    position:  Position::from_origin_nm(8., 0.),
                    elevation: None,
                    visual:    None,
                    navaids:   [].into(),
                },
                store::Waypoint {
                    name:      "OCEAN".into(),
                    position:  Position::from_origin_nm(12., -20.),
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
                store::route_presets_at_waypoints(
                    "DWIND18L",
                    "DWIND 18L",
                    route_dwind_18l(),
                    store::PresetDestination::arrival("MAIN"),
                ),
                store::route_presets_at_waypoints(
                    "DWIND18R",
                    "DWIND 18R",
                    route_dwind_18r(),
                    store::PresetDestination::arrival("MAIN"),
                ),
                store::route_presets_at_waypoints(
                    "POLAR18L",
                    "POLAR 18L",
                    route_polar_18l(),
                    store::PresetDestination::arrival("MAIN"),
                ),
                store::route_presets_at_waypoints(
                    "POLAR18R",
                    "POLAR 18R",
                    route_polar_18r(),
                    store::PresetDestination::arrival("MAIN"),
                ),
                [store::RoutePreset {
                    trigger:      store::RoutePresetTrigger::Waypoint(store::WaypointRef::Named(
                        "RETRY".into(),
                    )),
                    id:           "RETRY18R".into(),
                    ref_id:       Some(store::RoutePresetRef("RETRY.RETRY18R".into())),
                    title:        "Missed approach 18R".into(),
                    nodes:        route_retry_18r(),
                    destinations: [store::PresetDestination::arrival("MAIN")].into(),
                }]
                .into(),
                store::route_presets_at_waypoints(
                    "EXITS18R",
                    "EXITS 18R",
                    route_sid_exits_18r(),
                    store::PresetDestination::departure("EXITS"),
                ),
            ]
            .into_iter()
            .flatten()
            .collect(),
            spawn_sets:    [(
                store::SpawnSet {
                    route:    WeightedList::singleton(store::SpawnRoute {
                        preset:      store::RoutePresetRef("DWIND18L DWIND".into()),
                        destination: store::Destination::Landing { aerodrome: "MAIN".into() },
                        score:       Score(10),
                    }),
                    gen_name: [
                        (
                            store::NameGenerator::Airline {
                                prefix:          "RND".into(),
                                digits:          3,
                                trailing_letter: None,
                            },
                            1.0,
                        ),
                        (
                            store::NameGenerator::Airline {
                                prefix:          "FRT".into(),
                                digits:          3,
                                trailing_letter: Some("XYZ".into()),
                            },
                            1.0,
                        ),
                    ]
                    .into(),
                    types:    [(store::ObjectTypeRef("A359".into()), 1.0)].into(),
                    position: WeightedList::singleton(store::SpawnPosition::Airborne {
                        waypoint: "OCEAN".into(),
                        altitude: Position::from_amsl_feet(12000.0),
                        speed:    Speed::from_knots(280.0),
                        heading:  Heading::from_degrees(300.0),
                    }),
                },
                1.0,
            )]
            .into(),
            spawn_trigger: store::SpawnTrigger::Periodic { duration: Duration::from_secs(60) },
        },
        ui:    store::Ui {
            camera: store::Camera::TwoDimension(store::Camera2d {
                center:       Position::from_origin_nm(0., 0.),
                up:           Heading::NORTH,
                scale_axis:   store::AxisDirection::X,
                scale_length: Length::from_nm(100.),
            }),
        },

        stats:   store::Stats::default(),
        quests:  store::QuestTree {
            quests: [store::Quest {
                id:           "tutorial/drag".into(),
                title:        "Tutorial: Camera (1/3)".into(),
                description:  "Right-click the radar view and drag to move the camera.".into(),
                class:        store::QuestClass::Tutorial,
                dependencies: [].into(),
                conditions:   [store::CameraQuestCompletionCondition::Drag.into()].into(),
                ui_highlight: [store::HighlightableUiElement::RadarView].into(),
            },
                store::Quest {
                    id:           "tutorial/zoom".into(),
                    title:        "Tutorial: Camera (2/3)".into(),
                    description:  "Scroll on the radar view up and down to zoom in and out.".into(),
                    class:        store::QuestClass::Tutorial,
                    dependencies: ["tutorial/drag".into()].into(),
                    conditions:   [store::CameraQuestCompletionCondition::Zoom.into()].into(),
                    ui_highlight: [store::HighlightableUiElement::RadarView, store::HighlightableUiElement::SetCameraZoom].into(),
                },
                store::Quest {
                    id:           "tutorial/rotate".into(),
                    title:        "Tutorial: Camera (3/3)".into(),
                    description:  "Scroll on the radar view left and right, or use the slider in the Level menu to rotate.".into(),
                    class:        store::QuestClass::Tutorial,
                    dependencies: ["tutorial/zoom".into()].into(),
                    conditions:   [store::CameraQuestCompletionCondition::Rotate.into()].into(),
                    ui_highlight: [store::HighlightableUiElement::RadarView, store::HighlightableUiElement::SetCameraRotation].into(),
                },
                store::Quest {
                    id:           "tutorial/altitude".into(),
                    title:        "Tutorial: Aircraft control (1/4)".into(),
                    description:  r#"Click on an aircraft in the radar view or on the objects table. You can adjust the altitude by dragging the altitude slider and clicking "Send". You may also use up/down arrow keys and press Enter. Send the aircraft to 6000 feet."#.into(),
                    class:        store::QuestClass::Tutorial,
                    dependencies: [].into(),
                    conditions:   [store::ObjectControlQuestCompletionCondition::ReachAltitude(store::Range{min: Position::from_amsl_feet(5950.0), max: Position::from_amsl_feet(6050.0), }).into()].into(),
                    ui_highlight: [store::HighlightableUiElement::SetAltitude].into(),
                },
                store::Quest {
                    id:           "tutorial/speed".into(),
                    title:        "Tutorial: Aircraft control (2/4)".into(),
                    description:  r#"Drag the speed slider or use ","/"." to adjust the speed. Slow down the aircraft to 250 knots."#.into(),
                    class:        store::QuestClass::Tutorial,
                    dependencies: [].into(),
                    conditions:   [store::ObjectControlQuestCompletionCondition::ReachSpeed(store::Range{min: Speed::from_knots(245.0), max: Speed::from_knots(255.0), }).into()].into(),
                    ui_highlight: [].into(),
                },
            ]
            .into(),
        },
        objects: [
            store::Object::Plane(store::Plane {
                aircraft:    store::BaseAircraft {
                    name:             "ABC123".into(),
                    dest:             store::Destination::Landing { aerodrome: "MAIN".into() },
                    completion_score: Score(10),
                    position:         Position::from_origin_nm(2., -14.),
                    altitude:         Position::from_amsl_feet(12000.),
                    ground_speed:     Speed::from_knots(280.),
                    ground_dir:       Heading::from_degrees(250.),
                    vert_rate:        Speed::ZERO,
                },
                control:     store::PlaneControl {
                    heading:     Heading::from_degrees(80.),
                    yaw_speed:   AngularSpeed::ZERO,
                    horiz_accel: Accel::ZERO,
                },
                object_type: store::ObjectTypeRef("A359".into()),
                taxi_limits: a359_taxi_limits(),
                nav_limits:  a359_nav_limits(),
                nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                    yaw:              store::YawTarget::Heading(Heading::from_degrees(80.)),
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
                    completion_score: Score(10),
                    position:         Position::from_origin_nm(2., -18.),
                    altitude:         Position::from_amsl_feet(12000.),
                    ground_speed:     Speed::from_knots(280.),
                    ground_dir:       Heading::from_degrees(250.),
                    vert_rate:        Speed::ZERO,
                },
                control:     store::PlaneControl {
                    heading:     Heading::from_degrees(80.),
                    yaw_speed:   AngularSpeed::ZERO,
                    horiz_accel: Accel::ZERO,
                },
                object_type: store::ObjectTypeRef("A359".into()),
                taxi_limits: a359_taxi_limits(),
                nav_limits:  a359_nav_limits(),
                nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                    yaw:              store::YawTarget::Heading(Heading::from_degrees(80.)),
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
                    completion_score: Score(10),
                    position:         Position::from_origin_nm(8., 28.),
                    altitude:         Position::from_amsl_feet(7000.),
                    ground_speed:     Speed::from_knots(220.),
                    ground_dir:       Heading::from_degrees(250.),
                    vert_rate:        Speed::ZERO,
                },
                control:     store::PlaneControl {
                    heading:     Heading::from_degrees(200.),
                    yaw_speed:   AngularSpeed::ZERO,
                    horiz_accel: Accel::ZERO,
                },
                object_type: store::ObjectTypeRef("A359".into()),
                taxi_limits: a359_taxi_limits(),
                nav_limits:  a359_nav_limits(),
                nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                    yaw:              store::YawTarget::Heading(Heading::from_degrees(80.)),
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
                    dest:             store::Destination::Departure {
                        min_altitude:       Some(Position::from_amsl_feet(18000.)),
                        waypoint_proximity: Some((
                            store::WaypointRef::Named("EXITS".into()),
                            Length::from_nm(1.),
                        )),
                    },
                    completion_score: Score(1),
                    position:         Position::from_origin_nm(10., -1.),
                    altitude:         Position::from_amsl_feet(8000.),
                    ground_speed:     Speed::from_knots(250.),
                    ground_dir:       Heading::EAST,
                    vert_rate:        Speed::ZERO,
                },
                control:     store::PlaneControl {
                    heading:     Heading::EAST,
                    yaw_speed:   a359_nav_limits().max_yaw_speed,
                    horiz_accel: Accel::ZERO,
                },
                object_type: store::ObjectTypeRef("A359".into()),
                taxi_limits: a359_taxi_limits(),
                nav_limits:  a359_nav_limits(),
                nav_target:  store::NavTarget::Airborne(Box::new(store::AirborneNavTarget {
                    yaw:              store::YawTarget::Heading(Heading::NORTH),
                    horiz_speed:      Speed::from_knots(250.),
                    vert_rate:        Speed::from_fpm(1000.),
                    expedite:         false,
                    target_altitude:  Some(store::TargetAltitude {
                        altitude: MAIN_AERODROME_ELEVATION,
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
                    completion_score: Score(5),
                    position:         Position::from_origin_nm(1., 0.),
                    altitude:         MAIN_AERODROME_ELEVATION,
                    ground_speed:     Speed::from_knots(140.),
                    ground_dir:       Heading::SOUTH,
                    vert_rate:        Speed::ZERO,
                },
                control:     store::PlaneControl {
                    heading:     Heading::SOUTH,
                    yaw_speed:   AngularSpeed::ZERO,
                    horiz_accel: Accel::ZERO,
                },
                object_type: store::ObjectTypeRef("A359".into()),
                taxi_limits: a359_taxi_limits(),
                nav_limits:  a359_nav_limits(),
                nav_target:  store::NavTarget::Ground(store::GroundNavTarget {
                    segment: store::SegmentRef {
                        aerodrome: "MAIN".into(),
                        label:     store::SegmentLabel::Runway("36R".into()),
                    },
                }),
                route:       store::Route { id: None, nodes: route_taxi_runway_east_to_tango() },
            }),
            store::Object::Plane(store::Plane {
                aircraft:    store::BaseAircraft {
                    name:             "DEP256".into(),
                    dest:             store::Destination::Departure {
                        min_altitude:       Some(Position::from_amsl_feet(18000.)),
                        waypoint_proximity: Some((
                            store::WaypointRef::Named("EXITS".into()),
                            Length::from_nm(1.),
                        )),
                    },
                    completion_score: Score(5),
                    position:         TOP_LEFT_ORIGIN
                        + Length::from_components(
                            FIRST_TAXIWAY_OFFSET.midpoint(SECOND_TAXIWAY_OFFSET),
                            -HORIZONTAL_TAXIWAY_OFFSET,
                        ),
                    altitude:         MAIN_AERODROME_ELEVATION,
                    ground_speed:     Speed::ZERO,
                    ground_dir:       Heading::WEST,
                    vert_rate:        Speed::ZERO,
                },
                control:     store::PlaneControl {
                    heading:     Heading::WEST,
                    yaw_speed:   AngularSpeed::ZERO,
                    horiz_accel: Accel::ZERO,
                },
                object_type: store::ObjectTypeRef("A359".into()),
                taxi_limits: a359_taxi_limits(),
                nav_limits:  a359_nav_limits(),
                nav_target:  store::NavTarget::Ground(store::GroundNavTarget {
                    segment: store::SegmentRef {
                        aerodrome: "MAIN".into(),
                        label:     store::SegmentLabel::Taxiway("T".into()),
                    },
                }),
                route:       store::Route {
                    id:    Some("EXITS18R".into()),
                    nodes: route_sid_exits_18r(),
                },
            }),
        ]
        .into(),
    }
}
