use std::f32::consts::FRAC_PI_8;
use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::core_pipeline::core_2d::Camera2d;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, With, Without};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, Single, SystemParam};
use bevy::ecs::world::Mut;
use bevy::math::Vec2;
use bevy::render::mesh::{Mesh, Mesh2d, PrimitiveTopology, VertexAttributeValues};
use bevy::render::view::Visibility;
use bevy::sprite::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::{GlobalTransform, Transform};
use itertools::Itertools;
use omniatc::level::object::Object;
use omniatc::level::route::{self, Route};
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{nav, plane};
use omniatc::units::{Angle, Distance, Heading, Position, TurnDirection};
use omniatc::util::EnumScheduleConfig;
use omniatc::{math, try_log, try_log_return};

use super::{Conf, SetColorThemeSystemSet};
use crate::render::object_info;
use crate::render::twodim::Zorder;
use crate::util::shapes;
use crate::{config, render};

const ARC_DENSITY: Angle<f32> = Angle(FRAC_PI_8 / 4.);

pub(super) struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            update_system.in_set(render::SystemSets::Update).after_all::<SetColorThemeSystemSet>(),
        );
    }
}

/// Marks an entity as a preview viewable.
#[derive(Component, Default)]
struct Viewable;

fn update_system(
    mut materials: Local<Materials>,
    mut stages: ParamSet<(Init, DrawCurrent, DrawRoute, DrawPresets)>,
) {
    let materials = &mut *materials;
    let (object, current_material, route_material, preset_material) = {
        let mut init = stages.p0();

        for mut vis in init.vis_query {
            *vis = if init.object.0.is_some() { Visibility::Visible } else { Visibility::Hidden };
        }

        let [normal_material, set_heading_material, preset_material] = [
            (&mut materials.normal, init.conf.preview_line_color_normal),
            (&mut materials.set_heading, init.conf.preview_line_color_set_heading),
            (&mut materials.preset, init.conf.preview_line_color_preset),
        ]
        .map(|(local, color)| &*match local {
            None => local.insert(init.materials.add(color)),
            Some(handle) => {
                *init.materials.get_mut(&*handle).expect("strong reference must exist") =
                    ColorMaterial::from_color(color);
                handle
            }
        });

        let Some(object) = init.object.0 else { return };
        let current_material = match init.override_query.get(object) {
            Ok(TargetOverride { cause: TargetOverrideCause::SetHeading, .. }) => {
                set_heading_material
            }
            _ => normal_material,
        };

        (object, current_material, normal_material, preset_material)
    };

    let target = stages.p1().draw(object, current_material);
    stages.p2().draw_current_plan(object, route_material);
    if let Some(target) = target {
        stages.p3().draw_avail_presets(target, preset_material);
    } else {
        let mut stage = stages.p3();
        for (entity, _) in stage.viewable_query {
            stage.draw_once.commands.entity(entity).despawn();
        }
    }
}

#[derive(SystemParam)]
struct Init<'w, 's> {
    vis_query:      Query<'w, 's, &'static mut Visibility, With<Viewable>>,
    object:         Res<'w, object_info::CurrentObject>,
    override_query: Query<'w, 's, &'static TargetOverride>,
    materials:      ResMut<'w, Assets<ColorMaterial>>,
    conf:           config::Read<'w, 's, super::Conf>,
}

#[derive(Default)]
struct Materials {
    normal:      Option<Handle<ColorMaterial>>,
    set_heading: Option<Handle<ColorMaterial>>,
    preset:      Option<Handle<ColorMaterial>>,
}

#[derive(SystemParam)]
struct DrawCurrent<'w, 's> {
    conf:           config::Read<'w, 's, super::Conf>,
    object_query:   Query<'w, 's, DrawCurrentObject>,
    waypoint_query: Query<'w, 's, &'static Waypoint>,
    turn_query: Option<Single<'w, (&'static Mesh2d, &'static mut Transform), With<TurnViewable>>>,
    direct_query:
        Option<Single<'w, &'static mut Transform, (With<DirectViewable>, Without<TurnViewable>)>>,
    commands:       Commands<'w, 's>,
    meshes:         ResMut<'w, Assets<Mesh>>,
    shapes:         Res<'w, shapes::Meshes>,
    camera:         Single<'w, &'static GlobalTransform, With<Camera2d>>,
}

#[derive(QueryData)]
struct DrawCurrentObject {
    object:          &'static Object,
    vel_target:      &'static nav::VelocityTarget,
    limits:          &'static nav::Limits,
    plane_control:   Option<&'static plane::Control>,
    target_waypoint: Option<&'static nav::TargetWaypoint>,
    target_override: Option<&'static TargetOverride>,
}

