use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result};
use bevy::camera::Camera;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{ReadOnlyQueryData, With};
use bevy::ecs::world::World;
use bevy::input::keyboard;
use bevy::input::mouse::MouseButton;
use bevy::math::Vec2;
use bevy::transform::components::GlobalTransform;
use math::{Angle, Heading, Length, Position, Speed};
use omniatc::level::object::{Display, Object};
use omniatc::level::quest::{Active, Quest};
use omniatc::level::{nav, object};
use omniatc::load::StoredEntity;
use omniatc_client::render::twodim;
use omniatc_client_test::{ClientTest, start_test};

#[expect(clippy::too_many_lines, reason = "choreography is inherently verbose")]
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
        let drag_end = Vec2::new(window_center.x * 0.7, window_center.y);
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
        test.drive_until(|world| {
            query_object_by_name::<(), _>(world, "ABC123", |()| ()).is_some()
        })?;
        let plane_viewport_pos = object_viewport_pos(test.world(), "ABC123")?;
        let plane_screen_pos = test.viewport_to_window(plane_viewport_pos)?;
        test.click_at(MouseButton::Left, plane_screen_pos)?;
        test.drive_frames(2);
        assert_quest_inactive(test, "Tutorial: Aircraft control (1/5)")?;
        Ok(())
    })?;

    {
        const QUEST_ALTITUDE: Position<f32> = Position::from_amsl_feet(6000.0);

        test.with_screenshot("object-altitude-setpoint", |test| {
            for _ in 0..2 {
                test.press_key(keyboard::KeyCode::ArrowDown, keyboard::Key::ArrowDown)?;
            }

            test.press_key(keyboard::KeyCode::Enter, keyboard::Key::Enter)?;

            query_object_by_name::<&nav::TargetAltitude, _>(
                test.world(),
                "ABC123",
                |target_altitude| {
                    target_altitude
                        .altitude
                        .assert_near(QUEST_ALTITUDE, Length::from_feet(1.0))
                        .context("Expected target altitude to be 6000 ft")
                },
            )
            .context("Expect object to exist")??;

            Ok(())
        })?;

        test.with_screenshot("object-altitude-complete", |test| {
            test.with_time_scale(30.0, |test| {
                test.drive_until(|world| {
                    query_object_by_name::<&Object, _>(world, "ABC123", |object| {
                        object.position.altitude().distance_cmp(QUEST_ALTITUDE)
                            < Length::from_feet(10.0)
                    }) == Some(true)
                })
            })?;

            test.drive_frames(2);
            assert_quest_inactive(test, "Tutorial: Aircraft control (2/5)")?;
            Ok(())
        })?;
    }

    {
        const QUEST_SPEED: Speed<f32> = Speed::from_knots(230.0);

        test.with_screenshot("object-speed-setpoint", |test| {
            for _ in 0..5 {
                test.press_key(keyboard::KeyCode::Comma, keyboard::Key::Character(",".into()))?;
            }

            test.press_key(keyboard::KeyCode::Enter, keyboard::Key::Enter)?;

            query_object_by_name::<&nav::VelocityTarget, _>(
                test.world(),
                "ABC123",
                |target_speed| {
                    target_speed
                        .horiz_speed
                        .assert_near(QUEST_SPEED, Speed::from_knots(1.0))
                        .context("Expected target speed to be 230 knots")
                },
            )
            .context("Expect object to exist")??;

            Ok(())
        })?;

        test.with_screenshot("object-speed-complete", |test| {
            test.with_time_scale(10.0, |test| {
                test.drive_until(|world| {
                    query_object_by_name::<&object::Airborne, _>(world, "ABC123", |object| {
                        object
                            .airspeed
                            .horizontal()
                            .magnitude_exact()
                            .assert_approx(QUEST_SPEED, Speed::from_knots(1.0))
                            .is_ok()
                    }) == Some(true)
                })
            })?;

            test.drive_frames(2);
            assert_quest_inactive(test, "Tutorial: Aircraft control (3/5)")?;
            Ok(())
        })?;
    }

    {
        test.with_screenshot("object-heading-setpoint", |test| {
            let click_pos = {
                let world_pos =
                    query_object_by_name::<&Object, _>(test.world(), "ABC123", |object| {
                        object.position.horizontal() + Length::from_nm(8.0) * Heading::NORTH
                    })
                    .context("Expect object to exist")?;
                let (camera, global_tf) = test
                    .world()
                    .query_filtered::<(&Camera, &GlobalTransform), With<twodim::camera::UiState>>()
                    .single(test.world())
                    .context("Expected 2D camera")?;
                let viewport_pos = camera
                    .world_to_viewport(
                        global_tf,
                        world_pos.with_altitude(Position::SEA_LEVEL).get(),
                    )
                    .context("Convert world pos to viewport pos")?;
                test.viewport_to_window(viewport_pos)?
            };
            test.set_cursor_position(click_pos)?;
            test.press_key(keyboard::KeyCode::KeyV, keyboard::Key::Character("v".into()))?;

            test.drive_until(|world| {
                query_object_by_name::<&nav::VelocityTarget, _>(world, "ABC123", |target| {
                    target
                        .yaw
                        .heading()
                        .assert_approx(Heading::NORTH, Angle::from_degrees(5.0))
                        .is_ok()
                }) == Some(true)
            })?;

            Ok(())
        })?;

        test.with_screenshot("object-heading-complete", |test| {
            test.with_time_scale(10.0, |test| {
                test.drive_until(|world| {
                    query_object_by_name::<&object::Airborne, _>(world, "ABC123", |object| {
                        object
                            .airspeed
                            .horizontal()
                            .heading()
                            .assert_approx(Heading::NORTH, Angle::from_degrees(5.0))
                            .is_ok()
                    }) == Some(true)
                })
            })?;

            test.drive_frames(2);
            assert_quest_inactive(test, "Tutorial: Aircraft control (4/5)")?;
            Ok(())
        })?;
    }

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

fn query_object_by_name<D: ReadOnlyQueryData + 'static, R>(
    world: &mut World,
    name: &str,
    then: impl for<'w, 's> FnOnce(D::Item<'w, 's>) -> R,
) -> Option<R> {
    let mut query = world.query::<(D, &Display)>();
    query.iter(world).find(|(_, display)| display.name == name).map(|(data, _)| then(data))
}

fn object_viewport_pos(world: &mut World, name: &str) -> Result<Vec2> {
    let mut object_query = world.query::<(&Display, &twodim::object::HasSprite)>();
    let (_, sprite) = object_query
        .iter(world)
        .find(|(display, _)| display.name == name)
        .context("Plane display not found")?;
    let sprite_entity = sprite.entity();
    let sprite_translation = world
        .get::<GlobalTransform>(sprite_entity)
        .context("Plane sprite missing GlobalTransform")?
        .translation();
    let mut camera_query =
        world.query_filtered::<(&Camera, &GlobalTransform), With<twodim::camera::UiState>>();
    let (camera, camera_transform) = camera_query.single(world).context("Expected camera2d")?;
    camera
        .world_to_viewport(camera_transform, sprite_translation)
        .context("Plane not in camera viewport")
}
