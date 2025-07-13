//! Wake turbulence simulation.
//!
//! Objects with the [`Producer`] component spawn a [`Vortex`] entity
//! every [`Conf::spawn_period`] seconds.
//! The intensity of a vortex entity dissipates over time,
//! despawning when it reaches zero.
//!
//! Each vortex affects aircraft in
//! the 27 0.25nm &times; 0.25nm &times; 500ft cuboids around it.
//! An aircraft experiences the *max* (not sum) intensity of the vortices
//! affecting the cuboid it belongs to.
//!
//! Wake intensity is measured in terms of [virtual clock time](time::Virtual).
//! An intensity of one second diminishes after one second of virtual clock time.

use std::collections::{hash_map, HashMap};
use std::hash::Hash;
use std::mem;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::event::{Event, EventWriter};
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::{IntoScheduleConfigs, SystemSet};
use bevy::ecs::system::{Commands, Local, Query, Res, ResMut};
use bevy::math::Vec3;
use bevy::time::{self, Time};
use math::{Distance, Position, Speed};
use smallvec::SmallVec;

use super::{object, wind, SystemSets};
use crate::try_log;
use crate::util::RateLimit;

const GRID_SIZE: Distance<Vec3> =
    Distance::from_nm(0.25).splat2().with_vertical(Distance::from_feet(500.));

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Conf>();
        app.init_resource::<VortexIndex>();
        app.add_event::<SpawnEvent>();
        app.add_systems(app::Update, dissipate_vortex_system.in_set(SystemSets::PrepareEnviron));
        app.add_systems(
            app::Update,
            wind_move_vortex_system
                .after(dissipate_vortex_system)
                .in_set(SystemSets::PrepareEnviron),
        );
        app.add_systems(app::Update, spawn_vortex_system.in_set(SystemSets::AffectEnviron));
        app.add_systems(
            app::Update,
            detect_vortex_system
                .before(DetectorReaderSystemSet)
                .in_set(SystemSets::AffectEnviron)
                .before(spawn_vortex_system),
        );
    }
}

#[derive(Resource)]
pub struct Conf {
    /// The period at which vortex entities are spawned.
    pub spawn_period:  Duration,
    /// The period at which vortex entities are spawned.
    pub detect_period: Duration,
    /// Vertical rate (negative sink rate) for vortex entities.
    pub vert_rate:     Speed<f32>,
}

impl Default for Conf {
    fn default() -> Self {
        Self {
            spawn_period:  Duration::from_secs(5),
            detect_period: Duration::from_secs(1),
            vert_rate:     Speed::from_fpm(400.),
        }
    }
}

#[derive(Component)]
pub struct Vortex {
    pub intensity: Intensity,
    pub position:  Position<Vec3>,
    pub source:    Entity,
}

/// Wake intensity, in terms of the number of milliseconds to dissipate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Intensity(pub u32);

impl Intensity {
    #[must_use]
    pub fn map(self, f: impl FnOnce(u32) -> u32) -> Self { Self(f(self.0)) }
}

fn wind_move_vortex_system(
    time: Res<Time<time::Virtual>>,
    vortex_query: Query<(Entity, &mut Vortex)>,
    conf: Res<Conf>,
    wind_locator: wind::Locator,
    mut last_run: Local<Duration>,
    mut vortex_index: ResMut<VortexIndex>,
) {
    let delta_time = time.elapsed() - mem::replace(&mut *last_run, time.elapsed());

    for (entity, mut vortex) in vortex_query {
        let speed = wind_locator.locate(vortex.position);

        let old_position = vortex.position;
        vortex.position += speed.with_vertical(conf.vert_rate) * delta_time;
        vortex_index.relocate(old_position, vortex.position, entity);
    }
}

#[derive(Default, Resource)]
struct VortexIndex {
    index: HashMap<[i32; 3], SmallVec<[Entity; 1]>>,
}

impl VortexIndex {
    fn insert(&mut self, position: Position<Vec3>, entity: Entity) {
        let key = vortex_position_index(position, GRID_SIZE);
        self.index.entry(key).or_default().push(entity);
    }

    fn relocate(
        &mut self,
        old_position: Position<Vec3>,
        new_position: Position<Vec3>,
        entity: Entity,
    ) {
        let old_key = vortex_position_index(old_position, GRID_SIZE);
        let new_key = vortex_position_index(new_position, GRID_SIZE);
        if old_key == new_key {
            return;
        }

        let hash_map::Entry::Occupied(mut old_entry) = self.index.entry(old_key) else {
            panic!("old key did not exist")
        };
        let index = old_entry
            .get()
            .iter()
            .position(|&e| e == entity)
            .expect("entity does not exist in old key");
        if old_entry.get().len() == 1 {
            old_entry.remove();
        } else {
            old_entry.get_mut().swap_remove(index);
        }

        self.index.entry(new_key).or_default().push(entity);
    }

