use bevy::ecs::entity::Entity;
use bevy::ecs::query::QueryData;
use bevy::ecs::system::{Commands, Query, Res, SystemParam};
use bevy_egui::egui;
use math::{Heading, TurnDirection};
use omniatc::level::waypoint::Waypoint;
use omniatc::level::{comm, ground, nav, object, plane};
use omniatc::{QueryTryLog, try_log_return};
use store::YawTarget;

use super::Writer;
use crate::input;
use crate::util::{heading_to_approx_name, new_type_id};

#[derive(QueryData)]
pub struct ObjectQuery {
    entity:           Entity,
    object:           &'static object::Object,
    airborne:         Option<&'static object::Airborne>,
    plane_control:    Option<&'static plane::Control>,
    nav_vel:          Option<&'static nav::VelocityTarget>,
    target_waypoint:  Option<&'static nav::TargetWaypoint>,
    target_alignment: Option<(&'static nav::TargetAlignment, &'static nav::TargetAlignmentStatus)>,
    ground:           Option<&'static object::OnGround>,
}

#[derive(SystemParam)]
pub struct WriteParams<'w, 's> {
    waypoint_query: Query<'w, 's, &'static Waypoint>,
    segment_query:  Query<'w, 's, (&'static ground::Segment, &'static ground::SegmentLabel)>,
    endpoint_query: Query<'w, 's, &'static ground::Endpoint>,
    commands:       Commands<'w, 's>,
    hotkeys:        Res<'w, input::Hotkeys>,
}

impl Writer for ObjectQuery {
    type SystemParams<'w, 's> = WriteParams<'w, 's>;

    fn title() -> &'static str { "Direction" }

    fn should_show(_this: &Self::Item<'_, '_>) -> bool { true }

    fn show(this: &Self::Item<'_, '_>, ui: &mut egui::Ui, params: &mut WriteParams) {
        ui.label(format!(
            "Ground track: {:.0}\u{b0}",
            this.object.ground_speed.horizontal().heading().degrees()
        ));
        if this.airborne.is_some() {
            if let Some(control) = this.plane_control {
                ui.label(format!("Current yaw: {:.0}\u{b0}", control.heading.degrees()));
            }
            if let Some(nav_vel) = this.nav_vel {
                show_yaw_target(ui, nav_vel, &mut params.commands, this.entity, &params.hotkeys);
            }
            if let Some(target) = this.target_waypoint {
                let Some(waypoint) = params.waypoint_query.log_get(target.waypoint_entity) else {
                    return;
                };

                let distance = this.object.position.horizontal_distance_exact(waypoint.position);
                ui.label(format!(
                    "Target position: {} ({:.1} nm)",
                    &waypoint.name,
                    distance.into_nm()
                ));
            }
            if let Some((target, target_status)) = this.target_alignment {
                show_target_alignment(this, ui, &params.waypoint_query, target, target_status);
            }
        }
        if let Some(ground) = this.ground {
            show_ground(
                ui,
                ground,
                &params.segment_query,
                &params.endpoint_query,
                &params.waypoint_query,
            );
        }
    }
}

fn show_yaw_target(
    ui: &mut egui::Ui,
    nav_vel: &nav::VelocityTarget,
    commands: &mut Commands,
    object: Entity,
    hotkeys: &input::Hotkeys,
) {
    let target = &nav_vel.yaw;

    let target_degrees = match target {
        YawTarget::Heading(heading) => {
            ui.label(format!("Target yaw: {:.0}\u{b0}", heading.degrees()));
            heading.degrees()
        }
        YawTarget::TurnHeading { heading, direction, remaining_crosses } => {
            let dir = match direction {
                TurnDirection::CounterClockwise => "left",
                TurnDirection::Clockwise => "right",
            };

            match remaining_crosses {
                0 => ui.label(format!("Target yaw: {:.0}\u{b0} ({dir} turn)", heading.degrees())),
                1 => ui.label(format!(
                    "Target yaw: {:.0}\u{b0} after one full {dir} circle)",
                    heading.degrees()
                )),
                crosses => ui.label(format!(
                    "Target yaw: {:.0}\u{b0} after {crosses} full {dir} circles)",
                    heading.degrees()
                )),
            };

            heading.degrees()
        }
    };

    let mut slider_degrees = target_degrees;
    let slider_resp = ui.add(egui::Slider::new(&mut slider_degrees, 0. ..=360.).suffix('\u{b0}'));
    if hotkeys.set_heading {
        slider_resp.request_focus();
    }
    if hotkeys.inc_heading {
        slider_degrees = (slider_degrees / 5.).floor() * 5. + 5.;
    }
    if hotkeys.dec_heading {
        slider_degrees = (slider_degrees / 5.).ceil() * 5. - 5.;
    }

    #[expect(clippy::float_cmp)] // this is normally equal if user did not interact
    if target_degrees != slider_degrees {
        commands.write_message(comm::InstructionMessage {
            object,
            body: comm::SetHeading {
                target: YawTarget::Heading(Heading::from_degrees(slider_degrees)),
            }
            .into(),
        });
    }
}

