use std::any::TypeId;

use bevy::app::{App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{ParamSet, Query, Res, ResMut, SystemParam};
use bevy_egui::{egui, EguiContextPass, EguiContexts};
use omniatc::level::aerodrome::Aerodrome;
use omniatc::level::route::{self, Route};
use omniatc::level::runway::Runway;
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{nav, object, plane, wake, wind};
use omniatc::math::Sign;
use omniatc::try_log_return;
use omniatc::units::{Angle, TurnDirection};

use crate::{EguiSystemSets, EguiUsedMargins};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<CurrentHoveredObject>();
        app.init_resource::<CurrentObject>();
        app.add_systems(EguiContextPass, setup_layout_system.in_set(EguiSystemSets::ObjectInfo));
    }
}

#[derive(Default, Resource)]
pub struct CurrentHoveredObject(pub Option<Entity>);

/// Current object the user selected.
#[derive(Default, Resource)]
pub struct CurrentObject(pub Option<Entity>);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct CurrentObjectSelectorSystemSet;

fn setup_layout_system(
    mut contexts: EguiContexts,
    current_object: Res<CurrentObject>,
    object_query: Query<(WriteQueryData, &object::Display)>,
    mut margins: ResMut<EguiUsedMargins>,
    mut write_params: WriteParams,
) {
    let Some(ctx) = contexts.try_ctx_mut() else { return };

    let width = egui::SidePanel::right("object_info")
        .resizable(true)
        .show(ctx, |ui| {
            let Some(object_entity) = current_object.0 else {
                ui.label("Click on an aircraft to view details");
                return;
            };

            let object = try_log_return!(
                object_query.get(object_entity),
                expect "CurrentObject points to non-object"
            );

            ui.heading(&object.1.name);
            egui::ScrollArea::vertical().show(ui, |ui| {
                show_writers(ui, &object.0, &mut write_params);
            });
            ui.allocate_rect(ui.available_rect_before_wrap(), egui::Sense::click());
        })
        .response
        .rect
        .width();
    margins.right += width;
}

trait Writer: QueryData {
    type SystemParams<'w, 's>: SystemParam;

    fn title() -> &'static str;

    fn default_open() -> bool { true }

    fn should_show(this: &Self::Item<'_>) -> bool;

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, param: &mut Self::SystemParams<'_, '_>);
}

macro_rules! writer_def {
    ($($index:ident: $writer:ident,)*) => {
        #[derive(QueryData)]
        struct WriteQueryData {
            $(
                $index: $writer,
            )*
        }

        #[derive(SystemParam)]
        struct WriteParams<'w, 's> {
            sets: ParamSet<'w, 's, ($(<$writer as Writer>::SystemParams<'w, 's>,)*)>,
        }

        fn show_writers(ui: &mut egui::Ui, qd: &WriteQueryDataItem, params: &mut WriteParams) {
            $(
                {
                    let qd = &qd.$index;

                    if <$writer as Writer>::should_show(qd) {
                        let mut params = params.sets.$index();
                        egui::CollapsingHeader::new(<$writer as Writer>::title())
                            .default_open(<$writer as Writer>::default_open())
                            .show(ui, |ui| {
                                <$writer as Writer>::show(qd, ui, &mut params);
                            });
                    }
                }
            )*
        }
    }
}

writer_def! {
    p0: WriteDestination,
    p1: WriteDirection,
    p2: WriteAltitude,
    p3: WriteSpeed,
    p4: WriteEnv,
    p5: WriteRoute,
}

#[derive(QueryData)]
struct WriteDestination {
    dest: &'static object::Destination,
}

#[derive(SystemParam)]
struct WriteDestinationParams<'w, 's> {
    aerodrome: Query<'w, 's, &'static Aerodrome>,
    waypoint:  Query<'w, 's, &'static Waypoint>,
}

impl Writer for WriteDestination {
    type SystemParams<'w, 's> = WriteDestinationParams<'w, 's>;

