//! A moveable vehicle in the level,
//! in particular, a ground vehicle or an aircraft.

use std::collections::VecDeque;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::system::SystemState;
use bevy::math::{Quat, Vec3, Vec3A};
use bevy::prelude::{
    Component, Entity, EntityCommand, Event, IntoSystemConfigs, Query, Res, With, World,
};
use bevy::time::{self, Time, Timer, TimerMode};

use super::{wind, Config, SystemSets};
use crate::math::{
    PRESSURE_DENSITY_ALTITUDE_POW, STANDARD_LAPSE_RATE, STANDARD_SEA_LEVEL_TEMPERATURE,
    TAS_DELTA_PER_NM, TROPOPAUSE_ALTITUDE,
};

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnEvent>();
        app.add_systems(app::Update, update_airborne_system.in_set(SystemSets::Environ));
        app.add_systems(
            app::Update,
            move_object_system.after(update_airborne_system).in_set(SystemSets::Environ),
        );
        app.add_systems(app::Update, track_position_system.in_set(SystemSets::Reconcile));
    }
}

/// Marker component for object entities.
#[derive(Component)]
pub struct Marker;

/// Display details of an object.
#[derive(Component)]
pub struct Display {
    /// Label of the object, used for identification and lookup.
    pub name: String,
}

/// Objective for the flight.
#[derive(Component)]
pub enum Destination {
    /// An outbound flight from the aerodrome.
    Departure { aerodrome: Entity },
    /// An inbound flight to the aerodrome.
    Arrival { aerodrome: Entity },
    /// A local flight from `from` to `to`.
    Ferry {
        /// Source aerodrome.
        from_aerodrome: Entity,
        /// Destination aerodrome.
        to_aerodrome:   Entity,
    },
}

/// Position relative to level origin at mean sea level, in (nm, nm, nm).
///
/// Altitude is not the pressure altitude.
#[derive(Component)]
pub struct Position(pub Vec3A);

/// Rotation of the object, for display only.
#[derive(Component, Default)]
pub struct Rotation(pub Quat);

/// Speed relative to ground, in (kt, kt, kt).
/// The vertical component (Z) is independent of terrain.
#[derive(Component, Default)]
pub struct GroundSpeed(pub Vec3A);

#[derive(Component)]
pub struct Airborne {
    /// Indicated airspeed, in (kt, kt, kt).
    pub airspeed: Vec3A,
}

pub struct SpawnCommand {
    pub position:     Position,
    pub ground_speed: GroundSpeed,
    pub display:      Display,
    pub destination:  Destination,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity).insert((
            self.position,
            Rotation::default(),
            self.ground_speed,
            self.display,
            self.destination,
            Track { log: VecDeque::new(), timer: Timer::new(Duration::ZERO, TimerMode::Once) },
            Marker,
        ));
        world.send_event(SpawnEvent(entity));
    }
}

/// Sent when a plane entity is spawned.
#[derive(Event)]
pub struct SpawnEvent(pub Entity);

/// Sets an entity as airborne.
pub struct SetAirborneCommand;

impl EntityCommand for SetAirborneCommand {
    fn apply(self, entity: Entity, world: &mut World) {
        let (position, ground_speed) = {
            let Ok(entity_ref) = world.get_entity(entity) else {
                bevy::log::error!("attempt to set airborne for nonexistent entity {entity:?}");
                return;
            };
            let Some(&Position(position)) = entity_ref.get() else {
                bevy::log::error!("attempt to set airborne for entity {entity:?} without Position");
                return;
            };
            let Some(&GroundSpeed(ground_speed)) = entity_ref.get() else {
                bevy::log::error!("attempt to set airborne for entity {entity:?} without Position");
                return;
            };
            (position, ground_speed)
        };

        let wind = {
            let mut locator = SystemState::<wind::Locator>::new(world);
            locator.get(world).locate(position)
        };

        world
            .entity_mut(entity)
            .insert(Airborne { airspeed: ground_speed - Vec3A::from((wind, 0.)) });
    }
}

fn move_object_system(
    time: Res<Time<time::Virtual>>,
    mut object_query: Query<(&mut Position, &GroundSpeed), With<Marker>>,
) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(|(mut position, ground_speed)| {
        position.0 += ground_speed.0 * time.delta_secs() / 3600.;
    });
}

fn update_airborne_system(
    time: Res<Time<time::Virtual>>,
    wind: wind::Locator,
    mut object_query: Query<(&mut GroundSpeed, &Position, &Airborne)>,
) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(|(mut ground_speed, position, airborne)| {
        let sea_level_temperature = STANDARD_SEA_LEVEL_TEMPERATURE; // TODO do we have temperature?
        let pressure_altitude = position.0.z; // TODO calibrate by pressure
        let actual_temperature = sea_level_temperature
            - STANDARD_LAPSE_RATE * pressure_altitude.min(TROPOPAUSE_ALTITUDE);
        let density_altitude = pressure_altitude
            + STANDARD_SEA_LEVEL_TEMPERATURE / STANDARD_LAPSE_RATE
                * (1.
                    - (STANDARD_SEA_LEVEL_TEMPERATURE / actual_temperature)
                        .powf(PRESSURE_DENSITY_ALTITUDE_POW));

        let tas_ratio = 1. + TAS_DELTA_PER_NM * density_altitude;

        ground_speed.0 = airborne.airspeed * tas_ratio + Vec3A::from((wind.locate(position.0), 0.));
    });
}

#[derive(Component)]
pub struct Track {
    pub log: VecDeque<Vec3>,
    timer:   Timer,
}

fn track_position_system(
    time: Res<Time<time::Virtual>>,
    config: Res<Config>,
    mut query: Query<(&mut Track, &Position)>,
) {
    query.iter_mut().for_each(|(mut track, position)| {
        track.timer.tick(time.delta());
        if track.timer.finished() {
            track.timer.set_duration(config.track_density);
            track.timer.reset();

            if track.log.len() >= config.max_track_log {
                track.log.pop_front();
            }

            track.log.push_back(position.0.into());
        }
    });
}
