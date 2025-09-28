use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI};
use std::hash::Hash;
use std::{fmt, ops};

use bevy_math::{Dir2, Quat, Vec2, Vec3, Vec3A, Vec3Swizzles};
use ordered_float::{FloatIsNan, NotNan};

use super::{Angle, Length, Position};

#[cfg(test)]
mod tests;

/// An absolute directional bearing.
#[derive(Clone, Copy, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Heading(
    Angle, // always -PI < heading <= PI
);

impl Heading {
    /// Heading north.
    pub const NORTH: Self = Self(Angle::new(0.));
    /// Heading east.
    pub const EAST: Self = Self(Angle::new(FRAC_PI_2));
    /// Heading south.
    pub const SOUTH: Self = Self(Angle::new(PI));
    /// Heading west.
    pub const WEST: Self = Self(Angle::new(FRAC_PI_2 * 3.));

    /// Heading northeast.
    pub const NORTHEAST: Self = Self(Angle::new(FRAC_PI_4));
    /// Heading southeast.
    pub const SOUTHEAST: Self = Self(Angle::new(FRAC_PI_2 + FRAC_PI_4));
    /// Heading southwest.
    pub const SOUTHWEST: Self = Self(Angle::new(PI + FRAC_PI_4));
    /// Heading northwest.
    pub const NORTHWEST: Self = Self(Angle::new(PI + FRAC_PI_2 + FRAC_PI_4));

    /// Returns the heading of the vector.
    ///
    /// Returns a NaN heading if and only if the argument is zero or contains NaN components.
    #[must_use]
    pub fn from_vec2(vec: Vec2) -> Self { Self(Angle::new(vec.x.atan2(vec.y))) }

    /// Converts the heading into a direction vector.
    #[must_use]
    pub fn into_dir2(self) -> Dir2 {
        let (x, y) = self.0.0.sin_cos();
        Dir2::from_xy_unchecked(x, y)
    }

    /// Returns the horizontal heading of the vector.
    #[must_use]
    pub fn from_vec3(vec: impl Into<Vec3>) -> Self {
        let vec: Vec3 = vec.into();
        Self::from_vec2(vec.xy())
    }

    /// Returns the horizontal heading after rotating a northward vector by the quaternion.
    #[must_use]
    pub fn from_quat(quat: Quat) -> Self { Self::from_vec3(quat.mul_vec3a(Vec3A::Y)) }

    /// Creates a heading from an absolute bearing.
    #[must_use]
    pub fn from_degrees(degrees: f32) -> Self { Self::from_radians(Angle::from_degrees(degrees)) }

    /// Returns the heading in degrees in the range 0..360.
    #[must_use]
    pub fn degrees(self) -> f32 {
        let degrees = self.0.into_degrees();
        if degrees < 0. { degrees + 360. } else { degrees }
    }

    /// Creates a heading from an absolute bearing in radians.
    #[must_use]
    pub fn from_radians(mut radians: Angle) -> Self {
        if radians > Angle::STRAIGHT {
            radians -= Angle::FULL;
        }
        Self(radians)
    }

    /// Returns the heading in radians in the range `-STRAIGHT < value <= STRAIGHT`.
    #[must_use]
    pub fn radians(self) -> Angle { self.0 }

    /// Returns the heading as an ordered value.
    ///
    /// The returned value is defined to be ordered by the minimum angular displacement required
    /// to rotate an arbitrary but constant heading to the receiver in clockwise direction.
    ///
    /// # Errors
    /// Returns an error if the heading is NaN.
    pub fn as_ordered(self) -> Result<impl Copy + Ord + Hash, FloatIsNan> { NotNan::new(self.0.0) }

    /// Returns the heading in radians in the range `0 <= value < FULL`.
    #[must_use]
    pub fn radians_nonnegative(self) -> Angle {
        if self.0.is_negative() { self.0 + Angle::FULL } else { self.0 }
    }

    #[must_use]
    pub fn into_rotation_quat(self) -> Quat { Quat::from_rotation_z(-self.0.0) }

    /// Radians to turn from `self` to `other` in the given direction.
    /// The output is always in the range [0, FULL) for `Clockwise`,
    /// or (-FULL, 0] for `CounterClockwise`.
    #[must_use]
    pub fn distance(self, other: Heading, dir: TurnDirection) -> Angle {
        let mut output = (other.0 - self.0) % Angle::FULL;
        match dir {
            TurnDirection::Clockwise => {
                if output.is_negative() {
                    output += Angle::FULL;
                }
            }
            TurnDirection::CounterClockwise => {
                if output.is_positive() {
                    output -= Angle::FULL;
                }
            }
        }

        output
    }

    /// Radians to turn from `self` to `other` in the given direction.
    /// The output is always in the range (0, FULL] for `Clockwise`,
    /// or [-FULL, 0) for `CounterClockwise`.
    #[must_use]
    pub fn nonzero_distance(self, other: Heading, dir: TurnDirection) -> Angle {
        if self.0 == other.0 {
            return match dir {
                TurnDirection::Clockwise => Angle::FULL,
                TurnDirection::CounterClockwise => -Angle::FULL,
            };
        }

        let mut output = (other.0 - self.0) % Angle::FULL;
        match dir {
            TurnDirection::Clockwise => {
                if output.is_negative() {
                    output += Angle::FULL;
                }
            }
            TurnDirection::CounterClockwise => {
                if output.is_positive() {
                    output -= Angle::FULL;
                }
            }
        }

        output
    }

