use std::f32::consts::{FRAC_PI_2, FRAC_PI_6, PI};
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::math::bounding::Aabb3d;
use bevy::math::{Vec2, Vec3, Vec3A};
use bevy::prelude::{BuildChildren, ChildBuild, Commands};
use omniatc_core::level::runway::Runway;
use omniatc_core::level::waypoint::Waypoint;
use omniatc_core::level::{aerodrome, nav, object, plane, runway, waypoint, wind};
use omniatc_core::units::{
    Accel, AccelRate, Angle, AngularAccel, AngularSpeed, Distance, Heading, Position, Speed,
};

pub struct Plug;

pub fn default_plane_limits() -> plane::Limits {
    plane::Limits {
        max_vert_accel:    Accel::from_knots_per_sec(1.),
        exp_climb:         plane::ClimbProfile {
            vert_rate: Speed::from_knots(30.),
            accel:     Accel::from_knots_per_sec(0.2),
            decel:     Accel::from_knots_per_sec(-1.8),
        },
        std_climb:         plane::ClimbProfile {
            vert_rate: Speed::from_knots(15.),
            accel:     Accel::from_knots_per_sec(0.6),
            decel:     Accel::from_knots_per_sec(-1.4),
        },
        level:             plane::ClimbProfile {
            vert_rate: Speed::from_knots(0.),
            accel:     Accel::from_knots_per_sec(1.),
            decel:     Accel::from_knots_per_sec(-1.),
        },
        exp_descent:       plane::ClimbProfile {
            vert_rate: Speed::from_knots(-15.),
            accel:     Accel::from_knots_per_sec(1.4),
            decel:     Accel::from_knots_per_sec(-0.6),
        },
        std_descent:       plane::ClimbProfile {
            vert_rate: Speed::from_knots(-30.),
            accel:     Accel::from_knots_per_sec(1.8),
            decel:     Accel::from_knots_per_sec(-0.2),
        },
        drag_coef:         3. / 500. / 500.,
        accel_change_rate: AccelRate(0.3),
        max_yaw_accel:     AngularAccel(PI / 600.),
    }
}

pub fn default_nav_limits() -> nav::Limits {
    nav::Limits {
        min_horiz_speed: Speed::from_knots(120.),
        max_yaw_speed:   AngularSpeed(PI / 60.),
    }
}

impl Plugin for Plug {
    #[allow(clippy::too_many_lines)] // we will rewrite this later
    fn build(&self, app: &mut App) {
        // during early stage of development, just spawn dummy objects for testing
        app.add_systems(app::Startup, |mut commands: Commands| {
            let main_airport = commands
                .spawn((
                    bevy::core::Name::new(String::from("Aerodrome ALFA")),
                    aerodrome::Display { id: 0, name: "MAIN".into() },
                ))
                .id();

            let runway = {
                let mut entity = commands.spawn(bevy::core::Name::new(String::from("Runway 18")));
                entity.queue(runway::SpawnCommand {
                    waypoint: Waypoint {
                        name:         "18".into(),
                        display_type: waypoint::DisplayType::Runway,
                        position:     Position::new(Vec3::ZERO),
                    },
                    runway:   Runway {
                        usable_length: Distance(2.) * Heading::SOUTH.into_dir2(),
                        glide_angle:   Angle(FRAC_PI_6 / 10.),
                        display_width: Distance(0.04),
                        display_start: Position::new(Vec3::ZERO),
                        display_end:   Position::new(Vec3::NEG_Y * 2.),
                    },
                });
                entity.with_children(|b| {
                    b.spawn((
                        waypoint::Navaid {
                            heading_range:       Heading::NORTH..Heading::NORTH,
                            min_pitch:           Angle(0.),
                            max_pitch:           Angle(FRAC_PI_2),
                            min_dist_horizontal: Distance(0.2),
                            min_dist_vertical:   Distance(0.03),
                            max_dist_horizontal: Distance(10.),
                            max_dist_vertical:   Distance(1.),
                        },
                        waypoint::HasCriticalRegion {},
                    ));
                    b.spawn((
                        waypoint::Navaid {
                            heading_range:       Heading::NORTH..Heading::NORTH,
                            min_pitch:           Angle(0.),
                            max_pitch:           Angle(FRAC_PI_2),
                            min_dist_horizontal: Distance(0.),
                            min_dist_vertical:   Distance(0.0),
                            max_dist_horizontal: Distance(10.),
                            max_dist_vertical:   Distance(1.),
                        },
                        waypoint::Visual,
                    ));
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
                        position:     Position::new(Vec3::new(0., 12., 0.)),
                    },
                });
                entity.id()
            };

            let mut wind = commands.spawn(bevy::core::Name::new(String::from("Wind")));
            wind.queue(wind::SpawnCommand {
                bundle: wind::Comps {
                    vector:        wind::Vector {
                        bottom: Speed::from_knots(Vec2::new(5.0, 5.0)),
                        top:    Speed::from_knots(Vec2::new(5.0, 5.0)),
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
                    position:     Position::new(Vec3::new(1.0, 15., 0.6)),
                    ground_speed: Speed::from_knots(Vec3::new(-40., 130., 0.)),
                    display:      object::Display { name: String::from("ABC123") },
                    destination:  object::Destination::Arrival { aerodrome: main_airport },
                });
                plane.queue(object::SetAirborneCommand);
                plane.queue(plane::SpawnCommand {
                    control: Some(plane::Control::stabilized(Heading::from_degrees(210.))),
                    limits:  default_plane_limits(),
                });
                plane.insert(default_nav_limits());

                plane.insert(nav::TargetAlignment {
                    activation_range: Distance(0.2),
                    lookahead:        Duration::from_secs(20),
                    start_waypoint:   join,
                    end_waypoint:     runway,
                });
            }

            {
                let mut plane =
                    commands.spawn(bevy::core::Name::new(String::from("Plane: ADE127")));
                plane.queue(object::SpawnCommand {
                    position:     Position::new(Vec3::new(10., 0., 3.)),
                    ground_speed: Speed::from_knots(Vec3::new(200.0, 0., 0.)),
                    display:      object::Display { name: String::from("ADE127") },
                    destination:  object::Destination::Departure { aerodrome: main_airport },
                });
                plane.queue(object::SetAirborneCommand);
                plane.queue(plane::SpawnCommand {
                    control: Some(plane::Control::stabilized(Heading::EAST)),
                    limits:  default_plane_limits(),
                });
                plane.insert((
                    nav::VelocityTarget {
                        yaw:         nav::YawTarget::Speed(default_nav_limits().max_yaw_speed),
                        horiz_speed: Speed::from_knots(200.),
                        vert_rate:   Speed::from_knots(0.),
                        expedite:    false,
                    },
                    default_nav_limits(),
                    nav::TargetWaypoint { waypoint_entity: runway },
                ));
            }
        });
    }
}
