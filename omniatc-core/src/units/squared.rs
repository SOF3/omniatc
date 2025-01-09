use std::{cmp, ops};

use super::Unit;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Squared<U: Unit>(pub U::Value);

impl<U: Unit> Unit for Squared<U> {
    type Value = U::Value;

    fn from_raw(value: Self::Value) -> Self { Self(value) }
    fn into_raw(self) -> Self::Value { self.0 }
}

impl<U: Unit> ops::Add for Squared<U>
where
    U::Value: ops::AddAssign,
{
    type Output = Self;

    fn add(mut self, other: Self) -> Self {
        self.0 += other.0;
        self
    }
}

impl<U: Unit> ops::AddAssign for Squared<U>
where
    U::Value: ops::AddAssign,
{
    fn add_assign(&mut self, other: Self) { self.0 += other.0; }
}

impl<U: Unit> ops::Sub for Squared<U>
where
    U::Value: ops::SubAssign,
{
    type Output = Self;

    fn sub(mut self, other: Self) -> Self {
        self.0 -= other.0;
        self
    }
}

impl<U: Unit> ops::SubAssign for Squared<U>
where
    U::Value: ops::SubAssign,
{
    fn sub_assign(&mut self, other: Self) { self.0 -= other.0; }
}

impl<U: Unit> ops::Mul<f32> for Squared<U>
where
    U::Value: ops::MulAssign<f32>,
{
    type Output = Self;

    fn mul(mut self, other: f32) -> Self {
        self.0 *= other;
        self
    }
}

impl<U: Unit> ops::Div<f32> for Squared<U>
where
    U::Value: ops::DivAssign<f32>,
{
    type Output = Self;

    fn div(mut self, other: f32) -> Self {
        self.0 /= other;
        self
    }
}

impl<U: Unit> ops::Mul for Squared<U>
where
    U::Value: ops::Mul<Output = U::Value>,
{
    type Output = Squared<Self>;

    fn mul(self, other: Self) -> Squared<Self> { Squared::from_raw(self.0 * other.0) }
}

impl<U: Unit> ops::Neg for Squared<U>
where
    U::Value: ops::Neg<Output = U::Value>,
{
    type Output = Self;

    fn neg(self) -> Self { Self(-self.0) }
}

impl<U: Unit> ops::Div for Squared<U>
where
    U::Value: ops::Div,
{
    type Output = <U::Value as ops::Div>::Output;

    fn div(self, other: Self) -> Self::Output { self.0 / other.0 }
}

impl<U: Unit<Value = f32>> Squared<U> {
    #[must_use]
    pub fn sqrt(self) -> U { U::from_raw(self.0.sqrt()) }

    #[must_use]
    pub fn cmp_sqrt(self) -> impl PartialOrd<U> { SquaredNorm(self.0) }

    #[must_use]
    pub fn squared(self) -> Squared<Self> { self * self }

    #[must_use]
    pub fn is_zero(self) -> bool { self.0 == 0. }

    #[must_use]
    pub fn is_positive(self) -> bool { self.0 > 0. }

    #[must_use]
    pub fn is_negative(self) -> bool { self.0 < 0. }

    #[must_use]
    pub fn abs(self) -> Self { Self(self.0.abs()) }

    #[must_use]
    pub fn signum(self) -> f32 { self.0.signum() }
}

/// A wrapper type for squared distance,
/// used to compare with other distances without the pow2 boilerplate.
pub(super) struct SquaredNorm(pub(super) f32);

impl<U: Unit<Value = f32>> PartialEq<U> for SquaredNorm {
    fn eq(&self, other: &U) -> bool { self.0 == other.into_raw().powi(2) }
}

impl<U: Unit<Value = f32>> PartialOrd<U> for SquaredNorm {
    fn partial_cmp(&self, other: &U) -> Option<cmp::Ordering> {
        self.0.partial_cmp(&other.into_raw().powi(2))
    }
}
