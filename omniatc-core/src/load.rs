use std::borrow::Cow;
use std::collections::HashMap;
use std::io;

use bevy::app::{App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::resource::Resource;
use bevy::prelude::{Command as BevyCommand, Entity, With, World};
use math::sweep;

use crate::level::route::{self};
use crate::level::waypoint::{self};
use crate::level::{aerodrome, object, score, wind};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) { app.init_resource::<CameraAdvice>(); }
}

/// Marks that an entity was loaded from a save file, and should be deleted during reload.
#[derive(Component)]
pub struct LoadedEntity;

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
        .query_filtered::<Entity, With<LoadedEntity>>()
        .iter(world)
        .collect::<Vec<_>>()
        .into_iter()
        .for_each(|entity| world.entity_mut(entity).despawn());

    wind::loader::spawn(world, &file.level.environment.winds);
    let aerodromes = aerodrome::loader::spawn(world, &file.level.aerodromes)?;
    let waypoints = waypoint::loader::spawn(world, &file.level.waypoints);
    let route_presets =
        route::loader::spawn_presets(world, &aerodromes, &waypoints, &file.level.route_presets)?;
    score::loader::spawn(world, &file.stats);

    object::loader::spawn(world, &aerodromes, &waypoints, &route_presets, &file.objects)?;

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
    #[error("No route preset called {0:?}")]
    UnresolvedRoutePreset(String),
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
