//! A flying object with forward thrust and takes off/lands on a runway.
//! All plane entities are object entities.

use std::f32::consts::FRAC_PI_2;

use bevy::app::{self, App, Plugin};
use bevy::math::{Quat, Vec3Swizzles};
use bevy::prelude::{
    Component, Entity, EntityCommand, Event, IntoSystemConfigs, Query, Res, World,
};
use bevy::time::{self, Time};

use super::nav::{VelocityTarget, YawTarget};
use super::{object, SystemSets};
use crate::math::{lerp, unlerp, Heading, TurnDirection};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnEvent>();
        app.add_systems(app::Update, apply_forces_system.in_set(SystemSets::Machine));
        app.add_systems(app::Update, rotate_object_system.in_set(SystemSets::Reconcile));
    }
}

/// Mutable states modified by control systems.
#[derive(Component)]
pub struct Control {
    /// Heading of the plane, must be a unit vector.
    /// This is the horizontal direction of the thrust generated.
    pub heading:     Heading,
    /// Rate of yaw change, in rad/s. Considered to be directly proportional to roll.
    pub yaw_speed:   f32,
    /// Current horizontal acceleration.
    pub horiz_accel: f32,
}

impl Control {
    /// Stabilize at current velocity.
    pub fn stabilized(heading: Heading) -> Self {
        Control { heading, yaw_speed: 0., horiz_accel: 0. }
    }
}

/// Structural limitations of a plane.
#[derive(Component)]
pub struct Limits {
    // Pitch/vertical rate limits.
    /// Climb profile during expedited altitude increase.
    ///
    /// `exp_climb.vert_rate` may be negative during stall.
    pub exp_climb:      ClimbProfile,
    /// Climb profile during standard altitude increase.
    pub std_climb:      ClimbProfile,
    /// Climb profile during no altitude change intended.
    ///
    /// The `vert_rate` field is typically 0,
    /// but could be changed during uncontrolled scenarios like engine failure.
    pub level:          ClimbProfile,
    /// Climb profile during standard altitude decrease.
    pub std_descent:    ClimbProfile,
    /// Climb profile during expedited altitude decrease.
    pub exp_descent:    ClimbProfile,
    /// Absolute change rate for vertical rate acceleration, in kt/s.
    pub max_vert_accel: f32,

    // Forward limits.
    /// Absolute change rate for airbourne horizontal acceleration, in kt/s^2. Always positive.
    pub accel_change_rate: f32, // ah yes we have d^3/dt^3 now...
    /// Drag coefficient, in nm^-1.
    ///
    /// Acceleration is subtracted by `drag_coef * airspeed^2`.
    /// Note that the dimension is inconsistent
    /// since airspeed^2 is nm^2/h^2 but acceleration is nm/h/s.
    ///
    /// Simple formula to derive a reasonable drag coefficient:
    /// `level.accel / (max cruise speed in kt)^2`.
    pub drag_coef:         f32,

    // Z axis rotation limits.
    /// Max rate of change of yaw speed, in rad/s^2.
    pub max_yaw_accel: f32,
    /// Max absolute yaw speed, in rad/s.
    pub max_yaw_speed: f32,
}

impl Limits {
    /// Returns the maximum horizontal acceleration rate at the given climb rate.
    ///
    /// The returned value could be negative.
    pub fn accel(&self, climb_rate: f32) -> f32 {
        self.find_field(climb_rate, |profile| profile.accel)
    }

    /// Returns the maximum horizontal deceleration rate at the given descent rate.
    /// The returned value is negative.
    pub fn decel(&self, climb_rate: f32) -> f32 {
        self.find_field(climb_rate, |profile| profile.decel)
    }

    fn find_field(&self, climb_rate: f32, field: impl Fn(&ClimbProfile) -> f32) -> f32 {
        if climb_rate < self.exp_descent.vert_rate {
            return field(&self.exp_descent);
        }

        for pair in
            [&self.exp_descent, &self.std_descent, &self.level, &self.std_climb, &self.exp_climb]
                .windows(2)
        {
            let &[left, right] = pair else { unreachable!() };
            if climb_rate < right.vert_rate {
                let ratio = unlerp(left.vert_rate, right.vert_rate, climb_rate);
                return lerp(field(left), field(right), ratio);
            }
        }

        field(&self.exp_climb)
    }
}

/// Speed limitations during a certain climb rate.
pub struct ClimbProfile {
    /// Vertical rate for this climb profile, in nm/h.
    /// A negative value indicates this is a descent profile.
    pub vert_rate: f32,
    /// Standard horizontal acceleration rate when requested, in kt/s.
    pub accel:     f32,
    /// Standard horizontal deceleration rate, in kt/s.
    /// The value is negative.
    pub decel:     f32,
}

