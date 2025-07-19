use std::{fmt, ops};

use bevy_math::{NormedVectorSpace, Vec2, Vec3, VectorSpace};
use bevy_mod_config::impl_scalar_config_field;

use super::Length;
use crate::{AsSqrt, DtZero, LengthUnit, PowOne, QuantityTrait, Squared, SEA_ALTITUDE};

#[derive(Clone, Copy, PartialEq, PartialOrd, serde::Serialize)]
pub struct Position<T>(pub Length<T>);

impl<'de, T: serde::Deserialize<'de> + super::IsFinite> serde::Deserialize<'de> for Position<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        <Length<T> as serde::Deserialize<'de>>::deserialize(d).map(Self)
    }
}

impl<T> Position<T> {
    pub const fn new(value: T) -> Self { Position(Length::new(value)) }

    pub fn get(self) -> T { self.0 .0 }
}

impl Position<f32> {
    pub const SEA_LEVEL: Self = Self(Length::new(0.));

    #[must_use]
    pub fn from_amsl_feet(z: f32) -> Self { Position(Length::from_feet(z)) }
}

impl Position<Vec2> {
    pub const ORIGIN: Self = Self(Length::new(Vec2::new(0., 0.)));

    #[must_use]
    pub fn from_origin_nm(x: f32, y: f32) -> Self { Position(Length::new(Vec2 { x, y })) }

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
            .field("x", &self.0 .0.x)
            .field("y", &self.0 .0.y)
            .field("z", &self.altitude().amsl().into_feet())
            .finish()
    }
}

impl<T: ops::AddAssign> ops::Add<Length<T>> for Position<T> {
    type Output = Self;

    fn add(mut self, rhs: Length<T>) -> Self::Output {
        self.0 += rhs;
        self
    }
}

impl<T: ops::AddAssign> ops::AddAssign<Length<T>> for Position<T> {
    fn add_assign(&mut self, rhs: Length<T>) { self.0 += rhs; }
}

impl<T: ops::SubAssign> ops::Sub<Length<T>> for Position<T> {
    type Output = Self;

    fn sub(mut self, rhs: Length<T>) -> Self::Output {
        self.0 -= rhs;
        self
    }
}

impl<T: ops::SubAssign> ops::SubAssign<Length<T>> for Position<T> {
    fn sub_assign(&mut self, rhs: Length<T>) { self.0 -= rhs; }
}

impl<T: ops::Sub<Output = T>> ops::Sub for Position<T> {
    type Output = Length<T>;

    fn sub(self, rhs: Self) -> Length<T> { self.0 - rhs.0 }
}

impl<T: VectorSpace> Position<T> {
    #[must_use]
    pub fn lerp(self, other: Self, s: f32) -> Self { Self(self.0.lerp(other.0, s)) }
}

impl<T: ops::SubAssign + NormedVectorSpace> Position<T> {
    /// Returns a wrapper that can be compared with a linear distance quantity.
    pub fn distance_cmp(self, other: Self) -> AsSqrt<DtZero, PowOne> {
        (self - other).magnitude_cmp()
    }

    pub fn distance_squared(self, other: Self) -> Squared<Length<f32>> {
        (self - other).magnitude_squared()
    }

    pub fn distance_exact(self, other: Self) -> Length<f32> { (self - other).magnitude_exact() }
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
    pub fn amsl(self) -> Length<f32> { self - SEA_ALTITUDE }

    pub fn set_amsl(&mut self, amsl: Length<f32>) { *self = SEA_ALTITUDE + amsl; }
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

    #[must_use]
    pub fn horizontal_distance_cmp(self, other: Self) -> AsSqrt<DtZero, PowOne> {
        self.horizontal().distance_cmp(other.horizontal())
    }

    #[must_use]
    pub fn horizontal_distance_squared(self, other: Self) -> Squared<Length<f32>> {
        self.horizontal().distance_squared(other.horizontal())
    }

    #[must_use]
    pub fn horizontal_distance_exact(self, other: Self) -> Length<f32> {
        self.horizontal().distance_exact(other.horizontal())
    }
}

impl_scalar_config_field!(
    Position<f32>,
    PositionMetadata,
    |metadata: &PositionMetadata| metadata.default,
    'a => Position<f32>,
    |&value: &Position<f32>| value,
);

#[derive(Clone)]
pub struct PositionMetadata {
    /// The default value.
    pub default: Position<f32>,
    pub min:     Position<f32>,
    pub max:     Position<f32>,
    pub unit:    LengthUnit,
}

impl Default for PositionMetadata {
    fn default() -> Self {
        Self {
            default: Position::SEA_LEVEL,
            min:     Position::SEA_LEVEL,
            max:     Position::from_amsl_feet(50000.),
            unit:    LengthUnit::Feet,
        }
    }
}

#[cfg(feature = "egui")]
const _: () = {
    use bevy_mod_config::manager::egui::{DefaultStyle, Editable};

    impl Editable<DefaultStyle> for Position<f32> {
        type TempData = ();

        fn show(
            ui: &mut egui::Ui,
            value: &mut Self,
            metadata: &Self::Metadata,
            _: &mut Option<()>,
            _: impl std::hash::Hash,
            _: &DefaultStyle,
        ) -> egui::Response {
            use crate::units::display::UnitEnum;

            let quantity_to_float = metadata.unit.quantity_to_float();
            let mut edited = quantity_to_float(value.amsl());
            let resp = ui.add(
                egui::Slider::new(
                    &mut edited,
                    quantity_to_float(metadata.min.amsl())..=quantity_to_float(metadata.max.amsl()),
                )
                .suffix(metadata.unit.to_str()),
            );
            value.set_amsl(metadata.unit.float_to_quantity()(edited));
            resp
        }
    }
};
