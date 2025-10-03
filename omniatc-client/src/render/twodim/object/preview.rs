//! Draws the preview lines for the currently selected object.
//!
//! # Airborne objects
//! ## Current viewable
//! Consists of a "turn" arc and a "direct" line,
//! representing the trajectory of the object if it were to continue towards its current target.
//!
//! The arc is updated by modifying the mesh vertex positions,
//! reflecting the change from the current ground heading to the target heading
//! with the turn radius inferred by converting ground speed and yaw rate to angular velocity.
//!
//! The turn arc is not drawn if the turn angle is less than 1 degree.
//! The direct line is not drawn when the target is a waypoint within the circle of turn radius.
//!
//! If the current target is a waypoint, the direct line terminates at the waypoint.
//! Otherwise this line extends at a sufficiently far distance (10000 nm) in the target heading.
//!
//! ## Route viewable
//! Consists of straight lines connecting the waypoints in the current route.
//! Each line segment is rendered by a separate entity.
//!
//! ## Preset viewable
//! If the current target is a waypoint,
//! each available route preset from that waypoint is drawn similarly to the route viewable.
//!
//! # Ground objects
//! ## Ground path viewable
//! Only displayed when the current active node is a taxi node.
//! Each path planned by `route::taxi` is drawn by
//! connecting the waypoints in the path with straight lines in separate entities.

use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::asset::{Assets, Handle, RenderAssetUsages};
use bevy::camera::visibility::Visibility;
use bevy::color::Color;
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{Has, QueryData, With, Without};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, Single, SystemParam};
use bevy::ecs::world::Mut;
use bevy::math::Vec2;
use bevy::mesh::{Mesh, Mesh2d, PrimitiveTopology, VertexAttributeValues};
use bevy::sprite_render::{ColorMaterial, MeshMaterial2d};
use bevy::transform::components::Transform;
use bevy_mod_config::{Config, ReadConfig};
use either::Either;
use itertools::Itertools;
use math::{Angle, Heading, Length, Position, TurnDirection, find_circle_tangent_towards};
use omniatc::QueryTryLog;
use omniatc::level::object::{self, Object};
use omniatc::level::route::{self, Route};
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{ground, nav, plane};
use omniatc::util::EnumScheduleConfig;
use store::{NavLimits, YawTarget};

use super::SetColorThemeSystemSet;
use crate::render;
use crate::render::object_info;
use crate::render::twodim::Zorder;
use crate::util::{ActiveCamera2d, shapes};

const ARC_DENSITY: Angle = Angle::from_degrees(10.0);

pub(super) struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            update_system.in_set(render::SystemSets::Update).after_all::<SetColorThemeSystemSet>(),
        );
    }
}

/// Marks an entity as a preview viewable for airborne objects.
#[derive(Component, Default)]
struct AirborneViewable;

/// Marks an entity as a preview viewable for ground objects.
#[derive(Component, Default)]
struct GroundViewable;

fn update_system(
    mut materials: Local<Materials>,
    mut stages: ParamSet<(Init, DrawCurrent, DrawMainRoute, DrawPresets, DrawGroundPaths)>,
) {
    let materials = &mut *materials;
    let Some(init) = stages.p0().init(materials) else { return };

    if init.is_airborne {
        let target = stages.p1().draw(init.object, init.materials.current);
        stages.p2().draw_current_plan(init.object, init.materials.route);
        if let Some(target) = target {
            stages.p3().draw_avail_presets(target, init.materials.preset);
        } else {
            let mut stage = stages.p3();
            for (entity, _) in stage.viewable_query {
                stage.draw_once.commands.entity(entity).despawn();
            }
        }
    }
    if init.is_ground {
        stages.p4().draw(
            init.object,
            init.materials.ground_path_best,
            init.materials.ground_path_alt,
            init.materials.ground_path_preview,
        );
    }
}

