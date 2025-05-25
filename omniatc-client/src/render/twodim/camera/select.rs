use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventWriter;
use bevy::ecs::query::With;
use bevy::ecs::system::{ParamSet, Query, Res, ResMut, SystemParam};
use bevy::gizmos::gizmos::Gizmos;
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::transform::components::GlobalTransform;
use omniatc::level::object::Object;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{comm, nav, object, plane};
use omniatc::try_log_return;
use omniatc::units::{Angle, AngularSpeed, Distance, Heading, Position, Speed, TurnDirection};
use ordered_float::OrderedFloat;

use super::Conf;
use crate::render::object_info;
use crate::{config, input};

pub(super) fn input_system(
    mut params: ParamSet<(DetermineMode, SelectObjectParams, SetHeadingParams)>,
) {
    let determine_mode = params.p0();
    if let Some(cursor_camera_value) = determine_mode.current_cursor_camera.0 {
        if let Ok(&camera_tf) = determine_mode.camera_query.get(cursor_camera_value.camera_entity) {
            let mode = determine_mode.determine();
            match mode {
                Mode::SelectObject => params.p1().run(cursor_camera_value.world_pos, camera_tf),
                Mode::SetHeading => params.p2().run(cursor_camera_value.world_pos, camera_tf, true),
                Mode::PreviewHeading => {
                    params.p2().run(cursor_camera_value.world_pos, camera_tf, false);
                }
            }
        }
    }
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

#[derive(SystemParam)]
pub(super) struct SetHeadingParams<'w, 's> {
    waypoint_query: Query<'w, 's, (Entity, &'static Waypoint)>,
    object_query:
        Query<'w, 's, (&'static Object, &'static nav::Limits, Option<&'static plane::Control>)>,
    conf:           config::Read<'w, 's, Conf>,
    instr_writer:   EventWriter<'w, comm::InstructionEvent>,
    current_object: Res<'w, object_info::CurrentObject>,
    buttons:        Res<'w, ButtonInput<KeyCode>>,
    gizmos:         Gizmos<'w, 's>,
}

impl SetHeadingParams<'_, '_> {
    fn run(&mut self, cursor_world_pos: Position<Vec2>, camera_tf: GlobalTransform, commit: bool) {
        let click_tolerance = Distance(camera_tf.scale().x) * self.conf.waypoint_select_tolerance;

        let closest_waypoint = self
            .waypoint_query
            .iter()
            .map(|(entity, waypoint)| {
                let waypoint_pos = waypoint.position.horizontal();
                ((entity, waypoint_pos), waypoint_pos.distance_squared(cursor_world_pos))
            })
            .filter(|(_, dist_sq)| *dist_sq < click_tolerance.squared())
            .min_by_key(|(_, dist_sq)| OrderedFloat(dist_sq.0))
            .map(|(result, _)| result);

        if let Some((waypoint, waypoint_pos)) = closest_waypoint {
            if let Some(object) = self.current_object.0 {
                self.propose_set_waypoint(object, waypoint, waypoint_pos, commit);
            } else {
                // TODO show waypoint info?
            }
        } else {
            if let Some(object) = self.current_object.0 {
                self.propose_set_heading(object, cursor_world_pos, commit);
            }
        }
    }

    fn propose_set_waypoint(
        &mut self,
        object: Entity,
        waypoint: Entity,
        waypoint_pos: Position<Vec2>,
        commit: bool,
    ) {
        if commit {
            self.instr_writer.write(comm::InstructionEvent {
                object,
                body: comm::SetWaypoint { waypoint }.into(),
            });
        } else {
            let (&Object { position: object_pos, ground_speed }, limits, plane) = try_log_return!(
                self.object_query.get(object),
                expect "object selected by cursor is not Object or has no nav::Limits"
            );
            let object_pos = object_pos.horizontal();
            let waypoint_heading = (waypoint_pos - object_pos).heading();
            let current_track =
                if let Some(plane) = plane { plane.heading } else { waypoint_heading };
            let speed = ground_speed.horizontal().magnitude_exact();
            let completion_threshold = speed * self.conf.set_heading_preview_density;
            self.draw_gizmos(
                object_pos,
                speed,
                current_track,
                |current_pos| {
                    let delta = waypoint_pos - current_pos;
                    (delta.magnitude_cmp() > completion_threshold).then(|| delta.heading())
                },
                current_track.closer_direction_to(waypoint_heading),
                limits.max_yaw_speed,
            );
        }
    }

    fn propose_set_heading(&mut self, object: Entity, world_pos: Position<Vec2>, commit: bool) {
        let (&Object { position: object_pos, ground_speed }, limits, plane) = try_log_return!(
            self.object_query.get(object),
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
            let current_track =
                if let Some(plane) = plane { plane.heading } else { target_heading };
            let initial_direction = match target {
                nav::YawTarget::Heading(_) => current_track.closer_direction_to(target_heading),
                nav::YawTarget::TurnHeading { direction, .. } => direction,
            };
            self.draw_gizmos(
                object_pos,
                ground_speed.horizontal().magnitude_exact(),
                current_track,
                |_| Some(target_heading),
                initial_direction,
                limits.max_yaw_speed,
            );
        }
    }

    fn draw_gizmos(
        &mut self,
        object_pos: Position<Vec2>,
        speed: Speed<f32>,
        mut track: Heading,
        target_heading: impl Fn(Position<Vec2>) -> Option<Heading>,
        initial_direction: TurnDirection,
        turn_rate: AngularSpeed<f32>,
    ) {
        let mut start = object_pos;
        let mut dir = Some(initial_direction);
        let mut remaining = speed * self.conf.set_heading_preview_limit;

        let max_turn_size = turn_rate * self.conf.set_heading_preview_density;

        while remaining.is_positive() {
            let mut pos_delta = speed * track.into_dir2() * self.conf.set_heading_preview_density;
            let pos_delta_length = pos_delta.magnitude_exact();
            if pos_delta_length > remaining {
                pos_delta *= remaining / pos_delta_length;
                remaining = Distance::ZERO;
            } else {
                remaining -= pos_delta_length;
            }

            let end = start + pos_delta;

            self.gizmos.line_2d(start.get(), end.get(), self.conf.set_heading_preview_color);

            start = end;
            let Some(this_target_heading) = target_heading(start) else { break };
            if track.closest_distance(this_target_heading).abs() < Angle::RIGHT {
                dir = None;
            }
            let this_dir = dir.unwrap_or_else(|| track.closer_direction_to(this_target_heading));
            let mut this_turn_size = track.distance(this_target_heading, this_dir);
            if this_turn_size.abs() > max_turn_size {
                this_turn_size = max_turn_size * this_turn_size.signum();
            }
            track += this_turn_size;
        }
    }
}