    fn title() -> &'static str { "Destination" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        ui.label(match *this.dest {
            object::Destination::Landing { aerodrome } => {
                let data = try_log_return!(
                    params.aerodrome.get(aerodrome),
                    expect "Unknown aerodrome {aerodrome:?}"
                );
                format!("Arrival at {}", &data.name)
            }
            object::Destination::VacateAnyRunway => String::from("Land at any runway and vacate"),
            object::Destination::ReachWaypoint { min_altitude, waypoint_proximity } => {
                let mut waypoint_name = None;
                if let Some((waypoint_entity, _)) = waypoint_proximity {
                    if let Ok(data) = params.waypoint.get(waypoint_entity) {
                        waypoint_name = Some(&data.name);
                    }
                }

                match (min_altitude, waypoint_name) {
                    (Some(altitude), Some(waypoint)) => {
                        format!(
                            "Reach {waypoint:?} and climb past {:.0}ft",
                            altitude.amsl().into_feet()
                        )
                    }
                    (Some(altitude), None) => {
                        format!("Climb past {:.0}ft", altitude.amsl().into_feet())
                    }
                    (None, Some(waypoint)) => format!("Reach {waypoint:?}"),
                    (None, None) => String::from("None"),
                }
            }
        });
    }
}

#[derive(QueryData)]
struct WriteDirection {
    object:           &'static object::Object,
    plane_control:    Option<&'static plane::Control>,
    nav_vel:          Option<&'static nav::VelocityTarget>,
    target_waypoint:  Option<&'static nav::TargetWaypoint>,
    target_alignment: Option<(&'static nav::TargetAlignment, &'static nav::TargetAlignmentStatus)>,
}

impl Writer for WriteDirection {
    type SystemParams<'w, 's> = Query<'w, 's, &'static Waypoint>;

    fn title() -> &'static str { "Direction" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(
        this: &Self::Item<'_>,
        ui: &mut egui::Ui,
        waypoint_query: &mut Self::SystemParams<'_, '_>,
    ) {
        ui.label(format!(
            "Ground track: {:.0}\u{b0}",
            this.object.ground_speed.horizontal().heading().degrees()
        ));
        if let Some(control) = this.plane_control {
            ui.label(format!("Current yaw: {:.0}\u{b0}", control.heading.degrees()));
        }
        if let Some(nav_vel) = this.nav_vel {
            Self::show_yaw_target(ui, &nav_vel.yaw);
        }
        if let Some(target) = this.target_waypoint {
            let waypoint = try_log_return!(
                waypoint_query.get(target.waypoint_entity),
                expect "TargetWaypoint has invalid waypoint {:?}", target.waypoint_entity,
            );

            let distance = this.object.position.horizontal_distance_exact(waypoint.position);
            ui.label(format!("Target position: {} ({:.1} nm)", &waypoint.name, distance.into_nm()));
        }
        if let Some((target, target_status)) = this.target_alignment {
            Self::show_target_alignment(this, ui, waypoint_query, target, target_status);
        }
    }
}

impl WriteDirection {
    fn show_yaw_target(ui: &mut egui::Ui, target: &nav::YawTarget) {
        match target {
            nav::YawTarget::Heading(heading) => {
                ui.label(format!("Target yaw: {:.0}\u{b0}", heading.degrees()));
            }
            nav::YawTarget::TurnHeading { heading, direction, remaining_crosses } => {
                let dir = match direction {
                    TurnDirection::CounterClockwise => "left",
                    TurnDirection::Clockwise => "right",
                };

                match remaining_crosses {
                    0 => {
                        ui.label(format!("Target yaw: {:.0}\u{b0} ({dir} turn)", heading.degrees()))
                    }
                    1 => ui.label(format!(
                        "Target yaw: {:.0}\u{b0} after one full {dir} circle)",
                        heading.degrees()
                    )),
                    crosses => ui.label(format!(
                        "Target yaw: {:.0}\u{b0} after {crosses} full {dir} circles)",
                        heading.degrees()
                    )),
                };
            }
            nav::YawTarget::Speed(speed) => {
                if speed.is_zero() {
                    ui.label("Target yaw: maintain current heading");
                } else if speed.is_positive() {
                    ui.label(format!(
                        "Target yaw: {:.1}\u{b0}/s right",
                        speed.into_degrees_per_sec().abs()
                    ));
                } else {
                    ui.label(format!(
                        "Target yaw: {:.1}\u{b0}/s left",
                        speed.into_degrees_per_sec().abs()
                    ));
                }
            }
        }
    }

