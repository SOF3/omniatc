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
        app.add_systems(
            app::Update,
            glide_control_system.after(altitude_control_system).in_set(SystemSets::Navigate),
        );
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
#[derive(Component, Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Component, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct Limits {
    /// Minimum horizontal indicated airspeed.
    pub min_horiz_speed: Speed<f32>,
    /// Max absolute yaw speed.
    pub max_yaw_speed:   AngularSpeed<f32>,
}

/// Target yaw change.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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
#[derive(Component, serde::Serialize, serde::Deserialize)]
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
        let diff = altitude.altitude - position.altitude();
        let speed = Speed::per_second(diff * DELTA_RATE_PER_SECOND);
        target.vert_rate = speed;
        target.expedite = altitude.expedite;
    });
}

/// Pitch towards a glidepath of depression angle `glide_angle` towards `target_waypoint`,
/// without pitching beyond `min_pitch` (usually negative) and `max_pitch` (usually zero).
/// until the angle of depression from the object to `target_waypoint`
/// is within `glide_angle` &pm; `activation_range` (both of which should be positive),
/// then attempt to maintain ground speed angle at `glide_angle` aiming at `target_waypoint`.
///
/// Overrides [`TargetAltitude`] if both are present.
///
/// This is implemented with a pure pursuit algorithm by
/// pointing towards the glidepath position after `lookahead * ground_speed`.
/// However, the direction of ground speed is not taken into account,
/// which may result in confusing behavior if the object is not
/// moving (almost) directly towards the waypoint.
#[derive(Component)]
#[require(TargetGlideStatus)]
pub struct TargetGlide {
    /// Target point to aim at.
    pub target_waypoint: Entity,
    /// Angle of depression of the glide path.
    pub glide_angle:     Angle<f32>,
    /// Most negative pitch to use.
    pub min_pitch:       Angle<f32>,
    /// Highest pitch to use.
    pub max_pitch:       Angle<f32>,
    /// Lookahead time for pure pursuit.
    pub lookahead:       Duration,
    /// Whether the aircraft should expedit climb/descent to intersect with the glidepath.
    ///
    /// If false, the min/max pitch is further restricted by the standard climb/descent rate.
    /// If true, it is only restricted by the expedition rate (which would be the physical limit).
    pub expedite:        bool,
}

#[derive(Component, Default)]
pub struct TargetGlideStatus {
    /// Actual pitch the object currently aims at to move towards the glidepath.
    pub current_pitch:      Angle<f32>,
    /// Vertical distance from the glidepath to object altitude.
    /// A positive value means above glidescope.
    pub altitude_deviation: Distance<f32>,
    /// Horizontal distance from the object to its intersection point with the glidepath.
    /// Positive if the intersection point is between the object and the target waypoint
    /// (i.e. in front of the object),
    /// negative if it is behind.
    pub glidepath_distance: Distance<f32>,
}

fn glide_control_system(
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(&mut VelocityTarget, &TargetGlide, &mut TargetGlideStatus, &Object)>,
    waypoint_query: Query<&Waypoint>,
) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(
        |(mut signal, glide, mut glide_status, &Object { position, ground_speed })| {
            let Ok(&Waypoint { position: target_position, .. }) =
                waypoint_query.get(glide.target_waypoint)
            else {
                bevy::log::error!("Reference to non waypoint entity {:?}", glide.target_waypoint);
                return;
            };

            // from current position to target waypoint
            let direction = target_position - position;
            let ground_speed = ground_speed.horizontal().magnitude_exact();

            let horiz_distance = direction.horizontal().magnitude_exact();
            let lookahead_distance = ground_speed * glide.lookahead;

            let glide_tan = glide.glide_angle.tan();

            // elevation of the aim point relative to target waypoint.
            let aim_elevation = (horiz_distance - lookahead_distance) * -glide_tan;
            // elevation of current object position relative to target waypoint.
            let current_elevation = -direction.vertical();

            let target_pitch = (aim_elevation - current_elevation)
                .atan2(lookahead_distance)
                .clamp(glide.min_pitch, glide.max_pitch);

            glide_status.current_pitch = target_pitch;
            glide_status.altitude_deviation = current_elevation + horiz_distance * glide_tan;
            glide_status.glidepath_distance = horiz_distance + current_elevation / glide_tan;

            signal.vert_rate = ground_speed * target_pitch.tan();
            signal.expedite = glide.expedite;
        },
    );
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
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(&mut TargetGroundDirection, &TargetWaypoint, &Object)>,
    waypoint_query: Query<&Waypoint>,
) {
    if time.is_paused() {
        return;
    }

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
/// The object is then set to direct towards the closest point in the circle
/// intersecting the line segment closest to `end_waypoint`.
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
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(&mut TargetGroundDirection, &TargetAlignment, &Object)>,
    waypoint_query: Query<&Waypoint>,
) {
    if time.is_paused() {
        return;
    }

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
