//! Universal constants related to physics and units.

// we don't really want to read the mathematical constants in this file.
#![allow(clippy::excessive_precision, clippy::unreadable_literal)]

use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::{fmt, ops};

use bevy::math::{Dir2, Quat, Vec2, Vec3, Vec3A, Vec3Swizzles};

/// Converts nautical miles to feet.
pub const FEET_PER_NM: f32 = 6076.12;
/// Converts nautical miles to feet.
pub const MILE_PER_NM: f32 = 1.15078;
/// Converts nautical miles to meter.
pub const METER_PER_NM: f32 = 1852.;
/// Converts speed of sound to knots.
pub const KT_PER_MACH: f32 = 666.739;

/// Altitude of the tropopause, in kilometers.
pub const TROPOPAUSE_ALTITUDE: f32 = 36089.24 / FEET_PER_NM;

/// Gravitational acceleration in kt/s.
pub const GRAVITY_KNOT_PER_SEC: f32 = 19.06260;

/// Standard sea level temperature in K, used to calculate density altitude.
pub const STANDARD_SEA_LEVEL_TEMPERATURE: f32 = 288.15;
/// Standard lapse rate of temperature, in K/ft.
pub const STANDARD_LAPSE_RATE: f32 = 0.0019812 * FEET_PER_NM;
/// Proportional increase of true airspeed per nm above sea level.
/// Equivalent to 2% per 1000ft.
pub const TAS_DELTA_PER_NM: f32 = 0.02e-3 * FEET_PER_NM;
/// I don't know what this constant even means... see <http://www.edwilliams.org/avform147.htm>.
pub const PRESSURE_DENSITY_ALTITUDE_POW: f32 = 0.2349690;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitSystem {
    /// Units by nautical conventions.
    Nautical,
    /// Common metric units.
    Metric,
    /// Common imperial units.
    Imperial,
    /// Strictly SI base units only.
    Si,
    /// Units used in omniatc internals.
    Debug,
}

#[allow(clippy::match_same_arms)] // prefer more explicit mappings, no point in compression
impl UnitSystem {
    /// Unit used for ground altitude.
    pub fn altitude(self) -> LengthUnit {
        match self {
            Self::Nautical => LengthUnit::Feet,
            Self::Metric => LengthUnit::Meter,
            Self::Imperial => LengthUnit::Feet,
            Self::Si => LengthUnit::Meter,
            Self::Debug => LengthUnit::NauticalMile,
        }
    }

    /// Unit used for pressure altitude.
    pub fn flight_level(self) -> LengthUnit {
        match self {
            Self::Nautical => LengthUnit::FlightLevel,
            Self::Metric => LengthUnit::FlightLevelMeter,
            Self::Imperial => LengthUnit::FlightLevel,
            Self::Si => LengthUnit::FlightLevelMeter,
            Self::Debug => LengthUnit::NauticalMile,
        }
    }

    /// Unit used for distance.
    pub fn distance(self) -> LengthUnit {
        match self {
            Self::Nautical => LengthUnit::NauticalMile,
            Self::Metric => LengthUnit::Kilometer,
            Self::Imperial => LengthUnit::Mile,
            Self::Si => LengthUnit::Meter,
            Self::Debug => LengthUnit::NauticalMile,
        }
    }