#[derive(Default)]
struct Materials {
    normal:           Option<Handle<ColorMaterial>>,
    set_heading:      Option<Handle<ColorMaterial>>,
    preset:           Option<Handle<ColorMaterial>>,
    ground_path_best: Option<Handle<ColorMaterial>>,
    ground_path_alt:  Option<Handle<ColorMaterial>>,
}

#[derive(SystemParam)]
struct Init<'w, 's> {
    airborne_vis_query:    Query<'w, 's, &'static mut Visibility, With<AirborneViewable>>,
    ground_vis_query:
        Query<'w, 's, &'static mut Visibility, (With<GroundViewable>, Without<AirborneViewable>)>,
    object:                Res<'w, object_info::CurrentObject>,
    classify_object_query: Query<'w, 's, (Has<object::Airborne>, Has<object::OnGround>)>,
    override_query:        Query<'w, 's, &'static AirborneTargetOverride>,
    materials:             ResMut<'w, Assets<ColorMaterial>>,
    conf:                  ReadConfig<'w, 's, super::Conf>,
}

impl Init<'_, '_> {
    fn init<'materials>(
        &mut self,
        materials: &'materials mut Materials,
    ) -> Option<InitResult<'materials>> {
        let conf = self.conf.read();

        let (is_airborne, is_ground) = self
            .object
            .0
            .and_then(|object| self.classify_object_query.get(object).ok())
            .unwrap_or((false, false));

        for mut vis in &mut self.airborne_vis_query {
            *vis = if is_airborne { Visibility::Visible } else { Visibility::Hidden };
        }
        for mut vis in &mut self.ground_vis_query {
            *vis = if is_ground { Visibility::Visible } else { Visibility::Hidden };
        }

        let [
            normal_material,
            set_heading_material,
            preset_material,
            ground_path_material_best,
            ground_path_material_alt,
        ] = [
            (&mut materials.normal, conf.preview_line.color_normal),
            (&mut materials.set_heading, conf.preview_line.color_set_heading),
            (&mut materials.preset, conf.preview_line.color_preset),
            (&mut materials.ground_path_best, conf.preview_line.color_ground_path_best),
            (&mut materials.ground_path_alt, conf.preview_line.color_ground_path_alt),
        ]
        .map(|(local, color)| &*match local {
            None => local.insert(self.materials.add(color)),
            Some(handle) => {
                *self.materials.get_mut(&*handle).expect("strong reference must exist") =
                    ColorMaterial::from_color(color);
                handle
            }
        });

        let object = self.object.0?;
        let current_material = match self.override_query.get(object) {
            Ok(AirborneTargetOverride { cause: TargetOverrideCause::SetRoute, .. }) => {
                set_heading_material
            }
            _ => normal_material,
        };

        Some(InitResult {
            object,
            is_airborne,
            is_ground,
            materials: UsedMaterials {
                current:             current_material,
                route:               normal_material,
                preset:              preset_material,
                ground_path_best:    ground_path_material_best,
                ground_path_alt:     Some(ground_path_material_alt)
                    .filter(|_| conf.preview_line.render_ground_path_alt),
                ground_path_preview: set_heading_material,
            },
        })
    }
}

struct InitResult<'a> {
    object:      Entity,
    is_airborne: bool,
    is_ground:   bool,
    materials:   UsedMaterials<'a>,
}

struct UsedMaterials<'a> {
    current:             &'a Handle<ColorMaterial>,
    route:               &'a Handle<ColorMaterial>,
    preset:              &'a Handle<ColorMaterial>,
    ground_path_best:    &'a Handle<ColorMaterial>,
    ground_path_alt:     Option<&'a Handle<ColorMaterial>>,
    ground_path_preview: &'a Handle<ColorMaterial>,
}

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct AirborneTargetOverride {
    pub target: AirborneTarget,
    pub cause:  TargetOverrideCause,
}

#[derive(Component, Clone)]
#[component(storage = "SparseSet")]
pub struct GroundTargetOverride {
    pub target: GroundTarget,
    pub cause:  TargetOverrideCause,
}

