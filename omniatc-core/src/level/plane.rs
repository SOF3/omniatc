//! A flying object with forward thrust and takes off/lands on a runway.
//! All plane entities are object entities.

use bevy::app::{self, App, Plugin};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::Quat;
use bevy::prelude::{Component, Entity, EntityCommand, Event, IntoScheduleConfigs, Query, Res};
use bevy::time::{self, Time};

use super::nav::{self, VelocityTarget, YawTarget};
use super::object::Object;
use super::{navaid, object, SystemSets};
use crate::units::{Accel, Angle, AngularAccel, AngularSpeed, Heading, Speed, TurnDirection};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnEvent>();
        app.add_systems(app::Update, apply_forces_system.in_set(SystemSets::Aviate));
        app.add_systems(app::Update, rotate_object_system.in_set(SystemSets::ReconcileForRead));
    }
}

/// Mutable states modified by control systems.
#[derive(Component, serde::Serialize, serde::Deserialize)]
pub struct Control {
    /// Heading of the plane, must be a unit vector.
    /// This is the horizontal direction of the thrust generated.
    pub heading:     Heading,
    /// Rate of yaw change. Considered to be directly proportional to roll.
    pub yaw_speed:   AngularSpeed<f32>,
    /// Current horizontal acceleration.
    pub horiz_accel: Accel<f32>,
}

impl Control {
    /// Stabilize at current velocity.
    #[must_use]
    pub fn stabilized(heading: Heading) -> Self {
        Control { heading, yaw_speed: AngularSpeed::ZERO, horiz_accel: Accel::ZERO }
    }
}

/// Structural limitations of a plane.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize)]
pub struct Limits {}

pub struct SpawnCommand {
    pub control: Option<Control>,
    pub limits:  nav::Limits,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        if let Some(airborne) = entity.get::<object::Airborne>() {
            let horiz_speed = airborne.airspeed.magnitude_exact();

            let dt_target = VelocityTarget {
                yaw: YawTarget::Heading(airborne.airspeed.horizontal().heading()),
                horiz_speed,
                vert_rate: Speed::ZERO,
                expedite: false,
            };

            entity.insert(dt_target);
        }

        let control = if let Some(control) = self.control {
            control
        } else {
            let heading = Heading::from_quat(
                entity
                    .get::<object::Rotation>()
                    .expect("cannot spawn entity as plane without adding rotation component first")
                    .0,
            );
            Control::stabilized(heading)
        };

        entity.insert((control, self.limits));

        let entity_id = entity.id();
        entity.world_scope(|world| world.send_event(SpawnEvent(entity_id)));
    }
}

/// Sent when a plane entity is spawned.
#[derive(Event)]
pub struct SpawnEvent(pub Entity);

fn apply_forces_system(
    time: Res<Time<time::Virtual>>,
    mut plane_query: Query<(
        &mut VelocityTarget,
        &mut Control,
        &nav::Limits,
        &mut object::Airborne,
    )>,
) {
    if time.is_paused() {
        return;
    }

    plane_query.par_iter_mut().for_each(|(mut target, mut control, limits, mut airborne)| {
        // All components are always changed. Deref first to avoid borrowck issues.
        maintain_yaw(&time, &mut target, &mut control, limits, &airborne);
        maintain_accel(&time, &target, &mut control, limits, &mut airborne);
        maintain_vert(&time, &target, limits, &mut airborne);
    });
}

fn maintain_yaw(
    time: &Time<time::Virtual>,
    target: &mut VelocityTarget,
    control: &mut Control,
    limits: &nav::Limits,
    airborne: &object::Airborne,
) {
    let current_yaw = airborne.airspeed.horizontal().heading();
    let mut detect_crossing = None;
    let mut set_yaw_target = None;

    let desired_yaw_speed = match target.yaw {
        YawTarget::Heading(target_heading) => {
            let dir = current_yaw.closer_direction_to(target_heading);

            // Test if the target heading is overshot when yaw speed reduces to 0
            // if we start reducing yaw speed now.
            // By v^2 = u^2 + 2as and v=0, s = -u^2/2a.
            let brake_angle = Angle(control.yaw_speed.0.powi(2) / limits.max_yaw_accel.0)
                * control.yaw_speed.signum();
            let braked_yaw = current_yaw + brake_angle;

            if target_heading.is_between(current_yaw, braked_yaw) {
                // we are going to overshoot the target heading, start reducing speed now.
                AngularSpeed(0.)
            } else {
                match dir {
                    TurnDirection::CounterClockwise => -limits.max_yaw_speed,
                    TurnDirection::Clockwise => limits.max_yaw_speed,
                }
            }
        }
        YawTarget::TurnHeading {
            heading: target_heading,
            ref mut remaining_crosses,
            direction,
        } => {
            let distance = current_yaw.distance(target_heading, direction);
            if *remaining_crosses == 0 {
                if distance < Angle::RIGHT {
                    set_yaw_target = Some(target_heading);
                }
            } else {
                detect_crossing = Some((target_heading, remaining_crosses));
            }

            match direction {
                TurnDirection::CounterClockwise => -limits.max_yaw_speed,
                TurnDirection::Clockwise => limits.max_yaw_speed,
            }
        }
    };

    let delta = desired_yaw_speed - control.yaw_speed;
    control.yaw_speed += AngularAccel::per_second(delta)
        .clamp(-limits.max_yaw_accel, limits.max_yaw_accel)
        * time.delta();

    {
        let new_heading = control.heading + control.yaw_speed * time.delta();
        if let Some((boundary, counter)) = detect_crossing {
            if boundary.is_between(control.heading, new_heading) {
                *counter -= 1;
            }
        }
        control.heading = new_heading;
    }

    if let Some(target_yaw) = set_yaw_target {
        target.yaw = YawTarget::Heading(target_yaw);
    }
}

