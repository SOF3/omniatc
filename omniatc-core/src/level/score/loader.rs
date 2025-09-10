use bevy::ecs::world::World;

use crate::level::score;

pub fn spawn(world: &mut World, stats: &store::Stats) {
    *world.resource_mut::<score::Scores>() = score::Scores {
        total:          stats.score,
        num_arrivals:   stats.num_arrivals,
        num_departures: stats.num_departures,
    };
}
