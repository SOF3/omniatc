//! Displays a queue of non-modal messages for the user.

use std::collections::{BTreeMap, VecDeque};
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::color::Color;
use bevy::ecs::system::SystemParam;
use bevy::math::Vec2;
use bevy::prelude::{
    BuildChildren, Children, Commands, Component, DespawnRecursiveExt, DetectChangesMut, Entity,
    Event, EventReader, HierarchyQueryExt, IntoSystemConfigs, Mut, Query, Res, ResMut, Resource,
    Single, SystemSet, Text, With,
};
use bevy::text::{TextColor, TextFont, TextSpan};
use bevy::time::{self, Time};
use bevy::ui;
use enum_map::EnumMap;
use itertools::Itertools;

use super::SystemSets as UiSystemSets;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<PushLog>();
        app.init_resource::<LogQueue>();
        app.init_resource::<Config>();

        app.add_systems(app::Startup, setup_system);
        app.add_systems(
            app::Update,
            consume_event_system
                .in_set(UiSystemSets::RenderSpawn)
                .after(SystemSets::LogSender)
                .in_set(SystemSets::UpdateTextSpans)
                .ambiguous_with(SystemSets::UpdateTextSpans),
        );
        app.add_systems(
            app::Update,
            (
                update_status_system.after(SystemSets::StatusWriter),
                update_feedback_system.after(SystemSets::FeedbackWriter),
            )
                .in_set(UiSystemSets::RenderMove)
                .in_set(SystemSets::UpdateTextSpans)
                .ambiguous_with(SystemSets::UpdateTextSpans),
        );
        app.add_systems(app::Update, cleanup_expired_system.in_set(UiSystemSets::RenderMove));
        app.add_systems(app::Update, update_queue_position.in_set(UiSystemSets::RenderMove));

        app.allow_ambiguous_component::<Status>();
        app.allow_ambiguous_component::<Feedback>();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum SystemSets {
    LogSender,
    StatusWriter,
    FeedbackWriter,
    UpdateTextSpans,
}

fn setup_system(mut commands: Commands) {
    commands.spawn((
        LogContainer,
        Text::new(""),
        ui::Node { position_type: ui::PositionType::Absolute, ..Default::default() },
        bevy::core::Name::new("LogMessageContainer"),
    ));

    commands.spawn((
        Status { messages: EnumMap::default() },
        Text::new(""),
        ui::Node { position_type: ui::PositionType::Absolute, ..Default::default() },
        bevy::core::Name::new("StatusMessageContainer"),
    ));

    commands.spawn((
        Feedback { messages: BTreeMap::new() },
        Text::new(""),
        ui::Node { position_type: ui::PositionType::Absolute, ..Default::default() },
        bevy::core::Name::new("FeedbackMessageContainer"),
    ));
}

fn consume_event_system(
    mut commands: Commands,
    time: Res<Time<time::Fixed>>,
    container: Single<Entity, With<LogContainer>>,
    mut queue: ResMut<LogQueue>,
    config: Res<Config>,
    mut events: EventReader<PushLog>,
) {
    events.read().for_each(|event| {
        let message_entity = commands
            .spawn((
                TextSpan(event.message.clone() + "\n"),
                TextFont { font_size: config.log_container.text_size, ..Default::default() },
                TextColor(match event.ty {
                    LogType::Info => config.log_info_color,
                    LogType::Warning => config.log_warning_color,
                    LogType::Error => config.log_error_color,
                }),
            ))
            .set_parent(*container)
            .id();

        queue.queue.push_back(Log {
            entity:      message_entity,
            expire_time: time.elapsed() + config.log_retention_time,
        });
    });
}

fn cleanup_expired_system(
    time: Res<Time<time::Fixed>>,
    mut queue: ResMut<LogQueue>,
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

fn update_queue_position(
    config: Res<Config>,
    container: Single<(Entity, &mut ui::Node), With<LogContainer>>,
    mut container_config_params: ApplyContainerConfigParams,
) {
    let (entity, node) = container.into_inner();
    apply_container_config(entity, node, &mut container_config_params, &config.log_container);
}

/// Marks the container text entity.
#[derive(Component)]
struct LogContainer;

#[derive(Resource, Default)]
struct LogQueue {
    queue: VecDeque<Log>,
}

struct Log {
    entity:      Entity,
    expire_time: Duration,
}

/// Pushes a log message to be seen by the user.
#[derive(Event)]
pub struct PushLog {
    pub message: String,
    pub ty:      LogType,
}

/// Type of message, affecting its display color.
pub enum LogType {
    Info,
    Warning,
    Error,
}

/// Values to be written into the status text entity.
#[derive(Component)]
pub struct Status {
    pub messages: EnumMap<StatusType, Option<String>>,
}

impl Status {
    pub fn get_mut(&mut self, ty: StatusType) -> &mut String {
        let entry = &mut self.messages[ty];
        entry.get_or_insert_default()
    }

    pub fn set(&mut self, ty: StatusType, message: &str) { message.clone_into(self.get_mut(ty)) }

    pub fn unset(&mut self, ty: StatusType) { self.messages[ty] = None; }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, enum_map::Enum)]
