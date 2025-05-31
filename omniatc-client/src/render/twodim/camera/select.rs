use std::f32::consts::TAU;
use std::{mem, ops};

use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::EventWriter;
use bevy::ecs::query::{With, Without};
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, Single, SystemParam};
use bevy::input::keyboard::KeyCode;
use bevy::input::mouse::MouseButton;
use bevy::input::ButtonInput;
use bevy::math::primitives::Circle;
use bevy::math::Vec2;
use bevy::render::mesh::{Indices, Mesh, Mesh2d, PrimitiveTopology, VertexAttributeValues};
use bevy::render::view::Visibility;
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::{GlobalTransform, Transform};
use itertools::Itertools;
use omniatc::level::object::Object;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{comm, nav, object, plane};
use omniatc::try_log_return;
use omniatc::units::{Angle, AngularSpeed, Distance, Heading, Position, Speed, TurnDirection};
use ordered_float::OrderedFloat;

use super::Conf;
use crate::render::object_info;
use crate::render::twodim::Zorder;
use crate::util::shapes;
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
    waypoint_query:   Query<'w, 's, (Entity, &'static Waypoint)>,
    object_query:
        Query<'w, 's, (&'static Object, &'static nav::Limits, Option<&'static plane::Control>)>,
    conf:             config::Read<'w, 's, Conf>,
    instr_writer:     EventWriter<'w, comm::InstructionEvent>,
    current_object:   Res<'w, object_info::CurrentObject>,
    buttons:          Res<'w, ButtonInput<KeyCode>>,
    meshes:           ResMut<'w, Assets<Mesh>>,
    materials:        ResMut<'w, Assets<ColorMaterial>>,
    arc_mesh:         Local<'s, Option<Handle<Mesh>>>,
    commands:         Commands<'w, 's>,
    shapes:           ResMut<'w, shapes::Meshes>,
    ray_entity_query:
        Option<Single<'w, &'static mut Transform, (With<RayEntity>, Without<ArcEntity>)>>,
    arc_entity_query: Option<Single<'w, &'static mut Transform, With<ArcEntity>>>,
    camera:           Single<'w, &'static GlobalTransform, With<Camera2d>>,
    preview_vis:      Query<'w, 's, &'static mut Visibility, With<SetHeadingPreview>>,
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

        for mut vis in &mut self.preview_vis {
            *vis = if commit { Visibility::Hidden } else { Visibility::Visible };
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

            let turn_radius = Distance(speed.0 / limits.max_yaw_speed.0);
            let turn_center = object_pos
                + match current_track.closer_direction_to(waypoint_heading) {
                    TurnDirection::Clockwise => turn_radius * (current_track + Angle::RIGHT),
                    TurnDirection::CounterClockwise => turn_radius * (current_track - Angle::RIGHT),
                };

            // TODO find point `tangent` such that turn_center.distance(tangent) == turn_radius
            // && (tangent - turn_center).dot(tangent - waypoint_pos) == 0.

            // TODO render
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
            let direction = match target {
                nav::YawTarget::Heading(_) => current_track.closer_direction_to(target_heading),
                nav::YawTarget::TurnHeading { direction, .. } => direction,
            };
            let speed = ground_speed.horizontal().magnitude_exact();

            let turn_radius = Distance(speed.0 / limits.max_yaw_speed.0);
            let start_pos_to_center = match direction {
                TurnDirection::Clockwise => current_track + Angle::RIGHT,
                TurnDirection::CounterClockwise => current_track - Angle::RIGHT,
            };
            let turn_center = object_pos + turn_radius * start_pos_to_center;
            let turn_angle = current_track.distance(target_heading, direction);

            let start_heading = start_pos_to_center.opposite();
            let transition_heading = start_heading + turn_angle;

            let range = if turn_angle.is_negative() {
                transition_heading..start_heading
            } else {
                start_heading..transition_heading
            };
            self.draw_arc(turn_center, turn_radius, range);

            let transition_point = turn_center + turn_radius * transition_heading;
            self.draw_ray(transition_point, target_heading);
        }
    }

    fn draw_arc(
        &mut self,
        center: Position<Vec2>,
        radius: Distance<f32>,
        heading_range: ops::Range<Heading>,
    ) {
        fn draw_arc_mesh(
            positions: &mut Vec<[f32; 3]>,
            radius: Distance<f32>,
            heading_range: ops::Range<Heading>,
            thickness: f32,
            density: Angle<f32>,
        ) {
            positions.clear();

            let angular_dist =
                heading_range.start.distance(heading_range.end, TurnDirection::Clockwise);
            let steps = (angular_dist / density).ceil() as u32;
            for step in 0..=steps {
                let heading = heading_range.start + density * (step as f32);
                let inner = (radius - Distance(thickness / 2.)) * heading;
                let outer = (radius + Distance(thickness / 2.)) * heading;
                positions.push([inner.0.x, inner.0.y, 0.]);
                positions.push([outer.0.x, outer.0.y, 0.]);
            }
        }

        let mesh = match *self.arc_mesh {
            None => {
                let mut positions = Vec::new();
                draw_arc_mesh(
                    &mut positions,
                    radius,
                    heading_range,
                    self.conf.set_heading_preview_thickness * self.camera.scale().y,
                    self.conf.set_heading_preview_density,
                );
                let handle = self.meshes.add(
                    Mesh::new(PrimitiveTopology::TriangleStrip, RenderAssetUsages::all())
                        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions),
                );
                &*self.arc_mesh.insert(handle)
            }
            Some(ref handle) => {
                let mesh = self.meshes.get_mut(handle).expect("get by strong reference");

                let Some(VertexAttributeValues::Float32x3(positions)) =
                    mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
                else {
                    panic!(
                        "mesh attribute type changed to {:?}",
                        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
                    )
                };
                draw_arc_mesh(
                    positions,
                    radius,
                    heading_range,
                    self.conf.set_heading_preview_thickness * self.camera.scale().y,
                    self.conf.set_heading_preview_density,
                );

                handle
            }
        };

        match self.arc_entity_query {
            None => {
                self.commands.spawn((
                    Mesh2d(mesh.clone()),
                    MeshMaterial2d(
                        self.materials
                            .add(ColorMaterial::from_color(self.conf.set_heading_preview_color)),
                    ),
                    ArcEntity,
                    Transform {
                        translation: Zorder::SetHeadingPreview.pos2_to_translation(center),
                        ..Default::default()
                    },
                ));
            }
            Some(ref mut tf) => {
                tf.translation = Zorder::SetHeadingPreview.pos2_to_translation(center);
            }
        }
    }

    fn draw_ray(&mut self, start: Position<Vec2>, dir: Heading) {
        let end = start + Distance::from_nm(1000.) * dir;
        match self.ray_entity_query {
            None => {
                self.commands.spawn((
                    self.shapes.line_from_to(
                        self.conf.set_heading_preview_thickness,
                        Zorder::SetHeadingPreview,
                        start,
                        end,
                    ),
                    MeshMaterial2d(
                        self.materials
                            .add(ColorMaterial::from_color(self.conf.set_heading_preview_color)),
                    ),
                    RayEntity,
                ));
            }
            Some(ref mut tf) => {
                shapes::set_square_line_transform(&mut *tf, start, end);
            }
        }
    }
}

#[derive(Component)]
#[require(SetHeadingPreview)]
struct ArcEntity;

#[derive(Component)]
#[require(SetHeadingPreview)]
struct RayEntity;

#[derive(Component, Default)]
struct SetHeadingPreview;