#[derive(Clone)]
pub enum AirborneTarget {
    Yaw(YawTarget),
    Waypoint(Entity),
}

#[derive(Clone)]
pub enum GroundTarget {
    Endpoints(Vec<Entity>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TargetOverrideCause {
    None,
    SetRoute,
}

#[derive(SystemParam)]
struct DrawCurrent<'w, 's> {
    conf:           ReadConfig<'w, 's, super::Conf>,
    object_query:   Query<'w, 's, DrawCurrentObject>,
    waypoint_query: Query<'w, 's, &'static Waypoint>,
    turn_query: Option<
        Single<
            'w,
            's,
            (&'static mut Visibility, &'static Mesh2d, &'static mut Transform),
            With<TurnArcViewable>,
        >,
    >,
    direct_query: Option<
        Single<
            'w,
            's,
            &'static mut Transform,
            (With<DirectLineViewable>, Without<TurnArcViewable>),
        >,
    >,
    commands:       Commands<'w, 's>,
    meshes:         ResMut<'w, Assets<Mesh>>,
    shapes:         Res<'w, shapes::Meshes>,
    camera:         ActiveCamera2d<'w, 's>,
}

#[derive(QueryData)]
struct DrawCurrentObject {
    object:          &'static Object,
    vel_target:      &'static nav::VelocityTarget,
    limits:          &'static nav::Limits,
    plane_control:   Option<&'static plane::Control>,
    target_waypoint: Option<&'static nav::TargetWaypoint>,
    target_override: Option<&'static AirborneTargetOverride>,
}

impl DrawCurrent<'_, '_> {
    fn draw(
        &mut self,
        object_id: Entity,
        material: &Handle<ColorMaterial>,
    ) -> Option<AirborneTarget> {
        let Ok(DrawCurrentObjectItem {
            object: &Object { position: curr_pos, ground_speed: speed },
            vel_target: &nav::VelocityTarget { yaw: yaw_target, .. },
            limits: &nav::Limits(NavLimits { max_yaw_speed, .. }),
            plane_control,
            target_waypoint,
            target_override,
        }) = self.object_query.get(object_id)
        else {
            return None; // TODO ground object
        };

        let curr_pos = curr_pos.horizontal();
        let speed = speed.horizontal().magnitude_exact();
        let turn_radius = speed.arc_to_radius(max_yaw_speed);

        let target = target_override.map_or_else(
            || {
                let target = match target_waypoint {
                    Some(&nav::TargetWaypoint { waypoint_entity }) => {
                        AirborneTarget::Waypoint(waypoint_entity)
                    }
                    None => AirborneTarget::Yaw(yaw_target),
                };
                AirborneTargetOverride { target, cause: TargetOverrideCause::None }
            },
            AirborneTargetOverride::clone,
        );

        match target.target {
            AirborneTarget::Waypoint(waypoint_entity) => {
                let &Waypoint { position: waypoint_pos, .. } =
                    self.waypoint_query.log_get(waypoint_entity)?;
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
                        self.draw_direct_line(curr_pos, waypoint_pos, material);
                        if let Some((vis, _, _)) = self.turn_query.as_deref_mut() {
                            **vis = Visibility::Hidden;
                        }
                    }
                } else {
                    self.draw_direct_line(curr_pos, waypoint_pos, material);
                    if let Some((vis, _, _)) = self.turn_query.as_deref_mut() {
                        **vis = Visibility::Hidden;
                    }
                }
            }
            AirborneTarget::Yaw(yaw_target) => {
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
                    self.draw_direct_line(
                        curr_pos,
                        curr_pos + Length::from_nm(10000.) * target_heading,
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
        turn_radius: Length<f32>,
        curr_heading: Heading,
        waypoint_pos: Position<Vec2>,
        direction: TurnDirection,
        material: &Handle<ColorMaterial>,
    ) {
        let curr_pos_to_turn_center = curr_heading + Angle::RIGHT * direction;
        let turn_center = curr_pos + turn_radius * curr_pos_to_turn_center;

        if let Some(transition_point) =
            find_circle_tangent_towards(waypoint_pos, turn_center, turn_radius, direction)
        {
            let transition_heading = (transition_point - turn_center).heading();
            self.draw_turn_arc(
                turn_center,
                turn_radius,
                curr_pos_to_turn_center.opposite(),
                transition_heading,
                direction,
                material,
            );
            self.draw_direct_line(transition_point, waypoint_pos, material);
        } else {
            self.draw_turn_arc(
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
        yaw_target: YawTarget,
        curr_pos: Position<Vec2>,
        turn_radius: Length<f32>,
        curr_heading: Heading,
        material: &Handle<ColorMaterial>,
    ) {
        let (target_heading, direction) = match yaw_target {
            YawTarget::Heading(heading) => (heading, curr_heading.closer_direction_to(heading)),
            YawTarget::TurnHeading { heading, direction, .. } => (heading, direction),
        };
        let turn_angle = curr_heading.distance(target_heading, direction);

        if turn_angle.abs() > Angle::from_degrees(1.) {
            let curr_pos_to_turn_center = curr_heading + Angle::RIGHT * direction;
            let turn_center = curr_pos + turn_radius * curr_pos_to_turn_center;

            let turn_center_to_transition = curr_pos_to_turn_center.opposite() + turn_angle;

            self.draw_turn_arc(
                turn_center,
                turn_radius,
                curr_pos_to_turn_center.opposite(),
                turn_center_to_transition,
                direction,
                material,
            );
            let transition_point = turn_center + turn_radius * turn_center_to_transition;
            self.draw_direct_line(
                transition_point,
                transition_point + Length::from_nm(10000.) * target_heading,
                material,
            );
        } else {
            self.draw_direct_line(
                curr_pos,
                curr_pos + Length::from_nm(10000.) * target_heading,
                material,
            );
            if let Some((vis, _, _)) = self.turn_query.as_deref_mut() {
                **vis = Visibility::Hidden;
            }
        }
    }

    fn draw_turn_arc(
        &mut self,
        center: Position<Vec2>,
        radius: Length<f32>,
        mut start_heading: Heading,
        mut end_heading: Heading,
        direction: TurnDirection,
        material: &Handle<ColorMaterial>,
    ) {
        fn draw_arc_to(
            positions: &mut Vec<[f32; 3]>,
            radius: Length<f32>,
            start_heading: Heading,
            end_heading: Heading,
            thickness: f32,
        ) {
            positions.clear();
            let half_thickness = Length::new(thickness * 0.5);

            let angular_dist = start_heading.distance(end_heading, TurnDirection::Clockwise);
            // angular_dist < TAU, never overflows
            #[expect(clippy::cast_possible_truncation)]
            // angular_dist is a clockwise (positive) angle
            #[expect(clippy::cast_sign_loss)]
            let steps = (angular_dist / ARC_DENSITY).ceil() as u32;
            for step in 0..=steps {
                #[expect(clippy::cast_precision_loss)] // step <= steps derived from f32
                let heading = start_heading + ARC_DENSITY * (step as f32);
                let inner = (radius - half_thickness) * heading;
                let outer = (radius + half_thickness) * heading;
                positions.push([inner.0.x, inner.0.y, 0.]);
                positions.push([outer.0.x, outer.0.y, 0.]);
            }
        }

        if direction == TurnDirection::CounterClockwise {
            mem::swap(&mut start_heading, &mut end_heading);
        }

        let window_thickness =
            self.camera.scale() * self.conf.read().preview_line.airborne_thickness;
        if let Some(&mut (ref mut vis, mesh, ref mut tf)) = self.turn_query.as_deref_mut() {
            **vis = Visibility::Visible;
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
                TurnArcViewable,
            ));
        }
    }

    fn draw_direct_line(
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
                    self.conf.read().preview_line.airborne_thickness,
                    Zorder::ObjectTrackPreview,
                    start,
                    end,
                    &self.camera,
                ),
                MeshMaterial2d(material.clone()),
                DirectLineViewable,
            ));
        }
    }
}

