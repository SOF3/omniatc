use std::cmp;

use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventReader;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut, Single};
use bevy::input::mouse::{MouseButton, MouseMotion, MouseWheel};
use bevy::input::ButtonInput;
use bevy::math::{FloatExt, UVec2, Vec2, Vec3};
use bevy::render::camera::{Camera, Viewport};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy::window::Window;
use bevy_egui::EguiContextPass;
use omniatc_core::level::object;
use omniatc_core::units::{Angle, Distance};
use omniatc_core::{store, try_log_return};
use omniatc_macros::Config;
use ordered_float::OrderedFloat;

use crate::config::AppExt;
use crate::render::object_info;
use crate::{config, input, EguiSystemSets, EguiUsedMargins, UpdateSystemSets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<Conf>();

        app.add_systems(app::Startup, setup_system);
        app.add_systems(EguiContextPass, fit_layout_system.in_set(EguiSystemSets::TwoDim));

        app.add_systems(app::Update, consume_camera_advice.before(UpdateSystemSets::Input));

        app.add_systems(
            app::Update,
            handle_drag_system
                .in_set(UpdateSystemSets::Input)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
        app.add_systems(
            app::Update,
            handle_scroll_system
                .before(handle_drag_system)
                .in_set(UpdateSystemSets::Input)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
        app.add_systems(
            app::Update,
            handle_select_system
                .in_set(UpdateSystemSets::Input)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
        app.add_systems(
            app::Update,
            highlight_selected_system
                .after(handle_select_system)
                .in_set(super::object::SetColorThemeSystemSet::UserInteract),
        );
    }
}

/// Window layout position of the camera panel.
#[derive(Component)]
pub struct Layout {
    /// Order of the panel, from inner to outer.
    pub order:     usize,
    /// Direction to add the panel beyond the previous ones.
    pub direction: Direction,
    /// Ratio of the panel relative the previous ones.
    pub ratio:     f32,
}

pub enum Direction {
    Top,
    Bottom,
    Left,
    Right,
}

fn setup_system(mut commands: Commands) {
    commands.spawn((Camera2d, Layout { order: 0, direction: Direction::Top, ratio: 1. }));

    // Example
    // commands.spawn((Camera2d, Layout { order: 1, direction: Direction::Right, ratio: 0.5 }));
}

fn consume_camera_advice(
    mut advice: ResMut<store::CameraAdvice>,
    camera_query: Query<(&Camera, &mut Transform, &GlobalTransform)>,
) {
    let Some(store::Camera::TwoDimension(desired)) = &advice.0 else { return };

    for (camera, mut camera_tf, global_tf) in camera_query {
        camera_tf.translation = Vec3::from((desired.center.get(), 0.));
        camera_tf.rotation = desired.up.into_rotation_quat();

        let Some(viewport_size) = camera.logical_viewport_size() else { return };

        let (start_world_pos, end_world_pos, viewport_dim): (_, _, fn(Vec2) -> f32) =
            match desired.scale_axis {
                store::AxisDirection::X => (
                    desired.center - Distance::ZERO.with_x(desired.scale_length / 2.),
                    desired.center + Distance::ZERO.with_x(desired.scale_length / 2.),
                    |vec| vec.x,
                ),
                store::AxisDirection::Y => (
                    desired.center - Distance::ZERO.with_y(desired.scale_length / 2.),
                    desired.center + Distance::ZERO.with_y(desired.scale_length / 2.),
                    |vec| vec.y,
                ),
            };

        match (
            camera.world_to_viewport(global_tf, Vec3::from((start_world_pos.get(), 0.))),
            camera.world_to_viewport(global_tf, Vec3::from((end_world_pos.get(), 0.))),
        ) {
            (Ok(start_viewport_pos), Ok(end_viewport_pos)) => {
                let current_width = viewport_dim(end_viewport_pos - start_viewport_pos);
                let desired_width = viewport_dim(viewport_size);
                camera_tf.scale /= desired_width / current_width;
            }
            (ret1, ret2) => {
                bevy::log::error!("viewport coordinate conversion error: {ret1:?} {ret2:?}");
                return;
            }
        }
    }

    // Clear advice if all viewports have been successfully updated.
    advice.0 = None;
}

fn fit_layout_system(
    window: Option<Single<&mut Window>>,
    margins: Res<EguiUsedMargins>,
    mut camera_query: Query<(&Layout, &mut Camera)>,
) {
    let Some(window) = window else { return };

    let mut camera_order: Vec<_> = camera_query.iter_mut().collect();
    camera_order.sort_by_key(|(layout, _)| cmp::Reverse(layout.order));

    let mut start_pos = Vec2::new(margins.left, margins.top) * window.scale_factor();
    #[expect(clippy::cast_precision_loss)]
    let mut end_pos = Vec2::new(window.physical_width() as f32, window.physical_height() as f32)
        - Vec2::new(margins.right, margins.bottom) * window.scale_factor();

    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // TODO at least validate the float sign
    for (layout, mut camera) in camera_order {
        let (my_rect, rem_rect) = match layout.direction {
            Direction::Top => (
                (start_pos, Vec2::new(end_pos.x, start_pos.y.lerp(end_pos.y, layout.ratio))),
                (Vec2::new(start_pos.x, start_pos.y.lerp(end_pos.y, layout.ratio)), end_pos),
            ),
            Direction::Bottom => (
                (Vec2::new(start_pos.x, end_pos.y.lerp(start_pos.y, layout.ratio)), end_pos),
                (start_pos, Vec2::new(end_pos.x, end_pos.y.lerp(start_pos.y, layout.ratio))),
            ),
            Direction::Left => (
                (start_pos, Vec2::new(start_pos.x.lerp(end_pos.x, layout.ratio), end_pos.y)),
                (Vec2::new(start_pos.x.lerp(end_pos.x, layout.ratio), start_pos.y), end_pos),
            ),
            Direction::Right => (
                (Vec2::new(end_pos.x.lerp(start_pos.x, layout.ratio), start_pos.y), end_pos),
                (start_pos, Vec2::new(end_pos.x.lerp(start_pos.x, layout.ratio), end_pos.y)),
            ),
        };

        start_pos = rem_rect.0;
        end_pos = rem_rect.1;

        let my_start = UVec2::new(my_rect.0.x as u32, my_rect.0.y as u32);
        camera.viewport = Some(Viewport {
            physical_position: my_start,
            physical_size:     UVec2::new(my_rect.1.x as u32, my_rect.1.y as u32) - my_start,
            depth:             0.0..1.0,
        });
        camera.order = layout.order.try_into().expect("layout order out of bounds");
    }
}

struct DraggingState {
    camera_entity:      Entity,
    start_viewport_pos: Vec2,
    start_translation:  Vec3,
}

fn handle_drag_system(
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion_events: EventReader<MouseMotion>,
    mut dragging_camera: Local<Option<DraggingState>>,
    current_cursor_camera: Res<input::CurrentCursorCamera>,
    window: Option<Single<&Window>>,
    mut camera_query: Query<(&mut Transform, &Camera, &GlobalTransform), With<Camera2d>>,
) {
    let Some(window) = window else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };

    match (&mut *dragging_camera, buttons.pressed(MouseButton::Right)) {
        (option @ Some(_), false) => {
            // stop dragging
            *option = None;
        }
        (option @ None, true) => {
            // start dragging
            if let Some(ref camera_value) = current_cursor_camera.0 {
                if let Ok((camera_tf, _, _)) = camera_query
                    .get(camera_value.camera_entity) {
                    *option = Some(DraggingState {
                        camera_entity:      camera_value.camera_entity,
                        start_viewport_pos: camera_value.viewport_pos,
                        start_translation:  camera_tf.translation,
                    });
                }
            }
        }
        (Some(_), true) // continue dragging
        | (None, false) => {} // not dragging
    }

    let Some(DraggingState { camera_entity, start_viewport_pos, start_translation }) =
        *dragging_camera
    else {
        return;
    };

    let has_moved = motion_events.read().count() > 0; // drain the all events in the iterator
    if !has_moved {
        return;
    }

    let (mut camera_tf, camera, global_tf) = try_log_return!(
        camera_query.get_mut(camera_entity),
        expect "invalid camera entity"
    );

    let Some(viewport_rect) = camera.logical_viewport_rect() else { return };
    let viewport_pos = cursor_pos - viewport_rect.min;

    // We have moved from start_viewport_pos to viewport_pos,
    // so we want to add this delta to start_translation.

    let curr_world_pos = camera.viewport_to_world_2d(global_tf, viewport_pos);
    let start_equiv_world_pos = camera.viewport_to_world_2d(global_tf, start_viewport_pos);

    if let (Ok(start_equiv_world_pos), Ok(curr_world_pos)) = (start_equiv_world_pos, curr_world_pos)
    {
        camera_tf.translation =
            start_translation - Vec3::from((curr_world_pos - start_equiv_world_pos, 0.));
    }
}

fn handle_scroll_system(
    mut wheel_events: EventReader<MouseWheel>,
    current_cursor_camera: Res<input::CurrentCursorCamera>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
    conf: config::Read<Conf>,
) {
    for event in wheel_events.read() {
        if let Some(input::CurrentCursorCameraValue { camera_entity, world_pos: _, .. }) =
            current_cursor_camera.0
        {
            let mut camera_tf = camera_query.get_mut(camera_entity).expect(
                "CurrentCursorCamera::update_system should maintain an updated camera entity",
            );
            let scale_rate = conf.scroll_step.powf(-event.y);

            // ensure (camera_tf.translation - world_pos) / camera_tf.scale is unchanged
            // i.e. (new_translation - world_pos) / new_scale = (camera_tf.translation - world_pos) / camera_tf.scale
            // i.e. new_translation = (camera_tf.translation - world_pos) * (new_scale / camera_tf.scale) + world_pos
            // camera_tf.translation = (camera_tf.translation - Vec3::from((world_pos, 0.))) * scale_rate + Vec3::from((world_pos, 0.)); // TODO FIXME
            camera_tf.scale *= scale_rate;

            let rot_rate = conf.rotation_step * event.x;
            camera_tf.rotate_z(rot_rate.0);
        }
    }
}

fn handle_select_system(
    buttons: Res<ButtonInput<MouseButton>>,
    current_cursor_camera: Res<input::CurrentCursorCamera>,
    mut current_hovered_object: ResMut<object_info::CurrentHoveredObject>,
    mut current_object: ResMut<object_info::CurrentObject>,
    is_2d_query: Query<&GlobalTransform, With<Camera2d>>,
    object_query: Query<(Entity, &object::Object)>,
    conf: config::Read<Conf>,
) {
    current_hovered_object.0 = None;

    if let Some(camera_value) = &current_cursor_camera.0 {
        if let Ok(camera_tf) = is_2d_query.get(camera_value.camera_entity) {
            let click_tolerance = Distance(camera_tf.scale().x) * conf.click_tolerance;

            let closest_object = object_query
                .iter()
                .map(|(entity, object)| {
                    (entity, object.position.horizontal().distance_squared(camera_value.world_pos))
                })
                .filter(|(_, dist_sq)| *dist_sq < click_tolerance.squared())
                .min_by_key(|(_, dist_sq)| OrderedFloat(dist_sq.0));
            if let Some((object, _)) = closest_object {
                current_hovered_object.0 = Some(object);
                if buttons.just_pressed(MouseButton::Left) {
                    current_object.0 = Some(object);
                }
            }
        }
    }
}

fn highlight_selected_system(
    conf: config::Read<Conf>,
    current_hovered_object: Res<object_info::CurrentHoveredObject>,
    current_object: Res<object_info::CurrentObject>,
    mut color_theme_query: Query<&mut super::object::ColorTheme>,
) {
    if let Some(entity) = current_hovered_object.0 {
        let mut theme = try_log_return!(color_theme_query.get_mut(entity), expect "CurrentObject is Some and must reference valid object entity");
        theme.body = conf.hovered_color;
    }

    if let Some(entity) = current_object.0 {
        let mut theme = try_log_return!(color_theme_query.get_mut(entity), expect "CurrentObject is Some and must reference valid object entity");
        theme.body = conf.selected_color;
    }
}

#[derive(Resource, Config)]
#[config(id = "2d/camera", name = "Camera (2D)")]
struct Conf {
    /// Zoom speed based on vertical scroll.
    scroll_step:     f32,
    /// Rotation speed based on horizontal scroll.
    rotation_step:   Angle<f32>,
    /// Tolerated distance when clicking on an object in window coordinates.
    click_tolerance: f32,
    /// Hovered objects are highlighted with this color.
    hovered_color:   Color,
    /// Selected objects are highlighted with this color.
    selected_color:  Color,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            scroll_step:     1.05,
            rotation_step:   Angle::from_degrees(6.),
            click_tolerance: 50.,
            hovered_color:   Color::srgb(0.5, 1., 0.7),
            selected_color:  Color::srgb(0.5, 0.7, 1.),
        }
    }
}