impl DrawCurrent<'_, '_> {
    fn draw(&mut self, object_id: Entity, material: &Handle<ColorMaterial>) -> Option<Target> {
        let Ok(DrawCurrentObjectItem {
            object: &Object { position: curr_pos, ground_speed: speed },
            vel_target: &nav::VelocityTarget { yaw: yaw_target, .. },
            limits: &nav::Limits { max_yaw_speed, .. },
            plane_control,
            target_waypoint,
            target_override,
        }) = self.object_query.get(object_id)
        else {
            return None; // TODO ground object
        };

        let curr_pos = curr_pos.horizontal();
        let speed = speed.horizontal().magnitude_exact();
        let turn_radius = Distance(speed.0 / max_yaw_speed.0);

        let target = target_override.map_or_else(
            || {
                let target = match target_waypoint {
                    Some(&nav::TargetWaypoint { waypoint_entity }) => {
                        Target::Waypoint(waypoint_entity)
                    }
                    None => Target::Yaw(yaw_target),
                };
                TargetOverride { target, cause: TargetOverrideCause::None }
            },
            TargetOverride::clone,
        );

        match target.target {
            Target::Waypoint(waypoint_entity) => {
                let &Waypoint { position: waypoint_pos, .. } = try_log!(
                    self.waypoint_query.get(waypoint_entity),
                    expect "nav::TargetWaypoint must reference valid waypoint"
                    or return None
                );
                let waypoint_pos = waypoint_pos.horizontal();

                let direct_heading = (waypoint_pos - curr_pos).heading();
                if let Some(&plane::Control { heading: curr_heading, .. }) = plane_control {
                    if curr_heading.closest_distance(direct_heading).abs() > Angle::from_degrees(1.)
                    {
                        self.draw_turn_to_waypoint(
                            curr_pos,
                            turn_radius,
                            curr_heading,
                            waypoint_pos,
                            curr_heading.closer_direction_to(direct_heading),
                            material,
                        );
                    } else {
                        self.draw_direct(curr_pos, waypoint_pos, material);
                    }
                } else {
                    self.draw_direct(curr_pos, waypoint_pos, material);
                }
            }
            Target::Yaw(yaw_target) => {
                if let Some(&plane::Control { heading: curr_heading, .. }) = plane_control {
                    self.draw_turn_to_heading(
                        yaw_target,
                        curr_pos,
                        turn_radius,
                        curr_heading,
                        material,
                    );
                } else {
                    let target_heading = yaw_target.heading();
                    self.draw_direct(
                        curr_pos,
                        curr_pos + Distance::from_nm(10000.) * target_heading,
                        material,
                    );
                }
            }
        }

        Some(target.target)
    }

    fn draw_turn_to_waypoint(
        &mut self,
        curr_pos: Position<Vec2>,
        turn_radius: Distance<f32>,
        curr_heading: Heading,
        waypoint_pos: Position<Vec2>,
        direction: TurnDirection,
        material: &Handle<ColorMaterial>,
    ) {
        let curr_pos_to_turn_center = curr_heading + Angle::RIGHT * direction;
        let turn_center = curr_pos + turn_radius * curr_pos_to_turn_center;

        if let Some(transition_point) =
            math::find_circle_tangent_towards(waypoint_pos, turn_center, turn_radius, direction)
        {
            let transition_heading = (transition_point - turn_center).heading();
            self.draw_arc(
                turn_center,
                turn_radius,
                curr_pos_to_turn_center.opposite(),
                transition_heading,
                direction,
                material,
            );
            self.draw_direct(transition_point, waypoint_pos, material);
        } else {
            self.draw_arc(
                turn_center,
                turn_radius,
                curr_pos_to_turn_center.opposite(),
                curr_pos_to_turn_center.opposite() - Angle::STRAIGHT * direction,
                direction,
                material,
            );
        }
    }

