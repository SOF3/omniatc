//! A moveable vehicle in the level,
//! in particular, a ground vehicle or an aircraft.

use std::collections::VecDeque;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::system::{SystemParam, SystemState};
use bevy::math::{Dir2, Quat, Vec2, Vec3};
use bevy::prelude::{
    Component, Entity, EntityCommand, Event, IntoSystemConfigs, Query, Res, World,
};
use bevy::time::{self, Time, Timer, TimerMode};
use itertools::Itertools;

use super::{wind, Config, SystemSets};
use crate::math::{
    range_steps, solve_expected_ground_speed, PRESSURE_DENSITY_ALTITUDE_POW, STANDARD_LAPSE_RATE,
    STANDARD_SEA_LEVEL_TEMPERATURE, TAS_DELTA_PER_NM, TROPOPAUSE_ALTITUDE,
};
use crate::units::{Distance, Position, Speed};

#[cfg(test)]
mod tests;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.add_event::<SpawnEvent>();
        app.add_systems(app::Update, update_airborne_system.in_set(SystemSets::ExecuteEnviron));
        app.add_systems(
            app::Update,
            move_object_system.after(update_airborne_system).in_set(SystemSets::ExecuteEnviron),
        );
        app.add_systems(app::Update, track_position_system.in_set(SystemSets::ReconcileForRead));
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
    /// An outbound flight from the aerodrome to a specified waypoint.
    Departure { aerodrome: Entity, dest_waypoint: Entity },
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

#[derive(Component)]
pub struct Object {
    /// Position relative to level origin at mean sea level.
    ///
    /// Altitude is real AMSL altitude, independent of terrain and pressure.
    pub position:     Position<Vec3>,
    /// Speed relative to ground.
    ///
    /// The vertical component (Z) is independent of terrain.
    pub ground_speed: Speed<Vec3>,
}

/// Rotation of the object, for display only.
#[derive(Component, Default)]
pub struct Rotation(pub Quat);

#[derive(Component)]
pub struct Airborne {
    /// Indicated airspeed.
    pub airspeed: Speed<Vec3>,
}

pub struct SpawnCommand {
    pub position:     Position<Vec3>,
    pub ground_speed: Speed<Vec3>,
    pub display:      Display,
    pub destination:  Destination,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, entity: Entity, world: &mut World) {
        world.entity_mut(entity).insert((
            Rotation::default(),
            Object { position: self.position, ground_speed: self.ground_speed },
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
            let Some(&Object { position, ground_speed }) = entity_ref.get() else {
                bevy::log::error!("attempt to set airborne for non-Object entity {entity:?}");
                return;
            };
            (position, ground_speed)
        };

        let wind = {
            let mut locator = SystemState::<wind::Locator>::new(world);
            locator.get(world).locate(position)
        };

        world.entity_mut(entity).insert(Airborne { airspeed: ground_speed - wind.horizontally() });
        // TODO insert/remove nav::VelocityTarget?
    }
}

fn move_object_system(time: Res<Time<time::Virtual>>, mut object_query: Query<&mut Object>) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(|mut object| {
        let moved = object.ground_speed * time.delta();
        object.position += moved;
    });
}

fn update_airborne_system(
    time: Res<Time<time::Virtual>>,
    ggs: GroundSpeedCalculator,
    mut object_query: Query<(&mut Object, &Airborne)>,
) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(|(mut object, airborne)| {
        object.ground_speed = ggs.get_ground_speed(object.position, airborne.airspeed);
    });
}

#[must_use]
pub fn get_tas_ratio(altitude: Position<f32>, sea_level_temperature: f32) -> f32 {
    let pressure_altitude = altitude; // TODO calibrate by pressure
    let actual_temperature = sea_level_temperature
        - STANDARD_LAPSE_RATE * pressure_altitude.min(TROPOPAUSE_ALTITUDE).get();
    let density_altitude = pressure_altitude
        + Distance(
            STANDARD_SEA_LEVEL_TEMPERATURE / STANDARD_LAPSE_RATE
                * (1.
                    - (STANDARD_SEA_LEVEL_TEMPERATURE / actual_temperature)
                        .powf(PRESSURE_DENSITY_ALTITUDE_POW)),
        );

    1. + TAS_DELTA_PER_NM * density_altitude.get()
}

