use bevy::app::{self, App, Plugin};
use bevy::ecs::query::QueryData;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonInput;
use bevy::prelude::{
    in_state, Entity, EventReader, EventWriter, IntoSystemConfigs, KeyCode, NextState, Query, Res,
    ResMut, Resource, Single,
};
use itertools::Itertools;
use omniatc_core::level::route::{self, Route};
use omniatc_core::level::waypoint::Waypoint;
use omniatc_core::level::{aerodrome, nav, object};
use omniatc_core::units::{Angle, Distance, Speed, TurnDirection};

use crate::ui::{message, track, InputState, SystemSets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<SearchStack>();
        app.init_resource::<Selected>();

        app.add_systems(
            app::Update,
            start_search_system
                .in_set(InputState::Root)
                .in_set(message::SystemSets::FeedbackWriter),
        );
        app.add_systems(
            app::Update,
            incremental_search_system
                .in_set(InputState::ObjectSearch)
                .in_set(message::SystemSets::LogSender)
                .in_set(message::SystemSets::FeedbackWriter),
        );
        app.add_systems(
            app::Update,
            deselect_system
                .in_set(InputState::ObjectAction)
                .ambiguous_with(InputState::ObjectAction)
                .in_set(message::SystemSets::FeedbackWriter),
        );
        app.add_systems(
            app::Update,
            write_status_system
                .in_set(SystemSets::RenderMove)
                .in_set(message::SystemSets::StatusWriter)
                .run_if(in_state(InputState::ObjectAction)),
        );
    }
}

const STATUS_PREFIX: &str = "Search object: ";

fn start_search_system(
    inputs: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<NextState<InputState>>,
    mut search_stack: ResMut<SearchStack>,
    mut feedback: Single<&mut message::Feedback>,
) {
    if inputs.just_pressed(KeyCode::Slash) {
        input_state.set(InputState::ObjectSearch);
        search_stack.chars = Some(String::new());
        feedback.set(message::FeedbackType::ObjectSearch, STATUS_PREFIX);
    }
}

#[expect(clippy::too_many_arguments)]
fn incremental_search_system(
    mut inputs: EventReader<KeyboardInput>,
    mut input_state: ResMut<NextState<InputState>>,
    mut stack: ResMut<SearchStack>,
    object_query: Query<(Entity, &object::Display, &track::TrailOwnerRef)>,
    mut trail_owner_query: Query<&mut track::TrailDisplay>,
    mut messages: EventWriter<message::PushLog>,
    mut selected: ResMut<Selected>,
    mut feedback: Single<&mut message::Feedback>,
) {
    for input in inputs.read() {
        if !input.state.is_pressed() || input.repeat {
            continue;
        }

        let Some(chars) = &mut stack.chars else {
            // chars should be concurrently initialized when the input state is changed
            continue;
        };

        match input.logical_key {
            Key::Character(ref str) => {
                for ch in str.chars() {
                    match ch {
                        '0'..='9' | 'A'..='Z' => {
                            chars.push(ch);
                        }
                        'a'..='z' => {
                            chars.push(ch.to_ascii_uppercase());
                        }
                        '/' => chars.clear(),
                        _ => continue,
                    }
                }
            }
            Key::Backspace => _ = chars.pop(),
            Key::Escape => {
                input_state.set(InputState::Root);
                stack.chars = None;
                feedback.unset(message::FeedbackType::ObjectSearch);
                return; // do not process further keys since we have switched state
            }
            Key::Enter => {
                let all_matches: Vec<_> = object_query
                    .iter()
                    .filter(|(_, display, _)| is_subsequence(chars, &display.name))
                    .map(|(entity, _, trail)| (entity, trail))
                    .collect();
                let (match_, trail_ref) = match all_matches[..] {
                    [] => {
                        messages.send(message::PushLog {
                            message: format!("No objects matching \"{chars}\""),
                            ty:      message::LogType::Error,
                        });
                        return;
                    }
                    [result] => result,
                    _ => {
                        messages.send(message::PushLog {
                            message: format!(
                                "There are {len} objects matching \"{chars}\"",
                                len = all_matches.len()
                            ),
                            ty:      message::LogType::Error,
                        });
                        return;
                    }
                };

                stack.chars = None;
                feedback.unset(message::FeedbackType::ObjectSearch);
                selected.object_entity = Some(match_);
                input_state.set(InputState::ObjectAction);

                if let Ok(mut trail) = trail_owner_query.get_mut(trail_ref.0) {
                    trail.focused = true;
                } else {
                    bevy::log::error!("dangling trail owner reference {:?}", trail_ref.0);
                }

                return; // do not process further keys since we have switched state
            }
            _ => {}
        }
    }

    if let Some(ref chars) = stack.chars {
        let feedback_msg = feedback.get_mut(message::FeedbackType::ObjectSearch);
        STATUS_PREFIX.clone_into(feedback_msg);
        feedback_msg.push_str(chars);
    }
}

