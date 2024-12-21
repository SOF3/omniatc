//! Gameplay simulation.

use bevy::app::{self, App, Plugin};
use bevy::prelude::{IntoSystemSetConfigs, SystemSet};

pub mod nav;
pub mod object;
pub mod plane;
pub mod wind;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.configure_sets(app::Update, SystemSets::Navigate.before(SystemSets::Pilot));
        app.configure_sets(app::Update, SystemSets::Pilot.before(SystemSets::Machine));
        app.configure_sets(app::Update, SystemSets::Machine.before(SystemSets::Environ));
        app.configure_sets(app::Update, SystemSets::Environ.before(SystemSets::Reconcile));
        app.configure_sets(
            app::Update,
            (
                SystemSets::Navigate,
                SystemSets::Pilot,
                SystemSets::Machine,
                SystemSets::Environ,
                SystemSets::Reconcile,
            )
                .in_set(SystemSets::All),
        );
        app.add_plugins(object::Plug);
        app.add_plugins(plane::Plug);
        app.add_plugins(nav::Plug);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum SystemSets {
    /// Systems simulating absolute position navigation.
    Navigate,
    /// Systems simulating machine input control.
    Pilot,
    /// Systems simulating machine effects on environmental parameters.
    Machine,
    /// Systems simulating environmental physics.
    Environ,
    /// Reconcile components not involved in simulation but useful for other modules to read.
    Reconcile,
    /// All systems belong to this system set.
    All,
}