    fn show_target_alignment(
        this: &WriteDirectionItem,
        ui: &mut egui::Ui,
        waypoint_query: &Query<&Waypoint>,
        target: &nav::TargetAlignment,
        target_status: &nav::TargetAlignmentStatus,
    ) {
        let start_waypoint = try_log_return!(
            waypoint_query.get(target.start_waypoint),
            expect "TargetAlignment has invalid waypoint {:?}", target.start_waypoint,
        );
        let end_waypoint = try_log_return!(
            waypoint_query.get(target.end_waypoint),
            expect "TargetAlignment has invalid waypoint {:?}", target.end_waypoint,
        );

        let start_distance =
            this.object.position.horizontal_distance_exact(start_waypoint.position);
        let end_distance = this.object.position.horizontal_distance_exact(end_waypoint.position);
        ui.label(format!(
            "Target alignment: {} ({:.1} nm) -> {} ({:.1} nm)",
            &start_waypoint.name,
            start_distance.into_nm(),
            &end_waypoint.name,
            end_distance.into_nm(),
        ));

        {
            struct Indent;

            ui.indent(TypeId::of::<Indent>(), |ui| match target_status.activation {
                nav::TargetAlignmentActivationStatus::Uninit => {}
                nav::TargetAlignmentActivationStatus::PurePursuit(_) => {
                    ui.label(format!(
                        "Angular deviation: {:+.1}\u{b0}",
                        target_status.angular_deviation.into_degrees(),
                    ));
                }
                nav::TargetAlignmentActivationStatus::Unactivated => {
                    ui.label(format!(
                        "Angular deviation: {:+.1}\u{b0}",
                        target_status.angular_deviation.into_degrees(),
                    ));
                    ui.label(format!(
                        "Orthogonal deviation: {:.1} nm (> {:.1} nm)",
                        target_status.orthogonal_deviation.into_nm(),
                        target.activation_range.into_nm(),
                    ));
                }
                nav::TargetAlignmentActivationStatus::BeyondLookahead {
                    intersect_time,
                    projected_dist,
                } => {
                    let dist_to_start = projected_dist
                        - target.activation_range
                        - start_waypoint.position.horizontal_distance_exact(end_waypoint.position);
                    if dist_to_start.is_positive() {
                        let time_to_start =
                            dist_to_start / this.object.ground_speed.horizontal().magnitude_exact();
                        ui.label(format!(
                            "Entering segment projected range in {:.1}s",
                            time_to_start.as_secs_f32(),
                        ));
                    } else {
                        if let Some(intersect_time) = intersect_time {
                            ui.label(format!("Converging in {:.1}s", intersect_time.as_secs_f32()));
                        } else {
                            ui.label("Diverging from target and beyond alignment activation range");
                        }
                    }
                }
            });
        }
    }
}

#[derive(QueryData)]
struct WriteAltitude {
    object:       &'static object::Object,
    airborne:     Option<&'static object::Airborne>,
    target_alt:   Option<&'static nav::TargetAltitude>,
    target_glide: Option<(&'static nav::TargetGlide, &'static nav::TargetGlideStatus)>,
}

impl Writer for WriteAltitude {
    type SystemParams<'w, 's> = Query<'w, 's, &'static Waypoint>;

    fn title() -> &'static str { "Altitude" }

