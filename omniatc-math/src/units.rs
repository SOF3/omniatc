use std::f32::consts::{FRAC_PI_2, PI, TAU};
use std::marker::PhantomData;
use std::time::Duration;
use std::{cmp, fmt, iter, ops};

use bevy_math::{Dir2, Dir3, NormedVectorSpace, Vec2, Vec3, Vec3Swizzles, VectorSpace};
use ordered_float::OrderedFloat;

use crate::Sign;

mod display;
pub use display::{LengthUnit, SpeedUnit, UnitEnum};
mod heading;
pub use heading::{Heading, TurnDirection};
mod position;
pub use position::Position;
mod squared;
pub use squared::AsSqrt;

/// Converts nautical miles to feet.
pub const FEET_PER_NM: f32 = 6076.12;
/// Converts nautical miles to feet.
pub const MILES_PER_NM: f32 = 1.15078;
/// Converts nautical miles to meter.
pub const METERS_PER_NM: f32 = 1852.;
/// Converts speed of sound to knots.
pub const KNOTS_PER_MACH: f32 = 666.739;
/// Converts minutes to seconds.
pub const SECONDS_PER_MINUTE: f32 = 60.;
/// Converts hours to seconds.
pub const SECONDS_PER_HOUR: f32 = 3600.;

pub struct Quantity<T, Base, Dt, Pow>(pub T, pub PhantomData<(Base, Dt, Pow)>);

impl<T, Base, Dt, Pow> Quantity<T, Base, Dt, Pow> {
    pub const fn new(value: T) -> Self { Self(value, PhantomData) }
}

pub trait QuantityTrait: Sized {
    /// The type of the raw value of this unit.
    type Raw;
    /// Returns a unit with the same dimensional characteristics but with a different raw value type.
    type WithRaw<U>;
    /// The unit type representing the rate of change of this unit.
    /// Internal representation is in s^-1.
    type Rate;

    fn into_raw(self) -> Self::Raw;
    fn as_raw_mut(&mut self) -> &mut Self::Raw;
    fn from_raw(value: Self::Raw) -> Self;
}

impl<T, Base, Dt, Pow> QuantityTrait for Quantity<T, Base, Dt, Pow> {
    type Raw = T;
    type WithRaw<U> = Quantity<U, Base, Dt, Pow>;
    type Rate = Quantity<T, Base, Ddt<Dt>, Pow>;

    fn into_raw(self) -> T { self.0 }

    fn as_raw_mut(&mut self) -> &mut T { &mut self.0 }

    fn from_raw(value: T) -> Self { Self(value, PhantomData) }
}

impl<T, Base, Dt, Pow> Quantity<T, Base, Dt, Pow>
where
    T: VectorSpace,
{
    pub const ZERO: Self = Self(T::ZERO, PhantomData);

    #[must_use]
    pub fn lerp(self, other: Self, s: f32) -> Self { Self(self.0.lerp(other.0, s), PhantomData) }
}

impl<T, Base, Dt, Pow> Default for Quantity<T, Base, Dt, Pow>
where
    T: Default,
{
    fn default() -> Self { Self(T::default(), PhantomData) }
}

impl<T, Base, Dt, Pow> num_traits::Zero for Quantity<T, Base, Dt, Pow>
where
    T: Default + PartialEq + ops::Add<Output = T>,
{
    fn zero() -> Self { Self::default() }

    fn is_zero(&self) -> bool { self.0 == T::default() }
}

impl<T, Base, Dt, Pow> Clone for Quantity<T, Base, Dt, Pow>
where
    T: Clone,
{
    fn clone(&self) -> Self { Self(self.0.clone(), PhantomData) }
}

impl<T, Base, Dt, Pow> Copy for Quantity<T, Base, Dt, Pow> where T: Copy {}

impl<T, Base, Dt, Pow> PartialEq for Quantity<T, Base, Dt, Pow>
where
    T: PartialEq,
{
    fn eq(&self, other: &Self) -> bool { self.0 == other.0 }
}

impl<T, Base, Dt, Pow> Eq for Quantity<T, Base, Dt, Pow> where T: Eq {}

impl<T, Base, Dt, Pow> PartialOrd for Quantity<T, Base, Dt, Pow>
where
    T: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> { self.0.partial_cmp(&other.0) }
}

impl<T, Base, Dt, Pow> Ord for Quantity<T, Base, Dt, Pow>
where
    T: Ord,
{
    fn cmp(&self, other: &Self) -> cmp::Ordering { self.0.cmp(&other.0) }
}

impl<T, Base, Dt, Pow> ops::Add for Quantity<T, Base, Dt, Pow>
where
    T: ops::Add<Output = T>,
{
    type Output = Self;

    fn add(self, other: Self) -> Self { Self(self.0 + other.0, PhantomData) }
}

impl<T, Base, Dt, Pow> ops::AddAssign for Quantity<T, Base, Dt, Pow>
where
    T: ops::AddAssign,
{
    fn add_assign(&mut self, other: Self) { self.0 += other.0; }
}

impl<T, Base, Dt, Pow> ops::Sub for Quantity<T, Base, Dt, Pow>
where
    T: ops::Sub<Output = T>,
{
    type Output = Self;

    fn sub(self, other: Self) -> Self { Self(self.0 - other.0, PhantomData) }
}

