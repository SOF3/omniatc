//! Algorithms and constants related to aviation physics.

use bevy_math::{Dir2, Vec2};

use crate::units::Position;
use crate::{Speed, FEET_PER_NM};

/// Altitude of mean sea level.
pub const SEA_ALTITUDE: Position<f32> = Position::new(0.);

/// Altitude of the tropopause.
pub const TROPOPAUSE_ALTITUDE: Position<f32> = Position::new(36089.24 / FEET_PER_NM);

/// Standard sea level temperature in K, used to calculate density altitude.
pub const STANDARD_SEA_LEVEL_TEMPERATURE: f32 = 288.15;
/// Standard lapse rate of temperature, in K/ft.
pub const STANDARD_LAPSE_RATE: f32 = 0.0019812 * FEET_PER_NM;
/// Proportional increase of true airspeed per nm above sea level.
/// Equivalent to 2% per 1000ft.
pub const TAS_DELTA_PER_NM: f32 = 0.02e-3 * FEET_PER_NM;
/// I don't know what this constant even means... see <http://www.edwilliams.org/avform147.htm>.
pub const PRESSURE_DENSITY_ALTITUDE_POW: f32 = 0.2349690;

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
