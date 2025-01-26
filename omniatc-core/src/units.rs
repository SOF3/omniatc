use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::time::Duration;
use std::{fmt, iter, ops};

use bevy::math::{Dir2, Dir3, NormedVectorSpace, Vec2, Vec3, Vec3Swizzles, VectorSpace};

pub mod display;
mod heading;
pub use heading::{Heading, TurnDirection};
mod position;
pub use position::Position;
mod squared;
pub use squared::Squared;
use squared::SquaredNorm;

use crate::math::{FEET_PER_NM, METERS_PER_NM, MILES_PER_NM};

pub trait Unit: Copy {
    type Value: Copy;

    fn from_raw(value: Self::Value) -> Self;
    fn into_raw(self) -> Self::Value;
}

macro_rules! decl_units {
    ($(
        $(#[$meta:meta])*
        $ty:ident
        $([Linear $linear:literal])?
        $([Rate<$int_dt:ident>])?
        ,
    )*) => { $(
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, PartialOrd, Default)]
        #[derive(serde::Serialize, serde::Deserialize)]
        pub struct $ty<T>(pub T);

        impl<T: Copy> Unit for $ty<T> {
            type Value = T;

            fn from_raw(value: T) -> Self { Self(value) }
            fn into_raw(self) -> T { self.0 }
        }

        impl<T: ops::AddAssign> ops::Add for $ty<T> {
            type Output = Self;

            fn add(mut self, other: Self) -> Self {
                self.0 += other.0;
                self
            }
        }

        impl<T: ops::AddAssign> ops::AddAssign for $ty<T> {
            fn add_assign(&mut self, other: Self) {
                self.0 += other.0;
            }
        }

        impl<T: ops::SubAssign> ops::Sub for $ty<T> {
            type Output = Self;

            fn sub(mut self, other: Self) -> Self {
                self.0 -= other.0;
                self
            }
        }

        impl<T: ops::SubAssign> ops::SubAssign for $ty<T> {
            fn sub_assign(&mut self, other: Self) {
                self.0 -= other.0;
            }
        }

        impl<T: ops::MulAssign<f32>> ops::Mul<f32> for $ty<T> {
            type Output = Self;

            fn mul(mut self, other: f32) -> Self {
                self.0 *= other;
                self
            }
        }

        impl<T: ops::MulAssign<f32>> ops::MulAssign<f32> for $ty<T> {
            fn mul_assign(&mut self, other: f32) {
                self.0 *= other;
            }
        }

        impl<T: ops::DivAssign<f32>> ops::Div<f32> for $ty<T> {
            type Output = Self;

            fn div(mut self, other: f32) -> Self {
                self.0 /= other;
                self
            }
        }

        impl<T: ops::DivAssign<f32>> ops::DivAssign<f32> for $ty<T> {
            fn div_assign(&mut self, other: f32) {
                self.0 /= other;
            }
        }

        impl<T: ops::Div> ops::Div for $ty<T> {
            type Output = T::Output;

            fn div(self, other: Self) -> Self::Output {
                self.0 / other.0
            }
        }

        impl<T: ops::RemAssign<T>> ops::Rem for $ty<T> {
            type Output = Self;

            fn rem(mut self, other: Self) -> Self {
                self.0 %= other.0;
                self
            }
        }

        impl<T: ops::RemAssign<T>> ops::RemAssign for $ty<T> {
            fn rem_assign(&mut self, other: Self) {
                self.0 %= other.0;
            }
        }

        impl<T: ops::Neg<Output = T>> ops::Neg for $ty<T> {
            type Output = Self;

            fn neg(self) -> Self {
                Self(-self.0)
            }
        }

        impl<T: Default + ops::AddAssign> iter::Sum for $ty<T> {
            fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
                iter.fold(Self::default(), |sum, value| sum + value)
            }
        }

        impl<T: VectorSpace> $ty<T> {
            pub const ZERO: Self = Self(T::ZERO);

            #[must_use]
            pub fn lerp(self, other: Self, s: f32) -> Self {
                Self(self.0.lerp(other.0, s))
            }
        }

        impl<T: NormedVectorSpace> $ty<T> {
            pub fn magnitude_cmp(self) -> impl PartialOrd<$ty<f32>> { SquaredNorm(self.0.norm_squared()) }

            pub fn magnitude_squared(self) -> Squared<$ty<f32>> { Squared(self.0.norm_squared()) }

            pub fn magnitude_exact(self) -> $ty<f32> { $ty(self.0.norm()) }
        }

        impl $ty<f32> {
            #[must_use]
            pub fn is_positive(self) -> bool {
                self.0 > 0.
            }

            #[must_use]
            pub fn is_negative(self) -> bool {
                self.0 < 0.
            }

            #[must_use]
            pub fn is_zero(self) -> bool {
                self.0 == 0.
            }

            #[must_use]
            pub fn abs(self) -> Self {
                Self(self.0.abs())
            }

            #[must_use]
            pub fn signum(self) -> f32 {
                self.0.signum()
            }

            /// Inverse lerp function.
            #[must_use]
            pub fn ratio_between(self, start: Self, end: Self) -> f32 {
                (self - start).0 / (end - start).0
            }

            #[must_use]
            pub fn min(self, other: Self) -> Self {
                Self(self.0.min(other.0))
            }

            #[must_use]
            pub fn max(self, other: Self) -> Self {
                Self(self.0.max(other.0))
            }

            #[must_use]
            pub fn clamp(self, min: Self, max: Self) -> Self {
                Self(self.0.clamp(min.0, max.0))
            }

            #[must_use]
            pub fn with_heading(self, heading: Heading) -> $ty<Vec2> {
                $ty(heading.into_dir2() * self.0)
            }
        }

        impl ops::Mul<Dir2> for $ty<f32> {
            type Output = $ty<Vec2>;

            fn mul(self, other: Dir2) -> $ty<Vec2> {
                $ty(other * self.0)
            }
        }

        impl ops::Mul<Dir3> for $ty<f32> {
            type Output = $ty<Vec3>;

            fn mul(self, other: Dir3) -> $ty<Vec3> {
                $ty(other * self.0)
            }
        }

        impl $ty<Vec2> {
            #[must_use]
            pub fn x(self) -> $ty<f32> { $ty(self.0.x) }
            #[must_use]
            pub fn y(self) -> $ty<f32> { $ty(self.0.y) }

            #[must_use]
            pub fn horizontally(self) -> $ty<Vec3> {
                $ty(Vec3::from((self.0, 0.)))
            }

            #[must_use]
            pub fn with_vertical(self, vertical: $ty<f32>) -> $ty<Vec3> {
                $ty(Vec3::from((self.0, vertical.0)))
            }

            #[must_use]
            pub fn heading(self) -> Heading {
                Heading::from_vec2(self.0)
            }

            #[must_use]
            pub fn normalize_to_magnitude(self, magnitude: $ty<f32>) -> Self {
                $ty(self.0.normalize_or_zero() * magnitude.0)
            }

            /// Returns a `Vec3` such that
            /// the horizontal projection of the result is equal to `self`.
            #[must_use]
            pub fn projected_from_elevation_angle(self, angle: Angle<f32>) -> $ty<Vec3> {
                self.with_vertical(self.magnitude_exact() * angle.tan())
            }

            /// Rotates the `horizontally()` of this vector upwards by `angle`.
            /// The result has the same magnitude as `self`.
            #[must_use]
            pub fn rotate_with_elevation_angle(self, angle: Angle<f32>) -> $ty<Vec3> {
                let horizontal = self * angle.cos();
                let vertical = self.magnitude_exact() * angle.sin();
                horizontal.with_vertical(vertical)
            }
        }

        impl $ty<Vec3> {
            #[must_use]
            pub fn x(self) -> $ty<f32> { $ty(self.0.x) }
            #[must_use]
            pub fn y(self) -> $ty<f32> { $ty(self.0.y) }

            /// Returns the horizontal projection of this vector.
            #[must_use]
            pub fn horizontal(self) -> $ty<Vec2> {
                $ty(self.0.xy())
            }

            #[must_use]
            pub fn vertical(self) -> $ty<f32> {
                $ty(self.0.z)
            }

            pub fn set_vertical(&mut self, value: $ty<f32>) {
                self.0.z = value.0;
            }

            #[must_use]
            pub fn normalize_to_magnitude(self, magnitude: $ty<f32>) -> Self {
                $ty(self.0.normalize_or_zero() * magnitude.0)
            }
        }

        impl<T: Copy + ops::Mul<Output = T>> $ty<T> {
            pub fn squared(self) -> Squared<$ty<T>> {
                Squared(self.0 * self.0)
            }
        }

        impl<T: Copy + ops::Mul<Output = T>> ops::Mul for $ty<T> {
            type Output = Squared<$ty<T>>;

            fn mul(self, other: Self) -> Squared<$ty<T>> {
                Squared(self.0 * other.0)
            }
        }

        $(
            #[doc = $linear]
            impl $ty<f32> {
            #[must_use]
                pub fn atan2(self, x: Self) -> Angle<f32> {
                    Angle(self.0.atan2(x.0))
                }
            }
        )?

        $(
            impl<T> $ty<T> {
                // TODO this signature doesn't really make sense, revisit this.
                pub fn per_second(amount: $int_dt<T>) -> Self {
                    Self(amount.0)
                }
            }

            impl<T: ops::Mul<f32, Output = T>> ops::Mul<Duration> for $ty<T> {
                type Output = $int_dt<T>;

                fn mul(self, other: Duration) -> $int_dt<T> {
                    $int_dt(self.0 * other.as_secs_f32())
                }
            }

            impl ops::Div<$ty<f32>> for $int_dt<f32> {
                type Output = Duration;

                fn div(self, other: $ty<f32>) -> Duration {
                    Duration::from_secs_f32(self.0 / other.0)
                }
            }

            impl<T: ops::Div<f32, Output = T>> ops::Div<Duration> for $int_dt<T> {
                type Output = $ty<T>;

                fn div(self, other: Duration) -> $ty<T> {
                    $ty(self.0 / other.as_secs_f32())
                }
            }
        )?
    )* };
}

