//! A moveable vehicle in the level,
//! in particular, a ground vehicle or an aircraft.

use std::collections::VecDeque;
use std::marker::PhantomData;
use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::component::Component;
use bevy::ecs::entity::Entity;
use bevy::ecs::message::Message;
use bevy::ecs::query::Without;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{EntityCommand, Query, Res, SystemParam, SystemState};
use bevy::ecs::world::EntityWorldMut;
use bevy::math::{Dir2, Quat, Vec2, Vec3};
use bevy::time::{self, Time, Timer, TimerMode};
use bevy_mod_config::{AppExt, Config, ConfigFieldFor, Manager, ReadConfig};
use itertools::Itertools;
use math::{
    Accel, AngularSpeed, Heading, ISA_SEA_LEVEL_PRESSURE, ISA_SEA_LEVEL_TEMPERATURE, Length,
    Position, Pressure, Speed, Temp, compute_barometric, range_steps, solve_expected_ground_speed,
};
use store::Score;

use super::dest::Destination;
use super::{SystemSets, ground, message, nav, wind};
use crate::level::weather::{self, Weather};
use crate::level::{dest, plane, taxi};
use crate::try_log::EntityWorldMutExt;
use crate::{QueryTryLog, WorldTryLog};

pub mod loader;
pub mod types;
pub use types::Type;

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
        app.add_message::<SpawnMessage>();
        app.add_message::<DespawnMessage>();
        app.add_systems(
            app::Update,
            update_airborne_system
                .in_set(wind::DetectorReaderSystemSet)
                .in_set(weather::DetectorReaderSystemSet)
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

#[derive(Debug, Component)]
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

#[derive(Debug, Component)]
#[require(wind::Detector)]
#[require(weather::Detector)]
pub struct Airborne {
    /// Indicated airspeed.
    pub airspeed: Speed<Vec3>,

    /// True airspeed.
    pub true_airspeed: Speed<Vec3>,

    /// Outside air temperature.
    pub oat:          Temp,
    /// Outside air pressure.
    pub pressure:     Pressure,
    /// Current pressure altitude.
    pub pressure_alt: Position<f32>,
}

pub struct SpawnCommand {
    pub position:         Position<Vec3>,
    pub ground_speed:     Speed<Vec3>,
    pub display:          Display,
    pub destination:      Destination,
    pub completion_score: Option<Score>,
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

        if let Some(score) = self.completion_score {
            entity.insert(dest::CompletionScore { score });
        }

        let entity_id = entity.id();
        entity.world_scope(|world| world.write_message(SpawnMessage(entity_id)));
    }
}

/// Sent when a plane entity is spawned.
#[derive(Message)]
pub struct SpawnMessage(pub Entity);

pub struct DespawnCommand;

impl EntityCommand for DespawnCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        let entity_id = entity.id();
        entity.world_scope(|world| world.write_message(DespawnMessage(entity_id)));
        entity.despawn();
    }
}

/// Sent when a plane entity is despawned.
///
/// By the time this event is received, the entity is already removed from the world,
/// so it is invalid to query for its components.
#[derive(Message)]
pub struct DespawnMessage(pub Entity);

/// Sets an entity as airborne.
pub struct SetAirborneCommand;

impl EntityCommand for SetAirborneCommand {
    fn apply(self, mut entity: EntityWorldMut) {
        let entity_id = entity.id();
        entity.world_scope(|world| {
            let mut state = SystemState::<
                Query<(&mut plane::Control, &TaxiStatus, Option<&nav::Limits>)>,
            >::new(world);
            let mut query = state.get_mut(world);
            if let Ok((mut plane_control, taxi_status, nav_limits)) = query.get_mut(entity_id) {
                plane_control.heading = taxi_status.heading;
                plane_control.yaw_speed = AngularSpeed::ZERO;

                if let Some(nav_limits) = nav_limits {
                    plane_control.horiz_accel = nav_limits.0.std_climb.accel;
                } else {
                    plane_control.horiz_accel = Accel::ZERO;
                }
            }
        });

        entity.remove::<(OnGround, taxi::Target)>();

        let (position, ground_speed) = {
            let Some(&Object { position, ground_speed }) = entity.log_get() else { return };
            (position, ground_speed)
        };

        let wind = entity.world_scope(|world| {
            let mut locator = SystemState::<wind::Locator>::new(world);
            locator.get(world).locate(position)
        });

        let airspeed = ground_speed - wind.horizontally();
        entity.insert(Airborne {
            airspeed,
            true_airspeed: airspeed,
            oat: ISA_SEA_LEVEL_TEMPERATURE,
            pressure: ISA_SEA_LEVEL_PRESSURE,
            pressure_alt: position.altitude(),
        });
    }
}

/// Marks that the object is on ground. Exclusive with [`Airborne`].
///
/// This component is updated by the taxi plugin during the Navigate phase.
#[derive(Component)]
#[require(TaxiStatus)]
pub struct OnGround {
    /// Current segment the object is on.
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
    /// The Aviate phase updates [`Object::ground_speed`] to attain this target speed subject to
    /// taxi limits.
    pub target_speed: OnGroundTargetSpeed,
}

impl OnGround {
    pub fn target_endpoint<'a>(
        &self,
        segment_query: impl FnOnce(Entity) -> Option<&'a ground::Segment>,
    ) -> Option<Entity> {
        let segment = segment_query(self.segment)?;
        Some(match self.direction {
            ground::SegmentDirection::AlphaToBeta => segment.beta,
            ground::SegmentDirection::BetaToAlpha => segment.alpha,
        })
    }
}