impl<T, Base, Dt, Pow> ops::SubAssign for Quantity<T, Base, Dt, Pow>
where
    T: ops::SubAssign,
{
    fn sub_assign(&mut self, other: Self) { self.0 -= other.0; }
}

impl<T, Base, Dt, Pow> ops::Mul<f32> for Quantity<T, Base, Dt, Pow>
where
    T: ops::Mul<f32, Output = T>,
{
    type Output = Self;

    fn mul(self, other: f32) -> Self { Self(self.0 * other, PhantomData) }
}

impl<T, Base, Dt, Pow> ops::MulAssign<f32> for Quantity<T, Base, Dt, Pow>
where
    T: ops::MulAssign<f32>,
{
    fn mul_assign(&mut self, other: f32) { self.0 *= other; }
}

impl<T, Base, Dt, Pow> ops::Div<f32> for Quantity<T, Base, Dt, Pow>
where
    T: ops::Div<f32, Output = T>,
{
    type Output = Self;

    fn div(self, other: f32) -> Self { Self(self.0 / other, PhantomData) }
}

impl<T, Base, Dt, Pow> ops::DivAssign<f32> for Quantity<T, Base, Dt, Pow>
where
    T: ops::DivAssign<f32>,
{
    fn div_assign(&mut self, other: f32) { self.0 /= other; }
}

impl<T, Base, Dt, Pow> ops::Div for Quantity<T, Base, Dt, Pow>
where
    T: ops::Div,
{
    type Output = T::Output;

    fn div(self, other: Self) -> Self::Output { self.0 / other.0 }
}

impl<T, Base, Dt, Pow> ops::Rem for Quantity<T, Base, Dt, Pow>
where
    T: ops::Rem<Output = T>,
{
    type Output = Self;

    fn rem(self, rhs: Self) -> Self::Output { Self(self.0 % rhs.0, PhantomData) }
}

impl<T, Base, Dt, Pow> ops::RemAssign for Quantity<T, Base, Dt, Pow>
where
    T: ops::RemAssign,
{
    fn rem_assign(&mut self, rhs: Self) { self.0 %= rhs.0; }
}

impl<T, Base, Dt, Pow> ops::Neg for Quantity<T, Base, Dt, Pow>
where
    T: ops::Neg<Output = T>,
{
    type Output = Self;

    fn neg(self) -> Self { Self(-self.0, PhantomData) }
}

impl<T: Default + ops::Add<Output = T>, Base, Dt, Pow> iter::Sum for Quantity<T, Base, Dt, Pow> {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(Self::default(), |sum, value| sum + value)
    }
}

impl<T, Base, Dt, Pow> From<T> for Quantity<T, Base, Dt, Pow> {
    fn from(value: T) -> Self { Self(value, PhantomData) }
}

/// Used as `Dt` in `Quantity` to indicate that the unit is not a rate of change.
pub struct DtZero;
/// Used as `Dt` in `Quantity` to indicate that the unit is the rate of change of `Quantity<Dt=Dt>`.
pub struct Ddt<Dt>(Dt);

pub type DtOne = Ddt<DtZero>;
pub type DtTwo = Ddt<DtOne>;

pub trait DtTrait {
    type Squared;
}

impl DtTrait for DtZero {
    type Squared = DtZero;
}

impl<Dt> DtTrait for Ddt<Dt>
where
    Dt: DtTrait,
{
    // (P+1)*2 = P*2 + 1 + 1
    type Squared = Ddt<Ddt<Dt::Squared>>;
}

impl<T, Base, Dt, Pow> ops::Mul<Duration> for Quantity<T, Base, Ddt<Dt>, Pow>
where
    T: ops::Mul<f32, Output = T>,
{
    type Output = Quantity<T, Base, Dt, Pow>;

    fn mul(self, other: Duration) -> Self::Output {
        Quantity(self.0 * other.as_secs_f32(), PhantomData)
    }
}

impl<T, Base, Dt, Pow> ops::Div<Duration> for Quantity<T, Base, Dt, Pow>
where
    T: ops::Div<f32, Output = T>,
{
    type Output = Quantity<T, Base, Ddt<Dt>, Pow>;

    fn div(self, other: Duration) -> Self::Output {
        Quantity(self.0 / other.as_secs_f32(), PhantomData)
    }
}

/// (B / T^n) / (B / T^(n+1)) = T
impl<T, Base, Dt, Pow> ops::Div<Quantity<T, Base, Ddt<Dt>, Pow>> for Quantity<T, Base, Dt, Pow>
where
    T: ops::Div<Output = f32>,
{
    type Output = Duration;

    fn div(self, rhs: Quantity<T, Base, Ddt<Dt>, Pow>) -> Self::Output {
        Duration::from_secs_f32(self.0 / rhs.0)
    }
}

/// (B / T^n) / (B / T^(n+1)) = T
impl<T, Base, Dt, Pow> Quantity<T, Base, Dt, Pow>
where
    T: ops::Div<Output = f32>,
{
    pub fn try_div(self, rhs: Quantity<T, Base, Ddt<Dt>, Pow>) -> Option<Duration> {
        Duration::try_from_secs_f32(self.0 / rhs.0).ok()
    }
}