#[derive(SystemParam)]
pub struct GetAirspeed<'w, 's> {
    airborne_query: Query<'w, 's, (&'static Object, Option<&'static Airborne>)>,
    wind:           wind::Locator<'w, 's>,
}

impl GetAirspeed<'_, '_> {
    /// Infers the airspeed by either taking the airborne value
    /// or estimating it by reversing the IAS -> GS formula.
    ///
    /// # Panics
    /// Panics if the entity does not have an [`Object`] component.
    #[must_use]
    pub fn get_airspeed(&self, entity: Entity) -> Speed<Vec3> {
        let (&Object { position, ground_speed }, airborne) =
            self.airborne_query.get(entity).expect("entity is not an object");
        if let Some(airborne) = airborne {
            airborne.airspeed
        } else {
            (ground_speed - self.wind.locate(position).horizontally())
                / get_tas_ratio(position.altitude(), STANDARD_SEA_LEVEL_TEMPERATURE)
        }
    }
}

#[derive(SystemParam)]
pub struct GroundSpeedCalculator<'w, 's> {
    wind: wind::Locator<'w, 's>,
}

impl GroundSpeedCalculator<'_, '_> {
    /// Compute the ground speed if an object has the given IAS at the given position.
    #[must_use]
    pub fn get_ground_speed(&self, position: Position<Vec3>, ias: Speed<Vec3>) -> Speed<Vec3> {
        ias * get_tas_ratio(position.altitude(), STANDARD_SEA_LEVEL_TEMPERATURE)
            + self.wind.locate(position).horizontally()
    }

    /// Estimate the altitude change as an object flies from `start` to `end`,
    /// assuming constant vertical rate `vert_rate` and horizontal indicated airspeed `ias`.
    ///
    /// The reference altitude may be either the altitude at `start` or the altitude at `end`.
    /// The estimated altitude at the other endpoint is returned.
    #[must_use]
    pub fn estimate_altitude_change(
        &self,
        [start, end]: [Position<Vec2>; 2],
        vert_rate: Speed<f32>,
        ias: Speed<f32>,
        ref_altitude: Position<f32>,
        ref_altitude_type: RefAltitudeType,
        sample_density: Distance<f32>,
    ) -> Position<f32> {
        let distance = start.distance_exact(end);
        let Ok(ground_dir) = Dir2::new((end - start).0) else {
            return ref_altitude; // no altitude change if no horizontal motion
        };

        let range_step_iter = match ref_altitude_type {
            RefAltitudeType::Start => range_steps(Distance::ZERO, distance, sample_density),
            RefAltitudeType::End => range_steps(distance, Distance::ZERO, -sample_density),
        };
        let mut altitude = ref_altitude;

        // earlier_distance is the endpoint of the interval closer to the ref altitude,
        // not necessarily the one closer to `start`.
        for (earlier_distance, later_distance) in range_step_iter.tuple_windows() {
            let earlier_pos = start.lerp(end, earlier_distance / distance);

            let true_airspeed = ias * get_tas_ratio(altitude, STANDARD_SEA_LEVEL_TEMPERATURE);
            let ground_speed = solve_expected_ground_speed(
                true_airspeed,
                self.wind.locate(earlier_pos.with_altitude(altitude)),
                ground_dir,
            );
            let segment_duration = (earlier_distance - later_distance).abs() / ground_speed;

            match ref_altitude_type {
                RefAltitudeType::Start => altitude += vert_rate * segment_duration,
                RefAltitudeType::End => altitude -= vert_rate * segment_duration,
            }
        }

        altitude
    }
}

pub enum RefAltitudeType {
    /// The available reference is the starting altitude.
    Start,
    /// The available reference is the ending altitude.
    End,
}

#[derive(Component)]
pub struct Track {
    pub log: VecDeque<Position<Vec3>>,
    timer:   Timer,
}

fn track_position_system(
    time: Res<Time<time::Virtual>>,
    config: Res<Config>,
    mut query: Query<(&mut Track, &Object)>,
) {
    query.iter_mut().for_each(|(mut track, &Object { position, .. })| {
        track.timer.tick(time.delta());
        if track.timer.finished() {
            track.timer.set_duration(config.track_density);
            track.timer.reset();

            if track.log.len() >= config.max_track_log {
                track.log.pop_front();
            }

            track.log.push_back(position);
        }
    });
}
