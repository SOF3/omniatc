//! Controls machines for aerial navigation.

use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::query::QueryData;
use bevy::math::Vec2;
use bevy::prelude::{Component, Entity, IntoScheduleConfigs, Query, Res};
use bevy::time::{self, Time};
use math::{
    line_circle_intersect, line_intersect, Accel, AccelRate, Angle, AngularAccel, AngularSpeed,
    Distance, Heading, Position, Speed, TurnDirection,
};

use super::object::Object;
use super::waypoint::Waypoint;
use super::{navaid, object, SystemSets};
use crate::{pid, try_log_return};

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
/// This optional component is removed when the plane is not airborne.
#[derive(Component, Clone, serde::Serialize, serde::Deserialize)]
#[require(navaid::ObjectUsageList)]
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
#[derive(Component, Clone, serde::Serialize, serde::Deserialize)]
pub struct Limits {
    /// Minimum horizontal indicated airspeed.
    pub min_horiz_speed: Speed<f32>,
    /// Max absolute yaw speed.
    pub max_yaw_speed:   AngularSpeed<f32>,

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
    /// Maximum absolute change rate for vertical rate acceleration.
    pub max_vert_accel: Accel<f32>,

    // Forward limits.
    /// Absolute change rate for airborne horizontal acceleration. Always positive.
    pub accel_change_rate: AccelRate<f32>, // ah yes we have d^3/dt^3 now...
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
    /// Max absolute rate of change of yaw speed.
    pub max_yaw_accel: AngularAccel<f32>,

    /// Distance from runway threshold at which the aircraft
    /// must start reducing to `short_final_speed`.
    pub short_final_dist:  Distance<f32>,
    /// The runway threshold crossing speed.
    pub short_final_speed: Speed<f32>,
}

impl Limits {
    /// Returns the maximum horizontal acceleration rate at the given climb rate.
    ///
    /// The returned value could be negative.
    #[must_use]
    pub fn accel(&self, climb_rate: Speed<f32>) -> Accel<f32> {
        self.find_field(climb_rate, |profile| profile.accel)
    }

    /// Returns the maximum horizontal deceleration rate at the given descent rate.
    /// The returned value is negative.
    #[must_use]
    pub fn decel(&self, climb_rate: Speed<f32>) -> Accel<f32> {
        self.find_field(climb_rate, |profile| profile.decel)
    }

    fn find_field(
        &self,
        climb_rate: Speed<f32>,
        field: impl Fn(&ClimbProfile) -> Accel<f32>,
    ) -> Accel<f32> {
        if climb_rate < self.exp_descent.vert_rate {
            return field(&self.exp_descent);
        }

        for pair in
            [&self.exp_descent, &self.std_descent, &self.level, &self.std_climb, &self.exp_climb]
                .windows(2)
        {
            let &[left, right] = pair else { unreachable!() };
            if climb_rate < right.vert_rate {
                let ratio = climb_rate.ratio_between(left.vert_rate, right.vert_rate);
                return field(left).lerp(field(right), ratio);
            }
        }

        field(&self.exp_climb)
    }
}

/// Speed limitations during a certain climb rate.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct ClimbProfile {
    /// Vertical rate for this climb profile.
    /// A negative value indicates this is a descent profile.
    pub vert_rate: Speed<f32>,
    /// Standard horizontal acceleration rate when requested.
    pub accel:     Accel<f32>,
    /// Standard horizontal deceleration rate.
    /// The value is negative.
    pub decel:     Accel<f32>,
}

/// Target yaw change.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
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
}

impl YawTarget {
    /// The eventual target heading, regardless of direction.
    #[must_use]
    pub fn heading(self) -> Heading {
        match self {
            Self::Heading(heading) | Self::TurnHeading { heading, .. } => heading,
        }
    }
}

