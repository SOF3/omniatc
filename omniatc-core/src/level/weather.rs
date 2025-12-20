//! A weather entity specifies 2D environmental effects in a square,
//! such as ground elevation, temperature and pressure.

use std::marker::PhantomData;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::query::With;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{EntityCommand, Query, SystemParam};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::Vec2;
use bevy::math::bounding::Aabb2d;
use bevy_mod_config::{AppExt, Config, ConfigFieldFor, Manager, ReadConfig};
use math::{ISA_SEA_LEVEL_PRESSURE, ISA_SEA_LEVEL_TEMPERATURE, Position, Pressure, Temp};

use super::{SystemSets, object};
use crate::util::RateLimit;

pub mod loader;

pub struct Plug<M>(PhantomData<M>);

impl<M> Default for Plug<M> {
    fn default() -> Self { Self(PhantomData) }
}

impl<M: Manager + Default> Plugin for Plug<M>
where
    Conf: ConfigFieldFor<M>,
{
    fn build(&self, app: &mut App) {
        app.init_config::<M, Conf>("core:weather");
        app.add_systems(
            app::Update,
            detect_system.in_set(SystemSets::ExecuteEnviron).before(DetectorReaderSystemSet),
        );
    }
}

#[derive(Config)]
pub struct Conf {
    #[config(default = Duration::from_secs(1))]
    detect_period: Duration,
}

/// Weather environmental data.
#[derive(Component, Clone, Copy)]
pub struct Weather {
    /// Pressure at (extrapolated) sea level.
    pub sea_pressure: Pressure,
    /// Temperature at (extrapolated) sea level.
    pub sea_temp:     Temp,
}

impl Default for Weather {
    fn default() -> Self {
        Self { sea_pressure: ISA_SEA_LEVEL_PRESSURE, sea_temp: ISA_SEA_LEVEL_TEMPERATURE }
    }
}

/// This weather entity only applies to objects in the AABB.
#[derive(Component)]
pub struct EffectRegion(pub Aabb2d);

/// Marker component for weather entities.
#[derive(Component)]
pub struct Marker;

#[derive(Bundle)]
pub struct Comps {
    pub weather:       Weather,
    pub effect_region: EffectRegion,
}

pub struct SpawnCommand {
    pub bundle: Comps,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, mut entity: EntityWorldMut) { entity.insert((self.bundle, Marker)); }
}

/// Locates the weather effective at a point.
#[derive(SystemParam)]
pub struct Locator<'w, 's> {
    search_query:  Query<'w, 's, (Entity, &'static EffectRegion), With<Marker>>,
    weather_query: Query<'w, 's, &'static Weather>,
}

impl Locator<'_, '_> {
    /// Gets the weather component at the given position.
    #[must_use]
    pub fn locate(&self, object_pos: Position<Vec2>) -> Option<Entity> {
        // TODO use a quadtree.
        let object_pos = object_pos.get();
        self.search_query.iter().find_map(|(weather, EffectRegion(region))| {
            if (region.min.cmple(object_pos) & region.max.cmpge(object_pos)).all() {
                Some(weather)
            } else {
                None
            }
        })
    }

    #[must_use]
    pub fn get(&self, object_pos: Position<Vec2>) -> Weather {
        self.locate(object_pos)
            .map(|entity| {
                *self
                    .weather_query
                    .get(entity)
                    .expect("entities with Marker must also have Weather")
            })
            .unwrap_or_default()
    }
}

/// An [object](object::Object) that detects weather.
///
/// The value can be read by systems in [`DetectorReaderSystemSet`].
#[derive(Component, Default)]
pub struct Detector {
    pub last_match: Option<Entity>,
}

fn detect_system(
    mut rl: RateLimit,
    conf: ReadConfig<Conf>,
    locator: Locator,
    mut object_query: Query<(&mut Detector, &object::Object)>,
) {
    let conf = conf.read();

    if rl.should_run(conf.detect_period).is_none() {
        return;
    }

    object_query.par_iter_mut().for_each(|(mut detector, object)| {
        detector.last_match = locator.locate(object.position.horizontal());
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct DetectorReaderSystemSet;