    /// Unit used for speed.
    pub fn speed(self) -> SpeedUnit {
        match self {
            Self::Nautical => SpeedUnit::Knot,
            Self::Metric => SpeedUnit::KilometerHour,
            Self::Imperial => SpeedUnit::MileHour,
            Self::Si => SpeedUnit::MeterSecond,
            Self::Debug => SpeedUnit::Knot,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    NauticalMile,
    FlightLevel,
    FlightLevelMeter,
    Feet,
    Mile,
    Meter,
    Kilometer,
}

impl LengthUnit {
    pub fn convert(self, value: f32) -> Quantity {
        match self {
            Self::NauticalMile => Quantity { value, prefix: "", suffix: "nm" },
            Self::FlightLevel => {
                Quantity { value: value * FEET_PER_NM / 100., prefix: "FL", suffix: "" }
            }
            Self::FlightLevelMeter => {
                Quantity { value: value / METER_PER_NM, prefix: "FL", suffix: "m" }
            }
            Self::Feet => Quantity { value: value * FEET_PER_NM, prefix: "", suffix: "ft" },
            Self::Mile => Quantity { value: value * MILE_PER_NM, prefix: "", suffix: "mi" },
            Self::Meter => Quantity { value: value * METER_PER_NM, prefix: "", suffix: "m" },
            Self::Kilometer => {
                Quantity { value: value * METER_PER_NM / 1000., prefix: "", suffix: "km" }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedUnit {
    Knot,
    KilometerHour,
    MileHour,
    MeterSecond,
    Mach,
}

impl SpeedUnit {
    pub fn convert(self, value: f32) -> Quantity {
        match self {
            Self::Knot => Quantity { value, prefix: "", suffix: "kt" },
            Self::KilometerHour => {
                Quantity { value: value * METER_PER_NM / 1000., prefix: "", suffix: "km/h" }
            }
            Self::MileHour => Quantity { value: value * MILE_PER_NM, prefix: "", suffix: "mph" },
            Self::MeterSecond => {
                Quantity { value: value * METER_PER_NM / 3600., prefix: "", suffix: "m/s" }
            }
            Self::Mach => Quantity { value: value / KT_PER_MACH, prefix: "Ma", suffix: "" },
        }
    }
}

pub struct Quantity {
    pub value:  f32,
    pub prefix: &'static str,
    pub suffix: &'static str,
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:.0}{}", self.prefix, self.value, self.suffix)
    }
}

/// An absolute directional bearing.
#[derive(Clone, Copy)]
pub struct Heading(
    f32, // in radians, -PI < heading <= PI
);

impl Heading {
    /// Heading north.
    pub const NORTH: Self = Self(0.);
    /// Heading east.
    pub const EAST: Self = Self(FRAC_PI_2);
    /// Heading south.
    pub const SOUTH: Self = Self(PI);
    /// Heading west.
    pub const WEST: Self = Self(FRAC_PI_2 * 3.);

    /// Returns the heading of the vector.
    pub fn from_vec2(vec: Vec2) -> Self { Self(vec.x.atan2(vec.y)) }

    /// Converts the heading into a direction vector.
    pub fn into_dir2(self) -> Dir2 {
        let (x, y) = self.0.sin_cos();
        Dir2::from_xy_unchecked(x, y)
    }

    /// Returns the horizontal heading of the vector.
    pub fn from_vec3(vec: impl Into<Vec3>) -> Self {
        let vec: Vec3 = vec.into();
        Self::from_vec2(vec.xy())
    }

    /// Returns the horizontal heading after rotating a northward vector by the quaternion.
    pub fn from_quat(quat: Quat) -> Self { Self::from_vec3(quat.mul_vec3a(Vec3A::Y)) }

    /// Creates a heading from an absolute bearing.
    pub fn from_degrees(degrees: f32) -> Self { Self::from_radians(degrees.to_radians()) }

    /// Returns the heading in degrees in the range 0..360.
    pub fn degrees(self) -> f32 {
        let degrees = self.0.to_degrees();
        if degrees < 0. {
            degrees + 360.
        } else {
            degrees
        }
    }

    /// Creates a heading from an absolute bearing in radians.
    pub fn from_radians(mut radians: f32) -> Self {
        if radians > PI {
            radians -= PI;
        }
        Self(radians)
    }

    /// Returns the heading in radians in the range `-PI < value <= PI`.
    pub fn radians(self) -> f32 { self.0 }

    /// Returns the heading in radians in the range `0 <= value < PI*2`.
    pub fn radians_nonnegative(self) -> f32 {
        if self.0 < 0. {
            self.0 + TAU
        } else {
            self.0
        }
    }

    /// Radians to turn from `self` to `other` in the given direction.
    /// The output is always in the range [0, TAU) for `Clockwise`,
    /// or (-TAU, 0] for `CounterClockwise`.
    #[must_use]
    pub fn distance(self, other: Heading, dir: TurnDirection) -> f32 {
        let mut output = (other.0 - self.0) % TAU;
        match dir {
            TurnDirection::Clockwise => {
                if output < 0. {
                    output += TAU;
                }
            }
            TurnDirection::CounterClockwise => {
                if output > 0. {
                    output -= TAU;
                }
            }
        }
        output
    }

    /// Returns the closer direction to turn towards `other`.
    ///
    /// This assumes zero current angular velocity.
    /// The result is unspecified if `a` and `b` are exactly opposite.
    #[must_use]
    pub fn closer_direction_to(self, other: Heading) -> TurnDirection {
        if self.distance(other, TurnDirection::Clockwise) < PI {
            TurnDirection::Clockwise
        } else {
            TurnDirection::CounterClockwise
        }
    }

    /// Rotate by `delta` radians in the direction of `dir`.
    #[must_use]
    pub fn add_direction(self, dir: TurnDirection, delta: f32) -> Self {
        match dir {
            TurnDirection::CounterClockwise => self - delta,
            TurnDirection::Clockwise => self + delta,
        }
    }

    /// Checks whether `self` is in the non-reflex angle between `a` and `b`,
    ///
    /// The result is unspecified if `a` and `b` are exactly opposite.
    #[must_use]
    pub fn is_between(self, a: Heading, b: Heading) -> bool {
        let ab_dir = a.closer_direction_to(b);
        let ab_dist = a.distance(b, ab_dir);
        let a_self_dist = a.distance(self, ab_dir);

        a_self_dist.abs() < ab_dist * a_self_dist.signum()
    }
}

impl fmt::Debug for Heading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Heading")
            .field("radians", &self.radians())
            .field("degrees", &self.degrees())
            .finish()
    }
}

/// Returns the shortest bearing change such that
/// adding the return value to `other` approximately yields `self`.
impl ops::Sub for Heading {
    type Output = f32;
    fn sub(self, other: Self) -> f32 {
        if (self.0 - other.0).abs() <= PI {
            self.0 - other.0
        } else if self.0 > other.0 {
            self.0 - (other.0 + TAU)
        } else {
            self.0 + TAU - other.0
        }
    }
}

impl ops::Add<f32> for Heading {
    type Output = Self;
    /// Offsets `self` by `angle` clockwise.
    fn add(mut self, angle: f32) -> Self {
        self.0 += angle;
        self.0 %= TAU;
        if self.0 > PI {
            self.0 -= TAU;
        } else if self.0 <= -PI {
            self.0 += TAU;
        }
        self
    }
}

impl ops::AddAssign<f32> for Heading {
    /// Offsets `self` by `angle` clockwise.
    fn add_assign(&mut self, angle: f32) { *self = *self + angle; }
}

impl ops::Sub<f32> for Heading {
    type Output = Self;
    /// Offsets `self` by `angle` counter-clockwise.
    fn sub(self, angle: f32) -> Self { self + (-angle) }
}

impl ops::SubAssign<f32> for Heading {
    /// Offsets `self` by `angle` clockwise.
    fn sub_assign(&mut self, angle: f32) { *self = *self - angle; }
}

/// The direction for yaw change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnDirection {
    /// A left, counter-clockwise turn generating negative yaw speed.
    CounterClockwise,
    /// A right, clockwise turn generating positive yaw speed.
    Clockwise,
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }

pub fn unlerp(a: f32, b: f32, t: f32) -> f32 { (t - a) / (b - a) }