    fn remove(&mut self, position: Position<Vec3>, entity: Entity) {
        let key = vortex_position_index(position, GRID_SIZE);

        let hash_map::Entry::Occupied(mut old_entry) = self.index.entry(key) else {
            panic!("old key did not exist")
        };
        let index = old_entry
            .get()
            .iter()
            .position(|&e| e == entity)
            .expect("entity does not exist in old key");
        if old_entry.get().len() == 1 {
            old_entry.remove();
        } else {
            old_entry.get_mut().swap_remove(index);
        }
    }

    fn find_around(
        &self,
        vortex_query: &Query<&Vortex>,
        position: Position<Vec3>,
        exclude: Entity,
    ) -> Intensity {
        let center_key = vortex_position_index(position, GRID_SIZE);

        cuboid_around(center_key)
            .filter_map(|key| self.index.get(&key))
            .flatten()
            .filter_map(|&entity| {
                Some(try_log!(
                    vortex_query.get(entity),
                    expect "entity in index must be valid vortex entity"
                    or return None
                ))
            })
            .filter(|vortex| vortex.source != exclude)
            .map(|vortex| vortex.intensity)
            .max()
            .unwrap_or_default()
    }
}

fn vortex_position_index(position: Position<Vec3>, grid_size: Distance<Vec3>) -> [i32; 3] {
    let Vec3 { x, y, z } = (position.get() / grid_size.0).floor();
    #[expect(clippy::cast_possible_truncation)] // intended truncation
    [x, y, z].map(|f| f as i32)
}

fn cuboid_around(index: [i32; 3]) -> impl Iterator<Item = [i32; 3]> {
    let components = index.map(|v| [v - 1, v, v + 1]);
    components[0].into_iter().flat_map(move |x| {
        components[1]
            .into_iter()
            .flat_map(move |y| components[2].into_iter().map(move |z| [x, y, z]))
    })
}

fn dissipate_vortex_system(
    time: Res<Time<time::Virtual>>,
    vortex_query: Query<(Entity, &mut Vortex)>,
    mut commands: Commands,
    mut index: ResMut<VortexIndex>,
) {
    let delta_ms = time.elapsed().as_millis() - (time.elapsed() - time.delta()).as_millis();
    let delta_ms = u32::try_from(delta_ms).unwrap_or(u32::MAX);

    if delta_ms == 0 {
        return;
    }

    for (entity, mut vortex) in vortex_query {
        let intensity = &mut vortex.intensity.0;
        *intensity = intensity.saturating_sub(delta_ms);

        if *intensity == 0 {
            index.remove(vortex.position, entity);
            commands.entity(entity).despawn();
        }
    }
}

/// A component on airborne [objects](object) to indicate that it produces wake.
#[derive(Component)]
pub struct Producer {
    /// The base intensity of vortices created.
    ///
    /// Realistically, this is proportional to `weight / width`.
    /// The actual intensity will be divided by the airspeed in knots.
    pub base_intensity: Intensity,
}

fn spawn_vortex_system(
    mut rl: RateLimit,
    mut commands: Commands,
    conf: Res<Conf>,
    mut index: ResMut<VortexIndex>,
    aircraft_query: Query<(Entity, &object::Object, &object::Airborne, &Producer)>,
    mut spawn_event_writer: EventWriter<SpawnEvent>,
) {
    if rl.should_run(conf.spawn_period).is_none() {
        return;
    }

    let mut spawn_events = Vec::new();
    for (object_id, object, airborne, producer) in aircraft_query {
        let vortex = commands
            .spawn((
                #[expect(clippy::cast_possible_truncation)] // arbitrary rounding is allowed
                #[expect(clippy::cast_sign_loss)] // magnitude is never negative
                Vortex {
                    source:    object_id,
                    position:  object.position,
                    intensity: producer
                        .base_intensity
                        .map(|v| v / (airborne.airspeed.magnitude_exact().into_knots() as u32)),
                },
            ))
            .id();
        index.insert(object.position, vortex);
        spawn_events.push(SpawnEvent(vortex));
    }
    spawn_event_writer.write_batch(spawn_events);
}

/// A component on [objects](object) to indicate that it needs to know how much *external* wake it
/// senses.
#[derive(Component, Default)]
pub struct Detector {
    pub last_detected: Intensity,
}

fn detect_vortex_system(
    mut rl: RateLimit,
    conf: Res<Conf>,
    index: Res<VortexIndex>,
    mut object_query: Query<(Entity, &object::Object, &mut Detector)>,
    vortex_query: Query<&Vortex>,
) {
    if rl.should_run(conf.detect_period).is_none() {
        return;
    }

    object_query.par_iter_mut().for_each(|(object_id, object, mut detector)| {
        detector.last_detected = index.find_around(&vortex_query, object.position, object_id);
    });
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub struct DetectorReaderSystemSet;

#[derive(Event)]
pub struct SpawnEvent(pub Entity);
