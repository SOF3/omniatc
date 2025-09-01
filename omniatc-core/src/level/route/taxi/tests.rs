use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;
use math::{Length, Position, Speed};

use crate::level::ground;
use crate::level::route::{PathfindMode, PathfindOptions, pathfind_through_subseq};

const WIDE_SEGMENT: Length<f32> = Length::from_meters(100.0);
const NARROW_SEGMENT: Length<f32> = Length::from_meters(50.0);
const WIDE_OBJECT: Length<f32> = Length::from_meters(80.0);
const NARROW_OBJECT: Length<f32> = Length::from_meters(40.0);

const FAST_SEGMENT: Speed<f32> = Speed::from_knots(60.0);
const SLOW_SEGMENT: Speed<f32> = Speed::from_knots(30.0);
const FAST_OBJECT: Speed<f32> = Speed::from_knots(50.0);
const SLOW_OBJECT: Speed<f32> = Speed::from_knots(25.0);

const ELEVATION: Position<f32> = Position::SEA_LEVEL;

/// ```text
/// X ----- p(10) ----- Y -- p(1) -- Z
/// |                               /
/// q(8)                        r(10)
/// |                            /
/// |                           /
/// A -- s(3) -- B -- s(2) -- C
///              |
///            t(100)
///              |
///              D
/// ```
///
/// `BAXY` is wide and fast, `BCZY` is narrow and slow.
fn prepare_world(world: &mut World) -> Prepared {
    let mut commands = world.commands();

    let mut endpoints = PreparedEndpoints {
        d: Entity::PLACEHOLDER,
        a: Entity::PLACEHOLDER,
        b: Entity::PLACEHOLDER,
        c: Entity::PLACEHOLDER,
        x: Entity::PLACEHOLDER,
        y: Entity::PLACEHOLDER,
        z: Entity::PLACEHOLDER,
    };

    for (id, x, y) in [
        (&mut endpoints.d, 3.0, -100.0),
        (&mut endpoints.a, 0.0, 0.0),
        (&mut endpoints.b, 3.0, 0.0),
        (&mut endpoints.c, 5.0, 0.0),
        (&mut endpoints.x, 0.0, 8.0),
        (&mut endpoints.y, 10.0, 8.0),
        (&mut endpoints.z, 11.0, 8.0),
    ] {
        *id = commands
            .spawn_empty()
            .queue(ground::SpawnEndpoint { position: Position::from_origin_nm(x, y) })
            .id();
    }

    let mut segments = PreparedSegments {
        bd: Entity::PLACEHOLDER,
        ab: Entity::PLACEHOLDER,
        bc: Entity::PLACEHOLDER,
        ax: Entity::PLACEHOLDER,
        cz: Entity::PLACEHOLDER,
        xy: Entity::PLACEHOLDER,
        yz: Entity::PLACEHOLDER,
    };
    for (id, alpha, beta, width, max_speed, name) in [
        (&mut segments.bd, endpoints.b, endpoints.d, NARROW_SEGMENT, SLOW_SEGMENT, "t"),
        (&mut segments.ab, endpoints.a, endpoints.b, WIDE_SEGMENT, FAST_SEGMENT, "s"),
        (&mut segments.bc, endpoints.b, endpoints.c, NARROW_SEGMENT, SLOW_SEGMENT, "s"),
        (&mut segments.ax, endpoints.a, endpoints.x, WIDE_SEGMENT, FAST_SEGMENT, "q"),
        (&mut segments.cz, endpoints.c, endpoints.z, NARROW_SEGMENT, SLOW_SEGMENT, "r"),
        (&mut segments.xy, endpoints.x, endpoints.y, WIDE_SEGMENT, FAST_SEGMENT, "p"),
        (&mut segments.yz, endpoints.y, endpoints.z, NARROW_SEGMENT, SLOW_SEGMENT, "p"),
    ] {
        *id = commands
            .spawn_empty()
            .queue(ground::SpawnSegment {
                segment: ground::Segment { alpha, beta, width, max_speed, elevation: ELEVATION },
                label:   ground::SegmentLabel::Taxiway { name: name.into() },
            })
            .id();
    }
    world.flush();

    Prepared { endpoints, segments }
}

#[derive(Debug, Clone, Copy)]
struct Prepared {
    endpoints: PreparedEndpoints,
    segments:  PreparedSegments,
}

#[derive(Debug, Clone, Copy)]
struct PreparedEndpoints {
    d: Entity,
    a: Entity,
    b: Entity,
    c: Entity,
    x: Entity,
    y: Entity,
    z: Entity,
}