fn maintain_accel(
    time: &Time<time::Virtual>,
    target: &VelocityTarget,
    control: &mut Control,
    limits: &nav::Limits,
    airborne: &mut object::Airborne,
) {
    enum ThrottleAction {
        Increase,
        Decrease,
    }

    let current_speed = airborne.airspeed.horizontal().magnitude_exact();

    let max_accel = limits.accel(airborne.airspeed.vertical())
        - Accel(limits.drag_coef * current_speed.0.powi(2));
    let max_decel = limits.decel(airborne.airspeed.vertical())
        - Accel(limits.drag_coef * current_speed.0.powi(2));

    #[expect(clippy::collapsible_else_if)]
    let desired_action = if target.horiz_speed >= current_speed {
        if control.horiz_accel.is_negative() {
            // We are slower than we want to be and we are even further decelerating,
            // so increasing throttle is the only correct action.
            ThrottleAction::Increase
        } else {
            // Consider:
            // accel(t) = accel(0) + accel_change_rate * t
            // speed(t) = speed(0) + int[0..t] a(x) dx
            //          = speed(0) + accel(0) * t + 0.5 accel_change_rate * t^2
            // If we perform maximum throttle pull back now, when the acceleration decreases to 0,
            // accel(t_stop) = 0 => t_stop = accel(0) / accel_change_rate
            // => speed(t_stop) = speed(0) - 0.5 * accel(0) / accel_change_rate
            let speed_stop = current_speed
                - Speed(0.5 * control.horiz_accel.0.powi(2) / (-limits.accel_change_rate.0));

            // As we continue to accelerate, speed(0) increases over time,
            // so speed(t_stop) also increases over time.
            // We want speed(t_stop) to approach target.horiz_speed,
            // so start pulling back when speed(t_stop) >= target.horiz_speed.
            if speed_stop >= target.horiz_speed {
                // We will overshoot the speed and go too fast; reduce throttle now.
                ThrottleAction::Decrease
            } else {
                // Continue to increase throttle; we are still too slow.
                ThrottleAction::Increase
            }
        }
    } else {
        if control.horiz_accel.is_positive() {
            // We are faster than we want to be and we are even further accelerating,
            // so reducing throttle is the only correct action.
            ThrottleAction::Decrease
        } else {
            // With a similar approach as above, except accel_change_rate is positive this time.
            let speed_stop = current_speed
                - Speed(0.5 * control.horiz_accel.0.powi(2) / limits.accel_change_rate.0);

            // As we continue to decelerate, speed(0) decreases over time,
            // so speed(t_stop) also decreases over time.
            // We start increasing the throttle when speed(t_stop) <= target.horiz_speed.
            if speed_stop <= target.horiz_speed {
                // We will overshoot the speed and go too slow; increase throttle now.
                ThrottleAction::Increase
            } else {
                // Continue to decrease throttle; we are still too fast.
                ThrottleAction::Decrease
            }
        }
    };

    match desired_action {
        ThrottleAction::Increase => {
            // We cannot increase acceleration too quickly to avoid compressor stall.
            let actual_accel =
                max_accel.min(control.horiz_accel + limits.accel_change_rate * time.delta());
            control.horiz_accel = actual_accel;
        }
        ThrottleAction::Decrease => {
            // We cannot decelerate too quickly to avoid compressor stall.
            let actual_accel =
                max_decel.max(control.horiz_accel - limits.accel_change_rate * time.delta());
            control.horiz_accel = actual_accel;
        }
    }

    let new_speed = current_speed + control.horiz_accel * time.delta();
    airborne.airspeed = (new_speed * control.heading).with_vertical(airborne.airspeed.vertical());
}

fn maintain_vert(
    time: &Time<time::Virtual>,
    target: &VelocityTarget,
    limits: &nav::Limits,
    airborne: &mut object::Airborne,
) {
    let desired_vert_rate = if target.expedite {
        target.vert_rate.clamp(limits.exp_descent.vert_rate, limits.exp_climb.vert_rate)
    } else {
        target.vert_rate.clamp(limits.std_descent.vert_rate, limits.std_climb.vert_rate)
    };
    let actual_vert_rate = desired_vert_rate.clamp(
        airborne.airspeed.vertical() - limits.max_vert_accel * time.delta(),
        airborne.airspeed.vertical() + limits.max_vert_accel * time.delta(),
    );
    airborne.airspeed.set_vertical(actual_vert_rate);
}

fn rotate_object_system(mut query: Query<(&mut object::Rotation, &object::Object, &Control)>) {
    query.iter_mut().for_each(|(mut rot, &Object { ground_speed, .. }, thrust)| {
        let pitch = ground_speed.vertical().atan2(ground_speed.horizontal().magnitude_exact());
        rot.0 = Quat::from_rotation_x(pitch.0) * thrust.heading.into_rotation_quat();
    });
}