    /// Returns the signed angle closest to zero such that
    /// adding it to `self` approximately returns `other`.
    #[must_use]
    pub fn closest_distance(self, other: Heading) -> Angle {
        self.distance(other, self.closer_direction_to(other))
    }

    /// Returns the closer direction to turn towards `other`.
    ///
    /// This assumes zero current angular velocity.
    /// The result is unspecified if `a` and `b` are exactly opposite or equal.
    #[must_use]
    pub fn closer_direction_to(self, other: Heading) -> TurnDirection {
        if self.distance(other, TurnDirection::Clockwise) < Angle::STRAIGHT {
            TurnDirection::Clockwise
        } else {
            TurnDirection::CounterClockwise
        }
    }

    /// Rotate by `delta` radians in the direction of `dir`.
    #[must_use]
    pub fn add_direction(self, dir: TurnDirection, delta: Angle) -> Self {
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

    /// Returns the opposite direction of this heading.
    #[must_use]
    pub fn opposite(self) -> Self { self + Angle::STRAIGHT }

    /// Turns towards the desired heading, but does not exceed the maximum turn angle.
    ///
    /// `max_turn` must be non-negative.
    #[must_use]
    pub fn restricted_turn(self, desired: Heading, max_turn: Angle) -> Self {
        self + self.closest_distance(desired).clamp(-max_turn, max_turn)
    }

    /// Returns the midpoint of the non-reflex angle between the receiver and `other`.
    ///
    /// The result may be in either direction if `self + Angle::STRAIGHT == other`.
    #[must_use]
    pub fn closest_midpoint(self, other: Heading) -> Heading {
        self + self.closest_distance(other) * 0.5
    }
}

impl fmt::Debug for Heading {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Heading")
            .field("radians", &self.radians().0)
            .field("degrees", &self.degrees())
            .finish()
    }
}

/// Returns the shortest bearing change such that
/// adding the return value to `other` approximately yields `self`.
impl ops::Sub for Heading {
    type Output = Angle;
    fn sub(self, other: Self) -> Angle {
        if (self.0 - other.0).abs() <= Angle::STRAIGHT {
            self.0 - other.0
        } else if self.0 > other.0 {
            self.0 - (other.0 + Angle::FULL)
        } else {
            self.0 + Angle::FULL - other.0
        }
    }
}

impl ops::Add<Angle> for Heading {
    type Output = Self;
    /// Offsets `self` by `angle` clockwise.
    fn add(mut self, angle: Angle) -> Self {
        self.0 += angle;
        self.0 %= Angle::FULL;
        if self.0 > Angle::STRAIGHT {
            self.0 -= Angle::FULL;
        } else if self.0 <= -Angle::STRAIGHT {
            self.0 += Angle::FULL;
        }
        self
    }
}

impl ops::AddAssign<Angle> for Heading {
    /// Offsets `self` by `angle` clockwise.
    fn add_assign(&mut self, angle: Angle) { *self = *self + angle; }
}

impl ops::Sub<Angle> for Heading {
    type Output = Self;
    /// Offsets `self` by `angle` counter-clockwise.
    fn sub(self, angle: Angle) -> Self { self + (-angle) }
}

impl ops::SubAssign<Angle> for Heading {
    /// Offsets `self` by `angle` clockwise.
    fn sub_assign(&mut self, angle: Angle) { *self = *self - angle; }
}

/// The direction for yaw change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum TurnDirection {
    /// A left, counter-clockwise turn generating negative yaw speed.
    CounterClockwise,
    /// A right, clockwise turn generating positive yaw speed.
    Clockwise,
}

impl TurnDirection {
    /// Similar to [`Self::from_triangle`], but assumes `p1` is the origin.
    #[must_use]
    pub fn from_triangle_23(p1_to_p2: Length<Vec2>, p1_to_p3: Length<Vec2>) -> Option<Self> {
        let dot = p1_to_p2.0.perp_dot(p1_to_p3.0);
        if dot > 0. {
            Some(Self::CounterClockwise)
        } else if dot < 0. {
            Some(Self::Clockwise)
        } else {
            None
        }
    }

    /// Returns the closer turn direction from p1 to p2 to p3.
    #[must_use]
    pub fn from_triangle(
        p1: Position<Vec2>,
        p2: Position<Vec2>,
        p3: Position<Vec2>,
    ) -> Option<Self> {
        Self::from_triangle_23(p2 - p1, p3 - p1)
    }
}

impl ops::Neg for TurnDirection {
    type Output = Self;

    fn neg(self) -> Self {
        match self {
            TurnDirection::CounterClockwise => TurnDirection::Clockwise,
            TurnDirection::Clockwise => TurnDirection::CounterClockwise,
        }
    }
}

macro_rules! impl_angle_mul_dir {
    ($ty:ty) => {
        impl ops::Mul<TurnDirection> for $ty {
            type Output = Self;

            fn mul(mut self, dir: TurnDirection) -> Self {
                if dir == TurnDirection::CounterClockwise {
                    self.0 = -self.0;
                }
                self
            }
        }
    };
}

impl_angle_mul_dir!(Angle);
impl_angle_mul_dir!(super::AngularSpeed);
impl_angle_mul_dir!(super::AngularAccel);
