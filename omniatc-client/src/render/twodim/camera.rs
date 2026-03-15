use std::cmp;
use std::f32::consts::PI;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Assets};
use bevy::camera::{
    Camera, Camera2d, ClearColor, ClearColorConfig, ImageRenderTarget, RenderTarget, Viewport,
};
use bevy::color::{Color, Mix};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::{MessageReader, MessageWriter};
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut, Single, SystemParam};
use bevy::image::Image;
use bevy::input::mouse::{MouseMotion, MouseScrollUnit, MouseWheel};
use bevy::math::{FloatExt, UVec2, Vec2, Vec3};
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::time::{self, Time};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy::window::Window;
use bevy_egui::egui::WidgetText;
use bevy_egui::egui::load::SizedTexture;
use bevy_egui::{
    EguiContexts, EguiGlobalSettings, EguiPrimaryContextPass, EguiTextureHandle, EguiUserTextures,
    PrimaryEguiContext, egui,
};
use bevy_mod_config::{AppExt, Config, ReadConfig};
use math::{Angle, Length};
use omniatc::level::quest;
use omniatc::{QueryTryLog, load};
use serde::{Deserialize, Serialize};

use crate::render::{dock, tutorial_popup};
use crate::{ConfigManager, EguiState, EguiSystemSets, UpdateSystemSets, input, util};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("2d:camera");

        app.add_systems(app::Update, consume_camera_advice.before(UpdateSystemSets::Input));

        app.add_systems(
            app::Update,
            drag_camera_system
                .in_set(UpdateSystemSets::Input)
                .in_set(quest::UiEventWriterSystemSet)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
        app.add_systems(
            app::Update,
            scroll_zoom_system
                .before(drag_camera_system)
                .in_set(UpdateSystemSets::Input)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
        app.add_systems(EguiPrimaryContextPass, nullify_camera_system.before(EguiSystemSets::Dock));
    }
}

/// Marks the entity as a 2D world camera.
#[derive(Component)]
pub struct Marker;

pub enum Direction {
    Top,
    Bottom,
    Left,
    Right,
}

