use bevy_math::Vec2;
use math::{Heading, Length, Position};
use serde::{Deserialize, Serialize};

use crate::AxisDirection;

#[derive(Clone, Serialize, Deserialize)]
pub struct Ui {
    pub camera: Camera,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Camera {
    TwoDimension(Camera2d),
}

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
