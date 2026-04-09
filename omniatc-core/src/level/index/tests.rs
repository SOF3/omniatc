use bevy::app::App;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::math::Vec3;
use math::{Length, Position};

use super::{OctreeIndex, Plug};

#[derive(Component)]
struct Indexed {
    position: Position<Vec3>,
}

#[derive(Component)]
struct Include;

fn setup_app() -> App {
    let mut app = App::new();
    app.add_plugins(Plug::<Indexed, With<Include>, _>::new(|indexed| indexed.position));
    app
}

fn bounds_around(center: Position<Vec3>) -> (Position<Vec3>, Position<Vec3>) {
    let delta = Length::new(Vec3::splat(0.5));
    (center - delta, center + delta)
}

#[test]
fn plugin_builds_filtered_index() {
    let mut app = setup_app();

    let included_a = app
        .world_mut()
        .spawn((Include, Indexed { position: Position::new(Vec3::new(0.0, 0.0, 0.0)) }))
        .id();
    let included_b = app
        .world_mut()
        .spawn((Include, Indexed { position: Position::new(Vec3::new(1.0, 2.0, 3.0)) }))
        .id();
    app.world_mut().spawn(Indexed { position: Position::new(Vec3::new(9.0, 9.0, 9.0)) });

    app.update();

    let index = app.world().resource::<OctreeIndex<Indexed, With<Include>>>();
    let entities = index.entries().map(|entry| entry.entity).collect::<Vec<_>>();

    assert_eq!(entities.len(), 2);
    assert!(entities.contains(&included_a));
    assert!(entities.contains(&included_b));
}

#[test]
fn plugin_rebuilds_index_every_tick() {
    let mut app = setup_app();
    let entity = app
        .world_mut()
        .spawn((Include, Indexed { position: Position::new(Vec3::new(0.0, 0.0, 0.0)) }))
        .id();

    app.update();
    let first_tick = app
        .world()
        .resource::<OctreeIndex<Indexed, With<Include>>>()
        .entities_in_bounds([
            Position::new(Vec3::new(-0.5, -0.5, -0.5)),
            Position::new(Vec3::new(0.5, 0.5, 0.5)),
        ])
        .collect::<Vec<_>>();
    assert!(first_tick.contains(&entity));

    app.world_mut().get_mut::<Indexed>(entity).unwrap().position =
        Position::new(Vec3::new(5.0, 0.0, 0.0));
    app.update();

    let index = app.world().resource::<OctreeIndex<Indexed, With<Include>>>();
    let old_bounds = index
        .entities_in_bounds([
            Position::new(Vec3::new(-0.5, -0.5, -0.5)),
            Position::new(Vec3::new(0.5, 0.5, 0.5)),
        ])
        .collect::<Vec<_>>();
    let new_bounds = index
        .entities_in_bounds([
            Position::new(Vec3::new(4.5, -0.5, -0.5)),
            Position::new(Vec3::new(5.5, 0.5, 0.5)),
        ])
        .collect::<Vec<_>>();

    assert!(!old_bounds.contains(&entity));
    assert!(new_bounds.contains(&entity));
}

#[test]
fn plugin_tracks_entity_moving_beyond_initial_bounds_consecutively() {
    let mut app = setup_app();
    let entity = app
        .world_mut()
        .spawn((Include, Indexed { position: Position::new(Vec3::new(0.0, 0.0, 0.0)) }))
        .id();

    app.update();

    let mut previous = Position::new(Vec3::new(0.0, 0.0, 0.0));
    let next_positions = Vec::from([
        Position::new(Vec3::new(180.0, 0.0, 0.0)),
        Position::new(Vec3::new(420.0, -420.0, 0.0)),
        Position::new(Vec3::new(900.0, -900.0, 900.0)),
    ]);

    for next in next_positions {
        app.world_mut().get_mut::<Indexed>(entity).unwrap().position = next;
        app.update();

        let index = app.world().resource::<OctreeIndex<Indexed, With<Include>>>();
        let (next_min, next_max) = bounds_around(next);
        let in_next_bounds = index.entities_in_bounds([next_min, next_max]).collect::<Vec<_>>();

        let (prev_min, prev_max) = bounds_around(previous);
        let in_previous_bounds = index.entities_in_bounds([prev_min, prev_max]).collect::<Vec<_>>();

        assert!(in_next_bounds.contains(&entity));
        assert!(!in_previous_bounds.contains(&entity));
        previous = next;
    }
}