fn consume_camera_advice(
    mut advice: ResMut<load::CameraAdvice>,
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
                    desired.center - Length::ZERO.with_x(desired.scale_length / 2.),
                    desired.center + Length::ZERO.with_x(desired.scale_length / 2.),
                    |vec| vec.x,
                ),
                store::AxisDirection::Y => (
                    desired.center - Length::ZERO.with_y(desired.scale_length / 2.),
                    desired.center + Length::ZERO.with_y(desired.scale_length / 2.),
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

fn clear_color_system(
    window: Option<Single<&mut Window>>,
    request_highlight: Option<
        Single<(), (With<tutorial_popup::Focused>, With<quest::highlight::RadarView>)>,
    >,
    mut clear_color: ResMut<ClearColor>,
    fixed_time: Res<Time<time::Real>>,
) {
    let Some(window) = window else { return };

    if request_highlight.is_some() {
        const PERIOD: Duration = Duration::from_secs(3);
        let millis = fixed_time.elapsed().as_millis() % PERIOD.as_millis();
        #[expect(clippy::cast_precision_loss, reason = "PERIOD restricts millis to a small value")]
        let fract = millis as f32 / PERIOD.as_millis() as f32;
        let phase = ((fract * PI).sin() + 1.0) * 0.5;
        clear_color.0 = Color::BLACK.mix(&Color::WHITE, phase * 0.1);
    } else {
        clear_color.0 = Color::BLACK;
    }
}

struct DraggingState {
    camera_entity:      Entity,
    start_viewport_pos: Vec2,
    start_translation:  Vec3,
}

fn drag_camera_system(
    mut motion_events: MessageReader<MouseMotion>,
    mut dragging_camera: Local<Option<DraggingState>>,
    cursor_state: Res<input::CursorState>,
    mut drag_state: ResMut<input::CursorDragState>,
    window: Option<Single<&Window>>,
    mut camera_query: Query<(&mut Transform, &Camera, &GlobalTransform), With<Camera2d>>,
    conf: ReadConfig<Conf>,
    egui: Res<EguiState>,
    mut quest_ui_events: MessageWriter<quest::UiEvent>,
) {
    let conf = conf.read();

    let Some(window) = window else { return };
    let Some(cursor_pos) = window.cursor_position() else { return };
    if egui.pointer_acquired {
        return;
    }

    match (&mut *dragging_camera, drag_state.is_dragging(&cursor_state, util::new_type_id!(DragCamera), cursor_state.right.is_down)) {
        (option @ Some(_), false) => {
            // stop dragging
            *option = None;
        }
        (option @ None, true) => {
            // start dragging
            if let Some(ref camera_value) = cursor_state.value
                && let Ok((camera_tf, _, _)) = camera_query
                    .get(camera_value.camera_entity) {
                    *option = Some(DraggingState {
                        camera_entity:      camera_value.camera_entity,
                        start_viewport_pos: camera_value.viewport_pos,
                        start_translation:  camera_tf.translation,
                    });
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

    let Some((mut camera_tf, camera, global_tf)) = camera_query.log_get_mut(camera_entity) else {
        return;
    };

    let Some(viewport_rect) = camera.logical_viewport_rect() else { return };
    let viewport_pos = cursor_pos - viewport_rect.min;

    let curr_world_pos = camera.viewport_to_world_2d(global_tf, viewport_pos);
    let start_equiv_world_pos = camera.viewport_to_world_2d(global_tf, start_viewport_pos);

    // We have moved from start_viewport_pos to viewport_pos,
    // so we want to add this delta to start_translation.

    if let (Ok(start_equiv_world_pos), Ok(curr_world_pos)) = (start_equiv_world_pos, curr_world_pos)
    {
        let delta = Vec3::from((curr_world_pos - start_equiv_world_pos, 0.));
        match conf.camera_drag_direction {
            CameraDragDirectionRead::WithMap => camera_tf.translation = start_translation - delta,
            CameraDragDirectionRead::WithCamera => {
                camera_tf.translation = start_translation + delta;
            }
        }

        if delta != Vec3::ZERO {
            quest_ui_events.write(quest::UiEvent::CameraDragged);
        }
    }
}

fn scroll_zoom_system(
    mut wheel_events: MessageReader<MouseWheel>,
    current_cursor_camera: Res<input::CursorState>,
    mut camera_query: Query<&mut Transform, With<Camera>>,
    conf: ReadConfig<Conf>,
    egui: Res<EguiState>,
    mut ui_event_writer: MessageWriter<quest::UiEvent>,
) {
    let conf = conf.read();

    if egui.pointer_acquired {
        return;
    }

    for event in wheel_events.read() {
        if let Some(input::CurrentCursorCameraValue { camera_entity, .. }) =
            current_cursor_camera.value
        {
            let mut camera_tf = camera_query.get_mut(camera_entity).expect(
                "CurrentCursorCamera::update_system should maintain an updated camera entity",
            );
            let (scroll_step, rotation_step) = match event.unit {
                MouseScrollUnit::Line => (conf.scroll_step_line, conf.rotation_step_line),
                MouseScrollUnit::Pixel => (conf.scroll_step_pixel, conf.rotation_step_pixel),
            };

            let scale_rate = scroll_step.powf(-event.y);
            // ensure (camera_tf.translation - world_pos) / camera_tf.scale is unchanged
            // i.e. (new_translation - world_pos) / new_scale = (camera_tf.translation - world_pos) / camera_tf.scale
            // i.e. new_translation = (camera_tf.translation - world_pos) * (new_scale / camera_tf.scale) + world_pos
            // camera_tf.translation = (camera_tf.translation - Vec3::from((world_pos, 0.))) * scale_rate + Vec3::from((world_pos, 0.)); // TODO FIXME
            if event.y != 0.0 {
                camera_tf.scale *= scale_rate;
                ui_event_writer.write(quest::UiEvent::CameraZoomed);
            }

            let rot_rate = rotation_step * event.x;
            if !rot_rate.is_zero() {
                camera_tf.rotate_z(rot_rate.0);
                ui_event_writer.write(quest::UiEvent::CameraRotated);
            }
        }
    }
}

#[derive(Config)]
struct Conf {
    /// Zoom speed based on vertical scroll per line.
    #[config(default = 1.05)]
    scroll_step_line:      f32,
    /// Zoom speed based on vertical scroll per pixel.
    #[config(default = 1.001)]
    scroll_step_pixel:     f32,
    /// Rotation speed based on horizontal scroll per line.
    #[config(default = Angle::from_degrees(4.0))]
    rotation_step_line:    Angle,
    /// Rotation speed based on horizontal scroll per pixel.
    #[config(default = Angle::from_degrees(0.1))]
    rotation_step_pixel:   Angle,
    /// Direction to move camera when dragging with right button.
    camera_drag_direction: CameraDragDirection,
}

#[derive(Clone, Copy, Serialize, Deserialize, Config)]
#[config(expose(read))]
enum CameraDragDirection {
    /// Map follows cursor location.
    WithMap,
    /// Camera follows cursor location.
    WithCamera,
}

pub struct TabType {
    /// The camera entity.
    pub camera:       Entity,
    pub image_handle: asset::Handle<Image>,
    image_id:         Option<egui::TextureId>,
}

impl dock::TabType for TabType {
    type TitleSystemParam<'w, 's> = ();
    fn title(&self, (): ()) -> String { String::from("Radar") }

    type UiSystemParam<'w, 's> = (Query<'w, 's, &'static mut Camera>, ResMut<'w, Assets<Image>>);
    fn ui(
        &mut self,
        (mut param, mut images): Self::UiSystemParam<'_, '_>,
        ui: &mut egui::Ui,
        order: usize,
    ) {
        let Some(mut camera) = param.log_get_mut(self.camera) else { return };
        camera.viewport = Some(Viewport {
            physical_position: UVec2 { x: 0, y: 0 },
            physical_size:     size_to_uvec2(ui.max_rect().size()),
            depth:             0.0..1.0,
        });
        camera.order = -isize::try_from(order).expect("tab order is within isize bounds") - 1;

        let image = images.get_mut(&self.image_handle).expect("strong handle");
        #[expect(clippy::cast_sign_loss, reason = "rect dimensions should be nonnegative")]
        #[expect(
            clippy::cast_possible_truncation,
            reason = "rect dimensions should be within u32 bounds"
        )]
        image.resize(Extent3d {
            width: ui.max_rect().width() as u32,
            height: ui.max_rect().height() as u32,
            ..Default::default()
        });

        if let Some(image_id) = self.image_id {
            let resp = ui.image(SizedTexture::new(image_id, ui.max_rect().size()));
        }
    }

    type OnCloseSystemParam<'w, 's> = ();

    fn prepare_render(&mut self, contexts: &mut EguiContexts) {
        self.image_id = contexts.image_id(&self.image_handle);
    }
}

pub fn new_tab(params: &mut SpawnParams, commands: &mut Commands) -> dock::Tab {
    let image = Image {
        texture_descriptor: TextureDescriptor {
            label:           None,
            size:            Extent3d { width: 512, height: 512, ..Default::default() },
            dimension:       TextureDimension::D2,
            format:          TextureFormat::Bgra8UnormSrgb,
            mip_level_count: 1,
            sample_count:    1,
            usage:           TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_DST
                | TextureUsages::RENDER_ATTACHMENT,
            view_formats:    &[],
        },
        ..Default::default()
    };
    let image_handle = params.images.add(image);
    params.textures.add_image(EguiTextureHandle::Strong(image_handle.clone()));
    let camera = commands
        .spawn((
            Camera2d,
            Marker,
            RenderTarget::Image(ImageRenderTarget {
                handle:       image_handle.clone(),
                scale_factor: 1.0,
            }),
        ))
        .id();
    dock::Tab::TwoDimCamera(TabType { camera, image_handle, image_id: None })
}

#[derive(SystemParam)]
pub struct SpawnParams<'w> {
    images:   ResMut<'w, Assets<Image>>,
    textures: ResMut<'w, EguiUserTextures>,
}

fn nullify_camera_system(camera_query: Query<&mut Camera, With<Marker>>) {
    for mut camera in camera_query {
        camera.viewport = None;
        camera.order = isize::MIN;
    }
}

#[expect(clippy::cast_sign_loss, reason = "all positions are positive")]
#[expect(clippy::cast_possible_truncation, reason = "float positions are within bounds")]
fn pos_to_uvec2(v: egui::Pos2) -> UVec2 { UVec2 { x: v.x as u32, y: v.y as u32 } }

#[expect(clippy::cast_sign_loss, reason = "size should be nonnegative")]
#[expect(clippy::cast_possible_truncation, reason = "viewport size should be within bounds")]
fn size_to_uvec2(v: egui::Vec2) -> UVec2 { UVec2 { x: v.x as u32, y: v.y as u32 } }
