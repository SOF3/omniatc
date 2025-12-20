//! Algorithms and constants related to aviation physics.

use std::ops;

use bevy_math::{Dir2, Vec2};

use crate::units::Position;
use crate::{Accel, CanSqrt, FEET_PER_NM, Length, Pressure, Speed, Temp, TempDelta};

#[cfg(test)]
mod tests;

/// Altitude of the tropopause.
pub const TROPOPAUSE_ALTITUDE: Position<f32> = Position::new(36089.24 / FEET_PER_NM);

/// Standard pressure at sea level pressure (QNH).
pub const ISA_SEA_LEVEL_PRESSURE: Pressure = Pressure::new(101325.0);

/// Standard sea level temperature.
pub const ISA_SEA_LEVEL_TEMPERATURE: Temp = Temp::from_kelvins(288.15);

pub const ISA_SEA_LEVEL_AIR_DENSITY: f32 =
    air_density(ISA_SEA_LEVEL_PRESSURE, ISA_SEA_LEVEL_TEMPERATURE);

/// Standard pressure at tropopause.
//
// The value needs to be hardcoded because powf is not const yet
pub const ISA_TROPOPAUSE_PRESSURE: Pressure = Pressure::new(22632.1);

/// Standard temperature at tropopause.
pub const ISA_TROPOPAUSE_TEMPERATURE: Temp = Temp::from_kelvins(
    ISA_SEA_LEVEL_TEMPERATURE.into_kelvins()
        - isa_temp_lapse(TROPOPAUSE_ALTITUDE.amsl()).into_kelvins(),
);

/// Standard temperature lapse rate, in K/m.
///
/// Consider using [`isa_temperature_lapse`] instead for better dimensional safety.
pub const ISA_LAPSE_RATE: f32 = 6.5e-3;

/// Computes the ISA temperature change over a given distance.
///
/// The input and output have the same sign.
/// That is, if `distance` is positive, the output is positive,
/// indicating the temperature increase when altitude decreases by `distance`.
#[must_use]
pub const fn isa_temp_lapse(distance: Length<f32>) -> TempDelta {
    TempDelta::new(ISA_LAPSE_RATE * distance.into_meters())
}

/// Specific gas constant for dry air, in SI unit (J/kg/K).
pub const DRY_AIR_GAS_CONSTANT: f32 = 287.052874;

#[must_use]
pub const fn air_density(pressure: Pressure, temp: Temp) -> f32 {
    pressure.into_pascals() / (DRY_AIR_GAS_CONSTANT * temp.into_kelvins())
}

/// Standard gravity at Earth's surface.
pub const EARTH_SURFACE_GRAVITY: Accel<f32> = Accel::from_meters_per_sec2(9.80665);

/// g0 / R.
pub const G_OVER_R: f32 = EARTH_SURFACE_GRAVITY.into_meters_per_sec2() / DRY_AIR_GAS_CONSTANT;

/// g0 / RL, in SI units, used as the exponent in the barometric formula in the troposphere.
pub const GRL_EXPONENT: f32 = G_OVER_R / ISA_LAPSE_RATE;

#[must_use]
pub fn solve_expected_ground_speed(
    true_airspeed: Speed<f32>,
    wind: Speed<Vec2>,
    ground_dir: Dir2,
) -> Speed<f32> {
    let wind_dot_ground = wind.x() * ground_dir.x + wind.y() * ground_dir.y;
    wind_dot_ground
        + (true_airspeed.squared() - wind.magnitude_squared() - wind_dot_ground.squared()).sqrt()
}

/// Computes the barometrics at an airborne position.
pub struct Barometrics {
    /// Atmospheric pressure at the given true altitude.
    pub pressure:          Pressure,
    /// Indicated pressure altitude at the given true altitude.
    pub pressure_altitude: Position<f32>,
    /// Outside temperature at the given true altitude.
    pub temp:              Temp,
    /// Air density in kg/m^3.
    pub air_density:       f32,
    /// Ratio of TAS/IAS at the given true altitude.
    /// Equal to `sqrt(sea_level_air_density / air_density)`.
    pub tas_ias_ratio:     f32,
}

impl Barometrics {
    /// Computes the true airspeed from indicated airspeed.
    #[must_use]
    pub fn true_airspeed<T>(&self, indicated_airspeed: Speed<T>) -> Speed<T>
    where
        T: ops::Mul<f32, Output = T>,
    {
        indicated_airspeed * self.tas_ias_ratio
    }

    /// Computes the indicated airspeed from true airspeed.
    #[must_use]
    pub fn indicated_airspeed<T>(&self, true_airspeed: Speed<T>) -> Speed<T>
    where
        T: ops::Div<f32, Output = T>,
    {
        true_airspeed / self.tas_ias_ratio
    }
}

#[must_use]
pub fn compute_barometric(
    true_altitude: Position<f32>,
    sea_level_pressure: Pressure,
    sea_level_temp: Temp,
) -> Barometrics {
    let temp;
    let pressure;
    let pressure_altitude;

    if true_altitude <= TROPOPAUSE_ALTITUDE {
        temp = sea_level_temp - isa_temp_lapse(true_altitude.amsl());
        pressure = sea_level_pressure
            * (temp.from_abs_zero() / sea_level_temp.from_abs_zero()).powf(GRL_EXPONENT);

        pressure_altitude = Position::SEA_LEVEL
            + Length::from_meters(
                ISA_SEA_LEVEL_TEMPERATURE.into_kelvins() / ISA_LAPSE_RATE
                    * (1.0 - (pressure / ISA_SEA_LEVEL_PRESSURE).powf(1.0 / GRL_EXPONENT)),
            );
    } else {
        temp = sea_level_temp - isa_temp_lapse(TROPOPAUSE_ALTITUDE.amsl());
        let true_tropopause_pressure = sea_level_pressure
            * (temp.from_abs_zero() / sea_level_temp.from_abs_zero()).powf(GRL_EXPONENT);
        pressure = true_tropopause_pressure
            * (-G_OVER_R * (true_altitude - TROPOPAUSE_ALTITUDE).into_meters()
                / temp.into_kelvins())
            .exp();

        pressure_altitude = TROPOPAUSE_ALTITUDE
            + Length::from_meters(
                ISA_TROPOPAUSE_TEMPERATURE.into_kelvins() / G_OVER_R
                    * (ISA_TROPOPAUSE_PRESSURE / pressure).ln(),
            );
    }

    let air_density = air_density(pressure, temp);
    let tas_ias_ratio = (ISA_SEA_LEVEL_AIR_DENSITY / air_density).sqrt();

    Barometrics { pressure, pressure_altitude, temp, air_density, tas_ias_ratio }
}
