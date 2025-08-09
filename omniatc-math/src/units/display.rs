use crate::{Length, QuantityTrait, Speed};

pub trait UnitEnum: Copy + Eq + strum::IntoEnumIterator {
    type Quantity: QuantityTrait;

    fn to_str(self) -> &'static str;

    fn float_to_quantity(self) -> fn(f32) -> Self::Quantity;
    fn quantity_to_float(self) -> fn(Self::Quantity) -> f32;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumIter)]
pub enum LengthUnit {
    NauticalMiles,
    Kilometers,
    Feet,
    Miles,
    Meters,
}

impl UnitEnum for LengthUnit {
    type Quantity = Length<f32>;

    fn to_str(self) -> &'static str {
        match self {
            Self::NauticalMiles => "nmi",
            Self::Kilometers => "km",
            Self::Feet => "ft",
            Self::Miles => "mi",
            Self::Meters => "m",
        }
    }

    #[inline]
    fn float_to_quantity(self) -> fn(f32) -> Length<f32> {
        match self {
            Self::NauticalMiles => Length::from_nm,
            Self::Kilometers => Length::from_km,
            Self::Feet => Length::from_feet,
            Self::Miles => Length::from_miles,
            Self::Meters => Length::from_meters,
        }
    }

    #[inline]
    fn quantity_to_float(self) -> fn(Length<f32>) -> f32 {
        match self {
            Self::NauticalMiles => Length::<f32>::into_nm,
            Self::Kilometers => Length::<f32>::into_km,
            Self::Feet => Length::<f32>::into_feet,
            Self::Miles => Length::<f32>::into_miles,
            Self::Meters => Length::<f32>::into_meters,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::EnumIter)]
pub enum SpeedUnit {
    Knots,
    KilometersPerHour,
    MilePerHour,
    MetersPerSecond,
    FeetPerMinute,
}

impl UnitEnum for SpeedUnit {
    type Quantity = Speed<f32>;

    fn to_str(self) -> &'static str {
        match self {
            Self::Knots => "kn",
            Self::KilometersPerHour => "km/h",
            Self::MilePerHour => "mph",
            Self::MetersPerSecond => "m/s",
            Self::FeetPerMinute => "fpm",
        }
    }

    #[inline]
    fn float_to_quantity(self) -> fn(f32) -> Speed<f32> {
        match self {
            Self::Knots => Speed::from_knots,
            Self::KilometersPerHour => Speed::from_kmh,
            Self::MilePerHour => Speed::from_mph,
            Self::MetersPerSecond => Speed::from_meter_per_sec,
            Self::FeetPerMinute => Speed::from_fpm,
        }
    }

    #[inline]
    fn quantity_to_float(self) -> fn(Speed<f32>) -> f32 {
        match self {
            Self::Knots => Speed::into_knots,
            Self::KilometersPerHour => Speed::into_kmh,
            Self::MilePerHour => Speed::into_mph,
            Self::MetersPerSecond => Speed::into_meter_per_sec,
            Self::FeetPerMinute => Speed::into_fpm,
        }
    }
}
