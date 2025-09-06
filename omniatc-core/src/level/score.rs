use bevy::app::{self, App, Plugin};
use bevy::ecs::change_detection::DetectChanges;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Res, ResMut};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Statistics>();
        app.init_resource::<Calculator>();
        app.init_resource::<Score>();
        app.add_systems(app::Update, update_score.after(StatisticsUpdater));
    }
}

/// Tracks the statistics contributing to the score.
#[derive(Resource, Default)]
pub struct Statistics {
    pub num_departures: u32,
    pub num_arrivals:   u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct StatisticsUpdater;

/// Determines how the score is calculated from the statistics.
#[derive(Resource, Default)]
pub struct Calculator {
    /// Score awarded per completed departure.
    pub score_per_departure: f32,
    pub score_per_arrival:   f32,
}

/// The current score.
#[derive(Resource, Default)]
pub struct Score(pub f32);

#[expect(clippy::cast_precision_loss)] // Score should not exceed the f32 mantissa precision 2^24.
fn update_score(
    statistics: Res<Statistics>,
    calculator: Res<Calculator>,
    mut score: ResMut<Score>,
) {
    if statistics.is_changed() {
        score.0 = statistics.num_departures as f32 * calculator.score_per_departure
            + statistics.num_arrivals as f32 * calculator.score_per_arrival;
    }
}