    fn should_show(this: &Self::Item<'_>) -> bool { this.airborne.is_some() }

    fn show(
        this: &Self::Item<'_>,
        ui: &mut egui::Ui,
        waypoint_query: &mut Self::SystemParams<'_, '_>,
    ) {
        ui.label(format!("Current: {:.0} ft", this.object.position.altitude().amsl().into_feet()));
        if let Some(airborne) = this.airborne {
            ui.label(format!("Vert rate: {:+.0} fpm", airborne.airspeed.vertical().into_fpm()));
        }

        if let Some(target_alt) = this.target_alt {
            let expedite = if target_alt.expedite { " (expedite)" } else { "" };
            ui.label(format!("Target: {:.0} ft{expedite}", target_alt.altitude.amsl().into_feet()));
        }

        if let Some((glide, glide_status)) = this.target_glide {
            let waypoint = try_log_return!(
                waypoint_query.get(glide.target_waypoint),
                expect "TargetGlide has invalid waypoint {:?}", glide.target_waypoint,
            );
            let target_altitude = waypoint.position.altitude().amsl().into_feet();

            if glide.glide_angle.is_zero() {
                ui.label(format!("Target: maintain {target_altitude} ft until {}", &waypoint.name));
            } else if glide.glide_angle.is_positive() {
                ui.label(format!(
                    "Target: {}\u{b0} climb to {}",
                    glide.glide_angle.into_degrees(),
                    &waypoint.name
                ));
            } else {
                ui.label(format!(
                    "Target: {}\u{b0} descent to {}",
                    glide.glide_angle.into_degrees().abs(),
                    &waypoint.name
                ));
            }

            {
                struct Indent;

                ui.indent(TypeId::of::<Indent>(), |ui| {
                    ui.label(format!(
                        "Target pitch: {:.1}\u{b0}",
                        glide_status.current_pitch.into_degrees()
                    ));
                    ui.label(format!(
                        "Vertical deviation: {:+.0} ft",
                        glide_status.altitude_deviation.into_feet()
                    ));
                    ui.label(format!(
                        "Horizontal distance to glidepath: {:+.1} nm",
                        glide_status.glidepath_distance.into_nm()
                    ));
                });
            }
        }
    }
}

#[derive(QueryData)]
struct WriteSpeed {
    object:   &'static object::Object,
    airborne: Option<&'static object::Airborne>,
    nav_vel:  Option<&'static nav::VelocityTarget>,
}

impl Writer for WriteSpeed {
    type SystemParams<'w, 's> = ();

    fn title() -> &'static str { "Speed" }

    fn should_show(_this: &Self::Item<'_>) -> bool { true }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, (): &mut Self::SystemParams<'_, '_>) {
        ui.label(format!(
            "Current ground: {:.0} kt",
            this.object.ground_speed.magnitude_exact().into_knots()
        ));
        if let Some(airborne) = this.airborne {
            ui.label(format!(
                "Current true airspeed: {:.0} kt",
                airborne.true_airspeed.horizontal().magnitude_exact().into_knots()
            ));
            ui.label(format!(
                "Current indicated airspeed: {:.0} kt",
                airborne.airspeed.horizontal().magnitude_exact().into_knots()
            ));
        }
        if let Some(nav_vel) = this.nav_vel {
            ui.label(format!("Target IAS: {:.0} kt", nav_vel.horiz_speed.into_knots()));
        }
    }
}

#[derive(QueryData)]
struct WriteEnv {
    wake:  Option<&'static wake::Detector>,
    wind:  Option<&'static wind::Detector>,
    plane: Option<&'static plane::Control>,
}

impl Writer for WriteEnv {
    type SystemParams<'w, 's> = ();

    fn title() -> &'static str { "Environment" }

