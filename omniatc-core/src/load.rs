use std::borrow::Cow;
use std::collections::HashMap;
use std::num::NonZero;
use std::sync::Arc;

use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::Command as BevyCommand;
use bevy::ecs::world::World;
use math::sweep;

use crate::level::{aerodrome, object, quest, route, score, spawn, waypoint, wind};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraAdvice>();
        app.init_resource::<SpawnContext>();
    }
}

/// Marks an entity as part of a loaded level,
/// so it should be removed when loading a new level.
#[derive(Component)]
pub struct StoredEntity;

#[derive(Resource, Default)]
pub struct CameraAdvice(pub Option<store::Camera>);

#[cfg(test)]
mod tests;

pub enum Source {
    Raw(Cow<'static, [u8]>),
    Parsed(Box<store::File>),
}

pub struct Command {
    pub source:   Source,
    pub on_error: Box<dyn FnOnce(&mut World, Error) + Send>,
}

impl BevyCommand for Command {
    fn apply(self, world: &mut World) {
        if let Err(err) = do_load(world, &self.source) {
            (self.on_error)(world, err);
        }
    }
}

fn do_load(world: &mut World, source: &Source) -> Result<(), Error> {
    let file_owned: store::File;
    let file = match source {
        Source::Raw(bytes) => {
            file_owned = store::File::from_osav(bytes.as_ref()).map_err(Error::Deserialize)?;
            &file_owned
        }
        Source::Parsed(file) => file,
    };

    world
        .query_filtered::<Entity, With<StoredEntity>>()
        .iter(world)
        .collect::<Vec<_>>()
        .into_iter()
        .for_each(|entity| world.entity_mut(entity).despawn());

    let mut next_standby_id = const { NonZero::new(1).unwrap() };

    wind::loader::spawn(world, &file.level.environment.winds);
    let object_types = object::loader::spawn_types(world, &file.level.object_types);
    let aerodromes = aerodrome::loader::spawn(world, &file.level.aerodromes)?;
    let waypoints = waypoint::loader::spawn(world, &file.level.waypoints);
    let route_presets = route::loader::spawn_presets(
        world,
        &aerodromes,
        &waypoints,
        &mut next_standby_id,
        &file.level.route_presets,
    )?;
    spawn::loader::spawn_sets(
        world,
        &object_types,
        &aerodromes,
        &waypoints,
        &route_presets,
        &file.level.spawn_sets,
    )?;
    spawn::loader::spawn_trigger(world, &file.level.spawn_trigger);
    score::loader::spawn(world, &file.stats);
    for object in &file.objects {
        object::loader::spawn(
            world,
            &aerodromes,
            &waypoints,
            &route_presets,
            &mut next_standby_id,
            object,
        )?;
    }
    quest::loader::spawn(world, &file.quests, &aerodromes)?;

    world.resource_mut::<CameraAdvice>().0 = Some(file.ui.camera.clone());
    *world.resource_mut::<SpawnContext>() = SpawnContext {
        aerodromes: Arc::new(aerodromes),
        waypoints: Arc::new(waypoints),
        route_presets: Arc::new(route_presets),
        next_standby_id,
    };

    Ok(())
}

/// Stores the level loading state such as spawned entity maps.
///
/// This resource must not be used in the loader modules
/// since the resource is only updated after loading is complete;
/// it is available as a resource for executing completion hooks.
#[derive(Resource)]
pub struct SpawnContext {
    pub aerodromes:      Arc<aerodrome::loader::AerodromeMap>,
    pub waypoints:       Arc<waypoint::loader::WaypointMap>,
    pub route_presets:   Arc<route::loader::RoutePresetMap>,
    pub next_standby_id: NonZero<u32>,
}

impl Default for SpawnContext {
    fn default() -> Self {
        Self {
            aerodromes:      Arc::default(),
            waypoints:       Arc::default(),
            route_presets:   Arc::default(),
            next_standby_id: const { NonZero::new(1).unwrap() },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Deserialization error: {0}")]
    Deserialize(store::FileDeError),
    #[error("Too many aerodromes")]
    TooManyAerodromes,
    #[error("No aerodrome called {0:?}")]
    UnresolvedAerodrome(String),
    #[error("No runway called {runway:?} in aerodrome {aerodrome:?}")]
    UnresolvedRunway { aerodrome: String, runway: String },
    #[error("No waypoint called {0:?}")]
    UnresolvedWaypoint(String),
    #[error("No {variant} called {value:?} in aerodrome {aerodrome:?}")]
    UnresolvedSegment { variant: &'static str, value: String, aerodrome: String },
    #[error(
        "Object {object:?} is not near any {variant} named {value:?} in aerodrome {aerodrome:?}"
    )]
    NotOnSegment {
        object:    String,
        variant:   &'static str,
        value:     String,
        aerodrome: String,
    },
    #[error("No route preset called {0:?}")]
    UnresolvedRoutePreset(String),
    #[error("No object type called {0:?}")]
    UnresolvedObjectType(String),
    #[error("Non-finite value encountered at {0}")]
    NonFiniteFloat(&'static str),
    #[error(
        "The backward direction of apron {0} does not intersect with any taxiways within 100nm"
    )]
    UnreachableApron(String),
    #[error("Resolve ground lines: {0}")]
    GroundSweep(sweep::Error),
    #[error("No quest with ID {0:?}")]
    UnresolvedQuest(String),
}

pub type Result<T = (), E = Error> = std::result::Result<T, E>;
pub type VecResult<T> = Result<Vec<T>>;
pub type HashMapResult<K, V> = Result<HashMap<K, V>>;
