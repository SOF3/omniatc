use std::{fmt, ops};

use bevy::math::{NormedVectorSpace, Vec2, Vec3, VectorSpace};

use super::{Distance, Squared};
use crate::math::SEA_ALTITUDE;

#[derive(Clone, Copy, PartialEq, PartialOrd, serde::Serialize)]
pub struct Position<T>(pub Distance<T>);

impl<'de, T: serde::Deserialize<'de> + super::IsFinite> serde::Deserialize<'de> for Position<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        <Distance<T> as serde::Deserialize<'de>>::deserialize(d).map(Self)
    }
}

impl<T> Position<T> {
    pub const fn new(value: T) -> Self { Position(Distance(value)) }

    pub fn get(self) -> T { self.0 .0 }
}

impl Position<f32> {
    pub const SEA_LEVEL: Self = Self(Distance(0.));

    #[must_use]
    pub fn from_amsl_feet(z: f32) -> Self { Position(Distance::from_feet(z)) }
}

impl Position<Vec2> {
    pub const ORIGIN: Self = Self(Distance(Vec2::new(0., 0.)));

    #[must_use]
    pub fn from_origin_nm(x: f32, y: f32) -> Self { Position(Distance(Vec2 { x, y })) }

    #[must_use]
    pub fn midpoint(self, other: Self) -> Self {
        Self::from_origin_nm(
            self.get().x.midpoint(other.get().x),
            self.get().y.midpoint(other.get().y),
        )
    }
}

impl fmt::Debug for Position<f32> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Position")
            .field("nm", &self.0 .0)
            .field("feet", &self.0.into_feet())
            .finish()
    }
}

impl fmt::Debug for Position<Vec2> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Position").field("x", &self.0 .0.x).field("y", &self.0 .0.y).finish()
    }
}

impl fmt::Debug for Position<Vec3> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Position")
            .field("x.nm", &self.0 .0.x)
            .field("y.nm", &self.0 .0.y)
            .field("z.feet", &self.altitude().amsl().into_feet())
            .finish()
    }
}

impl<T: ops::AddAssign> ops::Add<Distance<T>> for Position<T> {
    type Output = Self;

    fn add(mut self, rhs: Distance<T>) -> Self::Output {
        self.0 += rhs;
        self
    }
}

impl<T: ops::AddAssign> ops::AddAssign<Distance<T>> for Position<T> {
    fn add_assign(&mut self, rhs: Distance<T>) { self.0 += rhs; }
}

impl<T: ops::SubAssign> ops::Sub<Distance<T>> for Position<T> {
    type Output = Self;

    fn sub(mut self, rhs: Distance<T>) -> Self::Output {
        self.0 -= rhs;
        self
    }
}

impl<T: ops::SubAssign> ops::SubAssign<Distance<T>> for Position<T> {
    fn sub_assign(&mut self, rhs: Distance<T>) { self.0 -= rhs; }
}

impl<T: ops::SubAssign> ops::Sub for Position<T> {
    type Output = Distance<T>;

    fn sub(self, rhs: Self) -> Distance<T> { self.0 - rhs.0 }
}

impl<T: VectorSpace> Position<T> {
    #[must_use]
    pub fn lerp(self, other: Self, s: f32) -> Self { Self(self.0.lerp(other.0, s)) }
}

impl<T: ops::SubAssign + NormedVectorSpace> Position<T> {
    /// Returns a wrapper that can be compared with a linear distance quantity.
    pub fn distance_cmp(self, other: Self) -> impl PartialOrd + PartialOrd<Distance<f32>> {
        (self - other).magnitude_cmp()
    }

    /// Converts the distance into a fully-ordered type.
    ///
    /// # Errors
    /// Returns error if the squared distance evaluates to NaN.
    pub fn distance_ord(self, other: Self) -> Result<impl Ord + Copy, ordered_float::FloatIsNan> {
        (self - other).magnitude_ord()
    }

    pub fn distance_squared(self, other: Self) -> Squared<Distance<f32>> {
        (self - other).magnitude_squared()
    }

    pub fn distance_exact(self, other: Self) -> Distance<f32> { (self - other).magnitude_exact() }
}

impl Position<f32> {
    /// Inverse lerp function.
    #[must_use]
    pub fn ratio_between(self, start: Self, end: Self) -> f32 {
        self.0.ratio_between(start.0, end.0)
    }

    #[must_use]
    pub fn min(self, other: Self) -> Self { Self(self.0.min(other.0)) }

    #[must_use]
    pub fn max(self, other: Self) -> Self { Self(self.0.max(other.0)) }

    #[must_use]
    pub fn clamp(self, min: Self, max: Self) -> Self { Self(self.0.clamp(min.0, max.0)) }

    #[must_use]
    pub fn amsl(self) -> Distance<f32> { self - SEA_ALTITUDE }
}

impl Position<Vec2> {
    #[must_use]
    pub fn x(self) -> Position<f32> { Position(self.0.x()) }
    #[must_use]
    pub fn y(self) -> Position<f32> { Position(self.0.y()) }

    #[must_use]
    pub fn with_altitude(self, altitude: Position<f32>) -> Position<Vec3> {
        Position::new((self.get(), altitude.get()).into())
    }
}

impl Position<Vec3> {
    #[must_use]
    pub fn x(self) -> Position<f32> { Position(self.0.x()) }
    #[must_use]
    pub fn y(self) -> Position<f32> { Position(self.0.y()) }

    #[must_use]
    pub fn horizontal(self) -> Position<Vec2> { Position(self.0.horizontal()) }
    #[must_use]
    pub fn altitude(self) -> Position<f32> { Position(self.0.vertical()) }
}