fn is_subsequence(sub: &str, full: &str) -> bool {
    let mut sub = sub.chars().peekable();
    for ch in full.chars() {
        let Some(&sub_next) = sub.peek() else {
            return true;
        };

        if sub_next.eq_ignore_ascii_case(&ch) {
            sub.next().unwrap();
        }
    }

    sub.next().is_none()
}

fn deselect_system(
    mut inputs: EventReader<KeyboardInput>,
    mut input_state: ResMut<NextState<InputState>>,
    mut selected: ResMut<Selected>,
    mut status: Single<&mut message::Status>,
    object_query: Query<&track::TrailOwnerRef>,
    mut trail_owner_query: Query<&mut track::TrailDisplay>,
) {
    for input in inputs.read() {
        if let Key::Escape = input.logical_key {
            if let Some(object) = selected.object_entity.take() {
                if let Ok(trail_ref) = object_query.get(object) {
                    if let Ok(mut trail) = trail_owner_query.get_mut(trail_ref.0) {
                        trail.focused = false;
                    } else {
                        bevy::log::warn!("dangling trail owner reference {:?}", trail_ref.0);
                    }
                } else {
                    bevy::log::warn!("Selected entity does not have a TrailOwnerRef");
                }
            } else {
                bevy::log::warn!("deselect system should not be called when Selected is empty");
            }
            input_state.set(InputState::Root);
            status.unset(message::StatusType::ObjectInfo);
        }
    }
}

#[derive(QueryData)]
struct ObjectStatusQuery {
    display:         &'static object::Display,
    dest:            &'static object::Destination,
    object:          &'static object::Object,
    airborne:        Option<&'static object::Airborne>,
    vel_target:      Option<&'static nav::VelocityTarget>,
    target_glide:    Option<(&'static nav::TargetGlide, &'static nav::TargetGlideStatus)>,
    target_altitude: Option<&'static nav::TargetAltitude>,
    route:           &'static Route,
}

#[derive(QueryData)]
struct AerodromeStatusQuery {
    display: &'static aerodrome::Aerodrome,
}

fn write_status_system(
    mut status: Single<&mut message::Status>,
    selected: Res<Selected>,
    object_query: Query<ObjectStatusQuery>,
    aerodrome_query: Query<AerodromeStatusQuery>,
    waypoint_query: Query<&Waypoint>,
) {
    let status = &mut **status;
    let Selected { object_entity: Some(entity) } = *selected else { return };
    let Ok(object) = object_query.get(entity) else {
        status.set(message::StatusType::ObjectInfo, "Invalid selection");
        return;
    };

    let out = status.get_mut(message::StatusType::ObjectInfo);
    out.clear();

    write_route_status(out, &object, &aerodrome_query, &waypoint_query);
    write_altitude_status(out, &object, &waypoint_query);
    write_speed_status(out, &object);
    write_direction_status(out, &object);
}

fn write_route_status(
    out: &mut String,
    object: &ObjectStatusQueryItem,
    aerodrome_query: &Query<AerodromeStatusQuery>,
    waypoint_query: &Query<&Waypoint>,
) {
    use std::fmt::Write;

    write!(out, "{}", &object.display.name).unwrap();
    match *object.dest {
        object::Destination::Landing { aerodrome } => {
            if let Ok(AerodromeStatusQueryItem {
                display: aerodrome::Aerodrome { name, .. }, ..
            }) = aerodrome_query.get(aerodrome)
            {
                write!(out, " to {name}").unwrap();
            }
        }
        object::Destination::VacateAnyRunway => out.push_str(" to any runway"),
        object::Destination::ReachWaypoint { min_altitude, waypoint_proximity } => {
            if let Some((waypoint, _)) = waypoint_proximity {
                if let Ok(Waypoint { name, .. }) = waypoint_query.get(waypoint) {
                    write!(out, " to {name}").unwrap();
                }
                if let Some(min_altitude) = min_altitude {
                    write!(out, " above {:.0} ft", min_altitude.amsl().into_feet()).unwrap();
                }
            } else if let Some(min_altitude) = min_altitude {
                write!(out, " towards {:.0} ft", min_altitude.amsl().into_feet()).unwrap();
            }
        }
    }

    let waypoints = object
        .route
        .iter()
        .filter_map(|node| match node {
            route::Node::DirectWaypoint(node) => Some(node.waypoint),
            route::Node::AlignRunway(node) => Some(node.runway),
            _ => None,
        })
        .map(|waypoint| match waypoint_query.get(waypoint) {
            Ok(Waypoint { name, .. }) => name,
            Err(err) => {
                bevy::log::error!("Nonexistent waypoint {waypoint:?} in route: {err}");
                "?"
            }
        })
        .join(" -> ");
    if !waypoints.is_empty() {
        write!(out, " through {waypoints}").unwrap();
    }

    writeln!(out).unwrap();
}

