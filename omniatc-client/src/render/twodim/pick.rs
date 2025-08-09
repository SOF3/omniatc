use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventWriter;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, SystemParam};
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::transform::components::GlobalTransform;
use bevy_mod_config::{AppExt, Config, ReadConfig};
use math::{Angle, Length, Position};
use omniatc::level::object::Object;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{comm, nav, object, plane};
use omniatc::QueryTryLog;
use ordered_float::OrderedFloat;

use super::object::preview;
use crate::render::object_info;
use crate::{input, ConfigManager, EguiUsedMargins, UpdateSystemSets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<ConfigManager, Conf>("2d:picking");
        app.add_systems(
            app::Update,
            input_system
                .in_set(UpdateSystemSets::Input)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
    }
}

#[derive(Config)]
pub struct Conf {
    /// Tolerated distance when clicking on an object in window coordinates.
    #[config(default = 50.0)]
    object_select_tolerance:       f32,
    /// Tolerated distance when clicking on a waypoint in window coordinates.
    #[config(default = 30.0)]
    waypoint_select_tolerance:     f32,
    /// Hovered objects are highlighted with this color.
    #[config(default = Color::srgb(0.5, 1.0, 0.7))]
    pub hovered_color:             Color,
    /// Selected objects are highlighted with this color.
    #[config(default = Color::srgb(0.5, 0.7, 1.0))]
    pub selected_color:            Color, // TODO reorganize these two fields to a better category
    /// Color of the preview line when setting heading.
    #[config(default = Color::srgb(0.9, 0.7, 0.8))]
    set_heading_preview_color:     Color,
    /// Thickness of the preview line when setting heading, in window coordinates.
    #[config(default = 1.5)]
    set_heading_preview_thickness: f32,
    /// Angle of line segments to render the arc for the preview line.
    #[config(default = Angle::from_degrees(15.0))]
    set_heading_preview_density:   Angle,
}

pub(super) fn input_system(
    margins: Res<EguiUsedMargins>,
    mut params: ParamSet<(
        DetermineMode,
        SelectObjectParams,
        SetHeadingParams,
        CleanupPreviewParams,
    )>,
) {
    if margins.pointer_acquired {
        return;
    }

    let mut determine_mode = params.p0();
    let mut is_preview = false;
    if let Some(cursor_camera_value) = determine_mode.current_cursor_camera.0 {
        if let Ok(&camera_tf) = determine_mode.camera_query.get(cursor_camera_value.camera_entity) {
            let mode = determine_mode.determine();
            match mode {
                Mode::SelectObject => params.p1().run(cursor_camera_value.world_pos, camera_tf),
                Mode::CommitHeading => {
                    params.p2().run(cursor_camera_value.world_pos, camera_tf, true);
                }
                Mode::PreviewHeading => {
                    is_preview = true;
                    params.p2().run(cursor_camera_value.world_pos, camera_tf, false);
                }
            }
        }
    }

    params.p3().run(is_preview);
}

#[derive(SystemParam)]
pub(super) struct DetermineMode<'w, 's> {
    current_cursor_camera: Res<'w, input::CurrentCursorCamera>,
    camera_query:          Query<'w, 's, &'static GlobalTransform, With<Camera2d>>,
    margins:               Res<'w, EguiUsedMargins>,
    was_setting_heading:   Local<'s, bool>,
    hotkeys:               Res<'w, input::Hotkeys>,
}

impl DetermineMode<'_, '_> {
    fn determine(&mut self) -> Mode {
        if self.hotkeys.pick_vector {
            *self.was_setting_heading = true;
            Mode::PreviewHeading
        } else if *self.was_setting_heading {
            Mode::CommitHeading
        } else {
            Mode::SelectObject
        }
    }
}

enum Mode {
    SelectObject,
    CommitHeading,
    PreviewHeading,
}