    fn draw_turn_to_heading(
        &mut self,
        yaw_target: nav::YawTarget,
        curr_pos: Position<Vec2>,
        turn_radius: Distance<f32>,
        curr_heading: Heading,
        material: &Handle<ColorMaterial>,
    ) {
        let (target_heading, direction) = match yaw_target {
            nav::YawTarget::Heading(heading) => {
                (heading, curr_heading.closer_direction_to(heading))
            }
            nav::YawTarget::TurnHeading { heading, direction, .. } => (heading, direction),
        };
        let turn_angle = curr_heading.distance(target_heading, direction);

        if turn_angle.abs() > Angle::from_degrees(1.) {
            let curr_pos_to_turn_center = curr_heading + Angle::RIGHT * direction;
            let turn_center = curr_pos + turn_radius * curr_pos_to_turn_center;

            let turn_center_to_transition = curr_pos_to_turn_center.opposite() + turn_angle;

            self.draw_arc(
                turn_center,
                turn_radius,
                curr_pos_to_turn_center.opposite(),
                turn_center_to_transition,
                direction,
                material,
            );
            let transition_point = turn_center + turn_radius * turn_center_to_transition;
            self.draw_direct(
                transition_point,
                transition_point + Distance::from_nm(10000.) * target_heading,
                material,
            );
        } else {
            self.draw_direct(
                curr_pos,
                curr_pos + Distance::from_nm(10000.) * target_heading,
                material,
            );
        }
    }

    fn draw_arc(
        &mut self,
        center: Position<Vec2>,
        radius: Distance<f32>,
        mut start_heading: Heading,
        mut end_heading: Heading,
        direction: TurnDirection,
        material: &Handle<ColorMaterial>,
    ) {
        fn draw_arc_to(
            positions: &mut Vec<[f32; 3]>,
            radius: Distance<f32>,
            start_heading: Heading,
            end_heading: Heading,
            thickness: f32,
        ) {
            positions.clear();

            let angular_dist = start_heading.distance(end_heading, TurnDirection::Clockwise);
            // angular_dist < TAU, never overflows
            #[expect(clippy::cast_possible_truncation)]
            // angular_dist is a clockwise (positive) angle
            #[expect(clippy::cast_sign_loss)]
            let steps = (angular_dist / ARC_DENSITY).ceil() as u32;
            for step in 0..=steps {
                #[expect(clippy::cast_precision_loss)] // step <= steps derived from f32
                let heading = start_heading + ARC_DENSITY * (step as f32);
                let inner = (radius - Distance(thickness / 2.)) * heading;
                let outer = (radius + Distance(thickness / 2.)) * heading;
                positions.push([inner.0.x, inner.0.y, 0.]);
                positions.push([outer.0.x, outer.0.y, 0.]);
            }
        }

        if direction == TurnDirection::CounterClockwise {
            mem::swap(&mut start_heading, &mut end_heading);
        }

        let window_thickness = self.conf.preview_line_thickness * self.camera.scale().y;
        if let Some(&mut (mesh, ref mut tf)) = self.turn_query.as_deref_mut() {
            let mesh = self.meshes.get_mut(&mesh.0).expect("strong reference must be valid");
            let Some(VertexAttributeValues::Float32x3(positions)) =
                mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
            else {
                panic!("Position attribute was initialized as Float32x3 during spawn");
            };
            draw_arc_to(positions, radius, start_heading, end_heading, window_thickness);

            tf.translation = Zorder::ObjectTrackPreview.pos2_to_translation(center);
        } else {
            let mut positions = Vec::new();
            draw_arc_to(&mut positions, radius, start_heading, end_heading, window_thickness);
            let mesh = self.meshes.add(
                Mesh::new(PrimitiveTopology::TriangleStrip, RenderAssetUsages::all())
                    .with_inserted_attribute(
                        Mesh::ATTRIBUTE_POSITION,
                        VertexAttributeValues::Float32x3(positions),
                    ),
            );

            self.commands.spawn((
                Mesh2d(mesh),
                MeshMaterial2d(material.clone()),
                Transform::from_translation(Zorder::ObjectTrackPreview.pos2_to_translation(center)),
                TurnViewable,
            ));
        }
    }

    fn draw_direct(
        &mut self,
        start: Position<Vec2>,
        end: Position<Vec2>,
        material: &Handle<ColorMaterial>,
    ) {
        if let Some(tf) = self.direct_query.as_deref_mut() {
            shapes::set_square_line_transform(tf, start, end);
        } else {
            self.commands.spawn((
                self.shapes.line_from_to(
                    self.conf.preview_line_thickness,
                    Zorder::ObjectTrackPreview,
                    start,
                    end,
                    &self.camera,
                ),
                MeshMaterial2d(material.clone()),
                DirectViewable,
            ));
        }
    }
}