/// (B^2 / T^(n+1)) / (B / T^n) = B/T
impl<T, Base, Dt, Pow> ops::Div<Quantity<T, Base, Dt, Pow>> for Quantity<T, Base, Ddt<Dt>, PowTwo>
where
    T: ops::Div,
{
    type Output = Quantity<T::Output, Base, DtOne, Pow>;

    fn div(self, rhs: Quantity<T, Base, Dt, Pow>) -> Self::Output {
        Quantity(self.0 / rhs.0, PhantomData)
    }
}

impl<T, Base, Dt, Pow> Quantity<T, Base, Dt, Pow>
where
    T: ops::Mul<f32, Output = T>,
{
    pub fn per_second(
        self,
        other: Quantity<f32, RatioBase, DtOne, PowZero>,
    ) -> Quantity<T, Base, Ddt<Dt>, Pow> {
        Quantity(self.0 * other.0, PhantomData)
    }
}

impl<T, Base, Dt, Pow> Quantity<T, Base, Ddt<Dt>, Pow>
where
    T: ops::Div<f32, Output = T>,
{
    pub fn div_per_second(
        self,
        other: Quantity<f32, RatioBase, DtOne, PowZero>,
    ) -> Quantity<T, Base, Dt, Pow> {
        Quantity(self.0 / other.0, PhantomData)
    }
}

/// Used as `Pow` in `Quantity` to indicate that the unit is linear (not squared).
pub struct PowZero;

pub struct PowPlusOne<Pow>(Pow);

pub trait PowTrait {
    type Squared;
}

impl PowTrait for PowZero {
    type Squared = PowZero;
}

impl<Pow> PowTrait for PowPlusOne<Pow>
where
    Pow: PowTrait,
{
    // (P+1)*2 = P*2 + 1 + 1
    type Squared = PowPlusOne<PowPlusOne<Pow::Squared>>;
}

pub trait GreaterOrEqPow<Than> {
    type Diff;
}

pub type PowOne = PowPlusOne<PowZero>;
pub type PowTwo = PowPlusOne<PowOne>;

impl<T, Dt, Pow> Quantity<T, LengthBase, Dt, Pow>
where
    T: NormedVectorSpace,
    Dt: DtTrait,
    Pow: PowTrait,
{
    /// Returns a wrapper that can be compared with another quantity directly
    /// without explicit squaring.
    pub fn magnitude_cmp(self) -> AsSqrt<Dt, Pow> {
        AsSqrt { norm_squared: OrderedFloat(self.0.norm_squared()), _ph: PhantomData }
    }

    pub fn magnitude_squared(self) -> Quantity<f32, LengthBase, Dt::Squared, Pow::Squared> {
        Quantity(self.0.norm_squared(), PhantomData)
    }

    pub fn magnitude_exact(self) -> Quantity<f32, LengthBase, Dt, Pow> {
        Quantity(self.0.norm(), PhantomData)
    }
}

impl<Base, Dt, Pow> ops::Div<Quantity<f32, Base, Dt, Pow>>
    for Quantity<f32, Base, Dt, PowPlusOne<Pow>>
{
    type Output = Quantity<f32, Base, DtZero, Pow>;

    fn div(self, rhs: Quantity<f32, Base, Dt, Pow>) -> Self::Output {
        Quantity(self.0 / rhs.0, PhantomData)
    }
}

pub trait CanSquare {
    type Squared;
}

pub type Squared<T> = <T as CanSquare>::Squared;

impl<T, Dt, Pow> CanSquare for Quantity<T, LengthBase, Dt, Pow>
where
    Dt: DtTrait,
    Pow: PowTrait,
{
    type Squared = Quantity<T, LengthBase, Dt::Squared, Pow::Squared>;
}

pub trait CanSqrt: QuantityTrait<Raw = f32> {
    type Sqrt: QuantityTrait<Raw = f32>;

    fn sqrt(self) -> Self::Sqrt { Self::Sqrt::from_raw(self.into_raw().sqrt()) }

    fn sqrt_or_zero(self) -> Self::Sqrt
    where
        Self: Copy,
    {
        if self.into_raw() < 0.0 {
            Self::Sqrt::from_raw(0.0)
        } else {
            self.sqrt()
        }
    }
}

pub trait IsEven {
    type Half;
}

impl IsEven for DtZero {
    type Half = DtZero;
}

impl IsEven for DtTwo {
    type Half = DtOne;
}

impl IsEven for Ddt<Ddt<DtTwo>> {
    type Half = DtTwo;
}

impl IsEven for Ddt<Ddt<Ddt<Ddt<DtTwo>>>> {
    type Half = Ddt<DtTwo>;
}

impl IsEven for PowZero {
    type Half = PowZero;
}

impl IsEven for PowPlusOne<PowPlusOne<PowZero>> {
    type Half = PowPlusOne<PowZero>;
}

impl IsEven for PowPlusOne<PowPlusOne<PowPlusOne<PowPlusOne<PowZero>>>> {
    type Half = PowPlusOne<PowPlusOne<PowZero>>;
}

impl IsEven for PowPlusOne<PowPlusOne<PowPlusOne<PowPlusOne<PowPlusOne<PowPlusOne<PowZero>>>>>> {
    type Half = PowPlusOne<PowPlusOne<PowPlusOne<PowZero>>>;
}

