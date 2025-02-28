use std::fmt;

use crate::math::{FEET_PER_NM, KNOTS_PER_MACH, METERS_PER_NM, MILES_PER_NM};

pub struct Quantity {
    pub value:  f32,
    pub prefix: &'static str,
    pub suffix: &'static str,
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{:.0}{}", self.prefix, self.value, self.suffix)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthUnit {
    NauticalMile,
    FlightLevel,
    FlightLevelMeter,
    Feet,
    Mile,
    Meter,
    Kilometer,
}

impl LengthUnit {
    #[must_use]
    pub fn convert(self, value: f32) -> Quantity {
        match self {
            Self::NauticalMile => Quantity { value, prefix: "", suffix: "nm" },
            Self::FlightLevel => {
                Quantity { value: value * FEET_PER_NM / 100., prefix: "FL", suffix: "" }
            }
            Self::FlightLevelMeter => {
                Quantity { value: value / METERS_PER_NM, prefix: "FL", suffix: "m" }
            }
            Self::Feet => Quantity { value: value * FEET_PER_NM, prefix: "", suffix: "ft" },
            Self::Mile => Quantity { value: value * MILES_PER_NM, prefix: "", suffix: "mi" },
            Self::Meter => Quantity { value: value * METERS_PER_NM, prefix: "", suffix: "m" },
            Self::Kilometer => {
                Quantity { value: value * METERS_PER_NM / 1000., prefix: "", suffix: "km" }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpeedUnit {
    Knot,
    KilometerHour,
    MileHour,
    MeterSecond,
    Mach,
}

impl SpeedUnit {
    #[must_use]
    pub fn convert(self, value: f32) -> Quantity {
        let knots = value * 3600.;

        match self {
            Self::Knot => Quantity { value: knots, prefix: "", suffix: "kt" },
            Self::KilometerHour => {
                Quantity { value: knots * METERS_PER_NM / 1000., prefix: "", suffix: "km/h" }
            }
            Self::MileHour => Quantity { value: knots * MILES_PER_NM, prefix: "", suffix: "mph" },
            Self::MeterSecond => {
                Quantity { value: knots * METERS_PER_NM / 3600., prefix: "", suffix: "m/s" }
            }
            Self::Mach => Quantity { value: knots / KNOTS_PER_MACH, prefix: "Ma", suffix: "" },
        }
    }
}
