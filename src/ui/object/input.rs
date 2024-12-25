use bevy::app::{self, App, Plugin};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonInput;
use bevy::prelude::{EventReader, IntoSystemConfigs, KeyCode, NextState, Res, ResMut, Resource};

use crate::ui::InputState;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<SearchStack>();

        app.add_systems(app::Update, start_search_system.in_set(InputState::Normal));
        app.add_systems(app::Update, incremental_search_system.in_set(InputState::ObjectSearch));
    }
}

fn start_search_system(
    inputs: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<NextState<InputState>>,
    mut search_stack: ResMut<SearchStack>,
) {
    if inputs.just_pressed(KeyCode::Slash) {
        input_state.set(InputState::ObjectSearch);
        search_stack.chars = Some(String::new());
    }
}

fn incremental_search_system(
    mut inputs: EventReader<KeyboardInput>,
    mut input_state: ResMut<NextState<InputState>>,
    mut stack: ResMut<SearchStack>,
) {
    for input in inputs.read() {
        if !input.state.is_pressed() || input.repeat {
            continue;
        }

        let Some(chars) = &mut stack.chars else { continue };

        match input.logical_key {
            Key::Character(ref str) => {
                for ch in str.chars() {
                    match ch {
                        '0'..='9' | 'a'..='z' => {
                            chars.push(ch);
                        }
                        'A'..='Z' => {
                            chars.push(ch.to_ascii_lowercase());
                        }
                        '/' => chars.clear(),
                        _ => continue,
                    }
                }
            }
            Key::Backspace => _ = chars.pop(),
            Key::Escape => {
                input_state.set(InputState::Normal);
                stack.chars = None;
            }
            _ => {}
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct SearchStack {
    pub(super) chars: Option<String>,
}