decl_units! {
    /// A distance quantity. Always in nautical miles.
    Distance[Linear "Linear"],

    /// A linear speed (rate of [distance](Distance) change) quantity.
    /// Always in nm/s.
    Speed[Linear "Linear"][Rate<Distance>],

    /// A linear acceleration (rate of linear [speed](Speed) change) quantity.
    /// Always in nm/s^2.
    Accel[Linear "Linear"][Rate<Speed>],

    /// Rate of linear [acceleration](Accel) change.
    /// Always in nm/s^3.
    AccelRate[Linear "Linear"][Rate<Accel>],

    /// A relative angle. Always in radians.
    Angle,

    /// An angular speed (rate of [angle](Angle) change) quantity.
    /// Always in rad/s.
    AngularSpeed[Rate<Angle>],

    /// An angular acceleration (rate of [angular speed](AngularSpeed) change) quantity.
    /// Always in rad/s^2.
    AngularAccel[Rate<AngularSpeed>],
}

impl fmt::Debug for Distance<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Distance")
            .field("nm", &self.into_nm())
            .field("feet", &self.into_feet())
            .finish()
    }
}

impl fmt::Debug for Distance<Vec2> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Distance")
            .field("x.nm", &self.x().into_nm())
            .field("y.nm", &self.y().into_nm())
            .finish()
    }
}

