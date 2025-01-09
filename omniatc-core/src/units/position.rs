use std::ops;

use bevy::math::{NormedVectorSpace, Vec2, Vec3, VectorSpace};

use super::{Distance, Squared};
use crate::math::SEA_ALTITUDE;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Position<T>(pub Distance<T>);

impl<T> Position<T> {
    pub const fn new(value: T) -> Self { Position(Distance(value)) }

    pub fn get(self) -> T { self.0 .0 }
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
    pub fn distance_cmp(self, other: Self) -> impl PartialOrd<Distance<f32>> {
        (self - other).magnitude_cmp()
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
}

impl Position<Vec3> {
    #[must_use]
    pub fn x(self) -> Position<f32> { Position(self.0.x()) }
    #[must_use]
    pub fn y(self) -> Position<f32> { Position(self.0.y()) }

    #[must_use]
    pub fn horizontal(self) -> Position<Vec2> { Position(self.0.horizontal()) }
    #[must_use]
    pub fn vertical(self) -> Position<f32> { Position(self.0.vertical()) }
}
