use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::system::{Command, Commands, Query, Res};
use bevy::ecs::world::World;
use bevy::time::{self, Time};


pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_systems(app::Update, expire_message_system); }
}

#[derive(Component)]
pub struct Message {
    /// The sender of the message. Must be a [`Sender`].
    pub source:  Entity,
    /// The virtual time when the message was created.
    pub created: Duration,
    /// The message content.
    pub content: String,
    /// Classify the message by verbosity.
    pub class:   Class,
}

#[derive(Component)]
pub struct Expiry {
    /// The message is removed when the [virtual](time::Virtual) time elapsed exceeds this
    /// duration.
    pub expiry: Duration,
}

fn expire_message_system(
    mut commands: Commands,
    messages: Query<(Entity, &Expiry)>,
    time: Res<Time<time::Virtual>>,
) {
    for (entity, expiry) in messages {
        if time.elapsed() > expiry.expiry {
            commands.entity(entity).despawn();
        }
    }
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

pub struct SendExpiring {
    pub source:   Entity,
    pub content:  String,
    pub class:    Class,
    pub duration: Duration,
}

impl Command for SendExpiring {
    fn apply(self, world: &mut World) {
        let time = world.resource::<Time<time::Virtual>>();
        let expiry = Expiry { expiry: time.elapsed() + self.duration };
        let message = Message {
            source:  self.source,
            created: time.elapsed(),
            content: self.content,
            class:   self.class,
        };
        world.spawn((message, expiry));
    }
}