impl<Dt, Pow> CanSqrt for Quantity<f32, LengthBase, Dt, Pow>
where
    Dt: IsEven,
    Pow: IsEven,
{
    type Sqrt = Quantity<f32, LengthBase, Dt::Half, Pow::Half>;
}

impl<T, Base, Dt, Pow> Quantity<T, Base, Dt, Pow>
where
    T: ops::Mul<Output = T> + Copy,
    Dt: DtTrait,
    Pow: PowTrait,
{
    pub fn squared(self) -> Quantity<T, Base, Dt::Squared, Pow::Squared> {
        Quantity(self.0 * self.0, PhantomData)
    }
}

impl<T, Base, Dt, Pow> ops::Mul for Quantity<T, Base, Dt, Pow>
where
    T: ops::Mul<Output = T>,
    Dt: DtTrait,
    Pow: PowTrait,
{
    type Output = Quantity<T, Base, Dt::Squared, Pow::Squared>;

    fn mul(self, other: Self) -> Self::Output { Quantity(self.0 * other.0, PhantomData) }
}

impl<Base, Dt, Pow> Quantity<f32, Base, Dt, Pow> {
    #[must_use]
    pub fn is_positive(self) -> bool { self.0 > 0. }

    #[must_use]
    pub fn is_negative(self) -> bool { self.0 < 0. }

    #[must_use]
    pub fn is_zero(self) -> bool { self.0 == 0. }

    #[must_use]
    pub fn sign(self) -> Sign {
        if self.0 == 0. {
            Sign::Zero
        } else if self.0 < 0. {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }

    #[must_use]
    pub fn abs(self) -> Self { Self(self.0.abs(), PhantomData) }

    #[must_use]
    pub fn copysign(self, other: Self) -> Self { Self(self.0.copysign(other.0), PhantomData) }

    #[must_use]
    pub fn signum(self) -> f32 { self.0.signum() }

    /// Inverse lerp function.
    #[must_use]
    pub fn ratio_between(self, start: Self, end: Self) -> f32 { (self - start).0 / (end - start).0 }

    #[must_use]
    pub fn min(self, other: Self) -> Self { Self(self.0.min(other.0), PhantomData) }

    #[must_use]
    pub fn max(self, other: Self) -> Self { Self(self.0.max(other.0), PhantomData) }

    #[must_use]
    pub fn clamp(self, min: Self, max: Self) -> Self {
        Self(self.0.clamp(min.0, max.0), PhantomData)
    }

    #[must_use]
    pub fn with_heading(self, heading: Heading) -> <Self as QuantityTrait>::WithRaw<Vec2> {
        Quantity(heading.into_dir2() * self.0, PhantomData)
    }

    #[must_use]
    pub fn midpoint(self, other: Self) -> Self { Self(self.0.midpoint(other.0), PhantomData) }

    #[must_use]
    pub const fn splat2(self) -> <Self as QuantityTrait>::WithRaw<Vec2> {
        Quantity(Vec2::new(self.0, self.0), PhantomData)
    }
}

impl<Dt, Pow> Quantity<f32, LengthBase, Dt, Pow> {
    #[must_use]
    pub fn atan2(self, x: Self) -> Angle { Angle::from_raw(self.0.atan2(x.0)) }
}

impl Length<f32> {
    /// Computes the arc length given an arc length (the receiver) and an angular quantity (the
    /// parameter).
    ///
    /// Given a parameter in rad/s^n for some n, this returns a distance quantity in nm/s^n.
    #[must_use]
    pub fn radius_to_arc<Dt>(
        self,
        angular: Quantity<f32, AngleBase, Dt, PowOne>,
    ) -> Quantity<f32, LengthBase, Dt, PowOne> {
        Quantity::new(self.0 * angular.0)
    }
}

impl<Dt> Quantity<f32, LengthBase, Dt, PowOne> {
    /// Computes the radius of a circle given an arc length (the receiver) and an angular quantity
    /// (the parameter).
    ///
    /// The receiver and parameter must have the same temporal dimensoin.
    #[must_use]
    pub fn arc_to_radius(self, angular: Quantity<f32, AngleBase, Dt, PowOne>) -> Length<f32> {
        Length::new(self.0 / angular.0)
    }
}

impl<Dt> ops::Mul<Dir2> for Quantity<f32, LengthBase, Dt, PowOne> {
    type Output = Quantity<Vec2, LengthBase, Dt, PowOne>;

    fn mul(self, other: Dir2) -> Quantity<Vec2, LengthBase, Dt, PowOne> {
        Quantity(other * self.0, PhantomData)
    }
}

impl<Dt> ops::Mul<Vec2> for Quantity<f32, LengthBase, Dt, PowOne> {
    type Output = Quantity<Vec2, LengthBase, Dt, PowOne>;

    fn mul(self, other: Vec2) -> Quantity<Vec2, LengthBase, Dt, PowOne> {
        Quantity(other * self.0, PhantomData)
    }
}

impl<Dt> ops::Mul<Heading> for Quantity<f32, LengthBase, Dt, PowOne> {
    type Output = Quantity<Vec2, LengthBase, Dt, PowOne>;

