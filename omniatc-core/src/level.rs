//! Gameplay simulation.

use std::marker::PhantomData;

use bevy::app::{self, App, Plugin};
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::prelude::SystemSet;
use bevy_mod_config::{ConfigFieldFor, Manager};
use itertools::Itertools;
use strum::IntoEnumIterator;

pub mod aerodrome;
pub mod comm;
pub mod ground;
pub mod message;
pub mod nav;
pub mod navaid;
pub mod object;
pub mod plane;
pub mod route;
pub mod runway;
pub mod taxi;
pub mod wake;
pub mod waypoint;
pub mod wind;

pub struct Plug<M>(PhantomData<M>);

impl<M> Default for Plug<M> {
    fn default() -> Self { Self(PhantomData) }
}

impl<M: Manager + Default> Plugin for Plug<M>
where
    object::Conf: ConfigFieldFor<M>,
    wake::Conf: ConfigFieldFor<M>,
    wind::Conf: ConfigFieldFor<M>,
{
    fn build(&self, app: &mut App) {
        for set in SystemSets::iter() {
            app.configure_sets(app::Update, set.in_set(AllSystemSets));
        }

        for (before, after) in SystemSets::iter().tuple_windows() {
            app.configure_sets(app::Update, before.before(after));
        }

        app.add_plugins(message::Plug);
        app.add_plugins(aerodrome::Plug);
        app.add_plugins(object::Plug::<M>::default());
        app.add_plugins(plane::Plug);
        app.add_plugins(nav::Plug);
        app.add_plugins(navaid::Plug);
        app.add_plugins(route::Plug);
        app.add_plugins(comm::Plug);
        app.add_plugins(runway::Plug);
        app.add_plugins(waypoint::Plug);
        app.add_plugins(ground::Plug);
        app.add_plugins(taxi::Plug);
        app.add_plugins(wake::Plug::<M>::default());
        app.add_plugins(wind::Plug::<M>::default());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet, strum::EnumIter)]
pub enum SystemSets {
    /// Direct response to environmental changes such as wind, cloud base and visibility.
    /// Does not directly affect aircraft.
    PrepareEnviron,
    /// Systems representing communication operations of an object.
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
    /// Systems simulating effects *on* the environment *from* controlled objects.
    AffectEnviron,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct AllSystemSets;
