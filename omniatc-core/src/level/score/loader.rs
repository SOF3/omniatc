use bevy::ecs::world::World;

use crate::level::score;

pub fn spawn(world: &mut World, stats: &store::Stats) {
    *world.resource_mut::<score::Stats>() = score::Stats {
        total:               stats.score,
        num_runway_arrivals: stats.num_runway_arrivals,
        num_apron_arrivals:  stats.num_apron_arrivals,
        num_departures:      stats.num_departures,
        num_conflicts:       stats.num_conflicts,
        total_conflict_time: stats.total_conflict_time,
    };
}
