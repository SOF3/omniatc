use bevy::app::{self, App, Plugin};
use bevy::prelude::{AppExtStates, IntoSystemSetConfigs, States, SystemSet};

mod camera;
mod render;
mod store;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_state::<InputState>();

        app.add_plugins(camera::Plug);
        app.add_plugins(render::Plug);
        app.add_plugins(store::Plug);

        app.configure_sets(app::Update, SystemSets::RenderSpawn.before(SystemSets::RenderMove));
        app.configure_sets(app::Update, SystemSets::RenderMove.ambiguous_with(SystemSets::RenderMove));
        app.configure_sets(app::Update, (SystemSets::RenderSpawn, SystemSets::RenderMove).in_set(SystemSets::RenderAll));
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
