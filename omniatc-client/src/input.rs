use bevy::app::{self, App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, Or, QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Query, ResMut, Single};
use bevy::math::primitives::InfinitePlane3d;
use bevy::math::{Vec2, Vec3};
use bevy::render::camera::Camera;
use bevy::transform::components::GlobalTransform;
use bevy::window::Window;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bevy_mod_config::{AppExt, Config, ReadConfig};
use math::Position;
use omniatc::try_log_return;

use crate::render::{threedim, twodim};
use crate::{ConfigManager, EguiSystemSets, EguiUsedMargins};

pub mod key_field;
pub use key_field::KeySet;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("input");
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
            if let Some(viewport_rect) = data.camera.logical_viewport_rect()
                && viewport_rect.contains(cursor_pos)
            {
                let viewport_pos = cursor_pos - viewport_rect.min;
                return Some((data, viewport_pos));
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
    fn update_system(mut this: ResMut<Self>, mut contexts: EguiContexts, conf: ReadConfig<Conf>) {
        let Ok(ctx) = contexts.ctx_mut() else { return };

        if ctx.wants_keyboard_input() {
            *this = Self::default();
            return;
        }

        let conf = conf.read();

        ctx.input(|state| {
            this.search = conf.level_control.search.clicked(state);
            this.deselect = conf.level_control.deselect.clicked(state);
            this.fast_forward = conf.level_control.fast_forward.down(state);
            this.toggle_pause = conf.level_control.toggle_pause.clicked(state);
            this.pick_vector = conf.picking.pick_vector.down(state);
            this.set_speed = conf.object_control.speed.set.clicked(state);
            this.inc_speed = conf.object_control.speed.inc.clicked_or_repeated(state);
            this.dec_speed = conf.object_control.speed.dec.clicked_or_repeated(state);
            this.set_heading = conf.object_control.heading.set.clicked(state);
            this.inc_heading = conf.object_control.heading.inc.clicked_or_repeated(state);
            this.dec_heading = conf.object_control.heading.dec.clicked_or_repeated(state);
            this.set_altitude = conf.object_control.altitude.set.clicked(state);
            this.inc_altitude = conf.object_control.altitude.inc.clicked_or_repeated(state);
            this.dec_altitude = conf.object_control.altitude.dec.clicked_or_repeated(state);
            this.toggle_expedite = conf.object_control.altitude.toggle_expedite.clicked(state);
            this.next_route = conf.object_control.next_route.clicked_or_repeated(state);
        });
    }
}

#[derive(Config)]
struct Conf {
    level_control:  LevelControlConf,
    picking:        PickingConf,
    object_control: ObjectControlConf,
}

#[derive(Config)]
struct LevelControlConf {
    #[config(default = KeySet::from(egui::Key::Slash))]
    search:       KeySet,
    #[config(default = KeySet::from(egui::Key::Escape))]
    deselect:     KeySet,
    #[config(default = KeySet::from(egui::Key::Space).shift(true))]
    fast_forward: KeySet,
    #[config(default = KeySet::from(egui::Key::Space).shift(false))]
    toggle_pause: KeySet,
}

#[derive(Config)]
struct PickingConf {
    #[config(default = KeySet::from(egui::Key::V))]
    pick_vector: KeySet,
}

#[derive(Config)]
struct ObjectControlConf {
    speed:      SpeedConf,
    heading:    HeadingConf,
    altitude:   AltitudeConf,
    #[config(default = KeySet::from(egui::Key::R))]
    next_route: KeySet,
}

#[derive(Config)]
struct SpeedConf {
    #[config(default = KeySet::from(egui::Key::S))]
    set: KeySet,
    #[config(default = KeySet::from(egui::Key::Period))]
    inc: KeySet,
    #[config(default = KeySet::from(egui::Key::Comma))]
    dec: KeySet,
}

#[derive(Config)]
struct HeadingConf {
    #[config(default = KeySet::from(egui::Key::H))]
    set: KeySet,
    #[config(default = KeySet::from(egui::Key::ArrowRight))]
    inc: KeySet,
    #[config(default = KeySet::from(egui::Key::ArrowLeft))]
    dec: KeySet,
}

#[derive(Config)]
struct AltitudeConf {
    #[config(default = KeySet::from(egui::Key::A))]
    set:             KeySet,
    #[config(default = KeySet::from(egui::Key::ArrowUp))]
    inc:             KeySet,
    #[config(default = KeySet::from(egui::Key::ArrowDown))]
    dec:             KeySet,
    #[config(default = KeySet::from(egui::Key::X))]
    toggle_expedite: KeySet,
}
