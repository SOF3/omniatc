use bevy::app::{self, App, Plugin};
use bevy::input::ButtonInput;
use bevy::prelude::{in_state, IntoSystemConfigs, KeyCode, Res, ResMut};
use bevy::time::{self, Time};

use super::Config;
use crate::ui::{InputState, SystemSets};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(
            app::Update,
            handle_input_system.run_if(in_state(InputState::Root)).in_set(SystemSets::Input),
        );
    }
}

fn handle_input_system(
    mut time: ResMut<Time<time::Virtual>>,
    config: Res<Config>,
    inputs: Res<ButtonInput<KeyCode>>,
) {
    if inputs.pressed(KeyCode::Tab) {
        time.set_relative_speed(config.fast_forward_speed);
    } else {
        time.set_relative_speed(1.);
    }

    if inputs.just_pressed(KeyCode::Space) {
        if time.is_paused() {
            time.unpause();
        } else {
            time.pause();
        }
    }
}