#[expect(clippy::too_many_lines)] // we got some long string literals here
fn write_altitude_status(
    out: &mut String,
    object: &ObjectStatusQueryItem,
    waypoint_query: &Query<&Waypoint>,
) {
    use std::fmt::Write;

    let position = object.object.position;

    if object.airborne.is_some() {
        // TODO use pressure altitudes where appropriate
        match (object.target_glide, object.target_altitude) {
            (
                Some((
                    &nav::TargetGlide { target_waypoint, glide_angle, .. },
                    &nav::TargetGlideStatus {
                        current_pitch,
                        altitude_deviation,
                        glidepath_distance,
                    },
                )),
                _,
            ) => {
                let target_waypoint = match waypoint_query.get(target_waypoint) {
                    Ok(waypoint) => waypoint.name.as_ref(),
                    Err(err) => {
                        bevy::log::warn!(
                            "target_waypoint {target_waypoint:?} of TargetGlide cannot be \
                             resolved: {err}",
                        );
                        ""
                    }
                };

                if altitude_deviation < Distance::from_feet(-30.) {
                    if current_pitch.is_zero() {
                        writeln!(
                            out,
                            "maintaining {:.0} feet to intercept the {:.1} degrees glidepath \
                             towards {target_waypoint} after {:.0} nautical miles",
                            position.altitude().amsl().into_feet(),
                            -glide_angle.into_degrees(),
                            glidepath_distance.into_nm(),
                        )
                        .unwrap();
                    } else {
                        writeln!(
                            out,
                            "passing {:.0} feet, {:.0} feet below the {:.1} degeres glidepath \
                             towards {target_waypoint}, {} at {:.1} degrees to intercept",
                            position.altitude().amsl().into_feet(),
                            -altitude_deviation.into_feet(),
                            -glide_angle.into_degrees(),
                            if current_pitch.is_positive() { "climbing" } else { "descending" },
                            current_pitch.abs().into_degrees(),
                        )
                        .unwrap();
                    }
                } else if altitude_deviation >= Distance::from_feet(30.) {
                    writeln!(
                        out,
                        "passing {:.0} feet, {:.0} feet above the {:.1} degrees glidepath towards \
                         {target_waypoint}, descending at {:.1} degrees",
                        position.altitude().amsl().into_feet(),
                        altitude_deviation.into_feet(),
                        -glide_angle.into_degrees(),
                        -current_pitch.into_degrees(),
                    )
                    .unwrap();
                } else {
                    writeln!(
                        out,
                        "passing {:.0} feet, on {:.1} degrees glidepath towards {target_waypoint}",
                        position.altitude().amsl().into_feet(),
                        -glide_angle.into_degrees(),
                    )
                    .unwrap();
                }
            }
            (None, Some(&nav::TargetAltitude { altitude: target, expedite })) => {
                if (target - position.altitude()).abs() < Distance::from_feet(100.) {
                    writeln!(out, "maintaining {:.0} feet", position.altitude().amsl().into_feet())
                        .unwrap();
                } else if target > position.altitude() {
                    writeln!(
                        out,
                        "{} from {:.0} feet to {:.0} feet",
                        if expedite { "expediting climb" } else { "climbing" },
                        position.altitude().amsl().into_feet(),
                        target.amsl().into_feet(),
                    )
                    .unwrap();
                } else {
                    writeln!(
                        out,
                        "{} from {:.0} feet to {:.0} feet",
                        if expedite { "expediting descent" } else { "descending" },
                        position.altitude().amsl().into_feet(),
                        target.amsl().into_feet(),
                    )
                    .unwrap();
                }
            }
            (None, None) => {
                writeln!(
                    out,
                    "passing {:.0} feet, uncontrolled",
                    (position.altitude().amsl()).into_feet(),
                )
                .unwrap();
            }
        }
    }
}

