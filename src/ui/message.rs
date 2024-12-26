//! Displays a queue of non-modal messages for the user.

use std::collections::VecDeque;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::math::Vec2;
use bevy::prelude::{
    BuildChildren, Commands, Component, DespawnRecursiveExt, Entity, Event, EventReader,
    IntoSystemConfigs, Res, ResMut, Resource, Single, SystemSet, Text, With,
};
use bevy::text::{TextColor, TextFont, TextSpan};
use bevy::time::{self, Time};
use bevy::ui;

use super::SystemSets;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<PushEvent>();
        app.init_resource::<Queue>();
        app.init_resource::<Config>();

        app.add_systems(app::Startup, setup_system);
        app.add_systems(
            app::Update,
            consume_event_system.in_set(SystemSets::RenderSpawn).after(SenderSystemSet),
        );
        app.add_systems(app::Update, cleanup_expired_system.in_set(SystemSets::RenderMove));
        app.add_systems(app::Update, update_queue_position.in_set(SystemSets::RenderMove));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct SenderSystemSet;

fn setup_system(mut commands: Commands) {
    commands.spawn((
        Container,
        Text::new(""),
        ui::Node { position_type: ui::PositionType::Absolute, ..Default::default() },
        bevy::core::Name::new("MessageContainer"),
    ));
}

fn consume_event_system(
    mut commands: Commands,
    time: Res<Time<time::Fixed>>,
    container: Single<Entity, With<Container>>,
    mut queue: ResMut<Queue>,
    config: Res<Config>,
    mut events: EventReader<PushEvent>,
) {
    events.read().for_each(|event| {
        let message_entity = commands
            .spawn((
                TextSpan(event.message.clone() + "\n"),
                TextFont { font_size: config.text_size, ..Default::default() },
                TextColor(match event.ty {
                    Type::Info => config.info_color,
                    Type::Warning => config.warning_color,
                    Type::Error => config.error_color,
                }),
            ))
            .set_parent(*container)
            .id();

        queue.queue.push_back(Message {
            entity:      message_entity,
            expire_time: time.elapsed() + config.retention_time,
        });
    });
}

fn cleanup_expired_system(
    time: Res<Time<time::Fixed>>,
    mut queue: ResMut<Queue>,
    mut commands: Commands,
) {
    while let Some(next) = queue.queue.front() {
        if next.expire_time > time.elapsed() {
            return;
        }
        let entity = next.entity;

        queue.queue.pop_front().expect("front is Some");

        commands.entity(entity).despawn_recursive();
    }
}

fn update_queue_position(config: Res<Config>, container: Single<&mut ui::Node, With<Container>>) {
    let mut node = container.into_inner();

    if config.anchor_position.x > 0. {
        node.left = ui::Val::Px(config.anchor_position.x);
        node.right = ui::Val::Auto;
    } else {
        node.left = ui::Val::Auto;
        node.right = ui::Val::Px(-config.anchor_position.x);
    }

    if config.anchor_position.y > 0. {
        node.top = ui::Val::Px(config.anchor_position.y);
        node.bottom = ui::Val::Auto;
    } else {
        node.top = ui::Val::Auto;
        node.bottom = ui::Val::Px(-config.anchor_position.y);
    }

    // node.min_width = ui::Val::Px(config.container_width);
}

/// Marks the container text component.
#[derive(Component)]
struct Container;

#[derive(Resource, Default)]
struct Queue {
    queue: VecDeque<Message>,
}

struct Message {
    entity:      Entity,
    expire_time: Duration,
}

/// Pushes a non-modal message to be seen by the user.
#[derive(Event)]
pub struct PushEvent {
    pub message: String,
    pub ty:      Type,
}

/// Type of message, affecting its display color.
pub enum Type {
    Info,
    Warning,
    Error,
}

#[derive(Resource)]
pub struct Config {
    /// Duration for which a message remains.
    pub retention_time:  Duration,
    /// Position of the message container border relative to the screen corner.
    pub anchor_position: Vec2,
    /// Width of the message container.
    pub container_width: f32,
    /// Color for info messages.
    pub info_color:      Color,
    /// Color for warning messages.
    pub warning_color:   Color,
    /// Color for error messages.
    pub error_color:     Color,
    /// Size of displayed messages.
    pub text_size:       f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            retention_time:  Duration::from_secs(10),
            anchor_position: Vec2::new(-20., 20.),
            container_width: 500.,
            info_color:      Color::WHITE,
            warning_color:   Color::srgb(1.0, 1.0, 0.2),
            error_color:     Color::srgb(1.0, 0.2, 0.4),
            text_size:       12.,
        }
    }
}