/// Marks an entity as the arc representing the turn from current heading to current yaw target.
#[derive(Component)]
#[require(AirborneViewable)]
struct TurnArcViewable;

/// Marks an entity as the straight line representing the straight part of the line
/// towards current target.
///
/// If the current target is a waypoint, this line terminates at the waypoint.
/// Otherwise this line extends very far away (10000 nm).
#[derive(Component)]
#[require(AirborneViewable)]
struct DirectLineViewable;

#[derive(SystemParam)]
struct DrawMainRoute<'w, 's> {
    object_query:   Query<'w, 's, &'static Route>,
    viewable_query: Query<'w, 's, (Entity, &'static mut Transform), With<RouteViewable>>,
    draw_once:      DrawRouteOnce<'w, 's>,
}

impl DrawMainRoute<'_, '_> {
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
    draw_once:              DrawRouteOnce<'w, 's>,
}

impl DrawPresets<'_, '_> {
    fn draw_avail_presets(&mut self, target: AirborneTarget, material: &Handle<ColorMaterial>) {
        let AirborneTarget::Waypoint(waypoint) = target else { return };
        let Ok(presets) = self.waypoint_presets_query.get(waypoint) else { return };
        let mut viewables = self.viewable_query.iter_mut();

        for preset_id in presets.iter() {
            let Some(preset) = self.preset_query.log_get(preset_id) else { continue };
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

/// Shared code for drawing route lines, used in [`DrawMainRoute`] and [`DrawPresets`].
#[derive(SystemParam)]
struct DrawRouteOnce<'w, 's> {
    commands:       Commands<'w, 's>,
    shapes:         Res<'w, shapes::Meshes>,
    conf:           ReadConfig<'w, 's, super::Conf>,
    waypoint_query: Query<'w, 's, &'static Waypoint>,
    camera:         ActiveCamera2d<'w, 's>,
}

impl DrawRouteOnce<'_, '_> {
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
                    let Some(waypoint) = self.waypoint_query.log_get(node.waypoint) else {
                        continue;
                    };
                    positions.push(waypoint.position.horizontal());
                }
                route::Node::AlignRunway(node) => {
                    let Some(waypoint) = self.waypoint_query.log_get(node.runway) else { continue };
                    positions.push(waypoint.position.horizontal());
                }
                _ => {}
            }
        }

