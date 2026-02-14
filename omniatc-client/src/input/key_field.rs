use bevy_egui::egui;
use bevy_mod_config::impl_scalar_config_field;
use bevy_mod_config::manager::egui::{DefaultStyle, Editable};
use serde::{Deserialize, Serialize};

#[derive(Default, Clone, Copy, Serialize, Deserialize)]
pub struct KeySet {
    pub key:     Option<egui::Key>,
    pub command: Option<bool>,
    pub alt:     Option<bool>,
    pub shift:   Option<bool>,
}

impl From<egui::Key> for KeySet {
    fn from(key: egui::Key) -> Self {
        Self { key: Some(key), command: None, alt: None, shift: None }
    }
}

impl KeySet {
    #[must_use]
    pub fn ctrl(mut self, ctrl: bool) -> Self {
        self.command = Some(ctrl);
        self
    }

    #[must_use]
    pub fn alt(mut self, alt: bool) -> Self {
        self.alt = Some(alt);
        self
    }

    #[must_use]
    pub fn shift(mut self, shift: bool) -> Self {
        self.shift = Some(shift);
        self
    }

    #[must_use]
    pub fn clicked(self, state: &egui::InputState) -> bool {
        self.test_modifiers(state.modifiers)
            && self.key.is_some_and(|desired| {
                state.events.iter().any(|event| match *event {
                    egui::Event::Key { key, pressed: true, repeat: false, .. } => key == desired,
                    _ => false,
                })
            })
    }

    #[must_use]
    pub fn clicked_or_repeated(self, state: &egui::InputState) -> bool {
        self.test_modifiers(state.modifiers) && self.key.is_some_and(|key| state.key_pressed(key))
    }

    #[must_use]
    pub fn down(self, state: &egui::InputState) -> bool {
        self.test_modifiers(state.modifiers) && self.key.is_some_and(|key| state.key_down(key))
    }

    fn test_modifiers(self, buttons: egui::Modifiers) -> bool {
        if let Some(command) = self.command
            && buttons.command != command
        {
            return false;
        }
        if let Some(alt) = self.alt
            && buttons.alt != alt
        {
            return false;
        }
        if let Some(shift) = self.shift
            && buttons.shift != shift
        {
            return false;
        }
        true
    }
}

impl_scalar_config_field!(
    KeySet,
    KeyMetadata,
    |metadata: &KeyMetadata| metadata.default,
    'a => &'a KeySet,
    |key: &'a KeySet| key,
);

#[derive(Default, Clone)]
pub struct KeyMetadata {
    pub default: KeySet,
}

impl Editable<DefaultStyle> for KeySet {
    type TempData = UiState;

    fn show(
        ui: &mut bevy_egui::egui::Ui,
        value: &mut Self,
        _: &Self::Metadata,
        state: &mut Option<Self::TempData>,
        _: impl std::hash::Hash,
        _: &DefaultStyle,
    ) -> egui::Response {
        let mut changed = false;

        let mut resp =
            ui.horizontal(|ui| {
                let mut waiting_for_key = false;
                if let Some(UiState { waiting_for_key: waiting_state @ true }) = state {
                    ui.input_mut(|state| {
                        if let Some(pos) = state.events.iter().position(|event| {
                            matches!(event, egui::Event::Key { pressed: true, .. })
                        }) {
                            let egui::Event::Key { key, .. } = state.events.remove(pos) else {
                                unreachable!("result from iter().position() must be a valid offset")
                            };
                            value.key = Some(key);
                            *waiting_state = false;
                            changed = true;
                        } else {
                            waiting_for_key = true;
                        }
                    });
                }

                let button = match value.key {
                    _ if waiting_for_key => ui.button("Press any key").highlight(),
                    None => ui.button("None"),
                    Some(key) => ui.button(key.name().to_string()),
                };

                if button.clicked() {
                    let state = state.get_or_insert_default();
                    state.waiting_for_key = !state.waiting_for_key;
                }

                // non-short-circuiting `|` to accumulate changes
                changed |= modifier_checkbox(ui, &mut value.command, command_label());
                changed |= modifier_checkbox(ui, &mut value.alt, alt_label());
                changed |= modifier_checkbox(ui, &mut value.shift, shift_label());
            });
        if changed {
            resp.response.mark_changed();
        }
        resp.response
    }
}

#[derive(Default)]
pub struct UiState {
    waiting_for_key: bool,
}

fn command_label() -> &'static str { if cfg!(target_os = "macos") { "\u{2318}" } else { "Ctrl" } }

fn alt_label() -> &'static str { if cfg!(target_os = "macos") { "\u{2325}" } else { "Alt" } }

fn shift_label() -> &'static str { if cfg!(target_os = "macos") { "\u{21e7}" } else { "Shift" } }

fn modifier_checkbox(ui: &mut egui::Ui, value: &mut Option<bool>, label: &str) -> bool {
    let initial_flag = *value == Some(true);
    let mut flag = initial_flag;
    ui.add(egui::Checkbox::new(&mut flag, label).indeterminate(value.is_none()));
    if flag == initial_flag {
        false
    } else {
        // checkbox toggled, perform cycling
        *value = match *value {
            None => Some(false),
            Some(false) => Some(true),
            Some(true) => None,
        };
        true
    }
}
