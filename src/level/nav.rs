//! Controls machines for navigation.

use bevy::app::{self, App, Plugin};
use bevy::ecs::query::QueryData;
use bevy::prelude::{Component, IntoSystemConfigs, IntoSystemSetConfigs, Query, Res};
use bevy::time::{self, Time};

use super::{object, SystemSets};
use crate::math::{Heading, TurnDirection};
use crate::pid;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            (altitude_control_system, ground_heading_control_system).in_set(SystemSets::Navigate),
        );
        app.configure_sets(app::Update, SystemSets::Navigate.ambiguous_with(SystemSets::Navigate));
    }
}

/// Current target states of the airspeed vector.
///
/// This optional component is omitted when the plane is not airborne.
#[derive(Component)]
pub struct VelocityTarget {
    /// Target yaw change.
    pub yaw:         YawTarget,
    /// Target horizontal indicated airspeed, in kt.
    pub horiz_speed: f32,
    /// Target vertical rate, in kt.
    pub vert_rate:   f32,
}

/// Target yaw change.
pub enum YawTarget {
    /// Perform a left or right turn to the `Heading`, whichever is closer.
    Heading(Heading),
    /// Maintain turn towards `direction`
    /// until the heading crosses `heading` for `remaining_crosses` times.
    ///
    /// Unlike other variants, this variant may be mutated by `apply_forces_system`.
    /// `remaining_crosses` is decremented by 1 every time the plane heading crosses `heading`.
    /// The entire variant becomes `Heading(heading)`
    /// when `remaining_crosses == 0` and there is less than &pi;/2 turn towards `heading`.
    TurnHeading {
        heading:           Heading,
        remaining_crosses: u8,
        direction:         TurnDirection,
    },
    /// Perform a constant turn at the given angular speed, in rad/s.
    Speed(f32),
}

/// Desired altitude in feet.
///
/// Optional component. Target vertical speed is uncontrolled without this component.
#[derive(Component)]
pub struct TargetAltitude(pub f32);

fn altitude_control_system(
    time: Res<Time<time::Virtual>>,
    mut query: Query<(&TargetAltitude, &object::Position, &mut VelocityTarget)>,
) {
    /// Maximum proportion of the altitude error to compensate per second.
    const DELTA_RATE_PER_SECOND: f32 = 0.3;

    if time.is_paused() {
        return;
    }

    query.par_iter_mut().for_each(|(altitude, position, mut target)| {
        let diff = altitude.0 - position.0.z;
        let speed = diff * DELTA_RATE_PER_SECOND;
        target.vert_rate = speed;
    });
}

/// Desired ground speed direction. Only applicable to airborne objects.
///
/// Optional component to control target heading.
#[derive(Component)]
pub struct TargetGroundDirection {
    pub target: Heading,
    pid_state:  pid::State,
}

#[derive(QueryData)]
#[query_data(mutable)]
struct GroundDirectionSystemQueryData {
    objective:      &'static mut TargetGroundDirection,
    current_ground: &'static object::GroundSpeed,
    current_air:    &'static object::Airborne,
    signal:         &'static mut VelocityTarget,
}

fn ground_heading_control_system(
    time: Res<Time<time::Virtual>>,
    mut query: Query<GroundDirectionSystemQueryData>,
) {
    if time.is_paused() {
        return;
    }

    query.par_iter_mut().for_each(|mut data| {
        let current_heading = Heading::from_vec3(data.current_ground.0);
        let error = data.objective.target - current_heading;

        let signal = pid::control(&mut data.objective.pid_state, error, time.delta_secs());
        data.signal.yaw =
            YawTarget::Heading(Heading::from_vec3(data.current_air.airspeed) + signal);
    });
}
