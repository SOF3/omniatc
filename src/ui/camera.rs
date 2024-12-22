use std::f32::consts::FRAC_PI_6;

use bevy::app::{self, App, Plugin};
use bevy::math::Vec2;
use bevy::prelude::{Camera2d, Commands, Resource, Transform};

mod input;
mod ruler;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();

        app.add_systems(app::Startup, |mut commands: Commands| {
            commands.spawn((Camera2d, Transform::IDENTITY.with_scale((0.1, 0.1, 1.).into())));
        });

        app.add_plugins(input::Plug);
        app.add_plugins(ruler::Plug);
    }
}

#[derive(Resource)]
pub struct Config {
    /// Whethre the scale ruler is visible.
    pub ruler: Option<RulerConfig>,

    /// Pixels moved per second when WASD is pressed.
    pub key_move_speed:   f32,
    /// Magnification per second when =- is pressed.
    pub key_zoom_speed:   f32,
    /// Rotated radians per second when QE is pressed.
    pub key_rotate_speed: f32,
}

pub struct RulerConfig {
    /// Base width of the scale ruler, relative to viewport width.
    ///
    /// The actual width is rounded to the nearest 1nm * power of 2.
    pub base_width_ratio: f32,
    /// Height of the scale ruler.
    pub height:           f32,
    /// Padding between ruler labels and the closer edge of the ruler.
    pub label_padding:    f32,
    /// Position of the left/right endpoint of the scale ruler.
    ///
    /// - A positive X value indicates the position of the left endpoint from the left viewport edge.
    /// - A negative X value indicates the position of the right endpoint from the right viewport edge.
    /// - A positive Y value indicates the distance of the centerline of the ruler from the top viewport edge.
    /// - A negative Y value indicates the distance of the centerline of the ruler from the bottom viewport edge.
    pub pos:              Vec2,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            ruler:            Some(RulerConfig {
                base_width_ratio: 0.3,
                height:           10.,
                label_padding:    3.,
                pos:              Vec2::new(20., -20.),
            }),
            key_move_speed:   100.,
            key_zoom_speed:   1.4,
            key_rotate_speed: FRAC_PI_6,
        }
    }
}