fn write_speed_status(out: &mut String, object: &ObjectStatusQueryItem) {
    use std::fmt::Write;

    let ground_speed = object.object.ground_speed;
    write!(
        out,
        "ground speed {:.1} knots towards {:.1} degrees, ",
        ground_speed.magnitude_exact().into_knots(),
        ground_speed.horizontal().heading().degrees(),
    )
    .unwrap();
    if ground_speed.vertical() >= Speed::from_fpm(50.) {
        writeln!(out, "climbing at {:.0} fpm", ground_speed.vertical().into_fpm()).unwrap();
    } else if ground_speed.vertical() <= Speed::from_fpm(-50.) {
        writeln!(out, "descending at {:.0} fpm", -ground_speed.vertical().into_fpm()).unwrap();
    } else {
        writeln!(out, "maintaining altitude").unwrap();
    }

    let Some(&object::Airborne { airspeed }) = object.airborne else { return };
    let airspeed = airspeed.horizontal().magnitude_exact().into_knots();

    match object.vel_target {
        None => writeln!(out, "speed {airspeed:.0} knots, uncontrolled").unwrap(),
        Some(&nav::VelocityTarget { horiz_speed: target_speed, .. }) => {
            let target_speed = target_speed.into_knots();
            if (target_speed - airspeed).abs() < 5. {
                writeln!(out, "maintaining speed {airspeed:.0} knots").unwrap();
            } else if target_speed > airspeed {
                writeln!(out, "increasing speed from {airspeed:.0} to {target_speed:.0} knots")
                    .unwrap();
            } else {
                writeln!(out, "reducing speed from {airspeed:.0} to {target_speed:.0} knots")
                    .unwrap();
            }
        }
    }
}

fn write_direction_status(out: &mut String, object: &ObjectStatusQueryItem) {
    use std::fmt::Write;

    let Some(&object::Airborne { airspeed }) = object.airborne else { return };
    let heading = airspeed.horizontal().heading();

    match object.vel_target {
        None => writeln!(out, "heading {:0>3.0}, uncontrolled", heading.degrees()).unwrap(),
        Some(nav::VelocityTarget { yaw, .. }) => match *yaw {
            nav::YawTarget::Speed(speed) if speed.is_zero() => {
                writeln!(out, "maintaining heading {:0>3.0}", heading.degrees()).unwrap();
            }
            nav::YawTarget::Speed(speed) if speed.is_positive() => {
                writeln!(out, "turning right {:.1} degrees per second", speed.0.to_degrees())
                    .unwrap();
            }
            nav::YawTarget::Speed(speed) => {
                writeln!(out, "turning left {:.1} degrees per second", -speed.0.to_degrees())
                    .unwrap();
            }
            nav::YawTarget::Heading(target_heading) => {
                let dir = heading.closer_direction_to(target_heading);
                let dist = heading.distance(target_heading, dir);
                if dist.abs() < Angle::from_degrees(2.5) {
                    writeln!(out, "maintaining heading {:0>3.0} degrees", target_heading.degrees())
                        .unwrap();
                } else {
                    writeln!(
                        out,
                        "turning {} to heading {:0>3.0} degrees",
                        match dir {
                            TurnDirection::CounterClockwise => "left",
                            TurnDirection::Clockwise => "right",
                        },
                        target_heading.degrees(),
                    )
                    .unwrap();
                }
            }
            nav::YawTarget::TurnHeading {
                heading: target_heading,
                remaining_crosses,
                direction,
            } => {
                let dir_str = match direction {
                    TurnDirection::CounterClockwise => "left",
                    TurnDirection::Clockwise => "right",
                };

                match remaining_crosses {
                    2.. => {
                        write!(
                            out,
                            "turning {dir_str} for {remaining_crosses} full circles and stopping \
                             at"
                        )
                        .unwrap();
                    }
                    1 => write!(out, "turning {dir_str} for one full circle and stopping at")
                        .unwrap(),
                    0 => write!(out, "turning {dir_str} to").unwrap(),
                }

                writeln!(out, " heading {:0>3.0} degrees", target_heading.degrees()).unwrap();
            }
        },
    }
}

#[derive(Resource, Default)]
pub(super) struct SearchStack {
    pub(super) chars: Option<String>,
}

#[derive(Resource, Default)]
pub(super) struct Selected {
    pub(super) object_entity: Option<Entity>,
}
