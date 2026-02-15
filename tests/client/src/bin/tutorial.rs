use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result};
use bevy::camera::{Camera, Camera2d};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::world::World;
use bevy::input::mouse::MouseButton;
use bevy::math::Vec2;
use bevy::transform::components::GlobalTransform;
use omniatc::level::object::Display;
use omniatc::level::quest::{Active, Quest};
use omniatc::load::StoredEntity;
use omniatc_client::render::twodim::object::HasSprite;
use omniatc_client_test::{ClientTest, start_test};

fn main() -> Result<()> {
    let mut test = start_test("tutorial")?;

    test.with_screenshot("level-load", |test| {
        test.drive_frames(2);
        // Wait for async IO first to avoid virtual time depending on IO speed.
        sleep(Duration::from_millis(100));

        test.drive_until(|world| {
            world.query_filtered::<Entity, With<StoredEntity>>().iter(world).next().is_some()
        })?;
        test.drive_frames(2);
        sleep(Duration::from_millis(100)); // also wait for fonts to load
        test.drive_frames(2);
        Ok(())
    })?;

    let window_center = test.window_center()?;
    test.with_screenshot("camera-drag", |test| {
        let drag_end = Vec2::new(window_center.x * 0.8, window_center.y);
        test.drag_mouse(MouseButton::Right, window_center, drag_end, 10)?;
        test.drive_frames(2);
        assert_quest_inactive(test, "Tutorial: Camera (1/3)")?;
        Ok(())
    })?;

    test.with_screenshot("camera-zoom", |test| {
        test.scroll_mouse(window_center, Vec2::new(0.0, 1.0), 5)?;
        test.drive_frames(2);
        assert_quest_inactive(test, "Tutorial: Camera (2/3)")?;
        Ok(())
    })?;

    test.with_screenshot("camera-rotate", |test| {
        test.scroll_mouse(window_center, Vec2::new(1.0, 0.0), 5)?;
        test.drive_frames(2);
        assert_quest_inactive(test, "Tutorial: Camera (3/3)")?;
        Ok(())
    })?;

    test.with_screenshot("object-pick", |test| {
        test.drive_until(|world| find_object_entity(world, "ABC123").is_some())?;
        let plane_screen_pos = plane_screen_pos(test.world(), "ABC123")?;
        test.click_at(MouseButton::Left, plane_screen_pos)?;
        test.drive_frames(2);
        assert_quest_inactive(test, "Tutorial: Aircraft control (1/5)")?;
        Ok(())
    })?;

    Ok(())
}

fn assert_quest_inactive(test: &mut ClientTest, title: &str) -> Result<()> {
    let mut query = test.world().query::<(&Quest, Option<&Active>)>();
    let Some((_, active)) = query.iter(test.world()).find(|(quest, _)| quest.title == title) else {
        anyhow::bail!("Quest not found: {title}");
    };
    if active.is_some() {
        anyhow::bail!("Quest still active: {title}");
    }
    Ok(())
}

fn find_object_entity(world: &mut World, name: &str) -> Option<Entity> {
    let mut query = world.query::<(Entity, &Display)>();
    query.iter(world).find(|(_, display)| display.name == name).map(|(entity, _)| entity)
}

fn plane_screen_pos(world: &mut World, name: &str) -> Result<Vec2> {
    let mut object_query = world.query::<(&Display, &HasSprite)>();
    let (_, sprite) = object_query
        .iter(world)
        .find(|(display, _)| display.name == name)
        .context("Plane display not found")?;
    let sprite_entity = sprite.entity();
    let sprite_translation = world
        .get::<GlobalTransform>(sprite_entity)
        .context("Plane sprite missing GlobalTransform")?
        .translation();
    let mut camera_query = world.query_filtered::<(&Camera, &GlobalTransform), With<Camera2d>>();
    let (camera, camera_transform) = camera_query.single(world).context("Expected camera2d")?;
    camera
        .world_to_viewport(camera_transform, sprite_translation)
        .context("Plane not in camera viewport")
}
