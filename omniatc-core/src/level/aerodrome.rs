use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Message;
use math::Position;

pub mod loader;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_message::<SpawnMessage>(); }
}

/// Display metadata of an aerodrome.
#[derive(Component)]
pub struct Aerodrome {
    /// Serial ID of the aerodrome, used to determine its color code.
    pub id:        u32,
    /// Identifier code of the aerodrome.
    pub code:      String,
    /// Display name of the aerodrome.
    pub name:      String,
    /// Elevation of the aerodrome.
    pub elevation: Position<f32>,
}

#[derive(Message)]
pub struct SpawnMessage(pub Entity);
