use bevy::app::{App, Plugin};
use bevy::prelude::Resource;

mod input;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();

        app.add_plugins(input::Plug);
    }
}

#[derive(Resource)]
pub struct Config {
    pub fast_forward_speed: f32,
}

impl Default for Config {
    fn default() -> Self { Self { fast_forward_speed: 20. } }
}