    fn mul(self, other: Heading) -> Quantity<Vec2, LengthBase, Dt, PowOne> {
        self * other.into_dir2()
    }
}

impl<Dt> ops::Mul<Dir3> for Quantity<f32, LengthBase, Dt, PowOne> {
    type Output = Quantity<Vec3, LengthBase, Dt, PowOne>;

    fn mul(self, other: Dir3) -> Quantity<Vec3, LengthBase, Dt, PowOne> {
        Quantity(other * self.0, PhantomData)
    }
}

impl<Dt, Pow> Quantity<Vec2, LengthBase, Dt, Pow>
where
    Dt: DtTrait,
    Pow: PowTrait,
{
    #[must_use]
    pub fn x(self) -> Quantity<f32, LengthBase, Dt, Pow> { Quantity(self.0.x, PhantomData) }

    #[must_use]
    pub fn y(self) -> Quantity<f32, LengthBase, Dt, Pow> { Quantity(self.0.y, PhantomData) }

    #[must_use]
    pub fn with_x(self, x: Quantity<f32, LengthBase, Dt, Pow>) -> Self {
        Self(self.0.with_x(x.0), PhantomData)
    }

    #[must_use]
    pub fn with_y(self, y: Quantity<f32, LengthBase, Dt, Pow>) -> Self {
        Self(self.0.with_y(y.0), PhantomData)
    }

    #[must_use]
    pub const fn horizontally(self) -> Quantity<Vec3, LengthBase, Dt, Pow> {
        Quantity(Vec3::new(self.0.x, self.0.y, 0.), PhantomData)
    }

    #[must_use]
    pub const fn with_vertical(
        self,
        vertical: Quantity<f32, LengthBase, Dt, Pow>,
    ) -> Quantity<Vec3, LengthBase, Dt, Pow> {
        Quantity(Vec3::new(self.0.x, self.0.y, vertical.0), PhantomData)
    }

    #[must_use]
    pub fn heading(self) -> Heading { Heading::from_vec2(self.0) }

    #[must_use]
    pub fn normalize_to_magnitude(self, magnitude: Quantity<f32, LengthBase, Dt, Pow>) -> Self {
        Self(self.0.normalize_or_zero() * magnitude.0, PhantomData)
    }

    /// Returns a `Vec3` such that
    /// the horizontal projection of the result is equal to `self`.
    #[must_use]
    pub fn projected_from_elevation_angle(
        self,
        angle: Angle,
    ) -> Quantity<Vec3, LengthBase, Dt, Pow> {
        self.with_vertical(self.magnitude_exact() * angle.acute_signed_tan())
    }

    /// Rotates the `horizontally()` of this vector upwards by `angle`.
    /// The result has the same magnitude as `self`.
    #[must_use]
    pub fn rotate_with_elevation_angle(self, angle: Angle) -> Quantity<Vec3, LengthBase, Dt, Pow> {
        let horizontal = self * angle.cos();
        let vertical = self.magnitude_exact() * angle.sin();
        horizontal.with_vertical(vertical)
    }

    /// Returns the vector component projected along `dir`.
    #[must_use]
    pub fn project_onto_dir(self, dir: Dir2) -> Quantity<f32, LengthBase, Dt, Pow> {
        Quantity(self.0.dot(*dir), PhantomData)
    }

    #[must_use]
    pub fn midpoint(self, other: Self) -> Self {
        Self(Vec2::new(self.0.x.midpoint(other.0.x), self.0.y.midpoint(other.0.y)), PhantomData)
    }

    #[must_use]
    pub fn rotate_right_angle_counterclockwise(self) -> Self {
        Self(Vec2::new(-self.0.y, self.0.x), PhantomData)
    }

    #[must_use]
    pub fn rotate_right_angle_clockwise(self) -> Self {
        Self(Vec2::new(self.0.y, -self.0.x), PhantomData)
    }
}

impl<Dt, Pow> From<(Quantity<f32, LengthBase, Dt, Pow>, Quantity<f32, LengthBase, Dt, Pow>)>
    for Quantity<Vec2, LengthBase, Dt, Pow>
{
    fn from(
        (x, y): (Quantity<f32, LengthBase, Dt, Pow>, Quantity<f32, LengthBase, Dt, Pow>),
    ) -> Self {
        Self(Vec2 { x: x.0, y: y.0 }, PhantomData)
    }
}

impl<Dt, Pow>
    From<(
        Quantity<f32, LengthBase, Dt, Pow>,
        Quantity<f32, LengthBase, Dt, Pow>,
        Quantity<f32, LengthBase, Dt, Pow>,
    )> for Quantity<Vec3, LengthBase, Dt, Pow>
{
    fn from(
        (x, y, z): (
            Quantity<f32, LengthBase, Dt, Pow>,
            Quantity<f32, LengthBase, Dt, Pow>,
            Quantity<f32, LengthBase, Dt, Pow>,
        ),
    ) -> Self {
        Self(Vec3 { x: x.0, y: y.0, z: z.0 }, PhantomData)
    }
}

impl<Dt, Pow> Quantity<Vec3, LengthBase, Dt, Pow>
where
    Dt: DtTrait,
    Pow: PowTrait,
{
    #[must_use]
    pub fn x(self) -> Quantity<f32, LengthBase, Dt, Pow> { Quantity(self.0.x, PhantomData) }

    #[must_use]
    pub fn y(self) -> Quantity<f32, LengthBase, Dt, Pow> { Quantity(self.0.y, PhantomData) }

    /// Returns the horizontal projection of this vector.
    #[must_use]
    pub fn horizontal(self) -> Quantity<Vec2, LengthBase, Dt, Pow> {
        Quantity(self.0.xy(), PhantomData)
    }

    #[must_use]
    pub fn vertical(self) -> Quantity<f32, LengthBase, Dt, Pow> { Quantity(self.0.z, PhantomData) }

    pub fn set_horizontal(&mut self, value: Quantity<Vec2, LengthBase, Dt, Pow>) {
        self.0.x = value.0.x;
        self.0.y = value.0.y;
    }

    pub fn set_vertical(&mut self, value: Quantity<f32, LengthBase, Dt, Pow>) {
        self.0.z = value.0;
    }

    #[must_use]
    pub fn normalize_to_magnitude(self, magnitude: Quantity<f32, LengthBase, Dt, Pow>) -> Self {
        Self(self.0.normalize_or_zero() * magnitude.0, PhantomData)
    }

    #[must_use]
    pub fn normalize_by_vertical(
        self,
        desired_vertical: Quantity<f32, LengthBase, Dt, Pow>,
    ) -> Self {
        Self(self.0 * (desired_vertical / self.vertical()), PhantomData)
    }

    #[must_use]
    pub fn midpoint(self, other: Self) -> Self {
        Self(
            Vec3::new(
                self.0.x.midpoint(other.0.x),
                self.0.y.midpoint(other.0.y),
                self.0.z.midpoint(other.0.z),
            ),
            PhantomData,
        )
    }
}

pub struct LengthBase;

/// A distance quantity. Internal representation is in nautical miles.
pub type Length<T> = Quantity<T, LengthBase, DtZero, PowOne>;

/// A linear speed (rate of [length](Length) change) quantity.
pub type Speed<T> = Quantity<T, LengthBase, DtOne, PowOne>;

/// A linear acceleration (rate of linear [speed](Speed) change) quantity.
pub type Accel<T> = Quantity<T, LengthBase, DtTwo, PowOne>;

/// Rate of linear [acceleration](Accel) change.
pub type AccelRate<T> = Quantity<T, LengthBase, Ddt<DtTwo>, PowOne>;

pub struct AngleBase;

/// A relative angle. Internal representation is in radians.
pub type Angle = Quantity<f32, AngleBase, DtZero, PowOne>;

/// An angular speed (rate of [angle](Angle) change) quantity.
/// Always in rad/s.
pub type AngularSpeed = Quantity<f32, AngleBase, DtOne, PowOne>;

/// An angular acceleration (rate of [angular speed](AngularSpeed) change) quantity.
pub type AngularAccel = Quantity<f32, AngleBase, DtTwo, PowOne>;

pub struct RatioBase;

pub type Frequency = Quantity<f32, RatioBase, DtOne, PowZero>;

impl fmt::Debug for Length<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Length")
            .field("nm", &self.into_nm())
            .field("feet", &self.into_feet())
            .finish()
    }
}