pub enum OnGroundTargetSpeed {
    /// Attempt to reach an exact speed.
    Exact(Speed<f32>),
    /// Accelerate unrestricted. Used for takeoff.
    TakeoffRoll,
}

impl OnGroundTargetSpeed {
    /// Whether the target speed is unrestricted for takeoff.
    #[must_use]
    pub fn is_takeoff_roll(&self) -> bool { matches!(self, OnGroundTargetSpeed::TakeoffRoll) }

    /// Whether the target speed is zero.
    ///
    /// This is an exact float match, which only returns true when
    /// the target speed is explicitly set to zero
    /// rather than a computed value that approximates to zero.
    #[must_use]
    pub fn is_holding(&self) -> bool {
        matches!(self, OnGroundTargetSpeed::Exact(speed) if speed.is_zero())
    }
}

impl PartialEq<Speed<f32>> for OnGroundTargetSpeed {
    fn eq(&self, other: &Speed<f32>) -> bool {
        match self {
            OnGroundTargetSpeed::Exact(speed) => speed.eq(other),
            // Takeoff roll is never equal to any finite speed.
            OnGroundTargetSpeed::TakeoffRoll => false,
        }
    }
}

impl PartialOrd<Speed<f32>> for OnGroundTargetSpeed {
    fn partial_cmp(&self, other: &Speed<f32>) -> Option<std::cmp::Ordering> {
        match self {
            OnGroundTargetSpeed::Exact(speed) => speed.partial_cmp(other),
            // Takeoff roll is always greater than any finite speed.
            OnGroundTargetSpeed::TakeoffRoll => Some(std::cmp::Ordering::Greater),
        }
    }
}

#[derive(Component)]
/// This component is assigned by the taxi plugin during the Aviate phase.
pub struct TaxiStatus {
    /// Current heading of the object.
    pub heading: Heading,
}

impl Default for TaxiStatus {
    fn default() -> Self { Self { heading: Heading::NORTH } }
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

        entity.remove::<(Airborne, nav::VelocityTarget, nav::AllTargets)>().insert((
            OnGround {
                segment:      self.segment,
                direction:    self.direction,
                target_speed: OnGroundTargetSpeed::Exact(Speed::ZERO),
            },
            TaxiStatus { heading },
        ));
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
    mut object_query: Query<(&mut Object, &mut Airborne, &wind::Detector, &weather::Detector)>,
    weather_query: Query<&Weather>,
) {
    if time.is_paused() {
        return;
    }

    object_query.par_iter_mut().for_each(|(mut object, mut airborne, wind, weather)| {
        let result = GroundSpeedCalculator::get_ground_speed_with_weather(
            object.position,
            airborne.airspeed,
            wind.last_computed,
            weather
                .last_match
                .and_then(|entity| weather_query.log_get(entity))
                .copied()
                .unwrap_or_default(),
        );
        airborne.oat = result.temp;
        airborne.pressure = result.pressure;
        airborne.pressure_alt = result.pressure_alt;
        airborne.true_airspeed = result.tas;
        object.ground_speed = result.ground_speed;
    });
}

#[derive(SystemParam)]
pub struct GetAirspeed<'w, 's> {
    airborne_query: Query<'w, 's, (&'static Object, Option<&'static Airborne>)>,
    wind:           wind::Locator<'w, 's>,
    weather:        weather::Locator<'w, 's>,
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
            let tas = ground_speed - self.wind.locate(position).horizontally();
            let weather = self.weather.get(position.horizontal());
            let atm =
                compute_barometric(position.altitude(), weather.sea_pressure, weather.sea_temp);
            atm.indicated_airspeed(tas)
        }
    }
}

#[derive(SystemParam)]
pub struct GroundSpeedCalculator<'w, 's> {
    wind:    wind::Locator<'w, 's>,
    weather: weather::Locator<'w, 's>,
}

impl GroundSpeedCalculator<'_, '_> {
    #[must_use]
    pub fn get_ground_speed_with_weather(
        position: Position<Vec3>,
        ias: Speed<Vec3>,
        wind: Speed<Vec2>,
        weather: Weather,
    ) -> GroundSpeedResult {
        let atm = compute_barometric(position.altitude(), weather.sea_pressure, weather.sea_temp);
        let tas = atm.true_airspeed(ias);
        let ground_speed = tas + wind.horizontally();
        GroundSpeedResult {
            pressure_alt: atm.pressure_altitude,
            pressure: atm.pressure,
            temp: atm.temp,
            tas,
            ground_speed,
        }
    }

    /// Compute the ground speed if an object has the given IAS at the given position.
    #[must_use]
    pub fn get_ground_speed(
        &self,
        position: Position<Vec3>,
        ias: Speed<Vec3>,
    ) -> GroundSpeedResult {
        Self::get_ground_speed_with_weather(
            position,
            ias,
            self.wind.locate(position),
            self.weather.get(position.horizontal()),
        )
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

            let weather = self.weather.get(earlier_pos);
            let atm = compute_barometric(altitude, weather.sea_pressure, weather.sea_temp);
            let true_airspeed = atm.true_airspeed(ias);
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
    pub pressure_alt: Position<f32>,
    pub pressure:     Pressure,
    pub temp:         Temp,
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
    mut query: Query<(&mut Rotation, &TaxiStatus), Without<Airborne>>,
) {
    query.iter_mut().for_each(|(mut rot, taxi_status)| {
        rot.0 = Quat::IDENTITY * taxi_status.heading.into_rotation_quat();
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
        if track.timer.is_finished() {
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
    #[config(default = Duration::from_secs(10))]
    pub track_density: Duration,
}
