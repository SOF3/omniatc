use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::camera::Camera2d;
use bevy::color::Color;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::{QueryData, With};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, ResMut, SystemParam};
use bevy::input::ButtonInput;
use bevy::input::keyboard::KeyCode;
use bevy::math::Vec2;
use bevy::transform::components::GlobalTransform;
use bevy_mod_config::{AppExt, Config, ReadConfig};
use math::{Angle, Length, Position, Squared, point_segment_closest};
use omniatc::level::instr::CommandsExt;
use omniatc::level::object::Object;
use omniatc::level::route::{
    self, ClosurePathfindContext, PathfindMode, PathfindOptions, Route, TaxiStopMode,
    pathfind_through_subseq,
};
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{ground, instr, object, plane, taxi};
use omniatc::{QueryTryLog, try_log, try_log_return};
use ordered_float::OrderedFloat;
use store::{TaxiLimits, YawTarget};

use super::object::preview;
use crate::render::object_info;
use crate::{ConfigManager, EguiUsedMargins, UpdateSystemSets, input};

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
        SetNavTargetParams,
        CleanupPreviewParams,
    )>,
) {
    if margins.pointer_acquired {
        return;
    }

    let mut determine_mode = params.p0();
    let mut is_preview = false;
    if let Some(cursor_camera_value) = determine_mode.current_cursor_camera.value
        && let Ok(&camera_tf) = determine_mode.camera_query.get(cursor_camera_value.camera_entity)
    {
        let mode = determine_mode.determine();
        match mode {
            Mode::SelectObject => params.p1().run(cursor_camera_value.world_pos, camera_tf),
            Mode::SetRoute(set_route) => {
                params.p2().run(cursor_camera_value.world_pos, camera_tf, set_route);
                is_preview = !set_route.commit;
            }
        }
    }

    params.p3().run(is_preview);
}

#[derive(Clone, Copy, Default)]
enum PickRouteKey {
    #[default]
    None,
    Reset,
    Append,
}

#[derive(SystemParam)]
pub(super) struct DetermineMode<'w, 's> {
    current_cursor_camera: Res<'w, input::CursorState>,
    camera_query:          Query<'w, 's, &'static GlobalTransform, With<Camera2d>>,
    margins:               Res<'w, EguiUsedMargins>,
    prev_pick_key:         Local<'s, PickRouteKey>,
    hotkeys:               Res<'w, input::Hotkeys>,
}

