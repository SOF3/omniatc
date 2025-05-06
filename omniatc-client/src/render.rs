use bevy::app::{self, App, Plugin};
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use itertools::Itertools;
use strum::IntoEnumIterator;

pub mod config_editor;
mod level_info;
mod messages;
mod object_info;
pub mod twodim;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            messages::Plug,
            config_editor::Plug,
            level_info::Plug,
            object_info::Plug,
            twodim::Plug,
        ));

        for set in SystemSets::iter() {
            app.configure_sets(app::Update, set.in_set(crate::UpdateSystemSets::Render));
        }
        for (before, after) in SystemSets::iter().tuple_windows() {
            app.configure_sets(app::Update, before.before(after));
        }

        app.configure_sets(app::Update, SystemSets::Spawn.ambiguous_with(SystemSets::Spawn));
        app.configure_sets(app::Update, SystemSets::Update.ambiguous_with(SystemSets::Update));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
pub enum SystemSets {
    /// Update existing entities for config changes.
    Reload,
    /// Spawn new entities.
    Spawn,
    /// Update existing entities regularly.
    Update,
}
