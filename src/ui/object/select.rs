use bevy::app::{self, App, Plugin};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::ButtonInput;
use bevy::prelude::{
    Entity, EventReader, EventWriter, IntoSystemConfigs, KeyCode, NextState, Query, Res, ResMut,
    Resource,
};

use crate::level::object;
use crate::ui::{message, InputState};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<SearchStack>();
        app.init_resource::<Selected>();

        app.add_systems(app::Update, start_search_system.in_set(InputState::Root));
        app.add_systems(
            app::Update,
            incremental_search_system
                .in_set(InputState::ObjectSearch)
                .in_set(message::SenderSystemSet),
        );
        app.add_systems(app::Update, deselect_system.in_set(InputState::ObjectAction));
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
    objects: Query<(Entity, &object::Display)>,
    mut messages: EventWriter<message::PushEvent>,
    mut selected: ResMut<Selected>,
) {
    for input in inputs.read() {
        if !input.state.is_pressed() || input.repeat {
            continue;
        }

        let Some(chars) = &mut stack.chars else {
            // chars should be concurrently initialized when the input state is changed
            continue;
        };

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
                input_state.set(InputState::Root);
                stack.chars = None;
            }
            Key::Enter => {
                let all_matches: Vec<_> = objects
                    .iter()
                    .filter(|(_, display)| is_subsequence(chars, &display.name))
                    .map(|(entity, _)| entity)
                    .collect();
                let match_ = match all_matches[..] {
                    [] => {
                        messages.send(message::PushEvent {
                            message: format!("No objects matching \"{chars}\""),
                            ty:      message::Type::Error,
                        });
                        return;
                    }
                    [entity] => entity,
                    _ => {
                        messages.send(message::PushEvent {
                            message: format!(
                                "There are {len} objects matching \"{chars}\"",
                                len = all_matches.len()
                            ),
                            ty:      message::Type::Error,
                        });
                        return;
                    }
                };

                stack.chars = None;
                selected.object_entity = Some(match_);
                input_state.set(InputState::ObjectAction);
            }
            _ => {}
        }
    }
}

fn deselect_system(
    mut inputs: EventReader<KeyboardInput>,
    mut input_state: ResMut<NextState<InputState>>,
    mut selected: ResMut<Selected>,
) {
    for input in inputs.read() {
        if let Key::Escape = input.logical_key {
            selected.object_entity = None;
            input_state.set(InputState::Root);
        }
    }
}

fn is_subsequence(sub: &str, full: &str) -> bool {
    let mut sub = sub.chars().peekable();
    for ch in full.chars() {
        let Some(&sub_next) = sub.peek() else {
            return true;
        };

        if sub_next.eq_ignore_ascii_case(&ch) {
            sub.next().unwrap();
        }
    }

    sub.next().is_none()
}

#[derive(Resource, Default)]
pub(super) struct SearchStack {
    pub(super) chars: Option<String>,
}

#[derive(Resource, Default)]
pub(super) struct Selected {
    pub(super) object_entity: Option<Entity>,
}
