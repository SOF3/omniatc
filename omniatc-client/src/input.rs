use std::hash::Hash;

use bevy::app::{self, App, Plugin};
use bevy::camera::Camera;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, Or, QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Query, Res, ResMut, Single};
use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::math::primitives::InfinitePlane3d;
use bevy::math::{Vec2, Vec3};
use bevy::transform::components::GlobalTransform;
use bevy::window::Window;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bevy_mod_config::{AppExt, Config, ReadConfig};
use math::Position;
use omniatc::try_log_return;
use omniatc::util::EqAny;

use crate::render::{threedim, twodim};
use crate::{ConfigManager, EguiSystemSets, EguiUsedMargins};

pub mod key_field;
pub use key_field::KeySet;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("input");
        app.init_resource::<CursorState>();
        app.add_systems(
            app::Update,
            CursorState::update_system.before(ReadCurrentCursorCameraSystemSet),
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
pub struct CursorState {
    pub value: Option<CurrentCursorCameraValue>,

    pub left:  ButtonState,
    pub right: ButtonState,

    pub capture: Capture,
    drag_state:  Option<Box<dyn EqAny + Send + Sync>>,
}

#[derive(Default)]
pub struct ButtonState {
    pub was_down: bool,
    pub is_down:  bool,
}

impl ButtonState {
    fn rotate_and_reset(&mut self) {
        self.was_down = self.is_down;
        self.is_down = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Capture {
    /// Pointer captured by the game UI.
    #[default]
    Game,
    /// Pointer captured by egui components.
    Egui,
}

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

impl CursorState {
    pub fn left_just_down(&self) -> bool { self.left.is_down && !self.left.was_down }

    pub fn left_just_up(&self) -> bool { !self.left.is_down && self.left.was_down }

    pub fn right_just_down(&self) -> bool { self.right.is_down && !self.right.was_down }

    pub fn right_just_up(&self) -> bool { !self.right.is_down && self.right.was_down }

    /// Determines whether dragging is occurring,
    /// selectively disabling dragging based on the current capture state.
    ///
    /// This function should be called every frame to ensure correct renewal,
    /// not just during button state changes.
    pub fn is_dragging(
        &mut self,
        id: impl EqAny + Send + Sync,
        getter: impl FnOnce(&Self) -> bool,
    ) -> bool {
        match self.capture {
            Capture::Game => {
                // When within game area,
                // all dragging overrides are allowed.
                let dragging = getter(self);
                if dragging {
                    self.drag_state = Some(Box::new(id));
                    true
                } else {
                    self.drag_state = None;
                    false
                }
            }
            Capture::Egui => {
                // When within egui area,
                // only renewal of the same dragging override is allowed.
                if id.eq_any(&self.drag_state) {
                    let dragging = getter(self);
                    if dragging {
                        // renew dragging state
                        true
                    } else {
                        // no more dragging until egui releases the cursor.
                        self.drag_state = None;
                        false
                    }
                } else {
                    // New dragging overrides are not allowed.
                    false
                }
            }
        }
    }

    fn update_system(
        mut target: ResMut<Self>,
        window: Option<Single<&Window>>,
        camera_query: Query<
            CameraData,
            Or<(With<twodim::camera::Layout>, With<threedim::CameraLayout>)>,
        >,
        buttons: Res<ButtonInput<MouseButton>>,
        mut contexts: EguiContexts,
    ) {
        target.capture = Capture::Game;

        let Some(window) = window else {
            // No possible camera capture without a window.
            // Pointer states may still be valid.
            target.value = None;
            return;
        };

        target.left.rotate_and_reset();
        target.right.rotate_and_reset();

        if let Ok(ctx) = contexts.ctx_mut()
            && ctx.wants_pointer_input()
        {
            // If pointer enters egui area,
            // just assume the pointer stays at the last position in game area,
            // and ignore all mouse button press changes.
            target.capture = Capture::Egui;
            return;
        }

        target.left.is_down = buttons.pressed(MouseButton::Left);
        target.right.is_down = buttons.pressed(MouseButton::Right);
        target.value = None;

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
            target.value = Some(CurrentCursorCameraValue {
                camera_entity: data.camera_entity,
                viewport_pos,
                world_pos: Position::new(world_pos),
            });
        } else if data.is_threedim {
            // TODO support 3D camera
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
                target.value = Some(CurrentCursorCameraValue {
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
    pub reset_speed:     bool,
    pub north:           bool,
    pub pick_route:      bool,
    pub append_route:    bool,
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
            this.reset_speed = conf.level_control.reset_speed.clicked(state);
            this.north = conf.level_control.north.clicked(state);
            this.pick_route = conf.picking.pick_route.down(state);
            this.append_route = conf.picking.append_route.down(state);
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
    #[config(default = KeySet::from(egui::Key::Num1))]
    reset_speed:  KeySet,
    #[config(default = KeySet::from(egui::Key::N))]
    north:        KeySet,
}

#[derive(Config)]
struct PickingConf {
    #[config(default = KeySet::from(egui::Key::V).shift(false))]
    pick_route:   KeySet,
    #[config(default = KeySet::from(egui::Key::V).shift(true))]
    append_route: KeySet,
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