impl fmt::Debug for Length<Vec2> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Length")
            .field("x.nm", &self.x().into_nm())
            .field("y.nm", &self.y().into_nm())
            .finish()
    }
}

impl fmt::Debug for Length<Vec3> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Length")
            .field("x.nm", &self.x().into_nm())
            .field("y.nm", &self.y().into_nm())
            .field("vertical.feet", &self.vertical().into_feet())
            .finish()
    }
}

impl fmt::Debug for Speed<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Speed")
            .field("knots", &self.into_knots())
            .field("fpm", &self.into_fpm())
            .finish()
    }
}

impl fmt::Debug for Speed<Vec2> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Speed")
            .field("x.knots", &self.x().into_knots())
            .field("y.knots", &self.y().into_knots())
            .finish()
    }
}

impl fmt::Debug for Speed<Vec3> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Speed")
            .field("x.knots", &self.x().into_knots())
            .field("y.knots", &self.y().into_knots())
            .field("vertical.fpm", &self.vertical().into_fpm())
            .finish()
    }
}

impl fmt::Debug for Accel<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Accel").field("knots/s", &self.into_knots_per_sec()).finish()
    }
}

impl fmt::Debug for Angle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Angle").field("degrees", &self.into_degrees()).finish()
    }
}

impl fmt::Debug for AngularSpeed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AngularSpeed").field("degrees/s", &self.into_degrees_per_sec()).finish()
    }
}

impl fmt::Debug for AngularAccel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AngularAccel").field("degrees/s2", &self.into_degrees_per_sec2()).finish()
    }
}

impl Length<f32> {
    #[must_use]
    pub const fn into_nm(self) -> f32 { self.0 }

    #[must_use]
    pub const fn from_nm(nm: f32) -> Self { Self(nm, PhantomData) }

    #[must_use]
    pub const fn into_feet(self) -> f32 { self.0 * FEET_PER_NM }

    #[must_use]
    pub const fn from_feet(feet: f32) -> Self { Self(feet / FEET_PER_NM, PhantomData) }

    #[must_use]
    pub const fn into_miles(self) -> f32 { self.0 * MILES_PER_NM }

