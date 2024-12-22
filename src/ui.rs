use bevy::app::{self, App, Plugin};
use bevy::prelude::{AppExtStates, IntoSystemSetConfigs, States, SystemSet};

mod billboard;
mod camera;
mod object;
mod store;
mod waypoint;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_state::<InputState>();

        app.add_plugins(camera::Plug);
        app.add_plugins(billboard::Plug);
        app.add_plugins(object::Plug);
        app.add_plugins(waypoint::Plug);
        app.add_plugins(store::Plug);

        app.configure_sets(app::Update, SystemSets::RenderSpawn.before(SystemSets::RenderMove));
        app.configure_sets(
            app::Update,
            SystemSets::RenderMove.ambiguous_with(SystemSets::RenderMove),
        );
        app.configure_sets(
            app::Update,
            (SystemSets::RenderSpawn, SystemSets::RenderMove).in_set(SystemSets::RenderAll),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum SystemSets {
    Input,
    RenderAll,
    RenderSpawn,
    RenderMove,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States)]
pub enum InputState {
    #[default]
    Normal,
}

/// Renderable layers.
///
/// The first item is the lowest layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumCount)]
#[repr(u32)]
pub enum Zorder {
    Terrain,
    Waypoint,
    WaypointLabel,
    Object,
    ObjectLabel,
    ScaleRuler,
    ScaleRulerLabel,
}

impl Zorder {
    #[allow(clippy::cast_precision_loss)] // the number of items is small
    pub const fn to_z(self) -> f32 {
        (self as u32 as f32) / (<Self as strum::EnumCount>::COUNT as f32)
    }
}
