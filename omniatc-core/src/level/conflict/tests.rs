use std::time::Duration;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::time::{self, Time};
use math::{ISA_TROPOPAUSE_PRESSURE, ISA_TROPOPAUSE_TEMPERATURE, Position, Speed};
use store::Score;

use super::{ActiveObject, Record};
use crate::level::object::{self, Object};
use crate::level::{SystemSets, conflict, message, score, weather};

fn base_app() -> App {
    let mut app = App::new();
    SystemSets::configure_ordering(&mut app);
    app.add_plugins((
        message::Plug,
        score::Plug,
        object::Plug::<()>::default(),
        weather::Plug::<()>::default(),
        conflict::Plug::<()>::default(),
    ));
    app.init_resource::<Time<time::Virtual>>();
    app.update();
    app.world_mut().resource_mut::<score::Stats>().total = Score(100);
    app
}

fn spawn_airborne(app: &mut App, x_nm: f32, alt_ft: f32) -> Entity {
    let position =
        Position::from_origin_nm(x_nm, 0.0).with_altitude(Position::from_amsl_feet(alt_ft));
    app.world_mut()
        .spawn((
            Object { position, ground_speed: Speed::ZERO },
            object::Airborne {
                airspeed:      Speed::ZERO,
                true_airspeed: Speed::ZERO,
                oat:           ISA_TROPOPAUSE_TEMPERATURE,
                pressure:      ISA_TROPOPAUSE_PRESSURE,
                pressure_alt:  Position::from_amsl_feet(alt_ft),
            },
            Record::default(),
        ))
        .id()
}

/// Advances virtual time by 1 second and runs one update tick.
fn advance(app: &mut App) {
    app.world_mut().flush();
    app.world_mut().resource_mut::<Time<time::Virtual>>().advance_by(Duration::from_secs(1));
    app.update();
}

fn current_score(app: &App) -> i32 { app.world().resource::<score::Stats>().total.0 }

fn is_active_object(app: &App, entity: Entity) -> bool {
    app.world().get::<ActiveObject>(entity).is_some()
}

/// Objects far enough apart -> no conflict, score unchanged.
#[test]
fn test_no_conflict() {
    let mut app = base_app();
    let entity_a = spawn_airborne(&mut app, 0.0, 3000.0);
    let entity_b = spawn_airborne(&mut app, 10.0, 3000.0);
    app.update(); // octree warm-up

    advance(&mut app);

    assert_eq!(current_score(&app), 100, "score must not change when objects are separated");
    assert!(!is_active_object(&app, entity_a), "no ActiveObject when no conflict");
    assert!(!is_active_object(&app, entity_b), "no ActiveObject when no conflict");
}

/// Conflict for exactly 1 second at 2.9nm/900ft.
#[test]
fn test_1s_at_2_9nm_900ft() {
    let mut app = base_app();
    let entity_a = spawn_airborne(&mut app, 0.0, 3000.0);
    let entity_b = spawn_airborne(&mut app, 2.9, 3900.0);
    app.update(); // octree warm-up

    advance(&mut app); // 1 conflict tick

    assert_eq!(current_score(&app), 98);
    assert!(is_active_object(&app, entity_a));
    assert!(is_active_object(&app, entity_b));
}

/// Conflict for 10 seconds at 2.9nm/900ft.
#[test]
fn test_10s_at_2_9nm_900ft() {
    let mut app = base_app();
    spawn_airborne(&mut app, 0.0, 3000.0);
    spawn_airborne(&mut app, 2.9, 3900.0);
    app.world_mut().flush();
    app.update(); // octree warm-up

    for _ in 0..10 {
        advance(&mut app);
    }

    assert_eq!(current_score(&app), 90);
}

/// Conflict for 60 seconds at 2.9nm/900ft.
#[test]
fn test_60s_at_2_9nm_900ft() {
    let mut app = base_app();
    spawn_airborne(&mut app, 0.0, 3000.0);
    spawn_airborne(&mut app, 2.9, 3900.0);
    app.world_mut().flush();
    app.update(); // octree warm-up

    for _ in 0..60 {
        advance(&mut app);
    }

    assert_eq!(current_score(&app), 77);
}

/// Conflict for 10 seconds at 0.5nm/900ft (much closer horizontally).
#[test]
fn test_10s_at_0_5nm_900ft() {
    let mut app = base_app();
    spawn_airborne(&mut app, 0.0, 3000.0);
    spawn_airborne(&mut app, 0.5, 3900.0);
    app.world_mut().flush();
    app.update(); // octree warm-up

    for _ in 0..10 {
        advance(&mut app);
    }

    assert_eq!(current_score(&app), 53);
}

