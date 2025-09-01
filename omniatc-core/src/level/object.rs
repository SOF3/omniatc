//! A moveable vehicle in the level,
//! in particular, a ground vehicle or an aircraft.

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::query::Without;
use bevy::ecs::system::{SystemParam, SystemState};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Dir2, Quat, Vec2, Vec3};
use bevy::prelude::{Component, Entity, EntityCommand, Event, IntoScheduleConfigs, Query, Res};
use bevy::time::{self, Time, Timer, TimerMode};
use bevy_mod_config::{AppExt, Config, ConfigFieldFor, Manager, ReadConfig};
use itertools::Itertools;
use math::{
    Heading, Length, PRESSURE_DENSITY_ALTITUDE_POW, Position, STANDARD_LAPSE_RATE,
    STANDARD_SEA_LEVEL_TEMPERATURE, Speed, TAS_DELTA_PER_NM, TROPOPAUSE_ALTITUDE, range_steps,
    solve_expected_ground_speed,
};

use super::{SystemSets, ground, message, nav, wind};
use crate::WorldTryLog;
use crate::try_log::EntityWorldMutExt;

mod dest;
pub use dest::Destination;

#[cfg(test)]
mod tests;

pub struct Plug<M>(PhantomData<M>);

impl<M> Default for Plug<M> {
    fn default() -> Self { Self(PhantomData) }
}

impl<M: Manager + Default> Plugin for Plug<M>
where
    Conf: ConfigFieldFor<M>,
{
    fn build(&self, app: &mut App) {
        app.init_config::<M, Conf>("core:object");
        app.add_event::<SpawnEvent>();
        app.add_systems(
            app::Update,
            update_airborne_system
                .in_set(wind::DetectorReaderSystemSet)
                .in_set(SystemSets::ExecuteEnviron),
        );
        app.add_systems(
            app::Update,
            move_object_system.after(update_airborne_system).in_set(SystemSets::ExecuteEnviron),
        );
        app.add_systems(
            app::Update,
            (rotate_ground_object_system, track_position_system)
                .in_set(SystemSets::ReconcileForRead),
        );
    }
}

/// Display details of an object.
#[derive(Component)]
pub struct Display {
    /// Label of the object, used for identification and lookup.
    pub name: String,
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
#[require(wind::Detector)]
pub struct Airborne {
    /// Indicated airspeed.
    pub airspeed: Speed<Vec3>,

    /// True airspeed.
    pub true_airspeed: Speed<Vec3>,
}

pub struct SpawnCommand {
    pub position:     Position<Vec3>,
    pub ground_speed: Speed<Vec3>,
    pub display:      Display,
    pub destination:  Destination,
}

impl EntityCommand for SpawnCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        entity.insert((
            Rotation::default(),
            Object { position: self.position, ground_speed: self.ground_speed },
            message::Sender { display: self.display.name.clone() },
            self.display,
            self.destination,
            Track { log: VecDeque::new(), timer: Timer::new(Duration::ZERO, TimerMode::Once) },
        ));

        let entity_id = entity.id();
        entity.world_scope(|world| world.send_event(SpawnEvent(entity_id)));
    }
}

/// Sent when a plane entity is spawned.
#[derive(Event)]
pub struct SpawnEvent(pub Entity);

/// Sets an entity as airborne.
pub struct SetAirborneCommand;

impl EntityCommand for SetAirborneCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        entity.remove::<OnGround>();

        let (position, ground_speed) = {
            let Some(&Object { position, ground_speed }) = entity.log_get() else { return };
            (position, ground_speed)
        };

        let wind = entity.world_scope(|world| {
            let mut locator = SystemState::<wind::Locator>::new(world);
            locator.get(world).locate(position)
        });

        let airspeed = ground_speed - wind.horizontally();
        entity.insert(Airborne { airspeed, true_airspeed: airspeed });
        // TODO insert/remove nav::VelocityTarget?
    }
}

#[derive(Component)]
pub struct OnGround {
    /// Current heading of the object.
    ///
    /// This field is assigned by the taxi plugin during the Aviate phase.
    pub heading:      Heading,
    /// Current segment the object is on.
    ///
    /// This field is updated by the taxi plugin during the Navigate phase.
    pub segment:      Entity,
    /// Direction of motion on the segment.
    ///
    /// When `target_speed` is negative, this is the direction that the object is reversing *towards*.
    /// For example, `AlphaToBeta` with a negative target speed means that
    /// the object is facing alpha and reversing towards beta.
    pub direction:    ground::SegmentDirection,
    /// Target speed to move.
    ///
    /// A negative value indicates that the object should reverse.
    ///
    /// This field is assigned by the taxi plugin during the Navigate phase.
    /// The Aviate phase updates [`Object::ground_speed`] to attain this target speed subject to
    /// taxi limits.
    pub target_speed: Speed<f32>,
}