impl fmt::Debug for Distance<Vec3> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Distance")
            .field("x.nm", &self.x().into_nm())
            .field("y.nm", &self.y().into_nm())
            .field("vertical.feet", &self.vertical().into_feet())
            .finish()
    }
}

impl fmt::Debug for Speed<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Speed")
            .field("knots", &self.into_knots())
            .field("fpm", &self.into_fpm())
            .finish()
    }
}

impl fmt::Debug for Speed<Vec2> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Speed")
            .field("x.knots", &self.x().into_knots())
            .field("y.knots", &self.y().into_knots())
            .finish()
    }
}

impl fmt::Debug for Speed<Vec3> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Speed")
            .field("x.knots", &self.x().into_knots())
            .field("y.knots", &self.y().into_knots())
            .field("vertical.fpm", &self.vertical().into_fpm())
            .finish()
    }
}

impl fmt::Debug for Accel<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Accel").field("knots/s", &self.into_knots_per_sec()).finish()
    }
}

impl fmt::Debug for Angle<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Angle").field("degrees", &self.into_degrees()).finish()
    }
}

impl fmt::Debug for AngularSpeed<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AngularSpeed").field("degrees/s", &self.into_degrees_per_sec()).finish()
    }
}

impl fmt::Debug for AngularAccel<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AngularAccel").field("degrees/s2", &self.into_degrees_per_sec2()).finish()
    }
}

