use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::Event;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_event::<SendEvent>(); }
}

#[derive(Event)]
pub struct SendEvent {
    /// The sender of the message. Must be a [`Sender`].
    pub source:  Entity,
    /// The message content.
    pub message: String,
    /// Classify the message by verbosity.
    pub class:   Class,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    /// Verbose information that does not need acknowledgement,
    /// e.g. handover transmission.
    VerboseInfo,
    /// Normal transmission that needs response,
    /// e.g. deviation request.
    NeedAck,
    /// Information from abnormal events, e.g. a missed approach.
    AnomalyInfo,
    /// Transmission that needs urgent response,
    /// e.g. an imminient separation conflict.
    Urgent,
}

/// An entity that could produce events.
#[derive(Component)]
pub struct Sender {
    /// Display name of this entity.
    pub display: String,
}