#[derive(Debug, Clone, Copy)]
struct PreparedSegments {
    bd: Entity,
    ab: Entity,
    bc: Entity,
    ax: Entity,
    cz: Entity,
    xy: Entity,
    yz: Entity,
}

#[test]
fn pathfind_segment_start() {
    let mut app = App::new();
    let prepared = prepare_world(app.world_mut());

    let path = pathfind_through_subseq(
        app.world(),
        prepared.segments.bd,
        prepared.endpoints.b,
        &[
            ground::SegmentLabel::Taxiway { name: "s".into() },
            ground::SegmentLabel::Taxiway { name: "p".into() },
        ],
        PathfindMode::SegmentStart,
        PathfindOptions { min_width: Some(NARROW_OBJECT), initial_speed: Some(SLOW_OBJECT) },
    )
    .unwrap();
    assert_eq!(
        path.endpoints,
        vec![prepared.endpoints.b, prepared.endpoints.a, prepared.endpoints.x]
    );
    assert!((path.cost - Length::from_nm(11.0)).abs() < Length::from_nm(0.0001));
}

#[test]
fn pathfind_segment_end() {
    let mut app = App::new();
    let prepared = prepare_world(app.world_mut());

    let path = pathfind_through_subseq(
        app.world(),
        prepared.segments.bd,
        prepared.endpoints.b,
        &[
            ground::SegmentLabel::Taxiway { name: "s".into() },
            ground::SegmentLabel::Taxiway { name: "p".into() },
        ],
        PathfindMode::SegmentEnd,
        PathfindOptions { min_width: Some(NARROW_OBJECT), initial_speed: Some(SLOW_OBJECT) },
    )
    .unwrap();
    assert_eq!(
        path.endpoints,
        vec![
            prepared.endpoints.b,
            prepared.endpoints.c,
            prepared.endpoints.z,
            prepared.endpoints.y
        ]
    );
    assert!((path.cost - Length::from_nm(13.0)).abs() < Length::from_nm(0.0001));
}

#[test]
fn pathfind_dest_endpoint() {
    let mut app = App::new();
    let prepared = prepare_world(app.world_mut());

    let path = pathfind_through_subseq(
        app.world(),
        prepared.segments.bd,
        prepared.endpoints.b,
        &[
            ground::SegmentLabel::Taxiway { name: "s".into() },
            ground::SegmentLabel::Taxiway { name: "p".into() },
        ],
        PathfindMode::Endpoint(prepared.endpoints.y),
        PathfindOptions { min_width: Some(NARROW_OBJECT), initial_speed: Some(SLOW_OBJECT) },
    )
    .unwrap();
    assert_eq!(
        path.endpoints,
        vec![
            prepared.endpoints.b,
            prepared.endpoints.c,
            prepared.endpoints.z,
            prepared.endpoints.y
        ]
    );
    assert!((path.cost - Length::from_nm(13.0)).abs() < Length::from_nm(0.0001));
}

#[test]
fn pathfind_speed_restricted() {
    let mut app = App::new();
    let prepared = prepare_world(app.world_mut());

    let path = pathfind_through_subseq(
        app.world(),
        prepared.segments.bd,
        prepared.endpoints.b,
        &[
            ground::SegmentLabel::Taxiway { name: "s".into() },
            ground::SegmentLabel::Taxiway { name: "p".into() },
        ],
        PathfindMode::Endpoint(prepared.endpoints.y),
        PathfindOptions { min_width: None, initial_speed: Some(FAST_OBJECT) },
    )
    .unwrap();
    assert_eq!(
        path.endpoints,
        vec![
            prepared.endpoints.b,
            prepared.endpoints.a,
            prepared.endpoints.x,
            prepared.endpoints.y
        ]
    );
    assert!((path.cost - Length::from_nm(21.0)).abs() < Length::from_nm(0.0001));
}

#[test]
fn pathfind_width_restricted() {
    let mut app = App::new();
    let prepared = prepare_world(app.world_mut());

    let path = pathfind_through_subseq(
        app.world(),
        prepared.segments.bd,
        prepared.endpoints.b,
        &[
            ground::SegmentLabel::Taxiway { name: "s".into() },
            ground::SegmentLabel::Taxiway { name: "p".into() },
        ],
        PathfindMode::Endpoint(prepared.endpoints.y),
        PathfindOptions { min_width: Some(WIDE_OBJECT), initial_speed: None },
    )
    .unwrap();
    assert_eq!(
        path.endpoints,
        vec![
            prepared.endpoints.b,
            prepared.endpoints.a,
            prepared.endpoints.x,
            prepared.endpoints.y
        ]
    );
    assert!((path.cost - Length::from_nm(21.0)).abs() < Length::from_nm(0.0001));
}