/// Conflict for 10 seconds at 2.9nm/200ft (much closer vertically).
#[test]
fn test_10s_at_2_9nm_200ft() {
    let mut app = base_app();
    spawn_airborne(&mut app, 0.0, 3000.0);
    spawn_airborne(&mut app, 2.9, 3200.0);
    app.world_mut().flush();
    app.update(); // octree warm-up

    for _ in 0..10 {
        advance(&mut app);
    }

    assert_eq!(current_score(&app), 58);
}

/// Conflict for 10 seconds at 0.5nm/200ft (closest in both axes).
#[test]
fn test_10s_at_0_5nm_200ft() {
    let mut app = base_app();
    spawn_airborne(&mut app, 0.0, 3000.0);
    spawn_airborne(&mut app, 0.5, 3200.0);
    app.world_mut().flush();
    app.update(); // octree warm-up

    for _ in 0..10 {
        advance(&mut app);
    }

    assert_eq!(current_score(&app), 21);
}

/// Conflict for 10s, then separation, then conflict for 50s (all at 2.9nm/900ft).
///
/// Tests that cumulative conflict time is NOT reset on separation.
#[test]
fn test_10s_separate_50s_at_2_9nm_900ft() {
    let mut app = base_app();
    let entity_a = spawn_airborne(&mut app, 0.0, 3000.0);
    let entity_b = spawn_airborne(&mut app, 2.9, 3900.0);
    app.world_mut().flush();
    app.update(); // octree warm-up

    for _ in 0..10 {
        advance(&mut app);
    }

    // Move B far away to break the conflict.
    app.world_mut().get_mut::<Object>(entity_b).unwrap().position =
        Position::from_origin_nm(20.0, 0.0).with_altitude(Position::from_amsl_feet(3900.0));

    advance(&mut app);

    assert!(!is_active_object(&app, entity_a), "ActiveObject must be removed after separation");
    assert!(!is_active_object(&app, entity_b), "ActiveObject must be removed after separation");

    // Move B back to conflicting position.
    app.world_mut().get_mut::<Object>(entity_b).unwrap().position =
        Position::from_origin_nm(2.9, 0.0).with_altitude(Position::from_amsl_feet(3900.0));
    app.update(); // octree warm-up for resumed position

    for _ in 0..50 {
        advance(&mut app);
    }

    assert_eq!(current_score(&app), 76);
    assert!(
        is_active_object(&app, entity_a),
        "ActiveObject must be restored after resuming conflict"
    );
    assert!(
        is_active_object(&app, entity_b),
        "ActiveObject must be restored after resuming conflict"
    );
}

/// Three objects: A-B conflict for 5s, then A-B+B-C conflict for 5s, then B-C conflict for 5s.
/// All at 0.5nm/900ft separation.
#[test]
fn test_multiobject_ab_then_ab_bc_then_bc() {
    let mut app = base_app();

    // A at (0, 0), B at (0.5, 0), C at (5, 0). Initially only A-B conflict.
    let entity_a = spawn_airborne(&mut app, 0.0, 3000.0);
    let entity_b = spawn_airborne(&mut app, 0.5, 3900.0); // 0.5nm from A, 900ft vert
    let entity_c = spawn_airborne(&mut app, 5.0, 3900.0); // far from everyone
    app.world_mut().flush();
    app.update(); // octree warm-up

    for _ in 0..5 {
        advance(&mut app); // A-B conflict only
    }

    assert!(is_active_object(&app, entity_a));
    assert!(is_active_object(&app, entity_b));
    assert!(!is_active_object(&app, entity_c));

    // Move C close to B, but vertically separated from A
    // Use 900ft vert separation from B: C at altitude 3900+900=4800ft.
    app.world_mut().get_mut::<Object>(entity_c).unwrap().position =
        Position::from_origin_nm(0.5 + 0.5, 0.0).with_altitude(Position::from_amsl_feet(4800.0));
    app.update(); // octree warm-up for C's new position

    for _ in 0..5 {
        advance(&mut app); // A-B and B-C both active
    }

    // C is now active
    assert!(is_active_object(&app, entity_a), "A should have active conflict with B");
    assert!(is_active_object(&app, entity_b), "B should have active conflict with both A and C");
    assert!(is_active_object(&app, entity_c), "C should have active conflict with B");

    // Move A far away; only B-C conflict continues.
    app.world_mut().get_mut::<Object>(entity_a).unwrap().position =
        Position::from_origin_nm(20.0, 0.0).with_altitude(Position::from_amsl_feet(3000.0));
    app.update(); // octree warm-up for A's new position

    for _ in 0..5 {
        advance(&mut app); // B-C conflict only
    }

    assert_eq!(current_score(&app), 0);
    assert_eq!(app.world().resource::<score::Stats>().num_conflicts, 2);

    assert!(!is_active_object(&app, entity_a), "A has no active conflict after moving away");
    assert!(is_active_object(&app, entity_b), "B still has active conflict with C");
    assert!(is_active_object(&app, entity_c), "C has active conflict with B");
}