impl Distance<f32> {
    #[must_use]
    pub const fn into_nm(self) -> f32 { self.0 }

    #[must_use]
    pub const fn from_nm(nm: f32) -> Self { Self(nm) }

    #[must_use]
    pub const fn into_feet(self) -> f32 { self.0 * FEET_PER_NM }

    #[must_use]
    pub const fn from_feet(feet: f32) -> Self { Self(feet / FEET_PER_NM) }

    #[must_use]
    pub const fn into_mile(self) -> f32 { self.0 * MILES_PER_NM }

    #[must_use]
    pub const fn from_mile(mile: f32) -> Self { Self(mile / MILES_PER_NM) }

    #[must_use]
    pub const fn into_meters(self) -> f32 { self.0 * METERS_PER_NM }

    #[must_use]
    pub const fn from_meters(meters: f32) -> Self { Self(meters / METERS_PER_NM) }

    #[must_use]
    pub const fn into_km(self) -> f32 { self.0 * (METERS_PER_NM / 1000.) }

    #[must_use]
    pub const fn from_km(meters: f32) -> Self { Self(meters / (METERS_PER_NM / 1000.)) }
}

impl<T: ops::Mul<f32, Output = T> + ops::Div<f32, Output = T>> Speed<T> {
    #[must_use]
    pub fn into_knots(self) -> T { self.0 * 3600. }

    pub fn from_knots(knots: T) -> Self { Self(knots / 3600.) }

    pub fn into_fpm(self) -> T { self.0 * 60. * FEET_PER_NM }

    pub fn from_fpm(fpm: T) -> Self { Self(fpm / 60. / FEET_PER_NM) }
}

impl<T: ops::Mul<f32, Output = T> + ops::Div<f32, Output = T>> Accel<T> {
    #[must_use]
    pub fn into_knots_per_sec(self) -> T { self.0 * 3600. }

    pub fn from_knots_per_sec(knots: T) -> Self { Self(knots / 3600.) }

    pub fn into_fpm_per_sec(self) -> T { self.0 * 60. * FEET_PER_NM }

    pub fn from_fpm_per_sec(fpm: T) -> Self { Self(fpm / 60. / FEET_PER_NM) }
}

impl<T: ops::Mul<f32, Output = T> + ops::Div<f32, Output = T>> AccelRate<T> {
    #[must_use]
    pub fn into_knots_per_sec2(self) -> T { self.0 * 3600. }

    pub fn from_knots_per_sec2(knots: T) -> Self { Self(knots / 3600.) }
}

impl Angle<f32> {
    pub const RIGHT: Self = Self(FRAC_PI_2);
    pub const STRAIGHT: Self = Self(PI);
    pub const FULL: Self = Self(TAU);

    pub fn from_degrees(degrees: impl Into<f32>) -> Self {
        Self(Into::<f32>::into(degrees).to_radians())
    }

    #[must_use]
    pub fn into_degrees(self) -> f32 { self.0.to_degrees() }

    #[must_use]
    pub fn sin(self) -> f32 { self.0.sin() }
    #[must_use]
    pub fn cos(self) -> f32 { self.0.cos() }
    #[must_use]
    pub fn tan(self) -> f32 { self.0.tan() }
}

impl AngularSpeed<f32> {
    #[must_use]
    pub fn into_degrees_per_sec(self) -> f32 { self.0.to_degrees() }

    #[must_use]
    pub fn from_degrees_per_sec(degrees: f32) -> Self { Self(degrees.to_radians()) }
}

impl AngularAccel<f32> {
    #[must_use]
    pub fn into_degrees_per_sec2(self) -> f32 { self.0.to_degrees() }

    #[must_use]
    pub fn from_degrees_per_sec2(degrees: f32) -> Self { Self(degrees.to_radians()) }
}
