use anyhow::{Context, Result};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::World;
use bevy::math::Vec2;
use math::{Accel, AngularSpeed, Heading, Position, Speed};
use omniatc::level::conflict::ActiveObject;
use omniatc::level::{dest, nav, object, plane, score, taxi};
use omniatc_client_test::start_test;

fn main() -> Result<()> {
    let mut test = start_test("conflict", "omniatc.blank".into())?;

    test.with_screenshot("level-load", |test| {
        test.wait_for_level_load()?;
        let world = test.world();
        let mut query = world.query::<&object::Object>();
        if query.iter(world).next().is_some() {
            anyhow::bail!("Expected blank scenario to start without objects");
        }
        Ok(())
    })?;

    let (taxi_limits, nav_limits) = load_plane_limits(test.world())?;
    // Two reciprocal tracks (045 and 315), expected to rendezvous near (2, 1).
    spawn_test_plane(
        test.world(),
        "CF001",
        Position::from_origin_nm(-1.0, -2.0),
        Position::from_amsl_feet(4000.0),
        Heading::from_degrees(45.0),
        Speed::from_knots(300.0),
        &taxi_limits,
        &nav_limits,
    );
    spawn_test_plane(
        test.world(),
        "CF002",
        Position::from_origin_nm(5.0, -2.0),
        Position::from_amsl_feet(4500.0),
        Heading::from_degrees(315.0),
        Speed::from_knots(300.0),
        &taxi_limits,
        &nav_limits,
    );

    test.with_screenshot("conflict-active", |test| {
        test.with_time_scale(20.0, |test| {
            test.drive_until(|world| active_object_count(world) == 2)
        })?;
        if test.world().resource::<score::Stats>().num_conflicts == 0 {
            anyhow::bail!("Expected the spawned pair to enter conflict");
        }
        Ok(())
    })?;

    test.with_screenshot("conflict-flash", |test| {
        test.drive_frames(8);
        Ok(())
    })?;

    test.with_screenshot("conflict-resolved", |test| {
        test.with_time_scale(20.0, |test| test.drive_until(|world| active_object_count(world) == 0))
    })?;

    Ok(())
}

fn load_plane_limits(world: &mut World) -> Result<(taxi::Limits, nav::Limits)> {
    let mut query = world.query::<&object::types::Type>();
    let object_type = query.iter(world).next().context("Expected at least one object type")?;
    match object_type {
        object::types::Type::Plane { taxi, nav } => Ok((taxi.clone(), nav.clone())),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "test helper keeps callsites explicit about scenario geometry"
)]
fn spawn_test_plane(
    world: &mut World,
    name: &str,
    horizontal: Position<Vec2>,
    altitude: Position<f32>,
    heading: Heading,
    speed: Speed<f32>,
    taxi_limits: &taxi::Limits,
    nav_limits: &nav::Limits,
) {
    let entity = world.spawn_empty().id();
    object::SpawnCommand {
        position:         horizontal.with_altitude(altitude),
        ground_speed:     (speed * heading).with_vertical(Speed::ZERO),
        display:          object::Display { name: String::from(name) },
        destination:      dest::Destination::VacateAnyRunway,
        completion_score: None,
    }
    .apply(world.entity_mut(entity));

    world.entity_mut(entity).insert(taxi_limits.clone());
    plane::SpawnCommand {
        control: Some(plane::Control {
            heading,
            yaw_speed: AngularSpeed::ZERO,
            horiz_accel: Accel::ZERO,
        }),
        limits:  nav_limits.clone(),
    }
    .apply(world.entity_mut(entity));
    object::SetAirborneCommand.apply(world.entity_mut(entity));
}

fn active_object_count(world: &mut World) -> usize {
    let mut query = world.query_filtered::<Entity, With<ActiveObject>>();
    query.iter(world).count()
}
