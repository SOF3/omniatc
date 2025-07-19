//! A wind entity applies a velocity component to objects in its effective region.

use std::marker::PhantomData;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::bundle::Bundle;
use bevy::ecs::component::Component;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{EntityCommand, Query, Res, SystemParam};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::bounding::Aabb3d;
use bevy::math::{Vec2, Vec3A};
use bevy_mod_config::{AppExt, Config, ConfigFieldFor, Manager, ReadConfig};
use math::{Position, Speed};

use super::{object, SystemSets};
use crate::util::RateLimit;

pub struct Plug<M>(PhantomData<M>);

impl<M> Default for Plug<M> {
    fn default() -> Self { Self(PhantomData) }
}

impl<M: Manager + Default> Plugin for Plug<M>
where
    Conf: ConfigFieldFor<M>,
{
    fn build(&self, app: &mut App) {
        app.init_config::<M, Conf>("core:wind");
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

/// The direction and strength of wind.
#[derive(Component)]
pub struct Vector {
    /// The wind vector at the lowest altitude of the region.
    pub bottom: Speed<Vec2>,
    /// The wind vector at the highest altitude of the region.
    pub top:    Speed<Vec2>,
}

/// This wind entity only applies to objects in the AABB.
#[derive(Component)]
pub struct EffectRegion(pub Aabb3d);

/// Marker component for wind entities.
#[derive(Component)]
pub struct Marker;

#[derive(Bundle)]
pub struct Comps {
    pub vector:        Vector,
    pub effect_region: EffectRegion,
}

pub struct SpawnCommand {
    pub bundle: Comps,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, mut entity: EntityWorldMut) { entity.insert((self.bundle, Marker)); }
}

/// Locates the winds effective at a point.
#[derive(SystemParam)]
pub struct Locator<'w, 's> {
    wind_query: Query<'w, 's, (&'static Vector, &'static EffectRegion), With<Marker>>,
}

impl Locator<'_, '_> {
    /// Computes the total wind component at a point.
    pub fn locate(&self, object_pos: Position<impl Into<Vec3A>>) -> Speed<Vec2> {
        // TODO use an appropriate range query data structure if necessary.
        let object_pos = object_pos.get().into();
        self.wind_query
            .iter()
            .filter_map(|(vector, EffectRegion(region))| {
                if (region.min.cmple(object_pos) & region.max.cmpge(object_pos)).all() {
                    let level = (object_pos.z - region.min.z) / (region.max.z - region.min.z);
                    Some(vector.bottom.lerp(vector.top, level))
                } else {
                    None
                }
            })
            .sum()
    }
}

/// An [object](object::Object) that detects wind speed.
///
/// The value can be read by systems in [`DetectorReaderSystemSet`].
#[derive(Component, Default)]
pub struct Detector {
    pub last_computed: Speed<Vec2>,
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
        detector.last_computed = locator.locate(object.position);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct DetectorReaderSystemSet;
