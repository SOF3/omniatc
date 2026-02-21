//! A weather entity specifies 2D environmental effects in a square,
//! such as ground elevation, temperature, pressure and wind.

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
use bevy::math::bounding::Aabb2d;
use bevy::math::{Vec2, Vec3};
use bevy_mod_config::{AppExt, Config, ConfigFieldFor, Manager, ReadConfig};
use math::{
    Angle, ISA_SEA_LEVEL_PRESSURE, ISA_SEA_LEVEL_TEMPERATURE, Length, Position, Pressure, Speed,
    Temp,
};

use super::SystemSets;
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
    pub sea_pressure:         Pressure,
    /// Temperature at (extrapolated) sea level.
    pub sea_temp:             Temp,
    /// Wind velocity at sea level.
    pub sea_wind:             Speed<Vec2>,
    /// Base of the exponential magnitude scaling of wind per nautical mile of altitude.
    ///
    /// The wind magnitude at altitude `h` nm is `sea_wind * wind_scaling_per_nm.powf(h)`.
    pub wind_scaling_per_nm:  f32,
    /// Rotation of wind direction per nautical mile of altitude.
    pub wind_rotation_per_nm: Angle,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            sea_pressure:         ISA_SEA_LEVEL_PRESSURE,
            sea_temp:             ISA_SEA_LEVEL_TEMPERATURE,
            sea_wind:             Speed::ZERO,
            wind_scaling_per_nm:  1.0,
            wind_rotation_per_nm: Angle::ZERO,
        }
    }
}

impl Weather {
    /// Computes the wind velocity at the given altitude.
    #[must_use]
    pub fn wind_at_altitude(&self, altitude: Position<f32>) -> Speed<Vec2> {
        let altitude_nm = altitude.amsl() / Length::from_nm(1.0);
        let scale = self.wind_scaling_per_nm.powf(altitude_nm);
        let rotation = self.wind_rotation_per_nm * altitude_nm;
        self.sea_wind.rotate_clockwise(rotation) * scale
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
    /// Gets the weather entity at the given position.
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

    /// Gets the weather data at the given position, or the default if no weather entity covers it.
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

    /// Computes the wind at the given 3D position.
    #[must_use]
    pub fn wind(&self, object_pos: Position<Vec3>) -> Speed<Vec2> {
        self.get(object_pos.horizontal()).wind_at_altitude(object_pos.altitude())
    }
}

/// A component that detects weather at a position.
///
/// The value can be read by systems in [`DetectorReaderSystemSet`].
#[derive(Component)]
pub struct Detector {
    /// The 3D position to detect weather at, updated externally.
    pub position:   Position<Vec3>,
    pub last_match: Option<Entity>,
    pub last_wind:  Speed<Vec2>,
}

impl Default for Detector {
    fn default() -> Self {
        Self { position: Position::new(Vec3::ZERO), last_match: None, last_wind: Speed::ZERO }
    }
}

fn detect_system(
    mut rl: RateLimit,
    conf: ReadConfig<Conf>,
    locator: Locator,
    mut detector_query: Query<&mut Detector>,
) {
    let conf = conf.read();

    if rl.should_run(conf.detect_period).is_none() {
        return;
    }

    detector_query.par_iter_mut().for_each(|mut detector| {
        detector.last_match = locator.locate(detector.position.horizontal());
        let weather = detector
            .last_match
            .and_then(|entity| locator.weather_query.get(entity).ok())
            .copied()
            .unwrap_or_default();
        detector.last_wind = weather.wind_at_altitude(detector.position.altitude());
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct DetectorReaderSystemSet;
