//! Controls machines for navigation.

use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::query::QueryData;
use bevy::prelude::{Component, Entity, IntoSystemConfigs, IntoSystemSetConfigs, Query, Res};
use bevy::time::{self, Time};

use super::object::Object;
use super::waypoint::Waypoint;
use super::{object, SystemSets};
use crate::math::line_circle_intersect;
use crate::pid;
use crate::units::{Angle, AngularSpeed, Distance, Heading, Position, Speed, TurnDirection};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Update, altitude_control_system.in_set(SystemSets::Navigate));
        app.add_systems(app::Update, ground_heading_control_system.in_set(SystemSets::Navigate));
        app.add_systems(
            app::Update,
            (waypoint_control_system, alignment_control_system)
                .before(ground_heading_control_system)
                .in_set(SystemSets::Navigate),
        );
        app.configure_sets(app::Update, SystemSets::Navigate.ambiguous_with(SystemSets::Navigate));
    }
}

/// Current target states of the airspeed vector.
///
/// This optional component is omitted when the plane is not airborne.
#[derive(Component)]
#[require(Limits)]
pub struct VelocityTarget {
    /// Target yaw change.
    pub yaw:         YawTarget,
    /// Target horizontal indicated airspeed.
    pub horiz_speed: Speed<f32>,
    /// Target vertical rate.
    pub vert_rate:   Speed<f32>,
    /// Whether vertical rate should be expedited.
    /// If false, `vert_rate` is clamped by normal rate instead of the expedition rate.
    pub expedite:    bool,
}

/// Limits for setting velocity target.
#[derive(Component, Default)]
pub struct Limits {
    /// Minimum horizontal indicated airspeed.
    pub min_horiz_speed: Speed<f32>,
    /// Max absolute yaw speed.
    pub max_yaw_speed:   AngularSpeed<f32>,
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
    /// Perform a constant turn at the given angular speed.
    Speed(AngularSpeed<f32>),
}

/// Desired altitude in feet.
///
/// Optional component. Target vertical speed is uncontrolled without this component.
#[derive(Component)]
pub struct TargetAltitude {
    pub altitude: Position<f32>,
    pub expedite: bool,
}

fn altitude_control_system(
    time: Res<Time<time::Virtual>>,
    mut query: Query<(&TargetAltitude, &Object, &mut VelocityTarget)>,
) {
    /// Maximum proportion of the altitude error to compensate per second.
    const DELTA_RATE_PER_SECOND: f32 = 0.3;

    if time.is_paused() {
        return;
    }

    query.par_iter_mut().for_each(|(altitude, &Object { position, .. }, mut target)| {
        let diff = altitude.altitude - position.vertical();
        let speed = Speed::per_second(diff * DELTA_RATE_PER_SECOND);
        target.vert_rate = speed;
        target.expedite = altitude.expedite;
    });
}

/// Desired ground speed direction. Only applicable to airborne objects.
///
/// Optional component to control target heading.
#[derive(Component)]
pub struct TargetGroundDirection {
    pub active:    bool,
    pub target:    Heading,
    pub pid_state: pid::State,
}

impl Default for TargetGroundDirection {
    fn default() -> Self {
        Self { active: true, target: Heading::NORTH, pid_state: pid::State::default() }
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
struct GroundDirectionSystemQueryData {
    objective:      &'static mut TargetGroundDirection,
    current_object: &'static Object,
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
        if !data.objective.active {
            // Maintain current airspeed heading, no need to control.
            // If ground speed heading is to be maintained, `active` should be set to true
            // with the current ground speed heading as the target instead.
            return;
        }

        let current_heading = data.current_object.ground_speed.horizontal().heading();
        let error = data.objective.target - current_heading;

        let signal = Angle(pid::control(&mut data.objective.pid_state, error.0, time.delta_secs()));
        data.signal.yaw =
            YawTarget::Heading(data.current_air.airspeed.horizontal().heading() + signal);
    });
}

/// Target waypoint to direct to. Only applicable to airborne objects.
///
/// Optional component to control target ground direction, which controls target heading.
#[derive(Component)]
#[require(TargetGroundDirection)]
pub struct TargetWaypoint {
    pub waypoint_entity: Entity,
}

fn waypoint_control_system(
    mut object_query: Query<(&mut TargetGroundDirection, &TargetWaypoint, &Object)>,
    waypoint_query: Query<&Waypoint>,
) {
    object_query.par_iter_mut().for_each(|(mut ground_dir, waypoint, &Object { position, .. })| {
        let Ok(waypoint_pos) = waypoint_query.get(waypoint.waypoint_entity) else {
            bevy::log::error!("Invalid waypoint entity {:?}", waypoint.waypoint_entity);
            return;
        };
        ground_dir.target = (waypoint_pos.position.horizontal() - position.horizontal()).heading();
    });
}

/// Maintain the current heading until the line segment between `start_waypoint` and `end_waypoint`
/// is within the circle of radius `ground_speed * lookahead` around the object.
/// The object is then set to direct towards the closest point in the circle intersecting the line segment
/// closest to `end_waypoint`.
///
/// Does not do anything if the orthogonal distance between the line and the current position
/// exceeds `activation_range` or `ground_speed * lookahead`, whichever is lower.
#[derive(Component)]
#[require(TargetGroundDirection)]
pub struct TargetAlignment {
    /// Start point of the path.
    pub start_waypoint:   Entity,
    /// End point of the path.
    pub end_waypoint:     Entity,
    /// Lookahead time for pure pursuit.
    pub lookahead:        Duration,
    /// Maximum orthogonal distance between the line and the object
    /// within which direction control is activated for alignment.
    /// This is used to avoid prematurely turning directly towards the localizer.
    pub activation_range: Distance<f32>,
}

fn alignment_control_system(
    mut object_query: Query<(&mut TargetGroundDirection, &TargetAlignment, &Object)>,
    waypoint_query: Query<&Waypoint>,
) {
    object_query.par_iter_mut().for_each(
        |(mut signal, target, &Object { position, ground_speed })| {
            let Ok(&Waypoint { position: start, .. }) = waypoint_query.get(target.start_waypoint)
            else {
                return;
            };
            let start = start.horizontal();

            let Ok(&Waypoint { position: end, .. }) = waypoint_query.get(target.end_waypoint)
            else {
                return;
            };
            let end = end.horizontal();

            let position = position.horizontal();
            let radius = ground_speed * target.lookahead;
            let radius_sq = radius.magnitude_squared();

            let circle_intersects = line_circle_intersect(position, radius_sq, start, end)
                .and_then(|[low, high]| {
                    let high_pos = start.lerp(end, high);
                    let low_pos = start.lerp(end, low);

                    // compute apothem from radius and chord length
                    let ortho_dist_sq = radius_sq - low_pos.distance_squared(high_pos) / 4.;

                    if ortho_dist_sq.cmp_sqrt() < target.activation_range
                        || position.distance_squared(high_pos) < radius_sq
                    {
                        Some(high_pos)
                    } else {
                        None
                    }
                });
            if let Some(high_pos) = circle_intersects {
                signal.active = true;
                signal.target = (high_pos - position).heading();
            } else {
                // too far from path, maintain current heading
                signal.active = false;
            }
        },
    );
}
