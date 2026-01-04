use bevy_math::Vec2;
use math::{Angle, Length, Position, Quantity};
use serde::{Deserialize, Serialize};

/// A horizontal map axis.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum AxisDirection {
    /// The X direction.
    X,
    /// The Y direction.
    Y,
}

impl AxisDirection {
    /// Get the orthogonal axis direction.
    #[must_use]
    pub fn orthogonal(self) -> Self {
        match self {
            AxisDirection::X => AxisDirection::Y,
            AxisDirection::Y => AxisDirection::X,
        }
    }

    /// Get the component of a 2D vector in this axis direction.
    #[must_use]
    pub fn of_vec2(self, v: Vec2) -> f32 {
        match self {
            AxisDirection::X => v.x,
            AxisDirection::Y => v.y,
        }
    }

    /// Get the component of a 2D quantity in this axis direction.
    #[must_use]
    pub fn of_quantity<Dt, Pow>(
        self,
        v: Quantity<Vec2, math::LengthBase, Dt, Pow>,
    ) -> Quantity<f32, math::LengthBase, Dt, Pow>
    where
        Dt: math::DtTrait,
        Pow: math::PowTrait,
    {
        match self {
            AxisDirection::X => v.x(),
            AxisDirection::Y => v.y(),
        }
    }

    /// Get the component of a 2D position in this axis direction.
    #[must_use]
    pub fn of_position(self, v: Position<Vec2>) -> Position<f32> {
        match self {
            AxisDirection::X => v.x(),
            AxisDirection::Y => v.y(),
        }
    }
}

/// A 2D shape.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum Shape2d {
    /// An ellipse shape.
    Ellipse {
        /// Center of the ellipse.
        center:       Position<Vec2>,
        /// Length of the major axis.
        major_radius: Length<f32>,
        /// Length of the minor axis.
        minor_radius: Length<f32>,
        /// Direction of the major axis.
        major_dir:    Angle,
    },
    /// A polygon shape.
    Polygon {
        /// Vertices of the polygon, in order.
        points: Vec<Position<Vec2>>,
    },
}

/// A generic range.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Range<T> {
    /// Start of the range.
    pub min: T,
    /// End of the range.
    pub max: T,
}
