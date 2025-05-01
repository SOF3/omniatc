//! Gameplay simulation.

use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::prelude::{Resource, SystemSet};
use itertools::Itertools;
use strum::IntoEnumIterator;

pub mod aerodrome;
pub mod ground;
pub mod nav;
pub mod object;
pub mod plane;
pub mod route;
pub mod runway;
pub mod waypoint;
pub mod wind;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Config>();

        for set in SystemSets::iter() {
            app.configure_sets(app::Update, set.in_set(AllSystemSets));
        }

        for (before, after) in SystemSets::iter().tuple_windows() {
            app.configure_sets(app::Update, before.before(after));
        }

        app.add_plugins(aerodrome::Plug);
        app.add_plugins(object::Plug);
        app.add_plugins(plane::Plug);
        app.add_plugins(nav::Plug);
        app.add_plugins(route::Plug);
        app.add_plugins(runway::Plug);
        app.add_plugins(waypoint::Plug);
        app.add_plugins(ground::Plug);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
pub enum SystemSets {
    /// Direct response to environmental changes such as wind, cloud base and visibility.
    PrepareEnviron,
    /// Systems representing communication operations of a plane.
    Communicate,
    /// Systems executing a complex flight plan that decides navigation targets.
    Action,
    /// Systems simulating absolute position navigation.
    Navigate,
    /// Systems simulating machine effects on environmental parameters.
    Aviate,
    /// Systems simulating environmental physics such as wind.
    ExecuteEnviron,
    /// Reconcile aviation-related components not involved in simulation but useful for other modules to read.
    ReconcileForRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct AllSystemSets;

#[derive(Resource)]
pub struct Config {
    /// Number of positions tracked per object.
    ///
    /// The oldest positions are removed when the log exceeds the limit.
    pub max_track_log: usize,
    /// Duration between two points in an object track log.
    pub track_density: Duration,
}

impl Default for Config {
    fn default() -> Self { Self { max_track_log: 1024, track_density: Duration::from_secs(5) } }
}
