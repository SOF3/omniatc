use bevy::camera::Camera;
use bevy::ecs::component::Component;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::ecs::query::With;
use bevy::ecs::system::Query;
use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use bevy::math::{Vec2, Vec3};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy_mod_config::ReadConfig;
use math::Angle;
use omniatc::level::quest;
use omniatc::try_log;

#[derive(Component, Default)]
pub(super) struct CameraDragState {
    start_camera_translation: Option<Vec3>,
}

pub(super) fn drag_camera_system(
    camera_query: Query<(
        &mut Transform,
        &Camera,
        &GlobalTransform,
        &super::UiState,
        &mut CameraDragState,
    )>,
    conf: ReadConfig<super::Conf>,
    mut quest_ui_events: MessageWriter<quest::UiEvent>,
) {
    let conf = conf.read();

    for (mut camera_tf, camera, global_tf, ui_state, mut drag_state) in camera_query {
        let Some(dragging) = &ui_state.right_dragging else { continue };
        let Some(current) = ui_state.hovered else {
            // If not hovered, we just assume the cursor was at its last position within viewport.
            continue;
        };
        let viewport_delta = current.viewport - dragging.start.viewport;
        let world_delta =
            Vec3::from((viewport_to_world_delta(viewport_delta, camera, global_tf), 0.0));
        let start_translation = if dragging.just_started {
            *drag_state.start_camera_translation.insert(camera_tf.translation)
        } else {
            try_log!(drag_state.start_camera_translation, expect "!just_started implies start_camera_translation is set" or continue)
        };

        camera_tf.translation = match conf.camera_drag_direction {
            super::CameraDragDirectionRead::WithMap => start_translation - world_delta,
            super::CameraDragDirectionRead::WithCamera => start_translation + world_delta,
        };

        quest_ui_events.write(quest::UiEvent::CameraDragged);
    }
}

fn viewport_to_world_delta(
    viewport_delta: Vec2,
    camera: &Camera,
    global_tf: &GlobalTransform,
) -> Vec2 {
    camera.viewport_to_world_2d(global_tf, viewport_delta).unwrap_or_default()
        - camera.viewport_to_world_2d(global_tf, Vec2::ZERO).unwrap_or_default()
}

pub(super) fn scroll_zoom_system(
    mut wheel_events: MessageReader<MouseWheel>,
    camera_query: Query<(&super::UiState, &mut Transform), With<Camera>>,
    conf: ReadConfig<super::Conf>,
    mut ui_event_writer: MessageWriter<quest::UiEvent>,
) {
    let conf = conf.read();

    let mut total_scroll = 1.0;
    let mut total_rotation = Angle::ZERO;
    for event in wheel_events.read() {
        let (scroll_step, rotation_step) = match event.unit {
            MouseScrollUnit::Line => (conf.scroll_step_line, conf.rotation_step_line),
            MouseScrollUnit::Pixel => (conf.scroll_step_pixel, conf.rotation_step_pixel),
        };
        total_scroll *= scroll_step.powf(-event.y);
        total_rotation += rotation_step * event.x;
    }

    for (ui_state, mut camera_tf) in camera_query {
        if ui_state.hovered.is_none() {
            continue;
        }

        #[expect(
            clippy::float_cmp,
            reason = "total_scroll would be exactly 1.0 if there were no scroll events"
        )]
        if total_scroll != 1.0 {
            camera_tf.scale *= total_scroll;
            ui_event_writer.write(quest::UiEvent::CameraZoomed);
        }

        if !total_rotation.is_zero() {
            camera_tf.rotate_z(total_rotation.0);
            ui_event_writer.write(quest::UiEvent::CameraRotated);
        }
    }
}