fn show_ground(
    ui: &mut egui::Ui,
    ground: &object::OnGround,
    segment_query: &Query<(&ground::Segment, &ground::SegmentLabel)>,
    endpoint_query: &Query<&ground::Endpoint>,
    waypoint_query: &Query<&Waypoint>,
) {
    let Some((segment, label)) = segment_query.log_get(ground.segment) else { return };

    let (from_endpoint, to_endpoint) = match ground.direction {
        ground::SegmentDirection::AlphaToBeta => (segment.alpha, segment.beta),
        ground::SegmentDirection::BetaToAlpha => (segment.beta, segment.alpha),
    };
    let [from_endpoint, to_endpoint] = try_log_return!(
        endpoint_query.get_many([from_endpoint, to_endpoint]),
        expect "ground::Segment must reference valid endpoints {from_endpoint:?}, {to_endpoint:?}"
    );

    ui.label(format!(
        "{}bound through {}",
        heading_to_approx_name((to_endpoint.position - from_endpoint.position).heading()),
        display_segment_label(label, waypoint_query),
    ));
}

fn show_target_alignment(
    this: &ObjectQueryItem,
    ui: &mut egui::Ui,
    waypoint_query: &Query<&Waypoint>,
    target: &nav::TargetAlignment,
    target_status: &nav::TargetAlignmentStatus,
) {
    let Some(start_waypoint) = waypoint_query.log_get(target.start_waypoint) else { return };
    let Some(end_waypoint) = waypoint_query.log_get(target.end_waypoint) else { return };

    let start_distance = this.object.position.horizontal_distance_exact(start_waypoint.position);
    let end_distance = this.object.position.horizontal_distance_exact(end_waypoint.position);
    ui.label(format!(
        "Target alignment: {} ({:.1} nm) -> {} ({:.1} nm)",
        &start_waypoint.name,
        start_distance.into_nm(),
        &end_waypoint.name,
        end_distance.into_nm(),
    ));

    ui.indent(new_type_id!(), |ui| match target_status.activation {
        nav::TargetAlignmentActivationStatus::Uninit => {}
        nav::TargetAlignmentActivationStatus::PurePursuit(_) => {
            ui.label(format!(
                "Angular deviation: {:+.1}\u{b0} ({:.0}m)",
                target_status.angular_deviation.into_degrees(),
                target_status.orthogonal_deviation.into_meters().abs(),
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

pub(super) fn display_segment_label(
    label: &ground::SegmentLabel,
    waypoint_query: &Query<&Waypoint>,
) -> String {
    match label {
        &ground::SegmentLabel::RunwayPair([forward, backward]) => {
            let Some(Waypoint { name: forward_name, .. }) = waypoint_query.log_get(forward) else {
                return String::new();
            };
            let Some(Waypoint { name: backward_name, .. }) = waypoint_query.log_get(backward)
            else {
                return String::new();
            };
            format!("runway {forward_name}/{backward_name}")
        }
        ground::SegmentLabel::Taxiway { name } => format!("taxiway {name}"),
        ground::SegmentLabel::Apron { name } => format!("apron {name}"),
    }
}
