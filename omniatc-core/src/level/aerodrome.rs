use bevy::prelude::Component;

/// Display metadata of an aerodrome.
#[derive(Component)]
pub struct Display {
    /// Serial ID of the aerodrome, used to determine its color code.
    pub id:   u32,
    /// Identifier code of the aerodrome.
    pub code: String,
    /// Display name of the aerodrome.
    pub name: String,
}