#[derive(SystemParam)]
pub(super) struct SelectObjectParams<'w, 's> {
    buttons:                Res<'w, ButtonInput<MouseButton>>,
    current_hovered_object: ResMut<'w, object_info::CurrentHoveredObject>,
    current_object:         ResMut<'w, object_info::CurrentObject>,
    object_query:           Query<'w, 's, (Entity, &'static object::Object)>,
    conf:                   ReadConfig<'w, 's, Conf>,
}

impl SelectObjectParams<'_, '_> {
    fn run(&mut self, cursor_world_pos: Position<Vec2>, camera_tf: GlobalTransform) {
        let conf = self.conf.read();

        self.current_hovered_object.0 = None;
        // TODO we need to reconcile this value with 3D systems when supported

        let click_tolerance = Length::new(camera_tf.scale().x) * conf.object_select_tolerance;

        let closest_object = self
            .object_query
            .iter()
            .map(|(entity, object)| {
                (entity, object.position.horizontal().distance_squared(cursor_world_pos))
            })
            .filter(|(_, dist_sq)| *dist_sq < click_tolerance.squared())
            .min_by_key(|(_, dist_sq)| OrderedFloat(dist_sq.0))
            .map(|(object, _)| object);

        self.current_hovered_object.0 = closest_object;
        if self.buttons.just_pressed(MouseButton::Left) {
            self.current_object.0 = closest_object;
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
struct SetHeadingObjectQuery {
    object:           &'static Object,
    plane:            Option<&'static plane::Control>,
    preview_override: Option<&'static mut preview::TargetOverride>,
}

#[derive(SystemParam)]
pub(super) struct SetHeadingParams<'w, 's> {
    waypoint_query: Query<'w, 's, (Entity, &'static Waypoint)>,
    object_query:   Query<'w, 's, SetHeadingObjectQuery>,
    conf:           ReadConfig<'w, 's, Conf>,
    instr_writer:   EventWriter<'w, comm::InstructionEvent>,
    current_object: Res<'w, object_info::CurrentObject>,
    buttons:        Res<'w, ButtonInput<KeyCode>>,
    commands:       Commands<'w, 's>,
    margins:        Res<'w, EguiUsedMargins>,
}

impl SetHeadingParams<'_, '_> {
    fn run(&mut self, cursor_world_pos: Position<Vec2>, camera_tf: GlobalTransform, commit: bool) {
        let conf = self.conf.read();

        let click_tolerance = Length::new(camera_tf.scale().x) * conf.waypoint_select_tolerance;

        let closest_waypoint = self
            .waypoint_query
            .iter()
            .map(|(entity, waypoint)| {
                let waypoint_pos = waypoint.position.horizontal();
                (entity, waypoint_pos.distance_squared(cursor_world_pos))
            })
            .filter(|(_, dist_sq)| *dist_sq < click_tolerance.squared())
            .min_by_key(|(_, dist_sq)| OrderedFloat(dist_sq.0))
            .map(|(result, _)| result);

        if let Some(waypoint) = closest_waypoint {
            if let Some(object) = self.current_object.0 {
                self.propose_set_waypoint(object, waypoint, commit);
            } else {
                // TODO show waypoint info?
            }
        } else {
            if let Some(object) = self.current_object.0 {
                self.propose_set_heading(object, cursor_world_pos, commit);
            }
        }
    }

    fn propose_set_waypoint(&mut self, object: Entity, waypoint: Entity, commit: bool) {
        if commit {
            self.instr_writer.write(comm::InstructionEvent {
                object,
                body: comm::SetWaypoint { waypoint }.into(),
            });
        } else {
            let Some(object_data) = self.object_query.log_get_mut(object) else { return };
            let target_override_value = preview::TargetOverride {
                target: preview::Target::Waypoint(waypoint),
                cause:  preview::TargetOverrideCause::SetHeading,
            };
            if let Some(mut target_override) = object_data.preview_override {
                *target_override = target_override_value;
            } else {
                self.commands.entity(object).insert(target_override_value);
            }
        }
    }

    fn propose_set_heading(&mut self, object: Entity, world_pos: Position<Vec2>, commit: bool) {
        let Some(SetHeadingObjectQueryItem {
            object: &Object { position: object_pos, .. },
            plane,
            preview_override,
        }) = self.object_query.log_get_mut(object)
        else {
            return;
        };
        let object_pos = object_pos.horizontal();
        let target_heading = (world_pos - object_pos).heading();
        let mut target = nav::YawTarget::Heading(target_heading);
        if !self.margins.keyboard_acquired
            && self.buttons.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
        {
            if let Some(plane) = plane {
                let reflex_dir = -plane.heading.closer_direction_to(target_heading);
                target = nav::YawTarget::TurnHeading {
                    heading:           target_heading,
                    direction:         reflex_dir,
                    remaining_crosses: 0,
                };
            }
        }

        if commit {
            self.instr_writer
                .write(comm::InstructionEvent { object, body: comm::SetHeading { target }.into() });
        } else {
            let target_override_value = preview::TargetOverride {
                target: preview::Target::Yaw(target),
                cause:  preview::TargetOverrideCause::SetHeading,
            };
            if let Some(mut target_override) = preview_override {
                *target_override = target_override_value;
            } else {
                self.commands.entity(object).insert(target_override_value);
            }
        }
    }
}

#[derive(SystemParam)]
pub(super) struct CleanupPreviewParams<'w, 's> {
    query:      Query<'w, 's, (Entity, &'static preview::TargetOverride)>,
    need_rerun: Local<'s, bool>,
    commands:   Commands<'w, 's>,
}

impl CleanupPreviewParams<'_, '_> {
    fn run(&mut self, is_preview: bool) {
        if is_preview {
            *self.need_rerun = true;
            return;
        }
        if !mem::replace(&mut self.need_rerun, false) {
            return;
        }
        for (entity, comp) in self.query {
            if comp.cause == preview::TargetOverrideCause::SetHeading {
                self.commands.entity(entity).remove::<preview::TargetOverride>();
            }
        }
    }
}
