use std::marker::PhantomData;
use std::{fmt, ops};

use crate::{AssertApproxError, Quantity};

pub struct TempBase;

/// Difference in temperature.
///
/// Always in K (which is equivalent to celsius in terms of deltas).
pub type TempDelta = Quantity<f32, TempBase, super::Dt0, super::Pow1>;

impl TempDelta {
    #[must_use]
    pub const fn from_kelvins(kelvins: f32) -> Self { Self(kelvins, PhantomData) }

    #[must_use]
    pub const fn into_kelvins(self) -> f32 { self.0 }
}

impl fmt::Debug for TempDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Temp").field("kelvins", &self.0).finish()
    }
}

/// Absolute temperature value.
#[derive(Clone, Copy, PartialEq, PartialOrd, serde::Serialize, serde::Deserialize)]
pub struct Temp(pub TempDelta);

impl Temp {
    /// Absolute zero, equivalent to `from_celsius(-273.15)`.
    pub const ABSOLUTE_ZERO: Self = Self(TempDelta::new(0.0));

    /// Freezing point of water, equivalent to `from_celsius(0.0)`.
    pub const WATER_FREEZING: Self = Self(TempDelta::new(273.15));

    #[must_use]
    pub const fn from_kelvins(kelvins: f32) -> Self { Self(TempDelta::from_kelvins(kelvins)) }

    #[must_use]
    pub const fn into_kelvins(self) -> f32 { self.0.0 }

    /// Creates an absolute temperature from an Celsius value.
    #[must_use]
    pub const fn from_celsius(celsius: f32) -> Self {
        Self::from_kelvins(celsius + Self::WATER_FREEZING.0.0)
    }

    /// Converts an absolute Temperature into a Celsius value.
    #[must_use]
    pub const fn into_celsius(self) -> f32 { self.0.0 - Self::WATER_FREEZING.0.0 }

    /// Creates an absolute Temperature from a Fahrenheit value.
    ///
    /// This type is only intended for absolute values, not relative differences.
    #[must_use]
    pub const fn from_fahrenheit(fahrenheit: f32) -> Self {
        Self::from_celsius((fahrenheit - 32.0) / 1.8)
    }

    /// Converts an absolute Temperature into a Fahrenheit value.
    ///
    /// This type is only intended for absolute values, not relative differences.
    #[must_use]
    pub const fn into_fahrenheit(self) -> f32 { self.into_celsius() * 1.8 + 32.0 }

    /// Difference from absolute zero temperature.
    #[must_use]
    pub const fn from_abs_zero(self) -> TempDelta { self.0 }

    /// Asserts that the quantity is within `epsilon` of `other`.
    ///
    /// # Errors
    /// If the absolute difference between `self` and `other` is greater than `epsilon`.
    pub fn assert_approx(
        self,
        other: Temp,
        epsilon: TempDelta,
    ) -> Result<(), AssertApproxError<Self, TempDelta>> {
        if (self - other).abs() > epsilon {
            Err(AssertApproxError { actual: self, expect: other, epsilon })
        } else {
            Ok(())
        }
    }
}

#[cfg(feature = "schema")]
impl schemars::JsonSchema for Temp {
    fn schema_name() -> std::borrow::Cow<'static, str> { "Temperature".into() }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        <TempDelta as schemars::JsonSchema>::json_schema(generator)
    }
}

impl fmt::Debug for Temp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Temperature").field("celsius", &self.into_celsius()).finish()
    }
}

impl ops::Add<TempDelta> for Temp {
    type Output = Temp;

    fn add(self, rhs: TempDelta) -> Self::Output {
        Temp(TempDelta::from_kelvins(self.into_kelvins() + rhs.into_kelvins()))
    }
}

impl ops::Sub<TempDelta> for Temp {
    type Output = Temp;

    fn sub(self, rhs: TempDelta) -> Self::Output {
        Temp(TempDelta::from_kelvins(self.into_kelvins() - rhs.into_kelvins()))
    }
}

impl ops::Sub for Temp {
    type Output = TempDelta;

    fn sub(self, rhs: Self) -> Self::Output {
        TempDelta::from_kelvins(self.into_kelvins() - rhs.into_kelvins())
    }
}