/// Desired altitude in feet.
///
/// Optional component. Target vertical speed is uncontrolled without this component.
#[derive(Clone, Component, serde::Serialize, serde::Deserialize)]
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
    /// Angle of elevation of the glide path.
    ///
    /// Negative if the glide is a descent.
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
            let &Waypoint { position: target_position, .. } = try_log_return!(
                waypoint_query.get(glide.target_waypoint),
                expect "Reference to non waypoint entity {:?}",
                glide.target_waypoint,
            );

            // from current position to target waypoint
            let direction = target_position - position;
            let ground_speed = ground_speed.horizontal().magnitude_exact();

            let horiz_distance = direction.horizontal().magnitude_exact();
            let lookahead_distance = ground_speed * glide.lookahead;

            let glide_tan = glide.glide_angle.acute_signed_tan();

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

            signal.vert_rate = ground_speed * target_pitch.acute_signed_tan();
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
        let waypoint_pos = try_log_return!(
            waypoint_query.get(waypoint.waypoint_entity),
            expect "invalid waypoint entity {:?}",
            waypoint.waypoint_entity,
        );
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
#[require(TargetGroundDirection, TargetAlignmentStatus)]
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

#[derive(Component, Default)]
pub struct TargetAlignmentStatus {
    /// Whether the object is within the activation range.
    pub activation:           TargetAlignmentActivationStatus,
    /// Orthogonal distance between object and the line to align with.
    pub orthogonal_deviation: Distance<f32>,
    /// Current heading towards line end minus target direction.
    pub angular_deviation:    Angle<f32>,
}

#[derive(Default)]
pub enum TargetAlignmentActivationStatus {
    /// Uninitialized state.
    #[default]
    Uninit,
    /// In pure pursuit mode towards the specified position.
    PurePursuit(Position<Vec2>),
    /// Target line is within ground speed lookahead range, but not in activation range yet.
    Unactivated,
    /// Target line is not within the ground speed lookahead range.
    BeyondLookahead {
        /// Time until the current track intersects with the target line.
        ///
        /// `None` indicates that the current track diverges from the target line.
        intersect_time: Option<Duration>,
        /// Distance from object to line end, projected onto the target line.
        projected_dist: Distance<f32>,
    },
}

fn alignment_control_system(
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(
        &mut TargetGroundDirection,
        &TargetAlignment,
        &mut TargetAlignmentStatus,
        &Object,
    )>,
    waypoint_query: Query<&Waypoint>,
) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(
        |(mut signal, target, mut status, &Object { position, ground_speed })| {
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

            let (activation_status, ortho_dist) = if let Some([low, high]) =
                line_circle_intersect(position, radius_sq, start, end)
            {
                let high_pos = start.lerp(end, high);
                let low_pos = start.lerp(end, low);

                // compute apothem from radius and chord length
                // r >= norm(low - high) * 0.5 under normal circumstances since low and high are
                // points on the circle, but this value may be negative when they are almost equal,
                // i.e. when `position` is almost exactly on the target line.
                let ortho_dist =
                    (radius_sq - low_pos.distance_squared(high_pos) / 4.).sqrt_or_zero();

                if ortho_dist < target.activation_range
                    || position.distance_squared(high_pos) < radius_sq
                {
                    (TargetAlignmentActivationStatus::PurePursuit(high_pos), ortho_dist)
                } else {
                    (TargetAlignmentActivationStatus::Unactivated, ortho_dist)
                }
            } else {
                // Project (end - position) onto (end - start)
                let projected_dist =
                    Distance((end - position).0.dot((end - start).0) / start.distance_exact(end).0);
                let projected_point = end - (end - start).normalize_to_magnitude(projected_dist);

                let (_, current_intersect_secs) = line_intersect(
                    end.get(),
                    (start - end).0,
                    position.get(),
                    ground_speed.horizontal().0,
                );
                let intersect_time = Duration::try_from_secs_f32(current_intersect_secs).ok();

                (
                    TargetAlignmentActivationStatus::BeyondLookahead {
                        intersect_time,
                        projected_dist,
                    },
                    position.distance_exact(projected_point),
                )
            };

            if let TargetAlignmentActivationStatus::PurePursuit(high_pos) = activation_status {
                signal.active = true;
                signal.target = (high_pos - position).heading();
            } else {
                // too far from path, maintain current heading
                signal.active = false;
            }

            status.activation = activation_status;
            status.orthogonal_deviation = ortho_dist;
            status.angular_deviation = (end - position).heading() - (end - start).heading();
        },
    );
}

/// A bundle of all [`VelocityTarget`]-controlling components,
/// used as the type parameter to `EntityWorldMut::remove`.
pub type AllTargets = (
    TargetAltitude,
    TargetGlide,
    TargetGlideStatus,
    TargetGroundDirection,
    TargetWaypoint,
    TargetAlignment,
    TargetAlignmentStatus,
);
