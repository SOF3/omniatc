use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Message;
use bevy::ecs::system::EntityCommand;
use bevy::ecs::world::EntityWorldMut;
use bevy::math::Vec3;
use math::Position;

pub mod loader;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.add_message::<SpawnMessage>(); }
}

#[derive(Component)]
pub struct Waypoint {
    /// Display name of the waypoint.
    pub name:         String,
    /// Type of the waypoint as displayed.
    pub display_type: DisplayType,
    /// Position of the waypoint.
    ///
    /// The altitude component is only used to compute the navaid range.
    pub position:     Position<Vec3>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayType {
    /// A normal point on the map.
    Waypoint,
    /// A VHF Omnidirectional Range station with Distance Measuring Equipment.
    VorDme,
    /// A VHF Omnidirectional Range station.
    Vor,
    /// A Distance Measuring Equipment station.
    Dme,
    /// The waypoint should not be displayed on the map.
    /// Used for virtual waypoints.
    None,
    /// The waypoint is a runway.
    /// This will prevent the waypoint UI from rendering any icons,
    /// but unlike `None`, the label is still rendered.
    Runway,
}

impl DisplayType {
    /// Whether a label should be rendered for this display type.
    #[must_use]
    pub fn should_display_label(&self) -> bool { !matches!(self, Self::None) }
}

pub struct SpawnCommand {
    pub waypoint: Waypoint,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        entity.insert(self.waypoint);
        let entity_id = entity.id();
        entity.world_scope(|world| world.write_message(SpawnMessage(entity_id)));
    }
}

#[derive(Message)]
pub struct SpawnMessage(pub Entity);
