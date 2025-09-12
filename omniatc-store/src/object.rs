use std::time::Duration;

use bevy_math::Vec2;
use math::{
    Accel, AccelRate, Angle, AngularAccel, AngularSpeed, Heading, Length, Position, Speed,
    TurnDirection,
};
use serde::{Deserialize, Serialize};

use crate::{AerodromeRef, Route, Score, SegmentRef, WaypointRef};

/// An object in the world.
#[derive(Clone, Serialize, Deserialize)]
pub enum Object {
    /// A [`Plane`].
    Plane(Plane),
}

/// A plane, characterized by its ability to fly, takeoff and land,
/// limited to forward thrust only and subject to stall.
#[derive(Clone, Serialize, Deserialize)]
pub struct Plane {
    /// Common attributes of the aircraft.
    pub aircraft:    BaseAircraft,
    /// Control parameters of the plane.
    pub control:     PlaneControl,
    /// Physical and performance limits of the plane affecting taxiing.
    pub taxi_limits: TaxiLimits,
    /// Physical and performance limits of the plane affecting airborne navigation.
    pub nav_limits:  NavLimits,
    /// Higher-level control mode.
    pub nav_target:  NavTarget,
    /// Planned route.
    pub route:       Route,
}

/// Physical and performance limits of the plane affecting taxiing.
#[derive(Clone, Serialize, Deserialize)]
pub struct TaxiLimits {
    /// Maximum acceleration on ground.
    pub accel:        Accel<f32>,
    /// Braking deceleration under optimal conditions.
    /// Always positive.
    pub base_braking: Accel<f32>,

    /// Maximum speed during taxi.
    pub max_speed: Speed<f32>,
    /// Fastest pushback/reversal speed.
    ///
    /// Should be negative if the object can reverse,
    /// zero otherwise.
    pub min_speed: Speed<f32>,

    /// Maximum absolute rotation speed during taxi. Always positive.
    pub turn_rate: AngularSpeed,

    /// Minimum width of segments this object can taxi on.
    ///
    /// For planes, this is the wingspan.
    /// For helicopters, this is the rotor diameter.
    pub width:       Length<f32>,
    /// The distance between two objects on the same segment
    /// must be at least the sum of their half-lengths.
    ///
    /// This value could include extra padding to represent safety distance.
    pub half_length: Length<f32>,
}

/// Physical and performance limits of the plane affecting airborne navigation.
#[derive(Clone, Serialize, Deserialize)]
pub struct NavLimits {
    /// Minimum horizontal indicated airspeed.
    pub min_horiz_speed: Speed<f32>,
    /// Max absolute yaw speed.
    pub max_yaw_speed:   AngularSpeed,

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
    pub max_yaw_accel: AngularAccel,

    /// Distance from runway threshold at which the aircraft
    /// must start reducing to `short_final_speed`.
    pub short_final_dist:  Length<f32>,
    /// The runway threshold crossing speed.
    pub short_final_speed: Speed<f32>,
}

/// Speed limitations during a certain climb rate.
#[derive(Clone, Serialize, Deserialize)]
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

/// Common attributes of an aircraft.
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseAircraft {
    /// Name of the aircraft, used for visual display.
    pub name:             String,
    /// The completion condition for the object.
    pub dest:             Destination,
    /// Score awarded upon completion of control of the object.
    pub completion_score: Score,
    /// Current position.
    pub position:         Position<Vec2>,
    /// Current altitude.
    pub altitude:         Position<f32>,
    /// Current speed of ground projection displacement.
    pub ground_speed:     Speed<f32>,
    /// Current direction of ground projection displacement.
    pub ground_dir:       Heading,
    /// Current change in altitude.
    pub vert_rate:        Speed<f32>,
    /// Weight of the aircraft in kg, affecting wake turbulence.
    pub weight:           f32,
    /// Wingspan of the aircraft, affecting wake turbulence.
    pub wingspan:         Length<f32>,
}

