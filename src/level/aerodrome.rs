use bevy::prelude::Component;

#[derive(Component)]
pub struct Display {
    pub id:   u32,
    pub name: String,
}
