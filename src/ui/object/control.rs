use std::f32::consts::FRAC_PI_2;
use std::marker::PhantomData;
use std::mem;

use bevy::app::{self, App, Plugin};
use bevy::ecs::schedule::SystemConfigs;
use bevy::ecs::system::SystemParam;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};
use bevy::prelude::{
    Entity, EventReader, EventWriter, IntoSystemConfigs, KeyCode, NextState, Query, Res, ResMut,
    Resource,
};

use super::select::Selected;
use crate::level::{nav, object};
use crate::math::{Heading, TurnDirection};
use crate::ui::{message, InputState};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            ControllablePlug::<SetSpeed>(PhantomData),
            ControllablePlug::<SetHeading>(PhantomData),
        ));
    }
}

struct ControllablePlug<C>(PhantomData<C>);

impl<C: Controllable> Plugin for ControllablePlug<C> {
    fn build(&self, app: &mut App) {
        app.insert_resource::<ControllableState<C>>(ControllableState::Inactive);
        app.add_systems(
            app::Update,
            C::make_initiate_controllable_system()
                .in_set(message::SenderSystemSet)
                .in_set(InputState::ObjectAction)
                .ambiguous_with(InputState::ObjectAction),
        );
        app.add_systems(
            app::Update,
            C::make_execute_controllable_system().in_set(C::input_state()),
        );
    }
}

trait Controllable: Sized + Send + Sync + 'static {
    fn input_state() -> InputState;

    fn normal_step_size() -> u16;
    fn large_step_size() -> u16;
    fn small_step_size() -> u16;

    fn reset(&mut self);

    fn modify_by(&mut self, change: ChangeDirection, amount: u16);

    fn push_digit(&mut self, digit: u16);
    fn pop_digit(&mut self);

    fn init_keycode() -> KeyCode;

    fn make_initiate_controllable_system() -> SystemConfigs;

    type GetInitialParams<'w, 's>: SystemParam;
    fn get_initial(
        params: &mut Self::GetInitialParams<'_, '_>,
        object_entity: Entity,
    ) -> Result<Self, String>;

    fn make_execute_controllable_system() -> SystemConfigs;

    type ApplyResultParams<'w, 's>: SystemParam;
    fn apply_result(self, params: &mut Self::ApplyResultParams<'_, '_>, object_entity: Entity);
}

enum ChangeDirection {
    Increase,
    Decrease,
}

#[derive(Resource)]
enum ControllableState<C: Controllable> {
    Inactive,
    Active { current: C },
}

fn initiate_controllable_system<C: Controllable>(
    inputs: Res<ButtonInput<KeyCode>>,
    mut input_state: ResMut<NextState<InputState>>,
    mut controllable_state: ResMut<ControllableState<C>>,
    selected: Res<Selected>,
    mut messages: EventWriter<message::PushEvent>,
    mut params: C::GetInitialParams<'_, '_>,
) {
    let Some(selected) = selected.object_entity else { return };

    if inputs.just_pressed(C::init_keycode()) {
        input_state.set(C::input_state());

        match C::get_initial(&mut params, selected) {
            Ok(current) => {
                *controllable_state = ControllableState::Active { current };
            }
            Err(err) => {
                messages.send(message::PushEvent { message: err, ty: message::Type::Error });
            }
        }
    }
}

