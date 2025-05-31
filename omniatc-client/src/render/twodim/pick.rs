use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventWriter;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, SystemParam};
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::transform::components::GlobalTransform;
use omniatc::level::object::Object;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{comm, nav, object, plane};
use omniatc::try_log_return;
use omniatc::units::{Angle, Distance, Position};
use omniatc_macros::Config;
use ordered_float::OrderedFloat;

use super::object::preview;
use crate::config::AppExt;
use crate::render::object_info;
use crate::{config, input, UpdateSystemSets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_config::<Conf>();
        app.add_systems(
            app::Update,
            input_system
                .in_set(UpdateSystemSets::Input)
                .in_set(input::ReadCurrentCursorCameraSystemSet),
        );
    }
}

#[derive(Config, Resource)]
#[config(id = "2d/pick", name = "Picking (2D)")]
pub struct Conf {
    /// Tolerated distance when clicking on an object in window coordinates.
    object_select_tolerance:       f32,
    /// Tolerated distance when clicking on a waypoint in window coordinates.
    waypoint_select_tolerance:     f32,
    /// Hovered objects are highlighted with this color.
    pub hovered_color:             Color,
    /// Selected objects are highlighted with this color.
    pub selected_color:            Color, // TODO reorganize these two fields to a better category
    /// Color of the preview line when setting heading.
    set_heading_preview_color:     Color,
    /// Thickness of the preview line when setting heading, in window coordinates.
    set_heading_preview_thickness: f32,
    /// Angle of line segments to render the arc for the preview line.
    set_heading_preview_density:   Angle<f32>,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            object_select_tolerance:       50.,
            waypoint_select_tolerance:     30.,
            hovered_color:                 Color::srgb(0.5, 1., 0.7),
            selected_color:                Color::srgb(0.5, 0.7, 1.),
            set_heading_preview_color:     Color::srgb(0.9, 0.7, 0.8),
            set_heading_preview_thickness: 1.5,
            set_heading_preview_density:   Angle::from_degrees(15.),
        }
    }
}

pub(super) fn input_system(
    mut params: ParamSet<(
        DetermineMode,
        SelectObjectParams,
        SetHeadingParams,
        CleanupPreviewParams,
    )>,
) {
    let determine_mode = params.p0();
    let mut is_preview = false;
    if let Some(cursor_camera_value) = determine_mode.current_cursor_camera.0 {
        if let Ok(&camera_tf) = determine_mode.camera_query.get(cursor_camera_value.camera_entity) {
            let mode = determine_mode.determine();
            match mode {
                Mode::SelectObject => params.p1().run(cursor_camera_value.world_pos, camera_tf),
                Mode::SetHeading => params.p2().run(cursor_camera_value.world_pos, camera_tf, true),
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
    key_inputs:            Res<'w, ButtonInput<KeyCode>>,
}

impl DetermineMode<'_, '_> {
    fn determine(&self) -> Mode {
        if self.key_inputs.pressed(KeyCode::KeyY) {
            Mode::PreviewHeading
        } else if self.key_inputs.just_released(KeyCode::KeyY) {
            Mode::SetHeading
        } else {
            Mode::SelectObject
        }
    }
}

enum Mode {
    SelectObject,
    SetHeading,
    PreviewHeading,
}

#[derive(SystemParam)]
pub(super) struct SelectObjectParams<'w, 's> {
    buttons:                Res<'w, ButtonInput<MouseButton>>,
    current_hovered_object: ResMut<'w, object_info::CurrentHoveredObject>,
    current_object:         ResMut<'w, object_info::CurrentObject>,
    object_query:           Query<'w, 's, (Entity, &'static object::Object)>,
    conf:                   config::Read<'w, 's, Conf>,
}

impl SelectObjectParams<'_, '_> {
    fn run(&mut self, cursor_world_pos: Position<Vec2>, camera_tf: GlobalTransform) {
        self.current_hovered_object.0 = None;
        // TODO we need to reconcile this value with 3D systems when supported

        let click_tolerance = Distance(camera_tf.scale().x) * self.conf.object_select_tolerance;

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
    conf:           config::Read<'w, 's, Conf>,
    instr_writer:   EventWriter<'w, comm::InstructionEvent>,
    current_object: Res<'w, object_info::CurrentObject>,
    buttons:        Res<'w, ButtonInput<KeyCode>>,
    commands:       Commands<'w, 's>,
}

impl SetHeadingParams<'_, '_> {
    fn run(&mut self, cursor_world_pos: Position<Vec2>, camera_tf: GlobalTransform, commit: bool) {
        let click_tolerance = Distance(camera_tf.scale().x) * self.conf.waypoint_select_tolerance;

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
            let object_data = try_log_return!(
                self.object_query.get_mut(object),
                expect "object selected by cursor is not Object or has no nav::Limits"
            );
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
        let SetHeadingObjectQueryItem {
            object: &Object { position: object_pos, .. },
            plane,
            preview_override,
        } = try_log_return!(
            self.object_query.get_mut(object),
            expect "object selected by cursor"
        );
        let object_pos = object_pos.horizontal();
        let target_heading = (world_pos - object_pos).heading();
        let mut target = nav::YawTarget::Heading(target_heading);
        if self.buttons.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
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