pub enum StatusType {
    ObjectInfo,
}

fn update_status_system(
    config: Res<Config>,
    query: Single<(Entity, &mut Text, &Status, &mut ui::Node)>,
    mut container_config_params: ApplyContainerConfigParams,
) {
    let (entity, mut text, status, node) = query.into_inner();
    let value =
        status.messages.iter().rev().find_map(|(_, value)| value.as_deref()).unwrap_or_default();
    if text.0 != value {
        value.clone_into(&mut text.0);
    }

    apply_container_config(entity, node, &mut container_config_params, &config.status_container);
}

/// Stores the current feedback message for user interaction.
#[derive(Component)]
pub struct Feedback {
    pub messages: BTreeMap<FeedbackType, String>,
}

impl Feedback {
    pub fn set(&mut self, ty: FeedbackType, message: &str) {
        message.clone_into(self.messages.entry(ty).or_default());
    }

    pub fn get_mut(&mut self, ty: FeedbackType) -> &mut String {
        self.messages.entry(ty).or_default()
    }

    pub fn unset(&mut self, ty: FeedbackType) { self.messages.remove(&ty); }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub enum FeedbackType {
    ObjectSearch,
    ObjectControl,
}

fn update_feedback_system(
    config: Res<Config>,
    query: Single<(Entity, &mut Text, &Feedback, &mut ui::Node)>,
    mut container_config_params: ApplyContainerConfigParams,
) {
    let (entity, text, feedback, node) = query.into_inner();
    let value = feedback.messages.values().join("\n");
    Mut::map_unchanged(text, |Text(s)| s).set_if_neq(value);

    apply_container_config(entity, node, &mut container_config_params, &config.feedback_container);
}

#[derive(Resource)]
pub struct Config {
    /// Duration for which a log message remains.
    pub log_retention_time: Duration,
    /// Position of the log messages.
    pub log_container:      ContainerConfig,
    /// Color for info messages.
    pub log_info_color:     Color,
    /// Color for warning messages.
    pub log_warning_color:  Color,
    /// Color for error messages.
    pub log_error_color:    Color,

    /// Position of status messages.
    pub status_container: ContainerConfig,

    /// Position of user interaction feedback messages.
    pub feedback_container: ContainerConfig,
}

/// Configures the display position for a message container.
pub struct ContainerConfig {
    /// Position of the container border relative to the screen corner.
    pub anchor_position: Vec2,
    /// Display size of messages.
    pub text_size:       f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            log_container:      ContainerConfig {
                anchor_position: Vec2::new(-20., 20.),
                text_size:       12.,
            },
            log_retention_time: Duration::from_secs(10),
            log_info_color:     Color::WHITE,
            log_warning_color:  Color::srgb(1.0, 1.0, 0.2),
            log_error_color:    Color::srgb(1.0, 0.2, 0.4),

            status_container: ContainerConfig {
                anchor_position: Vec2::new(20., -50.),
                text_size:       12.,
            },

            feedback_container: ContainerConfig {
                anchor_position: Vec2::new(-20., -20.),
                text_size:       12.,
            },
        }
    }
}

#[derive(SystemParam)]
struct ApplyContainerConfigParams<'w, 's> {
    children_query:  Query<'w, 's, &'static Children>,
    text_font_query: Query<'w, 's, &'static mut TextFont>,
}

fn apply_container_config(
    entity: Entity,
    mut node: Mut<ui::Node>,
    ApplyContainerConfigParams { children_query, text_font_query }: &mut ApplyContainerConfigParams,
    config: &ContainerConfig,
) {
    let (left, right) = if config.anchor_position.x > 0. {
        (ui::Val::Px(config.anchor_position.x), ui::Val::Auto)
    } else {
        (ui::Val::Auto, ui::Val::Px(-config.anchor_position.x))
    };
    Mut::map_unchanged(node.reborrow(), |node| &mut node.left).set_if_neq(left);
    Mut::map_unchanged(node.reborrow(), |node| &mut node.right).set_if_neq(right);

    let (top, bottom) = if config.anchor_position.y > 0. {
        (ui::Val::Px(config.anchor_position.y), ui::Val::Auto)
    } else {
        (ui::Val::Auto, ui::Val::Px(-config.anchor_position.y))
    };
    Mut::map_unchanged(node.reborrow(), |node| &mut node.top).set_if_neq(top);
    Mut::map_unchanged(node, |node| &mut node.bottom).set_if_neq(bottom);

    for child in children_query.iter_descendants(entity).chain([entity]) {
        if let Ok(font) = text_font_query.get_mut(child) {
            Mut::map_unchanged(font, |font| &mut font.font_size).set_if_neq(config.text_size);
        }
    }
}