    fn should_show(this: &Self::Item<'_>) -> bool { this.wake.is_some() || this.wind.is_some() }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, (): &mut Self::SystemParams<'_, '_>) {
        if let Some(wake) = this.wake {
            ui.label(format!("Wake: {:.2}", f64::from(wake.last_detected.0) / 60000.));
        }
        if let Some(wind) = this.wind {
            let wind = wind.last_computed;

            let magnitude = wind.magnitude_exact().into_knots();
            ui.label(format!(
                "Wind: {magnitude:.1} kt from {:.0}\u{b0}",
                wind.heading().opposite().degrees()
            ));

            if let Some(plane) = this.plane {
                let tail_wind = wind.project_onto_dir(plane.heading.into_dir2());
                let cross_wind = wind.project_onto_dir((plane.heading + Angle::RIGHT).into_dir2());

                match tail_wind.sign() {
                    Sign::Negative => {
                        ui.small(format!("Head wind: {:.1} kt", -tail_wind.into_knots()));
                    }
                    Sign::Zero => {}
                    Sign::Positive => {
                        ui.small(format!("Tail wind: {:.1} kt", tail_wind.into_knots()));
                    }
                }

                match cross_wind.sign() {
                    Sign::Negative => {
                        ui.small(format!(
                            "Cross wind from right: {:.1} kt",
                            -cross_wind.into_knots()
                        ));
                    }
                    Sign::Zero => {}
                    Sign::Positive => {
                        ui.small(format!(
                            "Cross wind from left: {:.1} kt",
                            cross_wind.into_knots()
                        ));
                    }
                }
            }
        }
    }
}

#[derive(QueryData)]
struct WriteRoute {
    route: Option<&'static Route>,
}

#[derive(SystemParam)]
struct WriteRouteParams<'w, 's> {
    waypoint:  Query<'w, 's, &'static Waypoint>,
    runway:    Query<'w, 's, (&'static Runway, &'static Waypoint)>,
    aerodrome: Query<'w, 's, &'static Aerodrome>,
}

impl Writer for WriteRoute {
    type SystemParams<'w, 's> = WriteRouteParams<'w, 's>;

    fn title() -> &'static str { "Route" }

    fn should_show(this: &Self::Item<'_>) -> bool {
        this.route.is_some_and(|r| r.current().is_some())
    }

    fn show(this: &Self::Item<'_>, ui: &mut egui::Ui, params: &mut Self::SystemParams<'_, '_>) {
        let Some(route) = this.route else { return };

        for node in route.iter() {
            write_route_node(ui, node, params);
        }
    }
}

fn write_route_node(ui: &mut egui::Ui, node: &route::Node, params: &WriteRouteParams) {
    match node {
        route::Node::DirectWaypoint(node) => {
            let waypoint = try_log_return!(params.waypoint.get(node.waypoint), expect "route must reference valid waypoint {:?}", node.waypoint);
            match node.proximity {
                route::WaypointProximity::FlyBy => ui.label(format!("Fly by {}", &waypoint.name)),
                route::WaypointProximity::FlyOver => {
                    ui.label(format!("Fly over {}", &waypoint.name))
                }
            };

            if let Some(altitude) = node.altitude {
                struct Indent;
                ui.indent(TypeId::of::<Indent>(), |ui| {
                    ui.label(format!("Pass at altitude {:.0} ft", altitude.amsl().into_feet()));
                });
            }
        }
        route::Node::SetAirSpeed(node) => {
            ui.label(format!("Set speed to {:.0} kt", node.speed.into_knots()));
            if let Some(error) = node.error {
                struct Indent;
                ui.indent(TypeId::of::<Indent>(), |ui| {
                    ui.label(format!("Maintain until \u{b1}{:.0} kt", error.into_knots()));
                });
            }
        }
        route::Node::StartSetAltitude(node) => {
            let expedite = if node.expedite { " (expedite)" } else { "" };
            ui.label(format!(
                "Start approaching altitude {:.0} ft{expedite}",
                node.altitude.amsl().into_feet()
            ));
            if let Some(error) = node.error {
                struct Indent;
                ui.indent(TypeId::of::<Indent>(), |ui| {
                    ui.label(format!("Maintain until \u{b1}{:.0} ft", error.into_feet()));
                });
            }
        }
        route::Node::AlignRunway(node) => {
            let (runway, waypoint) = try_log_return!(params.runway.get(node.runway), expect "route must reference valid runway {:?}", node.runway);
            let aerodrome = try_log_return!(params.aerodrome.get(runway.aerodrome), expect "runway must reference valid aerodrome {:?}", runway.aerodrome);
            ui.label(format!("Align towards runway {} of {}", &waypoint.name, &aerodrome.name));
        }
    }
}
