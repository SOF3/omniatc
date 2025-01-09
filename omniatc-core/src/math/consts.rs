use crate::units::Position;

/// Converts nautical miles to feet.
pub const FEET_PER_NM: f32 = 6076.12;
/// Converts nautical miles to feet.
pub const MILE_PER_NM: f32 = 1.15078;
/// Converts nautical miles to meter.
pub const METER_PER_NM: f32 = 1852.;
/// Converts speed of sound to knots.
pub const KT_PER_MACH: f32 = 666.739;

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