impl DetermineMode<'_, '_> {
    fn determine(&mut self) -> Mode {
        let pick_key = if self.hotkeys.pick_route {
            PickRouteKey::Reset
        } else if self.hotkeys.append_route {
            PickRouteKey::Append
        } else {
            PickRouteKey::None
        };

        match (mem::replace(&mut *self.prev_pick_key, pick_key), pick_key) {
            // not picking at all
            (PickRouteKey::None, PickRouteKey::None) => Mode::SelectObject,
            // start/continue preview without append
            (_, PickRouteKey::Reset) => Mode::SetRoute(SetRoute { commit: false, append: false }),
            // start/continue preview with append
            (_, PickRouteKey::Append) => Mode::SetRoute(SetRoute { commit: false, append: true }),
            // stop picking, no append opted
            (PickRouteKey::Reset, PickRouteKey::None) => {
                Mode::SetRoute(SetRoute { commit: true, append: false })
            }
            // stop picking, had append opted
            (PickRouteKey::Append, PickRouteKey::None) => {
                Mode::SetRoute(SetRoute { commit: true, append: true })
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Mode {
    SelectObject,
    SetRoute(SetRoute),
}

#[derive(Clone, Copy)]
struct SetRoute {
    commit: bool,
    append: bool,
}

#[derive(SystemParam)]
pub(super) struct SelectObjectParams<'w, 's> {
    cursor_state:           Res<'w, input::CursorState>,
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
        if self.cursor_state.left_just_down() {
            self.current_object.0 = closest_object;
        }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
struct SetNavTargetObjectQuery {
    object:                    &'static Object,
    ground:                    Option<&'static object::OnGround>,
    plane:                     Option<&'static plane::Control>,
    airborne_preview_override: Option<&'static mut preview::AirborneTargetOverride>,
    ground_preview_override:   Option<&'static mut preview::GroundTargetOverride>,
    route:                     Option<&'static Route>,
}

#[derive(SystemParam)]
pub(super) struct SetNavTargetParams<'w, 's> {
    waypoint_query: Query<'w, 's, (Entity, &'static Waypoint)>,
    segment_query:  Query<'w, 's, (Entity, &'static ground::Segment)>,
    endpoint_query: Query<'w, 's, &'static ground::Endpoint>,
    object_query:   Query<'w, 's, SetNavTargetObjectQuery>,
    conf:           ReadConfig<'w, 's, Conf>,
    current_object: Res<'w, object_info::CurrentObject>,
    propose:        ProposeParams<'w, 's>,
}

fn find_closest_waypoint(
    waypoint_query: Query<(Entity, &'static Waypoint)>,
    cursor_world_pos: Position<Vec2>,
    click_tolerance: Length<f32>,
) -> Option<(Entity, Squared<Length<f32>>)> {
    waypoint_query
        .iter()
        .map(|(entity, waypoint)| {
            let waypoint_pos = waypoint.position.horizontal();
            (entity, waypoint_pos.distance_squared(cursor_world_pos))
        })
        .filter(|(_, dist_sq)| *dist_sq < click_tolerance.squared())
        .min_by_key(|(_, dist_sq)| OrderedFloat(dist_sq.0))
}

fn find_closest_segment(
    segment_query: Query<(Entity, &ground::Segment)>,
    endpoint_query: Query<&ground::Endpoint>,
    cursor_world_pos: Position<Vec2>,
    click_tolerance: Length<f32>,
) -> Option<(Entity, Squared<Length<f32>>)> {
    segment_query
        .iter()
        .filter_map(|(entity, segment)| {
            let alpha = try_log!(endpoint_query.get(segment.alpha), expect "segment must reference valid alpha endpoint {:?}"( segment.alpha) or return None);
            let beta = try_log!(endpoint_query.get(segment.beta), expect "segment must reference valid beta endpoint {:?}"( segment.beta) or return None);
            let closest = point_segment_closest(cursor_world_pos, alpha.position, beta.position);
            Some((entity, closest.distance_squared(cursor_world_pos)))
        })
        .filter(|(_, dist_sq)| *dist_sq < click_tolerance.squared())
        .min_by_key(|(_, dist_sq)| OrderedFloat(dist_sq.0))
}

impl SetNavTargetParams<'_, '_> {
    fn run(
        &mut self,
        cursor_world_pos: Position<Vec2>,
        camera_tf: GlobalTransform,
        set_route: SetRoute,
    ) {
        let conf = self.conf.read();

        let click_tolerance = Length::new(camera_tf.scale().x) * conf.waypoint_select_tolerance;

        if let Some(object) = self.current_object.0 {
            let mut object_data = try_log_return!(
                self.object_query.get_mut(object),
                expect "object selected by cursor"
            );

            if let Some(ground) = object_data.ground {
                // Object is on ground; click target must be a segment
                let closest_segment = find_closest_segment(
                    self.segment_query,
                    self.endpoint_query,
                    cursor_world_pos,
                    click_tolerance,
                );
                if let Some((segment, _)) = closest_segment {
                    self.propose.propose_segment(
                        object,
                        segment,
                        ground,
                        object_data.route,
                        object_data.ground_preview_override.as_deref_mut(),
                        set_route,
                    );
                }
            } else {
                // Object is airborne; click target may be a waypoint or just heading
                if let Some((waypoint, _)) =
                    find_closest_waypoint(self.waypoint_query, cursor_world_pos, click_tolerance)
                {
                    self.propose.propose_set_waypoint(object, waypoint, object_data, set_route);
                } else {
                    self.propose.propose_set_heading(
                        object,
                        cursor_world_pos,
                        object_data,
                        set_route,
                    );
                }
            }
        } else {
            // TODO should we show taxiway/waypoint info here?
        }
    }
}

#[derive(SystemParam)]
struct ProposeParams<'w, 's> {
    commands:         Commands<'w, 's>,
    margins:          Res<'w, EguiUsedMargins>,
    buttons:          Res<'w, ButtonInput<KeyCode>>, // TODO migrate to input::Hotkeys
    selected_segment: Local<'s, Option<Entity>>,
    endpoint_query:   Query<'w, 's, &'static ground::Endpoint>,
    segment_query:    Query<'w, 's, (&'static ground::Segment, &'static ground::SegmentLabel)>,
    object_query:     Query<'w, 's, &'static taxi::Limits>,
}

impl ProposeParams<'_, '_> {
    fn propose_set_waypoint(
        &mut self,
        object: Entity,
        waypoint: Entity,
        object_data: SetNavTargetObjectQueryItem,
        // TODO support append?
        SetRoute { commit, append: _ }: SetRoute,
    ) {
        if commit {
            self.commands.send_instruction(object, instr::SetWaypoint { waypoint });
        } else {
            let target_override_value = preview::AirborneTargetOverride {
                target: preview::AirborneTarget::Waypoint(waypoint),
                cause:  preview::TargetOverrideCause::SetRoute,
            };
            if let Some(mut target_override) = object_data.airborne_preview_override {
                *target_override = target_override_value;
            } else {
                self.commands.entity(object).insert(target_override_value);
            }
        }
    }

    fn propose_set_heading(
        &mut self,
        object: Entity,
        world_pos: Position<Vec2>,
        SetNavTargetObjectQueryItem {
            object: &Object { position: object_pos, .. },
            ground: _,
            plane,
            airborne_preview_override,
            ground_preview_override: _,
            route: _,
        }: SetNavTargetObjectQueryItem,
        SetRoute { commit, append: _ }: SetRoute, // TODO support append?
    ) {
        let object_pos = object_pos.horizontal();
        let target_heading = (world_pos - object_pos).heading();
        let mut target = YawTarget::Heading(target_heading);
        if !self.margins.keyboard_acquired
            && self.buttons.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight])
            && let Some(plane) = plane
        {
            let reflex_dir = -plane.heading.closer_direction_to(target_heading);
            target = YawTarget::TurnHeading {
                heading:           target_heading,
                direction:         reflex_dir,
                remaining_crosses: 0,
            };
        }

        if commit {
            self.commands.send_instruction(object, instr::SetHeading { target });
        } else {
            let target_override_value = preview::AirborneTargetOverride {
                target: preview::AirborneTarget::Yaw(target),
                cause:  preview::TargetOverrideCause::SetRoute,
            };
            if let Some(mut target_override) = airborne_preview_override {
                *target_override = target_override_value;
            } else {
                self.commands.entity(object).insert(target_override_value);
            }
        }
    }

    fn propose_segment(
        &mut self,
        object: Entity,
        picked_segment: Entity,
        ground: &object::OnGround,
        route: Option<&Route>,
        preview_override: Option<&mut preview::GroundTargetOverride>,
        SetRoute { commit, append }: SetRoute,
    ) {
        let Some(&taxi::Limits(TaxiLimits { width, .. })) = self.object_query.log_get(object)
        else {
            return;
        };

        let Some((segment, segment_label)) = self.segment_query.log_get(picked_segment) else {
            return;
        };
        if segment.width < width {
            // Segment too narrow for object

            return;
        }

        if commit {
            self.commands.send_instruction(
                object,
                instr::AppendSegment {
                    clear_existing: !append,
                    segment:        segment_label.clone(),
                    stop_mode:      match segment_label {
                        ground::SegmentLabel::Taxiway { .. } => TaxiStopMode::LineUp,
                        ground::SegmentLabel::RunwayPair(_) => TaxiStopMode::HoldShort,
                        ground::SegmentLabel::Apron { .. } => TaxiStopMode::Exhaust,
                    },
                },
            );
        } else {
            let mut required_labels = Vec::new();
            if append {
                required_labels = route
                    .into_iter()
                    .flat_map(|route| {
                        route.iter().filter_map(|node| match node {
                            route::Node::Taxi(node) => {
                                Some(route::SubseqItem { label: &node.label, direction: None })
                            }
                            _ => None,
                        })
                    })
                    .collect();
            }

            let Some(current_ground_target_endpoint) =
                ground.target_endpoint(|id| self.segment_query.log_get(id).map(|s| s.0))
            else {
                return;
            };
            let path = pathfind_through_subseq(
                ClosurePathfindContext {
                    endpoint_fn: |endpoint| self.endpoint_query.log_get(endpoint),
                    segment_fn:  |segment| self.segment_query.log_get(segment),
                },
                ground.segment,
                current_ground_target_endpoint,
                &required_labels,
                PathfindMode::Segment(picked_segment),
                PathfindOptions { initial_speed: None, min_width: Some(width) },
            );

            if let Some(path) = path {
                let target_override_value = preview::GroundTargetOverride {
                    target: preview::GroundTarget::Endpoints(path.endpoints),
                    cause:  preview::TargetOverrideCause::SetRoute,
                };
                if let Some(target_override) = preview_override {
                    *target_override = target_override_value;
                } else {
                    self.commands.entity(object).insert(target_override_value);
                }
            }
        }
    }
}

#[derive(SystemParam)]
pub(super) struct CleanupPreviewParams<'w, 's> {
    airborne_query: Query<'w, 's, (Entity, &'static preview::AirborneTargetOverride)>,
    ground_query:   Query<'w, 's, (Entity, &'static preview::GroundTargetOverride)>,
    need_rerun:     Local<'s, bool>,
    commands:       Commands<'w, 's>,
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
        for (entity, comp) in self.airborne_query {
            if comp.cause == preview::TargetOverrideCause::SetRoute {
                self.commands.entity(entity).remove::<preview::AirborneTargetOverride>();
            }
        }
        for (entity, comp) in self.ground_query {
            if comp.cause == preview::TargetOverrideCause::SetRoute {
                self.commands.entity(entity).remove::<preview::GroundTargetOverride>();
            }
        }
    }
}