#[derive(SystemParam)]
struct DrawRoute<'w, 's> {
    object_query:   Query<'w, 's, &'static Route>,
    viewable_query: Query<'w, 's, (Entity, &'static mut Transform), With<RouteViewable>>,
    draw_once:      DrawOnce<'w, 's>,
}

impl DrawRoute<'_, '_> {
    fn draw_current_plan(&mut self, object_id: Entity, material: &Handle<ColorMaterial>) {
        let Ok(route) = self.object_query.get(object_id) else { return };
        let nodes = route.iter();
        let mut viewables = self.viewable_query.iter_mut();

        self.draw_once.draw_route::<RouteViewable>(
            nodes,
            material,
            &mut viewables.by_ref().map(|(_, tf)| tf),
            Zorder::ObjectTrackPreview,
        );
        for (entity, _) in viewables {
            self.draw_once.commands.entity(entity).despawn();
        }
    }
}

#[derive(SystemParam)]
struct DrawPresets<'w, 's> {
    waypoint_presets_query: Query<'w, 's, &'static route::WaypointPresetList>,
    preset_query:           Query<'w, 's, &'static route::Preset>,
    viewable_query:         Query<'w, 's, (Entity, &'static mut Transform), With<PresetViewable>>,
    draw_once:              DrawOnce<'w, 's>,
}

impl DrawPresets<'_, '_> {
    fn draw_avail_presets(&mut self, target: Target, material: &Handle<ColorMaterial>) {
        let Target::Waypoint(waypoint) = target else { return };
        let Ok(presets) = self.waypoint_presets_query.get(waypoint) else { return };
        let mut viewables = self.viewable_query.iter_mut();

        for preset_id in presets.iter() {
            let preset = try_log!(
                self.preset_query.get(preset_id),
                expect "waypoint presets references invalid preset {preset_id:?}"
                or continue
            );
            self.draw_once.draw_route::<PresetViewable>(
                preset.nodes.iter(),
                material,
                &mut viewables.by_ref().map(|(_, tf)| tf),
                Zorder::RoutePresetPreview,
            );
        }
        for (entity, _) in viewables {
            self.draw_once.commands.entity(entity).despawn();
        }
    }
}

#[derive(SystemParam)]
struct DrawOnce<'w, 's> {
    commands:       Commands<'w, 's>,
    shapes:         Res<'w, shapes::Meshes>,
    conf:           config::Read<'w, 's, Conf>,
    waypoint_query: Query<'w, 's, &'static Waypoint>,
    camera:         Single<'w, &'static GlobalTransform, With<Camera2d>>,
}

impl DrawOnce<'_, '_> {
    fn draw_route<'w, MarkerT: Bundle + Default>(
        &mut self,
        nodes: impl Iterator<Item = &'w route::Node>,
        material: &Handle<ColorMaterial>,
        viewables: &mut impl Iterator<Item = Mut<'w, Transform>>,
        zorder: Zorder,
    ) {
        let mut positions = Vec::new();
        for node in nodes {
            match node {
                route::Node::DirectWaypoint(node) => {
                    let waypoint = try_log!(
                        self.waypoint_query.get(node.waypoint),
                        expect "planned waypoint {:?} does not exist" (node.waypoint)
                        or continue
                    );
                    positions.push(waypoint.position.horizontal());
                }
                route::Node::AlignRunway(node) => {
                    let waypoint = try_log!(
                        self.waypoint_query.get(node.runway),
                        expect "planned runway {:?} does not exist" (node.runway)
                        or continue
                    );
                    positions.push(waypoint.position.horizontal());
                }
                _ => {}
            }
        }

        for (&start, &end) in positions.iter().tuple_windows() {
            if let Some(mut tf) = viewables.next() {
                shapes::set_square_line_transform(&mut tf, start, end);
            } else {
                self.commands.spawn((
                    self.shapes.line_from_to(
                        self.conf.preview_line_thickness,
                        zorder,
                        start,
                        end,
                        &self.camera,
                    ),
                    MeshMaterial2d(material.clone()),
                    MarkerT::default(),
                ));
            }
        }
    }
}

/// Marks an entity as the arc representing the turn from current heading to current yaw target.
#[derive(Component)]
#[require(Viewable)]
struct TurnViewable;

/// Marks an entity as the straight line representing the straight part of the line
/// towards current target.
///
/// If the current target is a waypoint, this line terminates at the waypoint.
/// Otherwise this line extends very far away (10000 nm).
#[derive(Component)]
#[require(Viewable)]
struct DirectViewable;

/// Marks an entity as an extended route viewable to follow the current target.
#[derive(Component, Default)]
#[require(Viewable)]
struct RouteViewable;

/// Marks an entity as the route preset viewable available for selection from the current target
/// waypoint.
#[derive(Component, Default)]
#[require(Viewable)]
struct PresetViewable;

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct TargetOverride {
    pub target: Target,
    pub cause:  TargetOverrideCause,
}

#[derive(Clone)]
pub enum Target {
    Yaw(nav::YawTarget),
    Waypoint(Entity),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TargetOverrideCause {
    None,
    SetHeading,
}
