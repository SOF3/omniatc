use bevy_math::Vec2;
use math::{Heading, Length, Position};
use serde::{Deserialize, Serialize};

use crate::AxisDirection;

/// State of UI.
#[derive(Clone, Serialize, Deserialize)]
pub struct Ui {
    /// The camera state.
    pub camera: Camera,
}

/// State of a camera.
#[derive(Clone, Serialize, Deserialize)]
pub enum Camera {
    /// Render the world with 2D view.
    TwoDimension(Camera2d),
}

/// State of a 2D camera.
#[derive(Clone, Serialize, Deserialize)]
pub struct Camera2d {
    /// Level position that the camera is centered in.
    pub center: Position<Vec2>,

    /// Heading of the upward direction of the camera.
    /// 0 degrees means north is upwards; 90 degrees means east is upwards.
    pub up: Heading,

    /// Whether the camera scale is based on X (width) or Y (height) axis.
    pub scale_axis:   AxisDirection,
    /// Number of nautical miles to display in the scale axis.
    pub scale_length: Length<f32>,
}
