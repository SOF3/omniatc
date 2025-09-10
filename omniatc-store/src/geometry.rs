use bevy_math::Vec2;
use math::{Angle, Length, Position};
use serde::{Deserialize, Serialize};

/// A horizontal map axis.
#[derive(Clone, Serialize, Deserialize)]
pub enum AxisDirection {
    X,
    Y,
}

/// A 2D shape.
#[derive(Clone, Serialize, Deserialize)]
pub enum Shape2d {
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
    Polygon {
        points: Vec<Position<Vec2>>,
    },
}
