use std::f32::consts::PI;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::asset::{self, Assets};
use bevy::camera::{Camera, Camera2d, ClearColor, ImageRenderTarget, RenderTarget, Viewport};
use bevy::color::{Color, Mix};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Query, Res, ResMut, Single, SystemParam};
use bevy::image::Image;
use bevy::math::{UVec2, Vec2, Vec3};
use bevy::render::render_resource::{
    Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::time::{self, Time};
use bevy::transform::components::{GlobalTransform, Transform};
use bevy::window::Window;
use bevy_egui::egui::load::SizedTexture;
use bevy_egui::helpers::egui_vec2_into_vec2;
use bevy_egui::{EguiContexts, EguiTextureHandle, EguiUserTextures, egui};
use bevy_mod_config::{AppExt, Config};
use math::{Angle, Length, Position};
use omniatc::level::quest;
use omniatc::{QueryTryLog, load, try_log};
use serde::{Deserialize, Serialize};

use crate::render::{SystemSets, dock, tutorial_popup};
use crate::{ConfigManager, UpdateSystemSets, input};

mod inputs;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("2d:camera");

        app.add_systems(app::Update, consume_camera_advice.before(UpdateSystemSets::Input));

        app.add_systems(
            app::Update,
            inputs::drag_camera_system
                .in_set(UpdateSystemSets::Input)
                .in_set(quest::UiEventWriterSystemSet)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
        app.add_systems(
            app::Update,
            inputs::scroll_zoom_system
                .before(inputs::drag_camera_system)
                .in_set(UpdateSystemSets::Input)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
        app.add_systems(app::Update, clear_color_system.in_set(SystemSets::Update));
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PointerPosition {
    pub viewport: Vec2,
    pub world:    Position<Vec2>,
}

/// Marks the entity as a 2D world camera.
#[derive(Component, Default, Debug)]
#[require(Camera2d, inputs::CameraDragState)]
pub struct UiState {
    /// `Some` if the cursor is currently hovered over this camera
    /// and not dragging from another widget.
    pub hovered: Option<PointerPosition>,

    /// Whether the current frame has an incremental left click.
    pub left_clicked:  bool,
    /// Whether the current frame has an incremental right click.
    pub right_clicked: bool,

    /// `Some` if the user is currently dragging from this camera with the left button,
    /// even if the cursor is currently not hovered over this camera
    /// (e.g. dragging to another widget).
    pub left_dragging:  Option<Dragging>,
    /// `Some` if the user is currently dragging from this camera with the right button,
    /// even if the cursor is currently not hovered over this camera
    /// (e.g. dragging to another widget).
    pub right_dragging: Option<Dragging>,

    /// Top-left corner of the egui image widget in egui coordinates.
    /// Used to convert viewport positions to window coordinates:
    /// `window_pos = (viewport_pos + viewport_offset) * scale_factor`.
    pub viewport_offset: Vec2,
}

#[derive(Debug)]
pub struct Dragging {
    pub just_started: bool,
    pub start:        PointerPosition,
}

impl Dragging {
    #[must_use]
    pub fn start_at(pos: PointerPosition) -> Self { Self { just_started: true, start: pos } }
}

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

        let Some(viewport_size) = camera.logical_viewport_size() else { continue };

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
    let Some(_window) = window else { return };

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

    type UiSystemParam<'w, 's> = (
        Query<'w, 's, (&'static mut Camera, &'static mut UiState, &'static GlobalTransform)>,
        ResMut<'w, Assets<Image>>,
    );
    fn ui(
        &mut self,
        (mut param, mut images): Self::UiSystemParam<'_, '_>,
        ui: &mut egui::Ui,
        order: usize,
    ) {
        let Some((mut camera, mut ui_state, camera_tf)) = param.log_get_mut(self.camera) else {
            return;
        };
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
            let resp = ui.add(
                egui::Image::new(SizedTexture::new(image_id, ui.max_rect().size()))
                    .sense(egui::Sense::click_and_drag()),
            );

            ui_state.viewport_offset = egui_vec2_into_vec2(resp.rect.min.to_vec2());

            ui_state.hovered = resp.hover_pos().and_then(|pos| {
                let viewport_pos = egui_vec2_into_vec2(pos - resp.rect.min);
                let world_pos = camera.viewport_to_world_2d(camera_tf, viewport_pos);
                let world_pos = try_log!(world_pos, expect "convert viewport position {viewport_pos:?} to camera position" or return None);
                Some(PointerPosition { viewport: viewport_pos, world: Position::new(world_pos) })
            });
            ui_state.left_clicked = resp.clicked();
            ui_state.right_clicked = resp.secondary_clicked();

            let ui_state = &mut *ui_state;
            for (dragging, button) in [
                (&mut ui_state.left_dragging, egui::PointerButton::Primary),
                (&mut ui_state.right_dragging, egui::PointerButton::Secondary),
            ] {
                if resp.dragged_by(button)
                    && let Some(hovered) = ui_state.hovered
                {
                    if let Some(dragging) = dragging {
                        dragging.just_started = false;
                    } else {
                        *dragging = Some(Dragging::start_at(hovered));
                    }
                } else if resp.drag_stopped_by(button) {
                    *dragging = None;
                }
            }
        }
    }

    type OnCloseSystemParam<'w, 's> = ();

    type PrepareRenderSystemParam<'w, 's> =
        Query<'w, 's, (&'static mut Camera, &'static mut UiState)>;

    fn prepare_render(
        &mut self,
        contexts: &mut EguiContexts,
        mut param: Self::PrepareRenderSystemParam<'_, '_>,
    ) {
        self.image_id = contexts.image_id(&self.image_handle);
        if let Some((mut camera, mut ui_state)) = param.log_get_mut(self.camera) {
            camera.viewport = None;
            camera.order = isize::MIN;

            ui_state.hovered = None;
            ui_state.left_clicked = false;
            ui_state.right_clicked = false;
        }
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
            UiState::default(),
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

#[expect(clippy::cast_sign_loss, reason = "size should be nonnegative")]
#[expect(clippy::cast_possible_truncation, reason = "viewport size should be within bounds")]
fn size_to_uvec2(v: egui::Vec2) -> UVec2 { UVec2 { x: v.x as u32, y: v.y as u32 } }
