use math::{Accel, AccelRate, AngularAccel, AngularSpeed, Length, Speed};
use serde::{Deserialize, Serialize};

/// Describes an object type (model),
/// which characterizes the physical and performance limits of the object.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct ObjectType {
    /// Full display name of the object type.
    pub full_name:   String,
    /// Physical and performance limits of the object affecting taxiing.
    pub taxi_limits: TaxiLimits,
    /// Class-specific specifications of the object type.
    pub class:       ObjectClassSpec,
}

/// Class-specific specifications of an object type.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum ObjectClassSpec {
    /// The object is a plane.
    Plane {
        /// Physical and performance limits of an aircraft affecting airborne navigation.
        nav_limits: NavLimits,
    },
}

/// Physical and performance limits of the plane affecting taxiing.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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

    /// Weight of the aircraft, in kg.
    ///
    /// Affects wake turbulence.
    pub weight: f32,

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

    // Takeoff
    /// The takeoff airspeed.
    pub takeoff_speed: Speed<f32>,

    // Landing
    /// Distance from runway threshold at which the aircraft
    /// must start reducing to `short_final_speed`.
    pub short_final_dist:  Length<f32>,
    /// The runway threshold crossing speed.
    pub short_final_speed: Speed<f32>,
}

/// Speed limitations during a certain climb rate.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
