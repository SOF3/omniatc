use bevy::app::{self, App, Plugin};
use bevy::prelude::{in_state, AppExtStates, IntoSystemSetConfigs, States, SystemSet};
use strum::IntoEnumIterator;

mod billboard;
mod camera;
mod clock;
mod message;
mod object;
mod store;
mod track;
mod waypoint;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_state::<InputState>();

        app.add_plugins(camera::Plug);
        app.add_plugins(message::Plug);
        app.add_plugins(clock::Plug);
        app.add_plugins(billboard::Plug);
        app.add_plugins(object::Plug);
        app.add_plugins(waypoint::Plug);
        app.add_plugins(store::Plug);
        app.add_plugins(track::Plug);

        app.configure_sets(app::Update, SystemSets::RenderSpawn.before(SystemSets::RenderMove));
        app.configure_sets(
            app::Update,
            SystemSets::RenderMove.ambiguous_with(SystemSets::RenderMove),
        );
        app.configure_sets(
            app::Update,
            (SystemSets::RenderSpawn, SystemSets::RenderMove).in_set(SystemSets::RenderAll),
        );

        for (i, state) in InputState::iter().enumerate() {
            app.configure_sets(app::Update, state.in_set(SystemSets::Input));
            app.configure_sets(app::Update, state.run_if(in_state(state)));

            for other_state in InputState::iter().skip(i + 1) {
                app.configure_sets(app::Update, state.ambiguous_with(other_state));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum SystemSets {
    Input,
    RenderAll,
    RenderSpawn,
    RenderMove,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, States, SystemSet, strum::EnumIter)]
pub enum InputState {
    /// Root menu.
    #[default]
    Root,
    /// Searching objects by name.
    ObjectSearch,
    /// Operates on a specific object.
    ObjectAction,
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
    ObjectSeparation,
    ObjectLabel,
    ScaleRuler,
    ScaleRulerLabel,
    MessageText,
}

impl Zorder {
    #[allow(clippy::cast_precision_loss)] // the number of items is small
    pub const fn to_z(self) -> f32 {
        (self as u32 as f32) / (<Self as strum::EnumCount>::COUNT as f32)
    }
}