/// Sets an entity from airborne to ground.
pub struct SetOnGroundCommand {
    pub segment:   Entity,
    pub direction: ground::SegmentDirection,
    pub heading:   Option<Heading>,
}

impl EntityCommand for SetOnGroundCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        let Some(&ground::Segment { elevation, .. }) = entity.world().log_get(self.segment) else {
            return;
        };

        let Some(mut object) = entity.log_get_mut::<Object>() else { return };
        // must not descend anymore since we have hit the ground.
        object.ground_speed = object.ground_speed.horizontal().horizontally();
        // force the altitude to be aerodrome elevation
        object.position = object.position.horizontal().with_altitude(elevation);

        let heading = match self.heading {
            None => {
                let Some(&Airborne { true_airspeed, .. }) = entity.log_get::<Airborne>() else {
                    return;
                };
                true_airspeed.horizontal().heading()
            }
            Some(heading) => heading,
        };

        entity.remove::<(Airborne, nav::VelocityTarget, nav::AllTargets)>().insert((OnGround {
            segment: self.segment,
            direction: self.direction,
            heading,
            target_speed: Speed::ZERO,
        },));
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
    mut object_query: Query<(&mut Object, &mut Airborne, &wind::Detector)>,
) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(|(mut object, mut airborne, wind)| {
        let result = GroundSpeedCalculator::get_ground_speed_with_wind(
            object.position,
            airborne.airspeed,
            wind.last_computed,
        );
        airborne.true_airspeed = result.tas;
        object.ground_speed = result.ground_speed;
    });
}

#[must_use]
pub fn get_tas_ratio(altitude: Position<f32>, sea_level_temperature: f32) -> f32 {
    let pressure_altitude = altitude; // TODO calibrate by pressure
    let actual_temperature = sea_level_temperature
        - STANDARD_LAPSE_RATE * pressure_altitude.min(TROPOPAUSE_ALTITUDE).get();
    let density_altitude = pressure_altitude
        + Length::new(
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
    #[must_use]
    pub fn get_ground_speed_with_wind(
        position: Position<Vec3>,
        ias: Speed<Vec3>,
        wind: Speed<Vec2>,
    ) -> GroundSpeedResult {
        let tas = ias * get_tas_ratio(position.altitude(), STANDARD_SEA_LEVEL_TEMPERATURE);
        let ground_speed = tas + wind.horizontally();
        GroundSpeedResult { tas, ground_speed }
    }

    /// Compute the ground speed if an object has the given IAS at the given position.
    #[must_use]
    pub fn get_ground_speed(
        &self,
        position: Position<Vec3>,
        ias: Speed<Vec3>,
    ) -> GroundSpeedResult {
        Self::get_ground_speed_with_wind(position, ias, self.wind.locate(position))
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
        sample_density: Length<f32>,
    ) -> Position<f32> {
        let distance = start.distance_exact(end);
        let Ok(ground_dir) = Dir2::new((end - start).0) else {
            return ref_altitude; // no altitude change if no horizontal motion
        };

        let range_step_iter = match ref_altitude_type {
            RefAltitudeType::Start => range_steps(Length::ZERO, distance, sample_density),
            RefAltitudeType::End => range_steps(distance, Length::ZERO, -sample_density),
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

pub struct GroundSpeedResult {
    pub tas:          Speed<Vec3>,
    pub ground_speed: Speed<Vec3>,
}

pub enum RefAltitudeType {
    /// The available reference is the starting altitude.
    Start,
    /// The available reference is the ending altitude.
    End,
}

pub(super) fn rotate_ground_object_system(
    mut query: Query<(&mut Rotation, &OnGround), Without<Airborne>>,
) {
    query.iter_mut().for_each(|(mut rot, ground)| {
        rot.0 = Quat::IDENTITY * ground.heading.into_rotation_quat();
    });
}

#[derive(Component)]
pub struct Track {
    pub log: VecDeque<Position<Vec3>>,
    timer:   Timer,
}

fn track_position_system(
    time: Res<Time<time::Virtual>>,
    conf: ReadConfig<Conf>,
    mut query: Query<(&mut Track, &Object)>,
) {
    let conf = conf.read();

    query.iter_mut().for_each(|(mut track, &Object { position, .. })| {
        track.timer.tick(time.delta());
        if track.timer.finished() {
            track.timer.set_duration(conf.track_density);
            track.timer.reset();

            if track.log.len() >= conf.max_track_log {
                track.log.pop_front();
            }

            track.log.push_back(position);
        }
    });
}

#[derive(Config)]
pub struct Conf {
    /// Number of positions tracked per object.
    ///
    /// The oldest positions are removed when the log exceeds the limit.
    #[config(default = 1024)]
    pub max_track_log: usize,
    /// Duration between two points in an object track log.
    #[config(default = Duration::from_secs(1))]
    pub track_density: Duration,
}