        let conf = self.conf.read();
        for (&start, &end) in positions.iter().tuple_windows() {
            if let Some(mut tf) = viewables.next() {
                shapes::set_square_line_transform(&mut tf, start, end);
            } else {
                self.commands.spawn((
                    self.shapes.line_from_to(
                        conf.preview_line.airborne_thickness,
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

/// Marks an entity as an extended route viewable to follow the current target.
#[derive(Component, Default)]
#[require(AirborneViewable)]
struct RouteViewable;

/// Marks an entity as a segment of route preset viewable
/// for presets selectable from the current target waypoint.
#[derive(Component, Default)]
#[require(AirborneViewable)]
struct PresetViewable;

#[derive(SystemParam)]
struct DrawGroundPaths<'w, 's> {
    object_query:
        Query<'w, 's, (&'static route::PossiblePaths, Option<&'static GroundTargetOverride>)>,
    viewable_query: Query<
        'w,
        's,
        (Entity, &'static mut Transform, &'static mut MeshMaterial2d<ColorMaterial>),
        With<GroundPathViewable>,
    >,
    draw_once:      DrawGroundPathOnce<'w, 's>,
}

impl DrawGroundPaths<'_, '_> {
    fn draw(
        &mut self,
        object_id: Entity,
        material_best: &Handle<ColorMaterial>,
        material_alt: Option<&Handle<ColorMaterial>>,
        material_preview: &Handle<ColorMaterial>,
    ) {
        let mut viewables = self.viewable_query.iter_mut();

        let mut render_paths = Vec::new();
        if let Ok((paths, target_override)) = self.object_query.get(object_id) {
            if let Some(GroundTargetOverride { target, .. }) = target_override {
                match target {
                    GroundTarget::Endpoints(endpoints) => {
                        render_paths
                            .push((material_preview, Either::Right(endpoints.iter().copied())));
                    }
                }
            }

            render_paths.extend(paths.paths.iter().enumerate().filter_map(|(index, path)| {
                let material = if index == 0 { material_best } else { material_alt? };
                Some((material, Either::Left(path.endpoints())))
            }));
        }

        for (material, path) in render_paths {
            self.draw_once.draw(path, material, viewables.by_ref());
        }

        for (entity, _, _) in viewables {
            self.draw_once.commands.entity(entity).despawn();
        }
    }
}

#[derive(SystemParam)]
struct DrawGroundPathOnce<'w, 's> {
    commands:       Commands<'w, 's>,
    shapes:         Res<'w, shapes::Meshes>,
    conf:           ReadConfig<'w, 's, super::Conf>,
    endpoint_query: Query<'w, 's, &'static ground::Endpoint>,
    camera:         ActiveCamera2d<'w, 's>,
}

impl DrawGroundPathOnce<'_, '_> {
    fn draw<'w>(
        &mut self,
        path: impl Iterator<Item = Entity>,
        material: &Handle<ColorMaterial>,
        mut viewables: impl Iterator<
            Item = (Entity, Mut<'w, Transform>, Mut<'w, MeshMaterial2d<ColorMaterial>>),
        >,
    ) {
        let endpoint_pairs = path
            .filter_map(|endpoint_id| {
                let endpoint = self.endpoint_query.log_get(endpoint_id)?;
                Some(endpoint.position)
            })
            .tuple_windows();
        for (start, end) in endpoint_pairs {
            if let Some((_, mut tf, mut material_ref)) = viewables.next() {
                shapes::set_square_line_transform(&mut tf, start, end);
                if material_ref.0 != *material {
                    material_ref.0 = material.clone();
                }
            } else {
                let conf = self.conf.read();
                self.commands.spawn((
                    self.shapes.line_from_to(
                        conf.preview_line.ground_thickness,
                        Zorder::PossibleGroundPathPreview,
                        start,
                        end,
                        &self.camera,
                    ),
                    MeshMaterial2d(material.clone()),
                    GroundPathViewable,
                ));
            }
        }
    }
}

/// Marks an entity as a segment of ground path viewable
/// for the paths planned by the ground pathfinder.
#[derive(Component, Default)]
#[require(GroundViewable)]
struct GroundPathViewable;

#[derive(Config)]
pub(super) struct Conf {
    /// Thickness of planned track preview line for airborne objects.
    #[config(default = 1.0)]
    airborne_thickness:     f32,
    /// Thickness of planned track preview line for ground objects.
    #[config(default = 1.5)]
    ground_thickness:       f32,
    /// Color of planned track preview line,
    /// including both the immediate track and the planned route.
    #[config(default = Color::srgb(0.9, 0.7, 0.8))]
    color_normal:           Color,
    /// Color of planned track preview line when setting heading.
    #[config(default = Color::srgb(0.9, 0.9, 0.6))]
    color_set_heading:      Color,
    /// Color of available route presets from the current target waypoint.
    #[config(default = Color::srgb(0.5, 0.6, 0.8))]
    color_preset:           Color,
    /// Color of the best path found by the ground pathfinder.
    #[config(default = Color::srgb(0.5, 0.8, 0.6))]
    color_ground_path_best: Color,
    /// Color of the other paths found by the ground pathfinder.
    #[config(default = Color::srgb(0.4, 0.4, 0.1))]
    color_ground_path_alt:  Color,
    /// Whether to render alternative paths found by the ground pathfinder.
    #[config(default = false)]
    render_ground_path_alt: bool,
}
