use std::time::Duration;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::World;

use super::{altitude, heading, DoResync, Schedule, ScheduleEntry};
use crate::level::route::{trigger, CompletionCondition, Route};
use crate::level::waypoint::{self, Waypoint};
use crate::level::{nav, object, Config};
use crate::units::{
    Accel, AccelRate, AngularAccel, AngularSpeed, Distance, Heading, Position, Speed,
};

#[derive(Clone, Copy)]
struct Context {
    point1: Entity,
    point2: Entity,
    point3: Entity,
    object: Entity,
}

fn do_test(
    populate_schedule: impl FnOnce(&mut Schedule, Context),
    assertions: impl FnOnce(&World, Context),
) {
    let mut app = App::new();
    app.add_plugins((bevy::MinimalPlugins, object::Plug, nav::Plug, super::Plug));

    let point1 = app
        .world_mut()
        .spawn(Waypoint {
            name:         String::new(),
            display_type: waypoint::DisplayType::None,
            position:     Position::from_origin_nm(0., 5.).with_altitude(Position::SEA_LEVEL),
        })
        .id();
    let point2 = app
        .world_mut()
        .spawn(Waypoint {
            name:         String::new(),
            display_type: waypoint::DisplayType::None,
            position:     Position::from_origin_nm(5., 5.).with_altitude(Position::SEA_LEVEL),
        })
        .id();
    let point3 = app
        .world_mut()
        .spawn(Waypoint {
            name:         String::new(),
            display_type: waypoint::DisplayType::None,
            position:     Position::from_origin_nm(5., 10.).with_altitude(Position::SEA_LEVEL),
        })
        .id();

    app.insert_resource(Config { max_track_log: 0, track_density: Duration::MAX });

    let ctx = {
        let mut object = app.world_mut().spawn((
            object::Object {
                position:     Position::ORIGIN.with_altitude(Position::from_amsl_feet(10000.)),
                ground_speed: Speed::from_knots(300.).with_heading(Heading::NORTH).horizontally(),
            },
            object::Airborne {
                airspeed: Speed::from_knots(260.).with_heading(Heading::NORTH).horizontally(),
            },
            nav::VelocityTarget {
                yaw:         nav::YawTarget::Heading(Heading::NORTH),
                vert_rate:   Speed::ZERO,
                horiz_speed: Speed::from_knots(260.),
                expedite:    false,
            },
            nav::Limits {
                level:             nav::ClimbProfile {
                    vert_rate: Speed::from_fpm(0.),
                    accel:     Accel::from_knots_per_sec(1.),
                    decel:     Accel::from_knots_per_sec(-1.),
                },
                std_climb:         nav::ClimbProfile {
                    vert_rate: Speed::from_fpm(1.),
                    accel:     Accel::from_knots_per_sec(1.),
                    decel:     Accel::from_knots_per_sec(-1.),
                },
                std_descent:       nav::ClimbProfile {
                    vert_rate: Speed::from_fpm(-1.),
                    accel:     Accel::from_knots_per_sec(1.),
                    decel:     Accel::from_knots_per_sec(-1.),
                },
                exp_climb:         nav::ClimbProfile {
                    vert_rate: Speed::from_fpm(2.),
                    accel:     Accel::from_knots_per_sec(1.),
                    decel:     Accel::from_knots_per_sec(-1.),
                },
                exp_descent:       nav::ClimbProfile {
                    vert_rate: Speed::from_fpm(-2.),
                    accel:     Accel::from_knots_per_sec(1.),
                    decel:     Accel::from_knots_per_sec(-1.),
                },
                drag_coef:         1e-5,
                min_horiz_speed:   Speed::from_knots(100.),
                max_vert_accel:    Accel::from_fpm_per_sec(100.),
                max_yaw_accel:     AngularAccel::from_degrees_per_sec2(5.),
                max_yaw_speed:     AngularSpeed::from_degrees_per_sec(5.),
                accel_change_rate: AccelRate::from_knots_per_sec2(1.),
            },
        ));
        let ctx = Context { point1, point2, point3, object: object.id() };

        let mut schedule = Schedule::default();
        populate_schedule(&mut schedule, ctx);
        object.insert(Route { schedule });

        ctx
    };

    app.update();

    DoResync.apply(app.world_mut().entity_mut(ctx.object));

    assertions(app.world(), ctx);
}

#[test]
fn plan_multi_path_descent() {
    do_test(
        |schedule, ctx| {
            schedule.channels.heading.push_custom(heading::DirectWaypoint {
                waypoint:             ctx.point1,
                completion_condition: CompletionCondition::Tolerance(Distance::from_nm(3.)),
            });
            schedule.channels.heading.push_custom(heading::DirectWaypoint {
                waypoint:             ctx.point1,
                completion_condition: CompletionCondition::Tolerance(Distance::from_nm(3.)),
            });
            schedule.channels.heading.push_custom(heading::DirectWaypoint {
                waypoint:             ctx.point1,
                completion_condition: CompletionCondition::Tolerance(Distance::from_nm(3.)),
            });

            let condition = schedule.alloc_condition();
            schedule.channels.heading.push_notify(condition);
            schedule.channels.altitude.push_custom(altitude::ApproachBy {
                altitude:             Position::from_amsl_feet(6000.),
                deadline:             condition,
                completion_condition: CompletionCondition::Tolerance(Distance::from_feet(100.)),
            });
        },
        |world, ctx| {
            let object = world.entity(ctx.object);
            {
                let comp = object.get::<trigger::NearWaypoint>().unwrap();
                assert_eq!(comp.target_waypoint, ctx.point1);
                assert_eq!(comp.tolerance, Distance::from_nm(3.));
            }
            {
                let comp = object.get::<nav::TargetWaypoint>().unwrap();
                assert_eq!(comp.waypoint_entity, ctx.point1);
            }
        },
    );
}
