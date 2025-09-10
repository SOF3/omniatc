use bevy::app::{self, App, Plugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use store::Score;

pub mod loader;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Scores>();
        app.configure_sets(app::Update, Writer.ambiguous_with(Writer));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct Writer;

/// The current score.
#[derive(Resource, Default)]
pub struct Scores {
    /// Total score.
    pub total: Score,

    /// Number of arrivals completed.
    pub num_arrivals:   u32,
    /// Number of departures completed.
    pub num_departures: u32,
}
