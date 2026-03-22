use std::hash::Hash;

use bevy::app::{self, App, Plugin};
use bevy::camera::Camera;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{AnyOf, QueryData};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Query, Res, ResMut, Single};
use bevy::input::ButtonInput;
use bevy::input::mouse::MouseButton;
use bevy::math::Vec2;
use bevy::transform::components::GlobalTransform;
use bevy::window::Window;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use bevy_mod_config::{AppExt, Config, ReadConfig};
use math::{Length, Position};

use crate::render::{threedim, twodim};
use crate::{ConfigManager, EguiState, EguiSystemSets, UpdateSystemSets};

pub mod key_field;
pub use key_field::KeySet;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("input");
        app.init_resource::<CursorState>();
        app.add_systems(
            app::Update,
            CursorState::update_system
                .in_set(UpdateSystemSets::Input)
                .before(ReadCurrentCursorCameraSystemSet),
        );
        app.init_resource::<Hotkeys>();
        app.add_systems(
            EguiPrimaryContextPass,
            Hotkeys::update_system
                .in_set(EguiSystemSets::Init)
                .ambiguous_with(EguiState::reset_frame_system),
        );
    }
}

#[derive(Resource, Default, Debug)]
pub struct CursorState {
    pub hovered: Option<CursorTarget>,
    pub left:    CursorButtonState,
    pub right:   CursorButtonState,
}

#[derive(Default, Debug)]
pub struct CursorButtonState {
    /// Newly pressed down.
    pub clicked: Option<CursorTarget>,
}

#[derive(Debug, Clone, Copy)]
pub enum CursorTarget {
    TwoDim { world_pos: Position<Vec2>, pixel_precision: Length<f32> },
}

impl CursorTarget {
    /// Horizontal position of the intersection between ground terrain
    /// and the ray from the camera through the cursor.
    #[must_use]
    pub fn ground_position(&self) -> Option<Position<Vec2>> {
        match self {
            CursorTarget::TwoDim { world_pos, .. } => Some(*world_pos),
        }
    }

    /// Geometric mean distance moved on the ground terrain
    /// if the cursor moves by one pixel in any direction.
    #[must_use]
    pub fn ground_precision(&self) -> Option<Length<f32>> {
        match self {
            CursorTarget::TwoDim { pixel_precision, .. } => Some(*pixel_precision),
        }
    }
}

#[derive(QueryData)]
struct UpdateCursorCameraQueryData {
    camera_entity: Entity,
    camera:        &'static Camera,
    global_tf:     &'static GlobalTransform,
    marker:        AnyOf<(&'static twodim::camera::UiState, &'static threedim::UiState)>,
}

impl CursorState {
    fn update_system(
        mut target: ResMut<Self>,
        _window: Option<Single<&Window>>,
        camera_query: Query<UpdateCursorCameraQueryData>,
        _buttons: Res<ButtonInput<MouseButton>>,
    ) {
        let target = &mut *target;
        *target = Self::default();

        for data in camera_query {
            match data.marker {
                (Some(twodim), _) => {
                    if let Some(pos) = twodim.hovered {
                        let cursor_target = CursorTarget::TwoDim {
                            world_pos:       pos.world,
                            pixel_precision: Length::new(data.global_tf.scale().x),
                        };

                        target.hovered = Some(cursor_target);
                        if twodim.left_clicked {
                            target.left.clicked = Some(cursor_target);
                        }
                        if twodim.right_clicked {
                            target.right.clicked = Some(cursor_target);
                        }
                    }
                }
                (None, Some(_threedim)) => {}
                _ => unreachable!(),
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct ReadCurrentCursorCameraSystemSet;

#[derive(Resource, Default)]
#[expect(clippy::struct_excessive_bools, reason = "multiple independent flags")]
pub struct Hotkeys {
    pub search:          bool,
    pub deselect:        bool,
    pub fast_forward:    bool,
    pub toggle_pause:    bool,
    pub reset_speed:     bool,
    pub north:           bool,
    pub pick_route:      bool,
    pub append_route:    bool,
    pub send:            bool,
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
            this.send = conf.object_control.send.clicked(state);
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
    #[config(default = KeySet::from(egui::Key::Enter))]
    send:       KeySet,
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
