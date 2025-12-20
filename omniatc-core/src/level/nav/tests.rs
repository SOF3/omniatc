use std::time::Duration;

use bevy::app::App;
use bevy::ecs::entity::Entity;
use bevy::math::bounding::Aabb3d;
use bevy::math::{Quat, Vec3A};
use bevy::time::{self, Time};
use math::{
    Accel, AccelRate, AngularAccel, AngularSpeed, Heading, ISA_TROPOPAUSE_PRESSURE,
    ISA_TROPOPAUSE_TEMPERATURE, Length, Position, Speed,
};
use store::{NavLimits, YawTarget};

use crate::level::object::{self, Object};
use crate::level::{nav, plane, wind};

const NAV_LIMITS: NavLimits = NavLimits {
    min_horiz_speed:   Speed::from_knots(120.),
    max_yaw_speed:     AngularSpeed::from_degrees_per_sec(3.),
    max_vert_accel:    Accel::from_fpm_per_sec(200.),
    exp_climb:         store::ClimbProfile {
        vert_rate: Speed::from_fpm(3000.),
        accel:     Accel::from_knots_per_sec(0.2),
        decel:     Accel::from_knots_per_sec(-1.8),
    },
    std_climb:         store::ClimbProfile {
        vert_rate: Speed::from_fpm(1500.),
        accel:     Accel::from_knots_per_sec(0.6),
        decel:     Accel::from_knots_per_sec(-1.4),
    },
    level:             store::ClimbProfile {
        vert_rate: Speed::from_fpm(0.),
        accel:     Accel::from_knots_per_sec(1.),
        decel:     Accel::from_knots_per_sec(-1.),
    },
    std_descent:       store::ClimbProfile {
        vert_rate: Speed::from_fpm(-1500.),
        accel:     Accel::from_knots_per_sec(1.4),
        decel:     Accel::from_knots_per_sec(-0.6),
    },
    exp_descent:       store::ClimbProfile {
        vert_rate: Speed::from_fpm(-3000.),
        accel:     Accel::from_knots_per_sec(1.8),
        decel:     Accel::from_knots_per_sec(-0.2),
    },
    weight:            1e5,
    accel_change_rate: AccelRate::from_knots_per_sec2(0.3),
    drag_coef:         3. / 500. / 500.,
    max_yaw_accel:     AngularAccel::from_degrees_per_sec2(1.),
    takeoff_speed:     Speed::from_knots(150.),
    short_final_dist:  Length::from_nm(4.),
    short_final_speed: Speed::from_knots(150.),
};

/// Start at (0, 0) @ 3000ft, heading north at 200 kias, crosswind 10kt from west
fn base_world() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        object::Plug::<()>::default(),
        wind::Plug::<()>::default(),
        plane::Plug,
        super::Plug,
    ));

    app.init_resource::<Time<time::Virtual>>();

    app.world_mut().commands().spawn_empty().queue(wind::SpawnCommand {
        bundle: wind::Comps {
            vector:        wind::Vector {
                bottom: Speed::from_knots(10.0) * Heading::WEST,
                top:    Speed::from_knots(10.0) * Heading::WEST,
            },
            effect_region: wind::EffectRegion(Aabb3d {
                min: Vec3A::splat(-1000.0),
                max: Vec3A::splat(1000.0),
            }),
        },
    });
    let object = app
        .world_mut()
        .commands()
        .spawn((
            Object {
                position:     Position::ORIGIN.with_altitude(Position::from_amsl_feet(3000.0)),
                ground_speed: (Speed::from_knots(200.0) * Heading::NORTH).horizontally(),
            },
            object::Airborne {
                pressure_alt:  Position::from_amsl_feet(3000.0),
                pressure:      ISA_TROPOPAUSE_PRESSURE,
                oat:           ISA_TROPOPAUSE_TEMPERATURE,
                airspeed:      (Speed::from_knots(200.0) * Heading::NORTH).horizontally(),
                true_airspeed: (Speed::from_knots(200.0) * Heading::NORTH).horizontally(),
            },
            object::Rotation(Quat::IDENTITY),
        ))
        .queue(plane::SpawnCommand { control: None, limits: nav::Limits(NAV_LIMITS) })
        .insert((nav::VelocityTarget {
            yaw:         YawTarget::Heading(Heading::NORTH),
            horiz_speed: Speed::from_knots(200.0),
            vert_rate:   Speed::from_fpm(0.0),
            expedite:    false,
        },))
        .id();
    app.world_mut().flush();
    app.update();
    (app, object)
}

fn advance_world(app: &mut App, dt: Duration) {
    app.world_mut().resource_mut::<Time<time::Virtual>>().advance_by(dt);
    app.update();
}

#[test]
fn test_baseline() {
    let (mut app, object_id) = base_world();

    for _ in 0..10 {
        advance_world(&mut app, Duration::from_secs(1));
    }

    let object = app.world().get::<Object>(object_id).unwrap();
    object
        .position
        .altitude()
        .assert_near(Position::from_amsl_feet(3000.0), Length::from_feet(100.0))
        .expect("maintain original altitude");
    object
        .ground_speed
        .vertical()
        .assert_near(Speed::ZERO, Speed::from_knots(1.0))
        .expect("maintain horizontal motion");

    let airborne = app.world().get::<object::Airborne>(object_id).unwrap();
    airborne
        .airspeed
        .horizontal()
        .assert_near(Speed::from_knots_vec2(0.0, 200.0), Speed::from_knots(1.0))
        .expect("maintain airspeed");
    airborne
        .true_airspeed
        .horizontal()
        .assert_near(Speed::from_knots_vec2(0.0, 208.75), Speed::from_knots(1.0))
        .expect("true airspeed calculation");
}