    #[must_use]
    pub const fn from_miles(mile: f32) -> Self { Self(mile / MILES_PER_NM, PhantomData) }

    #[must_use]
    pub const fn into_meters(self) -> f32 { self.0 * METERS_PER_NM }

    #[must_use]
    pub const fn from_meters(meters: f32) -> Self { Self(meters / METERS_PER_NM, PhantomData) }

    #[must_use]
    pub const fn into_km(self) -> f32 { self.0 * (METERS_PER_NM / 1000.) }

    #[must_use]
    pub const fn from_km(meters: f32) -> Self {
        Self(meters / (METERS_PER_NM / 1000.), PhantomData)
    }
}

impl Length<Vec2> {
    #[must_use]
    pub const fn into_nm(self) -> Vec2 { self.0 }

    #[must_use]
    pub const fn vec2_from_nm(nm: Vec2) -> Self { Self(nm, PhantomData) }

    #[must_use]
    pub fn into_feet(self) -> Vec2 { self.0 * FEET_PER_NM }

    #[must_use]
    pub fn vec2_from_feet(feet: Vec2) -> Self { Self(feet / FEET_PER_NM, PhantomData) }

    #[must_use]
    pub fn into_mile(self) -> Vec2 { self.0 * MILES_PER_NM }

    #[must_use]
    pub fn vec2_from_mile(mile: Vec2) -> Self { Self(mile / MILES_PER_NM, PhantomData) }

    #[must_use]
    pub fn into_meters(self) -> Vec2 { self.0 * METERS_PER_NM }

    #[must_use]
    pub fn vec2_from_meters(meters: Vec2) -> Self { Self(meters / METERS_PER_NM, PhantomData) }

    #[must_use]
    pub fn into_km(self) -> Vec2 { self.0 * (METERS_PER_NM / 1000.) }

    #[must_use]
    pub fn vec2_from_km(meters: Vec2) -> Self {
        Self(meters / (METERS_PER_NM / 1000.), PhantomData)
    }
}

impl Speed<f32> {
    #[must_use]
    pub const fn into_knots(self) -> f32 { self.0 * SECONDS_PER_HOUR }

    #[must_use]
    pub const fn from_knots(knots: f32) -> Self { Self(knots / SECONDS_PER_HOUR, PhantomData) }

    #[must_use]
    pub const fn into_kmh(self) -> f32 { self.0 * (SECONDS_PER_HOUR * METERS_PER_NM / 1000.) }

    #[must_use]
    pub const fn from_kmh(kmh: f32) -> Self {
        Self(kmh / (SECONDS_PER_HOUR * METERS_PER_NM / 1000.), PhantomData)
    }

    #[must_use]
    pub const fn into_mph(self) -> f32 { self.0 * (SECONDS_PER_HOUR * MILES_PER_NM / 1000.) }

    #[must_use]
    pub const fn from_mph(mph: f32) -> Self {
        Self(mph / (SECONDS_PER_HOUR * MILES_PER_NM / 1000.), PhantomData)
    }

    #[must_use]
    pub const fn into_meter_per_sec(self) -> f32 { self.0 * METERS_PER_NM }

    #[must_use]
    pub const fn from_meter_per_sec(mps: f32) -> Self { Self(mps / METERS_PER_NM, PhantomData) }

    #[must_use]
    pub const fn into_fpm(self) -> f32 { self.0 * (SECONDS_PER_MINUTE * FEET_PER_NM) }

    #[must_use]
    pub const fn from_fpm(fpm: f32) -> Self {
        Self(fpm / (SECONDS_PER_MINUTE * FEET_PER_NM), PhantomData)
    }
}

impl<T: ops::Mul<f32, Output = T> + ops::Div<f32, Output = T>> Accel<T> {
    #[must_use]
    pub fn into_knots_per_sec(self) -> T { self.0 * SECONDS_PER_HOUR }

    #[must_use]
    pub fn from_knots_per_sec(knots: T) -> Self { Self(knots / SECONDS_PER_HOUR, PhantomData) }

    #[must_use]
    pub fn into_fpm_per_sec(self) -> T { self.0 * SECONDS_PER_MINUTE * FEET_PER_NM }

    #[must_use]
    pub fn from_fpm_per_sec(fpm: T) -> Self {
        Self(fpm / SECONDS_PER_MINUTE / FEET_PER_NM, PhantomData)
    }
}

impl<T: ops::Mul<f32, Output = T> + ops::Div<f32, Output = T>> AccelRate<T> {
    #[must_use]
    pub fn into_knots_per_sec2(self) -> T { self.0 * SECONDS_PER_HOUR }

    #[must_use]
    pub fn from_knots_per_sec2(knots: T) -> Self { Self(knots / SECONDS_PER_HOUR, PhantomData) }
}

impl Angle {
    pub const RIGHT: Self = Self(FRAC_PI_2, PhantomData);
    pub const STRAIGHT: Self = Self(PI, PhantomData);
    pub const FULL: Self = Self(TAU, PhantomData);

    #[must_use]
    pub const fn from_radians(radians: f32) -> Self { Self(radians, PhantomData) }

    #[must_use]
    pub const fn into_radians(self) -> f32 { self.0 }