pub struct SpawnCommand {
    pub control: Option<Control>,
    pub limits:  Limits,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, entity: Entity, world: &mut World) {
        let mut entity_ref = world.entity_mut(entity);

        if let Some(airbourne) = entity_ref.get::<object::Airbourne>() {
            let horiz_speed = airbourne.air_speed.length();

            let dt_target =
                VelocityTarget { yaw: YawTarget::Speed(0.), horiz_speed, vert_rate: 0. };

            entity_ref.insert(dt_target);
        }

        let control = if let Some(control) = self.control {
            control
        } else {
            let heading = Heading::from_quat(
                entity_ref
                    .get::<object::Rotation>()
                    .expect("cannot spawn entity as plane without adding rotation component first")
                    .0,
            );
            Control::stabilized(heading)
        };

        entity_ref.insert((control, self.limits));
        world.send_event(SpawnEvent(entity));
    }
}

/// Sent when a plane entity is spawned.
#[derive(Event)]
pub struct SpawnEvent(pub Entity);

fn apply_forces_system(
    time: Res<Time<time::Virtual>>,
    mut plane_query: Query<(&mut VelocityTarget, &mut Control, &Limits, &mut object::Airbourne)>,
) {
    if time.is_paused() {
        return;
    }

    plane_query.par_iter_mut().for_each(|(mut target, mut control, limits, mut airbourne)| {
        // All components are always changed. Deref first to avoid borrowck issues.
        maintain_yaw(&time, &mut target, &mut control, limits, &airbourne);
        maintain_accel(&time, &target, &mut control, limits, &mut airbourne);
        maintain_vert(&time, &target, limits, &mut airbourne);
    });
}

fn maintain_yaw(
    time: &Time<time::Virtual>,
    target: &mut VelocityTarget,
    control: &mut Control,
    limits: &Limits,
    airbourne: &object::Airbourne,
) {
    let current_yaw = Heading::from_vec3(airbourne.air_speed);
    let mut detect_crossing = None;
    let mut set_yaw_target = None;

    let desired_yaw_speed = match target.yaw {
        YawTarget::Speed(target_yaw_speed) => target_yaw_speed,
        YawTarget::Heading(target_heading) => {
            match current_yaw.closer_direction_to(target_heading) {
                TurnDirection::CounterClockwise => -limits.max_yaw_speed,
                TurnDirection::Clockwise => limits.max_yaw_speed,
            }
        }
        YawTarget::TurnHeading {
            heading: target_heading,
            ref mut remaining_crosses,
            direction,
        } => {
            let distance = current_yaw.distance(target_heading, direction);
            if *remaining_crosses == 0 {
                if distance < FRAC_PI_2 {
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
    control.yaw_speed +=
        delta.clamp(-limits.max_yaw_accel, limits.max_yaw_accel) * time.delta_secs();

    {
        let new_heading = control.heading + control.yaw_speed * time.delta_secs();
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
    limits: &Limits,
    airbourne: &mut object::Airbourne,
) {
    let current_speed = airbourne.air_speed.xy().length();
    if target.horiz_speed >= current_speed {
        // We want to accelerate, get the maximum possible acceleration first.
        let desired_accel =
            limits.accel(airbourne.air_speed.z) - limits.drag_coef * current_speed.powi(2);
        // We cannot accelerate too quickly to avoid compressor stall.
        let actual_accel =
            desired_accel.min(control.horiz_accel + limits.accel_change_rate * time.delta_secs());
        control.horiz_accel = actual_accel;
    } else {
        // We want to decelerate, but limited by maximum deceleration.
        let desired_decel =
            limits.decel(airbourne.air_speed.z) - limits.drag_coef * current_speed.powi(2);
        // We cannot decelerate too quickly to avoid compressor stall.
        let actual_accel =
            desired_decel.max(control.horiz_accel - limits.accel_change_rate * time.delta_secs());
        control.horiz_accel = actual_accel;
    }

    let new_speed = current_speed + control.horiz_accel * time.delta_secs();
    airbourne.air_speed = (control.heading.into_dir2() * new_speed, airbourne.air_speed.z).into();
}

fn maintain_vert(
    time: &Time<time::Virtual>,
    target: &VelocityTarget,
    limits: &Limits,
    airbourne: &mut object::Airbourne,
) {
    let desired_vert_rate =
        target.vert_rate.clamp(limits.exp_descent.vert_rate, limits.exp_climb.vert_rate);
    let actual_vert_rate = desired_vert_rate.clamp(
        airbourne.air_speed.z - limits.max_vert_accel * time.delta_secs(),
        airbourne.air_speed.z + limits.max_vert_accel * time.delta_secs(),
    );
    airbourne.air_speed.z = actual_vert_rate;
}

fn rotate_object_system(mut query: Query<(&mut object::Rotation, &object::GroundSpeed, &Control)>) {
    query.iter_mut().for_each(|(mut rot, gs, thrust)| {
        let yaw = thrust.heading.radians();
        let pitch = gs.0.z.atan2(gs.0.xy().length());
        rot.0 = Quat::from_rotation_x(pitch) * Quat::from_rotation_z(-yaw);
    });
}