fn execute_controllable_system<C: Controllable>(
    inputs: Res<ButtonInput<KeyCode>>,
    mut input_reader: EventReader<KeyboardInput>,
    mut input_state: ResMut<NextState<InputState>>,
    mut state: ResMut<ControllableState<C>>,
    selected: Res<Selected>,
    mut apply_result_params: C::ApplyResultParams<'_, '_>,
) {
    fn modify_by<C: Controllable>(
        inputs: &ButtonInput<KeyCode>,
        state: &mut C,
        direction: ChangeDirection,
    ) {
        let step = if inputs.pressed(KeyCode::ControlLeft) || inputs.pressed(KeyCode::ControlRight)
        {
            C::large_step_size()
        } else if inputs.pressed(KeyCode::ShiftLeft) || inputs.pressed(KeyCode::ShiftRight) {
            C::small_step_size()
        } else {
            C::normal_step_size()
        };

        state.modify_by(direction, step);
    }

    let Some(selected) = selected.object_entity else { return };
    let ControllableState::Active { ref mut current } = *state else { return };

    for input in input_reader.read() {
        match input {
            KeyboardInput { logical_key: Key::Backspace, state: ButtonState::Pressed, .. } => {
                if inputs.pressed(KeyCode::ControlLeft) || inputs.pressed(KeyCode::ControlRight) {
                    current.reset();
                } else {
                    current.pop_digit();
                }
            }
            KeyboardInput {
                key_code:
                    KeyCode::Equal | KeyCode::NumpadAdd | KeyCode::ArrowUp | KeyCode::ArrowRight,
                state: ButtonState::Pressed,
                ..
            } => modify_by(&inputs, current, ChangeDirection::Increase),
            KeyboardInput {
                key_code:
                    KeyCode::Minus | KeyCode::NumpadSubtract | KeyCode::ArrowDown | KeyCode::ArrowLeft,
                state: ButtonState::Pressed,
                ..
            } => modify_by(&inputs, current, ChangeDirection::Decrease),
            KeyboardInput { logical_key: Key::Escape, .. } => {
                *state = ControllableState::Inactive;
                input_state.set(InputState::ObjectAction);
                return;
            }
            KeyboardInput { logical_key: Key::Enter, state: ButtonState::Pressed, .. } => {
                let ControllableState::Active { current } =
                    mem::replace(&mut *state, ControllableState::Inactive)
                else {
                    unreachable!()
                };
                current.apply_result(&mut apply_result_params, selected);
                input_state.set(InputState::ObjectAction);
                return;
            }
            KeyboardInput {
                logical_key: Key::Character(ref chars),
                state: ButtonState::Pressed,
                repeat: false,
                ..
            } => {
                for ch in chars.chars() {
                    if let Some(digit) = ch.to_digit(10) {
                        current.push_digit(u16::try_from(digit).expect("digit < 10"));
                    }
                }
            }
            _ => {}
        }
    }
}

struct SetSpeed {
    initial: u16,
    speed:   u16,
}

impl Controllable for SetSpeed {
    fn input_state() -> InputState { InputState::ObjectSetSpeed }

    fn normal_step_size() -> u16 { 10 }
    fn large_step_size() -> u16 { 30 }
    fn small_step_size() -> u16 { 5 }

    fn reset(&mut self) { self.speed = self.initial; }

    fn modify_by(&mut self, change: ChangeDirection, amount: u16) {
        match change {
            ChangeDirection::Increase => self.speed = self.speed.saturating_add(amount),
            ChangeDirection::Decrease => self.speed = self.speed.saturating_sub(amount),
        }
    }

    fn push_digit(&mut self, digit: u16) {
        self.speed %= 100;
        self.speed *= 10;
        self.speed += digit;
    }

    fn pop_digit(&mut self) { self.speed /= 10; }

    fn init_keycode() -> KeyCode { KeyCode::KeyS }

    fn make_initiate_controllable_system() -> SystemConfigs {
        initiate_controllable_system::<Self>.into_configs()
    }

    type GetInitialParams<'w, 's> = SetSpeedGetInitialParams<'w, 's>;

    fn get_initial(
        SetSpeedGetInitialParams { target_query }: &mut SetSpeedGetInitialParams<'_, '_>,
        object_entity: Entity,
    ) -> Result<Self, String> {
        let Ok(&nav::VelocityTarget { horiz_speed, .. }) = target_query.get(object_entity) else {
            return Err("Object is not piloted".into());
        };

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let speed = horiz_speed.round() as u16;
        Ok(Self { initial: speed, speed })
    }

    fn make_execute_controllable_system() -> SystemConfigs {
        execute_controllable_system::<Self>.into_configs()
    }

    type ApplyResultParams<'w, 's> = SetSpeedApplyResultParams<'w, 's>;

    fn apply_result(
        self,
        SetSpeedApplyResultParams { target_query }: &mut SetSpeedApplyResultParams,
        object_entity: Entity,
    ) {
        if let Ok(mut target) = target_query.get_mut(object_entity) {
            target.horiz_speed = f32::from(self.speed);
        }
    }
}

#[derive(SystemParam)]
struct SetSpeedGetInitialParams<'w, 's> {
    target_query: Query<'w, 's, &'static nav::VelocityTarget>,
}

#[derive(SystemParam)]
struct SetSpeedApplyResultParams<'w, 's> {
    target_query: Query<'w, 's, &'static mut nav::VelocityTarget>,
}

struct SetHeading {
    initial_heading: Heading,
    heading:         Heading,
    digits:          Vec<u16>,
    rotation_offset: i16,
}