    #[must_use]
    pub const fn from_degrees(degrees: f32) -> Self { Self(degrees.to_radians(), PhantomData) }

    #[must_use]
    pub fn into_degrees(self) -> f32 { self.0.to_degrees() }

    #[must_use]
    pub fn sin(self) -> f32 { self.0.sin() }
    #[must_use]
    pub fn cos(self) -> f32 { self.0.cos() }

    /// Returns the slope of a line whose angle of elevation is the receiver value.
    ///
    /// This function clamps the angle between `-Angle::RIGHT..=Angle::RIGHT`,
    /// and defines the following special cases:
    /// - The tangent of `-Angle::RIGHT` (line downwards) is negative infinity.
    /// - The tangent of `Angle::RIGHT` (line upwards) is positive infinity.
    ///
    /// This function is monotonic, and is strictly monotonic within the clamped closed range.
    #[must_use]
    pub fn acute_signed_tan(self) -> f32 {
        if self <= -Self::RIGHT {
            f32::NEG_INFINITY
        } else if self >= Self::RIGHT {
            f32::INFINITY
        } else {
            self.0.tan()
        }
    }
}

impl AngularSpeed {
    #[must_use]
    pub fn into_degrees_per_sec(self) -> f32 { self.0.to_degrees() }

    #[must_use]
    pub fn from_degrees_per_sec(degrees: f32) -> Self { Self(degrees.to_radians(), PhantomData) }

    /// Reciprocal of this value, converting `rad/s` to `s/rad`.
    #[must_use]
    pub fn duration_per_radian(self) -> Duration { Duration::from_secs_f32(self.0.recip()) }

    #[must_use]
    pub fn into_radians_per_sec(self) -> Frequency { Frequency::from_raw(self.0) }
}

impl AngularAccel {
    #[must_use]
    pub fn into_degrees_per_sec2(self) -> f32 { self.0.to_degrees() }

    #[must_use]
    pub fn from_degrees_per_sec2(degrees: f32) -> Self { Self(degrees.to_radians(), PhantomData) }
}

pub trait IsFinite: Copy {
    fn is_finite(self) -> bool;
}

impl IsFinite for f32 {
    fn is_finite(self) -> bool { f32::is_finite(self) }
}

impl IsFinite for Vec2 {
    fn is_finite(self) -> bool { Vec2::is_finite(self) }
}

impl IsFinite for Vec3 {
    fn is_finite(self) -> bool { Vec3::is_finite(self) }
}

impl<T, Base, Dt, Pow> serde::Serialize for Quantity<T, Base, Dt, Pow>
where
    T: serde::Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de, T, Base, Dt, Pow> serde::Deserialize<'de> for Quantity<T, Base, Dt, Pow>
where
    T: serde::Deserialize<'de> + IsFinite,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::de::Deserializer<'de>,
    {
        let value = T::deserialize(deserializer)?;

        if !value.is_finite() {
            return Err(<D::Error as serde::de::Error>::custom("non-finite quantity"));
        }

        Ok(Self(value, PhantomData))
    }
}

bevy_mod_config::impl_scalar_config_field!(
    Length<f32>,
    QuantityMetadataWithUnit<Length<f32>, LengthUnit>,
    |metadata: &LengthMetadata| metadata.default,
    'a => Length<f32>,
    |&value: &Length<f32>| value,
);

#[derive(Clone)]
pub struct QuantityMetadataWithUnit<T, U> {
    pub default:   T,
    pub unit:      U,
    pub min:       T,
    pub max:       T,
    pub precision: Option<T>,
}

pub type LengthMetadata = QuantityMetadataWithUnit<Length<f32>, LengthUnit>;
impl Default for LengthMetadata {
    fn default() -> Self {
        Self {
            default:   Length::from_nm(1.0),
            unit:      LengthUnit::NauticalMiles,
            min:       Length::from_nm(0.0),
            max:       Length::from_nm(100.0),
            precision: None,
        }
    }
}

bevy_mod_config::impl_scalar_config_field!(
    Speed<f32>,
    SpeedMetadata,
    |metadata: &SpeedMetadata| metadata.default,
    'a => Speed<f32>,
    |&value: &Speed<f32>| value,
);

pub type SpeedMetadata = QuantityMetadataWithUnit<Speed<f32>, SpeedUnit>;
impl Default for QuantityMetadataWithUnit<Speed<f32>, SpeedUnit> {
    fn default() -> Self {
        Self {
            default:   Speed::from_knots(200.0),
            unit:      SpeedUnit::Knots,
            min:       Speed::from_knots(0.0),
            max:       Speed::from_knots(500.0),
            precision: Some(Speed::from_knots(1.0)),
        }
    }
}

bevy_mod_config::impl_scalar_config_field!(
    Angle,
    AngleMetadata,
    |metadata: &AngleMetadata| metadata.default,
    'a => Angle,
    |&value: &Angle| value,
);

#[derive(Clone)]
pub struct AngleMetadata {
    pub default: Angle,
    pub min:     Angle,
    pub max:     Angle,
}

impl Default for AngleMetadata {
    fn default() -> Self { Self { default: Angle::ZERO, min: Angle::ZERO, max: Angle::RIGHT } }
}

#[cfg(feature = "egui")]
mod egui_impl;
