use std::borrow::Cow;
use std::collections::HashMap;
use std::io;
use std::num::NonZero;

use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::system::Command as BevyCommand;
use bevy::ecs::world::World;
use math::sweep;

use crate::level::{aerodrome, object, route, score, spawn, waypoint, wind};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.init_resource::<CameraAdvice>(); }
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
            file_owned = ciborium::from_reader(bytes.as_ref()).map_err(Error::Serde)?;
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
    object::loader::spawn(
        world,
        &aerodromes,
        &waypoints,
        &route_presets,
        &mut next_standby_id,
        &file.objects,
    )?;

    world.resource_mut::<CameraAdvice>().0 = Some(file.ui.camera.clone());

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Deserialization error: {0}")]
    Serde(ciborium::de::Error<io::Error>),
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
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
pub type VecResult<T> = Result<Vec<T>>;
pub type HashMapResult<K, V> = Result<HashMap<K, V>>;
