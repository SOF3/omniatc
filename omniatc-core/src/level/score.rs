use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use store::Score;

pub mod loader;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stats>();
        app.configure_sets(app::Update, Writer.ambiguous_with(Writer));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct Writer;

/// The current score.
#[derive(Resource, Default)]
pub struct Stats {
    /// Total score.
    pub total: Score,

    /// Number of objects completed with runway arrival as their destination.
    ///
    /// Does not include apron arrivals.
    pub num_runway_arrivals: u32,
    /// Number of objects completed with apron arrival as their destination.
    pub num_apron_arrivals:  u32,
    /// Number of objects completed with waypoint departure as their destination.
    pub num_departures:      u32,

    /// Number of conflicting pairs that have been detected.
    pub num_conflicts:       u32,
    /// Total duration-pair time of all detected conflicts.
    pub total_conflict_time: Duration,
}
