use bevy::app::{self, App, Plugin};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::prelude::{
    EventReader, IntoSystemConfigs, KeyCode, NextState, Query, Res, ResMut, Resource,
};

use super::select::Selected;
use crate::level::nav;
use crate::ui::InputState;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<SetSpeedState>();
        app.add_systems(app::Update, select_action_system.in_set(InputState::ObjectAction));
        app.add_systems(app::Update, set_speed_system.in_set(InputState::ObjectSetSpeed));
    }
}

fn select_action_system(
    inputs: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<NextState<InputState>>,
    mut set_speed_state: ResMut<SetSpeedState>,
) {
    if inputs.just_pressed(KeyCode::KeyS) {
        input_state.set(InputState::ObjectSetSpeed);
        set_speed_state.value = 0;
    }
}

#[derive(Resource, Default)]
struct SetSpeedState {
    value: u16,
}

fn set_speed_system(
    inputs: Res<ButtonInput<KeyCode>>,
    mut input_reader: EventReader<KeyboardInput>,
    mut input_state: ResMut<NextState<InputState>>,
    mut state: ResMut<SetSpeedState>,
    selected: Res<Selected>,
    mut target_query: Query<&mut nav::VelocityTarget>,
) {
    enum ModifySpeedSign {
        Up,
        Down,
    }
    fn modify_speed(
        inputs: &ButtonInput<KeyCode>,
        state: &mut SetSpeedState,
        sign: ModifySpeedSign,
    ) {
        let mut step = 10u16;
        if inputs.pressed(KeyCode::ControlLeft) || inputs.pressed(KeyCode::ControlRight) {
            step *= 5;
        }
        if inputs.pressed(KeyCode::ShiftLeft) || inputs.pressed(KeyCode::ShiftRight) {
            step /= 10;
        }

        match sign {
            ModifySpeedSign::Up => state.value = state.value.saturating_add(step),
            ModifySpeedSign::Down => state.value = state.value.saturating_sub(step),
        }
    }

    let Some(selected) = selected.object_entity else { return };

    for input in input_reader.read() {
        match input {
            KeyboardInput { logical_key: Key::Backspace, state: ButtonState::Pressed, .. } => {
                if inputs.pressed(KeyCode::ControlLeft) || inputs.pressed(KeyCode::ControlRight) {
                    state.value = 0;
                } else {
                    state.value /= 10;
                }
            }
            KeyboardInput {
                key_code: KeyCode::Equal | KeyCode::NumpadAdd | KeyCode::ArrowUp,
                state: ButtonState::Pressed,
                ..
            } => modify_speed(&inputs, &mut state, ModifySpeedSign::Up),
            KeyboardInput {
                key_code: KeyCode::Minus | KeyCode::NumpadSubtract | KeyCode::ArrowDown,
                state: ButtonState::Pressed,
                ..
            } => modify_speed(&inputs, &mut state, ModifySpeedSign::Down),
            KeyboardInput { logical_key: Key::Escape, .. } => {
                state.value = 0;
                input_state.set(InputState::ObjectAction);
            }
            KeyboardInput { logical_key: Key::Enter, state: ButtonState::Pressed, .. } => {
                if let Ok(mut target) = target_query.get_mut(selected) {
                    target.horiz_speed = f32::from(state.value);
                }
                state.value = 0;
                input_state.set(InputState::ObjectAction);
            }
            KeyboardInput {
                logical_key: Key::Character(ref chars),
                state: ButtonState::Pressed,
                repeat: false,
                ..
            } => {
                for ch in chars.chars() {
                    if let Some(digit) = ch.to_digit(10) {
                        state.value = state
                            .value
                            .saturating_mul(10)
                            .saturating_add(u16::try_from(digit).unwrap_or(u16::MAX));
                    }
                }
            }
            _ => {}
        }
    }
}
