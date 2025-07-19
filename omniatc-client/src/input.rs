use bevy::app::{self, App, Plugin};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, Or, QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Query, Res, ResMut, Single};
use bevy::input::keyboard::KeyCode;
use bevy::input::ButtonInput;
use bevy::math::primitives::InfinitePlane3d;
use bevy::math::{Vec2, Vec3};
use bevy::render::camera::Camera;
use bevy::transform::components::GlobalTransform;
use bevy::window::Window;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass};
use math::Position;
use omniatc::try_log_return;

use crate::render::{threedim, twodim};
use crate::{EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentCursorCamera>();
        app.add_systems(
            app::Update,
            CurrentCursorCamera::update_system.before(ReadCurrentCursorCameraSystemSet),
        );
        app.init_resource::<Hotkeys>();
        app.add_systems(
            EguiPrimaryContextPass,
            Hotkeys::update_system
                .in_set(EguiSystemSets::Init)
                .ambiguous_with(EguiUsedMargins::reset_system),
        );
    }
}

#[derive(Resource, Default)]
pub struct CurrentCursorCamera(pub Option<CurrentCursorCameraValue>);

#[derive(Clone, Copy)]
pub struct CurrentCursorCameraValue {
    pub camera_entity: Entity,
    pub viewport_pos:  Vec2,
    pub world_pos:     Position<Vec2>,
}

#[derive(QueryData)]
struct CameraData {
    camera_entity: Entity,
    camera:        &'static Camera,
    global_tf:     &'static GlobalTransform,
    is_twodim:     Has<twodim::camera::Layout>,
    is_threedim:   Has<threedim::CameraLayout>,
}

impl CurrentCursorCamera {
    fn update_system(
        mut target: ResMut<Self>,
        window: Option<Single<&Window>>,
        camera_query: Query<
            CameraData,
            Or<(With<twodim::camera::Layout>, With<threedim::CameraLayout>)>,
        >,
    ) {
        let Some(window) = window else {
            target.0 = None;
            return;
        };

        target.0 = None;

        let Some(cursor_pos) = window.cursor_position() else {
            return;
        };
        let Some((data, viewport_pos)) = camera_query.iter().find_map(|data| {
            if let Some(viewport_rect) = data.camera.logical_viewport_rect() {
                if viewport_rect.contains(cursor_pos) {
                    let viewport_pos = cursor_pos - viewport_rect.min;
                    return Some((data, viewport_pos));
                }
            }
            None
        }) else {
            return;
        };
        if data.is_twodim {
            let world_pos = try_log_return!(
                data.camera.viewport_to_world_2d(data.global_tf, cursor_pos),
                expect "viewport should be valid"
            );
            target.0 = Some(CurrentCursorCameraValue {
                camera_entity: data.camera_entity,
                viewport_pos,
                world_pos: Position::new(world_pos),
            });
        } else if data.is_threedim {
            // TODO support 3D ca
            let ray = try_log_return!(
                data.camera.viewport_to_world(data.global_tf, cursor_pos),
                expect "viewport should be valid"
            );
            let dist = ray.intersect_plane(
                Position::from_origin_nm(0., 0.).with_altitude(Position::SEA_LEVEL).get(),
                InfinitePlane3d::new(Vec3::Z),
            );
            if let Some(dist) = dist.filter(|&dist| dist > 0.) {
                let world_pos = Position::new(ray.get_point(dist)).horizontal();
                target.0 = Some(CurrentCursorCameraValue {
                    camera_entity: data.camera_entity,
                    viewport_pos,
                    world_pos,
                });
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct ReadCurrentCursorCameraSystemSet;

#[derive(Resource, Default)]
#[expect(clippy::struct_excessive_bools)] // multiple independent flags
pub struct Hotkeys {
    pub search:          bool,
    pub deselect:        bool,
    pub fast_forward:    bool,
    pub toggle_pause:    bool,
    pub pick_vector:     bool,
    pub set_speed:       bool,
    pub inc_speed:       bool,
    pub dec_speed:       bool,
    pub set_heading:     bool,
    pub inc_heading:     bool,
    pub dec_heading:     bool,
    pub set_altitude:    bool,
    pub inc_altitude:    bool,
    pub dec_altitude:    bool,
    pub toggle_expedite: bool,
    pub next_route:      bool,
}

impl Hotkeys {
    fn update_system(
        mut this: ResMut<Self>,
        mut contexts: EguiContexts,
        buttons: Res<ButtonInput<KeyCode>>,
    ) {
        if contexts.ctx_mut().is_ok_and(|ctx| ctx.wants_keyboard_input()) {
            *this = Self::default();
            return;
        }

        this.search = buttons.just_pressed(KeyCode::Slash);
        this.deselect = buttons.just_pressed(KeyCode::Escape);
        this.fast_forward = buttons.pressed(KeyCode::Space) && buttons.pressed(KeyCode::ShiftLeft);
        this.toggle_pause =
            buttons.just_pressed(KeyCode::Space) && !buttons.pressed(KeyCode::ShiftLeft);
        this.pick_vector = buttons.pressed(KeyCode::KeyV);
        this.set_speed = buttons.just_pressed(KeyCode::KeyS);
        this.inc_speed = buttons.just_pressed(KeyCode::Period);
        this.dec_speed = buttons.just_pressed(KeyCode::Comma);
        this.set_heading = buttons.just_pressed(KeyCode::KeyH);
        this.inc_heading = buttons.just_pressed(KeyCode::ArrowRight);
        this.dec_heading = buttons.just_pressed(KeyCode::ArrowLeft);
        this.set_altitude = buttons.just_pressed(KeyCode::KeyA);
        this.inc_altitude = buttons.just_pressed(KeyCode::ArrowUp);
        this.dec_altitude = buttons.just_pressed(KeyCode::ArrowDown);
        this.toggle_expedite = buttons.just_pressed(KeyCode::KeyX);
        this.next_route = buttons.just_pressed(KeyCode::KeyR);
    }
}
