use bevy::app::{self, App, Plugin};
use bevy::input::ButtonInput;
use bevy::math::{Vec2, Vec3};
use bevy::prelude::{
    Camera, Camera2d, GlobalTransform, IntoSystemConfigs, KeyCode, Res, Single, Transform, With,
};
use bevy::time::{self, Time};

use super::Config;
use crate::ui::InputState;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Update, move_camera_system.in_set(InputState::Root));
    }
}

fn move_camera_system(
    time: Res<Time<time::Real>>,
    config: Res<Config>,
    mut camera_query: Single<(&mut Transform, &GlobalTransform, &Camera), With<Camera2d>>,
    inputs: Res<ButtonInput<KeyCode>>,
) {
    let (ref mut camera_tf, camera_global_tf, camera) = *camera_query;

    for (key, zoom_sign) in [(KeyCode::Equal, -1.), (KeyCode::Minus, 1.)] {
        if inputs.pressed(key) {
            let scale = config.key_zoom_speed.powf(time.delta_secs() * zoom_sign);
            camera_tf.scale.x *= scale;
            camera_tf.scale.y *= scale;
        }
    }

    let probe_window_pos = Vec2::new(1., 1.);
    let probe_world_pos = match camera.viewport_to_world_2d(camera_global_tf, probe_window_pos) {
        Ok(origin) => origin,
        Err(err) => {
            bevy::log::error!("cannot probe camera orientation: {err:?}");
            return;
        }
    };
    for (key, window_dir) in [
        (KeyCode::KeyW, Vec2::new(0., -1.)),
        (KeyCode::KeyS, Vec2::new(0., 1.)),
        (KeyCode::KeyA, Vec2::new(-1., 0.)),
        (KeyCode::KeyD, Vec2::new(1., 0.)),
    ] {
        if inputs.pressed(key) {
            let offset_world_pos = match camera
                .viewport_to_world_2d(camera_global_tf, probe_window_pos + window_dir)
            {
                Ok(origin) => origin,
                Err(err) => {
                    bevy::log::error!("cannot probe camera orientation: {err:?}");
                    return;
                }
            };

            let dir = offset_world_pos - probe_world_pos;
            camera_tf.translation +=
                Vec3::from((dir * config.key_move_speed * time.delta_secs(), 0.));
        }
    }

    for (key, rotate_sign) in [(KeyCode::KeyQ, -1.), (KeyCode::KeyE, 1.)] {
        if inputs.pressed(key) {
            camera_tf.rotate_z(rotate_sign * config.key_rotate_speed * time.delta_secs());
        }
    }
}
