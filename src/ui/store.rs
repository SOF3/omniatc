use std::f32::consts::PI;

use bevy::app::{self, App, Plugin};
use bevy::math::{Vec3, Vec3A};
use bevy::prelude::Commands;

use crate::level::waypoint::Waypoint;
use crate::level::{aerodrome, object, plane, waypoint};
use crate::math::Heading;

pub struct Plug;

pub const DEFAULT_LIMITS: plane::Limits = plane::Limits {
    max_vert_accel:    1.,
    exp_climb:         plane::ClimbProfile { vert_rate: 20., accel: 1., decel: -5. },
    std_climb:         plane::ClimbProfile { vert_rate: 10., accel: 2., decel: -4. },
    level:             plane::ClimbProfile { vert_rate: 0., accel: 3., decel: -3. },
    exp_descent:       plane::ClimbProfile { vert_rate: -10., accel: 4., decel: -2. },
    std_descent:       plane::ClimbProfile { vert_rate: -20., accel: 5., decel: -1. },
    drag_coef:         3. / 500. / 500.,
    accel_change_rate: 0.3,
    max_yaw_accel:     PI / 600.,
    max_yaw_speed:     PI / 60.,
};

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        // during early stage of development, just spawn dummy objects for testing
        app.add_systems(app::Startup, |mut commands: Commands| {
            let mut plane = commands.spawn_empty();
            plane.queue(object::SpawnCommand {
                position:     object::Position(Vec3A::new(0.0, 10., 5.)),
                ground_speed: object::GroundSpeed(Vec3A::new(0.0, 130., 0.)),
                display:      object::Display {
                    name:        String::from("ABC123"),
                    destination: object::Destination::Arrival(aerodrome::Id(0)),
                },
            });
            plane.queue(object::SetAirborneCommand);
            plane.queue(plane::SpawnCommand {
                control: Some(plane::Control::stabilized(Heading::SOUTH)),
                limits:  DEFAULT_LIMITS,
            });
        });
        app.add_systems(app::Startup, |mut commands: Commands| {
            let mut plane = commands.spawn_empty();
            plane.queue(object::SpawnCommand {
                position:     object::Position(Vec3A::new(10., 0., 3.)),
                ground_speed: object::GroundSpeed(Vec3A::new(200.0, 0., 0.)),
                display:      object::Display {
                    name:        String::from("ADE127"),
                    destination: object::Destination::Departure(aerodrome::Id(0)),
                },
            });
            plane.queue(object::SetAirborneCommand);
            plane.queue(plane::SpawnCommand {
                control: Some(plane::Control::stabilized(Heading::EAST)),
                limits:  DEFAULT_LIMITS,
            });
        });

        app.add_systems(app::Startup, |mut commands: Commands| {
            let mut waypoint = commands.spawn_empty();
            waypoint.queue(waypoint::SpawnCommand {
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
        });
    }
}