impl SetHeading {
    fn recalculate_heading_from_digits(&mut self) {
        self.heading = Heading::from_degrees(
            self.digits
                .iter()
                .copied()
                .rev()
                .take(3)
                .enumerate()
                .map(
                    #[allow(clippy::cast_possible_truncation)]
                    |(exp, digit)| digit * 10u16.pow(exp as u32),
                )
                .sum::<u16>()
                .into(),
        );
        self.rotation_offset = 0;
    }
}

impl Controllable for SetHeading {
    fn input_state() -> InputState { InputState::ObjectSetHeading }

    fn normal_step_size() -> u16 { 5 }
    fn large_step_size() -> u16 { 30 }
    fn small_step_size() -> u16 { 1 }

    fn reset(&mut self) {
        self.heading = self.initial_heading;
        self.digits.clear();
        self.rotation_offset = 0;
    }

    fn modify_by(&mut self, change: ChangeDirection, amount: u16) {
        match change {
            ChangeDirection::Increase => {
                self.heading += f32::from(amount);
                self.rotation_offset = self.rotation_offset.saturating_add_unsigned(amount);
            }
            ChangeDirection::Decrease => {
                self.heading -= f32::from(amount);
                self.rotation_offset = self.rotation_offset.saturating_sub_unsigned(amount);
            }
        }
    }

    fn push_digit(&mut self, digit: u16) {
        self.digits.push(digit);
        self.recalculate_heading_from_digits();
    }

    fn pop_digit(&mut self) {
        self.digits.pop();
        self.recalculate_heading_from_digits();
    }

    fn init_keycode() -> KeyCode { KeyCode::KeyV }

    fn make_initiate_controllable_system() -> SystemConfigs {
        initiate_controllable_system::<Self>.into_configs()
    }

    type GetInitialParams<'w, 's> = SetHeadingGetInitialParams<'w, 's>;

    fn get_initial(
        SetHeadingGetInitialParams {
        airborne_query, target_query
    }: &mut SetHeadingGetInitialParams<'_, '_>,
        object_entity: Entity,
    ) -> Result<Self, String> {
        let current_heading = match airborne_query.get(object_entity) {
            Ok(&object::Airborne { airspeed }) => Heading::from_vec3(airspeed),
            _ => return Err("Cannot set heading of ground objects".into()),
        };

        let (target_heading, dir) = match target_query.get(object_entity) {
            Ok(&nav::VelocityTarget { yaw: nav::YawTarget::Heading(heading), .. }) => {
                (heading, None)
            }
            Ok(&nav::VelocityTarget {
                yaw: nav::YawTarget::TurnHeading { heading, direction, .. },
                ..
            }) => (heading, Some(direction)),
            Ok(_) => (current_heading, None),
            _ => return Err("Object is not piloted".into()),
        };

        Ok(Self {
            initial_heading: target_heading,
            heading: target_heading,
            digits: Vec::new(),
            #[allow(clippy::cast_possible_truncation)]
            rotation_offset: match dir {
                Some(dir) => current_heading.distance(target_heading, dir) as i16,
                None => 0,
            },
        })
    }

    fn make_execute_controllable_system() -> SystemConfigs {
        execute_controllable_system::<Self>.into_configs()
    }

    type ApplyResultParams<'w, 's> =
        Query<'w, 's, (&'static mut nav::VelocityTarget, &'static object::Airborne)>;

    fn apply_result(self, query: &mut Self::ApplyResultParams<'_, '_>, object_entity: Entity) {
        let Ok((mut target, &object::Airborne { airspeed })) = query.get_mut(object_entity) else {
            return;
        };
        let current_heading = Heading::from_vec3(airspeed);

        // only set command as explicit reflex turn when rotation offset is obviously in that
        // direction.
        let dir = if self.rotation_offset >= 180 {
            TurnDirection::Clockwise
        } else if self.rotation_offset <= -180 {
            TurnDirection::CounterClockwise
        } else {
            current_heading.closer_direction_to(self.heading)
        };

        let distance = current_heading.distance(self.heading, dir);

        target.yaw = if distance.abs() >= FRAC_PI_2 {
            nav::YawTarget::TurnHeading {
                heading:           self.heading,
                direction:         dir,
                remaining_crosses: 0,
            }
        } else {
            nav::YawTarget::Heading(self.heading)
        };
    }
}

#[derive(SystemParam)]
struct SetHeadingGetInitialParams<'w, 's> {
    target_query:   Query<'w, 's, &'static nav::VelocityTarget>,
    airborne_query: Query<'w, 's, &'static object::Airborne>,
}
