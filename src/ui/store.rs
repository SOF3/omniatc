use std::f32::consts::PI;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::math::bounding::Aabb3d;
use bevy::math::{Vec2, Vec3, Vec3A};
use bevy::prelude::Commands;

use crate::level::waypoint::Waypoint;
use crate::level::{aerodrome, nav, object, plane, waypoint, wind};
use crate::math::Heading;

pub struct Plug;

pub const DEFAULT_PLANE_LIMITS: plane::Limits = plane::Limits {
    max_vert_accel:    1.,
    exp_climb:         plane::ClimbProfile { vert_rate: 30., accel: 0.2, decel: -1.8 },
    std_climb:         plane::ClimbProfile { vert_rate: 15., accel: 0.6, decel: -1.4 },
    level:             plane::ClimbProfile { vert_rate: 0., accel: 1., decel: -1. },
    exp_descent:       plane::ClimbProfile { vert_rate: -15., accel: 1.4, decel: -0.6 },
    std_descent:       plane::ClimbProfile { vert_rate: -30., accel: 1.8, decel: -0.2 },
    drag_coef:         3. / 500. / 500.,
    accel_change_rate: 0.3,
    max_yaw_accel:     PI / 600.,
};

pub const DEFAULT_NAV_LIMITS: nav::Limits =
    nav::Limits { min_horiz_speed: 120., max_yaw_speed: PI / 60. };

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        // during early stage of development, just spawn dummy objects for testing
        app.add_systems(app::Startup, |mut commands: Commands| {
            let main_airport = commands
                .spawn((
                    bevy::core::Name::new(String::from("Aerodrome ALFA")),
                    aerodrome::Display { id: 0, name: "MAIN".into() },
                ))
                .id();

            let origin = {
                let mut entity =
                    commands.spawn(bevy::core::Name::new(String::from("Waypoint: ORIGIN")));
                entity.queue(waypoint::SpawnCommand {
                    waypoint: Waypoint {
                        name:         "ORIGIN".into(),
                        display_type: waypoint::DisplayType::Vor,
                        position:     Vec3::ZERO,
                        navaid_range: vec![waypoint::NavaidRange {
                            heading_range: Heading::NORTH..Heading::NORTH,
                            min_pitch:     0.,
                            max_range:     50.,
                        }],
                    },
                });
                entity.id()
            };

            let join = {
                let mut entity =
                    commands.spawn(bevy::core::Name::new(String::from("Waypoint: JOIN")));
                entity.queue(waypoint::SpawnCommand {
                    waypoint: Waypoint {
                        name:         "JOIN".into(),
                        display_type: waypoint::DisplayType::Waypoint,
                        position:     Vec3::new(0., 12., 0.),
                        navaid_range: vec![],
                    },
                });
                entity.id()
            };

            let mut wind = commands.spawn(bevy::core::Name::new(String::from("Wind")));
            wind.queue(wind::SpawnCommand {
                bundle: wind::Comps {
                    vector:        wind::Vector {
                        bottom: Vec2::new(5.0, 5.0),
                        top:    Vec2::new(5.0, 5.0),
                    },
                    effect_region: wind::EffectRegion(Aabb3d::new(
                        Vec3A::ZERO,
                        Vec3A::new(128., 128., 5.),
                    )),
                },
            });

            {
                let mut plane =
                    commands.spawn(bevy::core::Name::new(String::from("Plane: ABC123")));
                plane.queue(object::SpawnCommand {
                    position:     object::Position(Vec3A::new(1.0, 15., 0.6)),
                    ground_speed: object::GroundSpeed(Vec3A::new(-40., 130., 0.)),
                    display:      object::Display { name: String::from("ABC123") },
                    destination:  object::Destination::Arrival { aerodrome: main_airport },
                });
                plane.queue(object::SetAirborneCommand);
                plane.queue(plane::SpawnCommand {
                    control: Some(plane::Control::stabilized(Heading::from_degrees(210.))),
                    limits:  DEFAULT_PLANE_LIMITS,
                });
                plane.insert(DEFAULT_NAV_LIMITS);

                plane.insert(nav::TargetAlignment {
                    activation_range: 0.2,
                    lookahead:        Duration::from_secs(20),
                    start_waypoint:   join,
                    end_waypoint:     origin,
                });
            }

            {
                let mut plane =
                    commands.spawn(bevy::core::Name::new(String::from("Plane: ADE127")));
                plane.queue(object::SpawnCommand {
                    position:     object::Position(Vec3A::new(10., 0., 3.)),
                    ground_speed: object::GroundSpeed(Vec3A::new(200.0, 0., 0.)),
                    display:      object::Display { name: String::from("ADE127") },
                    destination:  object::Destination::Departure { aerodrome: main_airport },
                });
                plane.queue(object::SetAirborneCommand);
                plane.queue(plane::SpawnCommand {
                    control: Some(plane::Control::stabilized(Heading::EAST)),
                    limits:  DEFAULT_PLANE_LIMITS,
                });
                plane.insert((
                    nav::VelocityTarget {
                        yaw:         nav::YawTarget::Speed(DEFAULT_NAV_LIMITS.max_yaw_speed),
                        horiz_speed: 200.,
                        vert_rate:   0.,
                        expedit:     false,
                    },
                    DEFAULT_NAV_LIMITS,
                    nav::TargetWaypoint { waypoint_entity: origin },
                ));
            }
        });
    }
}
