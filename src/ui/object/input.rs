use bevy::app::{self, App, Plugin};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonInput;
use bevy::prelude::{EventReader, IntoSystemConfigs, KeyCode, NextState, Res, ResMut, Resource};

use crate::ui::InputState;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_systems(app::Update, start_search_system.in_set(InputState::Normal));
        app.add_systems(app::Update, incremental_search_system.in_set(InputState::ObjectSearch));
    }
}

fn start_search_system(
    inputs: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<NextState<InputState>>,
) {
    if inputs.just_pressed(KeyCode::Slash) {
        input_state.set(InputState::ObjectSearch);
    }
}

fn incremental_search_system(
    mut inputs: EventReader<KeyboardInput>,
    mut stack: ResMut<SearchStack>,
) {
    for input in inputs.read() {
        if !input.state.is_pressed() || input.repeat {
            continue;
        }

        if let Key::Character(ref str) = input.logical_key {
            if let &[ascii] = str.as_bytes() {
                match ascii {
                    b'0'..=b'9' | b'a'..=b'z' => {
                        stack.chars.push(ascii);
                        continue;
                    }
                    b'A'..=b'Z' => {
                        stack.chars.push(ascii.to_ascii_lowercase());
                        continue;
                    }
                    _ => {}
                }
            }
        }

        match input.key_code {
            KeyCode::Slash => {
                stack.chars.clear();
            }
            KeyCode::Backspace | KeyCode::NumpadBackspace => {
                _ = stack.chars.pop();
            }
            _ => {}
        }
    }
}

#[derive(Resource)]
pub(super) struct SearchStack {
    chars: Vec<u8>,
}