/// Condition for the completion of control of an object.
#[derive(Clone, Serialize, Deserialize)]
pub enum Destination {
    /// Object can be handed over upon vacating a runway in the specific aerodrome.
    Landing {
        /// The object must land in this aerodrome.
        aerodrome: AerodromeRef,
    },
    /// Object can be handed over upon parking in a runway in the specific aerodrome.
    Parking {
        /// The object must park in an apron in this aerodrome.
        aerodrome: AerodromeRef,
    },
    /// Object can be handed over upon vacating any runway.
    VacateAnyRunway,
    // TODO: apron/taxiway arrival.
    /// Reach a given waypoint and a given altitude.
    ///
    /// Either condition is set to `None` upon completion.
    /// The control of the object is completed when both are `None`.
    Departure {
        /// Minimum altitude to reach.
        min_altitude:       Option<Position<f32>>,
        /// Target waypoint to have flown by.
        waypoint_proximity: Option<(WaypointRef, Length<f32>)>,
    },
}

/// Control parameters specific to planes.
#[derive(Clone, Serialize, Deserialize)]
pub struct PlaneControl {
    /// Forward thrust heading.
    pub heading:     Heading,
    /// Current change in yaw.
    pub yaw_speed:   AngularSpeed,
    /// Current thrust.
    pub horiz_accel: Accel<f32>,
}

/// Higher-level control target.
#[derive(Clone, Serialize, Deserialize)]
pub enum NavTarget {
    /// Airborne control target.
    Airborne(Box<AirborneNavTarget>),
    /// Ground control target.
    Ground(GroundNavTarget),
}

/// Higher-level airborne control target.
#[derive(Clone, Serialize, Deserialize)]
pub struct AirborneNavTarget {
    /// Target yaw change.
    pub yaw:         YawTarget,
    /// Target horizontal indicated airspeed.
    pub horiz_speed: Speed<f32>,
    /// Target vertical rate.
    pub vert_rate:   Speed<f32>,
    /// Whether vertical rate should be expedited.
    /// If false, `vert_rate` is clamped by normal rate instead of the expedition rate.
    pub expedite:    bool,

    /// Configured to maintain an altitude.
    pub target_altitude:  Option<TargetAltitude>,
    /// Configured to follow a glide path.
    pub target_glide:     Option<TargetGlide>,
    /// Configured to fly towards a waypoint.
    pub target_waypoint:  Option<TargetWaypoint>,
    /// Configured to align with a path between two waypoints.
    pub target_alignment: Option<TargetAlignment>,
}

/// Target altitude to maintain.
#[derive(Clone, Serialize, Deserialize)]
pub struct TargetAltitude {
    /// Altitude to move towards and maintain.
    pub altitude: Position<f32>,
    /// Whether to expedite towards the altitude.
    pub expedite: bool,
}

/// Target glide path to follow.
#[derive(Clone, Serialize, Deserialize)]
pub struct TargetGlide {
    /// Target waypoint to aim at.
    pub target_waypoint: WaypointRef,
    /// Angle of depression of the glide path. Typically negative.
    pub glide_angle:     Angle,
    /// Most negative pitch to use.
    pub min_pitch:       Angle,
    /// Highest pitch to use.
    pub max_pitch:       Angle,
    /// Lookahead time for pure pursuit.
    pub lookahead:       Duration,
    /// Whether the aircraft should expedite climb/descent to intersect with the glidepath.
    ///
    /// If false, the min/max pitch is further restricted by the standard climb/descent rate.
    /// If true, it is only restricted by the expedition rate (which would be the physical limit).
    pub expedite:        bool,
}

/// Target waypoint to fly towards.
#[derive(Clone, Serialize, Deserialize)]
pub struct TargetWaypoint {
    /// Name of target waypoint.
    pub waypoint: WaypointRef,
}

/// Target path between two waypoints to align with.
#[derive(Clone, Serialize, Deserialize)]
pub struct TargetAlignment {
    /// Name of start waypoint.
    pub start_waypoint:   WaypointRef,
    /// Name of end waypoint.
    pub end_waypoint:     WaypointRef,
    /// Lookahead time for pure pursuit.
    pub lookahead:        Duration,
    /// Maximum orthogonal distance between the line and the object
    /// within which direction control is activated for alignment.
    /// This is used to avoid prematurely turning directly towards the localizer.
    pub activation_range: Length<f32>,
}

/// Higher-level ground control target.
#[derive(Clone, Serialize, Deserialize)]
pub struct GroundNavTarget {
    /// The current segment the object is taxiing on.
    pub segment: SegmentRef,
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
        /// The eventual heading to reach.
        heading:           Heading,
        /// Number of times the heading must cross `heading` before completing the turn.
        remaining_crosses: u8,
        /// Direction to turn.
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
