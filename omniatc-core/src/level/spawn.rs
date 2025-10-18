use std::time::Duration;

use bevy::app::{self, App, Plugin};
use bevy::ecs::entity::Entity;
use bevy::ecs::name::Name;
use bevy::ecs::query::With;
use bevy::ecs::resource::Resource;
use bevy::ecs::schedule::IntoScheduleConfigs;
use bevy::ecs::system::{Commands, Local, ParamSet, Query, Res, SystemParam};
use bevy::math::Vec3;
use bevy::time::Time;
use math::{Accel, AngularSpeed, Heading, Length, Position, Speed};
use rand::SeedableRng;
use rand::rngs::SmallRng;
use rand::seq::IteratorRandom;
use store::{Score, WeightedList, YawTarget};

use crate::QueryTryLog;
use crate::level::aerodrome::loader::APRON_FORWARD_HEADING_DIRECTION;
use crate::level::dest::Destination;
use crate::level::waypoint::Waypoint;
use crate::level::{SystemSets, aerodrome, ground, nav, object, plane, route, wake};
use crate::load::StoredEntity;

pub mod loader;

pub struct Plug;

impl Plugin for Plug {
    fn build(&self, app: &mut App) {
        app.init_resource::<Sets>();
        app.init_resource::<Trigger>();
        app.add_systems(app::Update, spawn_system.in_set(SystemSets::Spawn));
    }
}

#[derive(Default, Resource)]
pub struct Sets(pub store::WeightedList<Set>);

pub struct Set {
    pub gen_name: WeightedList<store::NameGenerator>,
    pub types:    WeightedList<Entity>,
    pub route:    WeightedList<Route>,
    pub position: WeightedList<Location>,
}

pub struct Route {
    pub preset:      Entity,
    pub destination: Destination,
    pub score:       Score,
}

/// Location where an object is spawned.
pub enum Location {
    /// Apron segment entities to spawn on.
    Aprons(Vec<Entity>),
    /// Taxiways to spawn on.
    Runway(Vec<LocationRunwayTaxiway>),
    Airborne {
        waypoint: Entity,
        altitude: Position<f32>,
        speed:    Speed<f32>,
        heading:  Heading,
    },
}

pub struct LocationRunwayTaxiway {
    pub taxiway_segment: Entity,
    pub direction:       ground::SegmentDirection,
}

fn spawn_system(mut params: ParamSet<(TriggerParams, Spawner)>, mut rng: Local<Option<SmallRng>>) {
    let rng = rng.get_or_insert_with(|| SmallRng::from_rng(&mut rand::rng()));
    if params.p0().need_more() {
        let result = params.p1().spawn_once(rng);
        if result.is_some() {
            params.p0().on_successful_spawn();
        }
    }
}

#[derive(SystemParam)]
struct TriggerParams<'w, 's> {
    time:         Res<'w, Time>,
    last_spawned: Local<'s, Option<Duration>>,
    mode:         Res<'w, Trigger>,
    object_query: Query<'w, 's, (), With<object::Object>>,
}

/// Determines when new objects should be spawned.
#[derive(Resource, Default)]
pub enum Trigger {
    #[default]
    Disabled,
    Periodic(Duration),
    ObjectCount {
        count: usize,
    },
}

impl TriggerParams<'_, '_> {
    /// Whether a new object needs to be spawned.
    fn need_more(&self) -> bool {
        match *self.mode {
            Trigger::Disabled => false,
            Trigger::Periodic(period) => match *self.last_spawned {
                None => true,
                Some(last) => self.time.elapsed().checked_sub(last).is_some_and(|v| v >= period),
            },
            Trigger::ObjectCount { count: threshold } => {
                let current_count = self.object_query.iter().len();
                current_count < threshold
            }
        }
    }

    /// Records a successful spawn.
    fn on_successful_spawn(&mut self) { *self.last_spawned = Some(self.time.elapsed()); }
}

#[derive(SystemParam)]
struct Spawner<'w, 's> {
    sets:              Res<'w, Sets>,
    commands:          Commands<'w, 's>,
    object_type_query: Query<'w, 's, &'static object::Type>,
    endpoint_query:    Query<'w, 's, &'static ground::Endpoint>,
    aerodrome_query:   Query<'w, 's, &'static aerodrome::Aerodrome>,
    segment_query:     Query<'w, 's, (&'static ground::Segment, &'static ground::SegmentOf)>,
    waypoint_query:    Query<'w, 's, &'static Waypoint>,
    preset_query:      Query<'w, 's, &'static route::Preset>,
}

impl Spawner<'_, '_> {
    /// Attempts to spawn a single object.
    /// Returns `Some(())` if an object was spawned, or `None` if spawning failed.
    fn spawn_once(&mut self, rng: &mut impl rand::Rng) -> Option<()> {
        let Some(set) = self.sets.0.sample(rng) else {
            bevy::log::warn_once!("Unable to spawn objects due to empty spawn sets");
            return None;
        };
        let Some(gen_name) = set.gen_name.sample(rng) else {
            bevy::log::warn_once!("Unable to spawn objects due to empty name generator");
            return None;
        };
        let name = gen_name.generate(rng);

        let Some(&object_type_id) = set.types.sample(rng) else {
            bevy::log::warn_once!("Unable to spawn objects due to empty object types");
            return None;
        };
        let object_type = self.object_type_query.log_get(object_type_id)?;

        let Some(location) = set.position.sample(rng) else {
            bevy::log::warn_once!("Unable to spawn objects due to empty spawn locations");
            return None;
        };

        let resolved_location = self.resolve_location(location, object_type, rng)?;
        let Some(route) = set.route.sample(rng) else {
            bevy::log::warn_once!("Unable to spawn objects due to empty routes");
            return None;
        };

        let preset = self.preset_query.log_get(route.preset)?;

        let mut object = self.commands.spawn((StoredEntity, Name::new(format!("Plane: {name}"))));
        object.queue(object::SpawnCommand {
            position:         resolved_location.position,
            ground_speed:     (resolved_location.speed * resolved_location.heading).horizontally(),
            display:          object::Display { name },
            destination:      route.destination.clone(),
            completion_score: Some(route.score),
        });

        match object_type {
            object::Type::Plane { taxi, nav } => {
                object.insert(taxi.clone());
                object.queue(plane::SpawnCommand {
                    limits:  nav.clone(),
                    control: Some(plane::Control {
                        heading:     resolved_location.heading,
                        yaw_speed:   AngularSpeed::ZERO,
                        horiz_accel: Accel::ZERO,
                    }),
                });
                object.insert((
                    wake::Producer {
                        base_intensity: object::loader::compute_wake(&taxi.0, &nav.0),
                    },
                    wake::Detector::default(),
                ));

                match resolved_location.spawn_type {
                    SpawnType::Airborne => {
                        object.queue(object::SetAirborneCommand);
                        object.insert(nav::VelocityTarget {
                            yaw:         YawTarget::Heading(resolved_location.heading),
                            horiz_speed: resolved_location.speed,
                            vert_rate:   Speed::ZERO,
                            expedite:    false,
                        });
                    }
                    SpawnType::Ground { segment, direction } => {
                        object.queue(object::SetOnGroundCommand {
                            segment,
                            direction,
                            heading: Some(resolved_location.heading),
                        });
                    }
                }
            }
        }

        object.insert(route::Id(Some(preset.id.clone())));
        object.queue(route::ReplaceNodes(preset.nodes.clone()));

        Some(())
    }

    fn resolve_location(
        &self,
        spawn_location: &Location,
        object_type: &object::Type,
        rng: &mut impl rand::Rng,
    ) -> Option<ResolvedLocation> {
        match spawn_location {
            Location::Aprons(aprons) => {
                let Some(apron_entity) = aprons
                    .iter()
                    .copied()
                    .filter(|&entity| self.is_apron_available(entity))
                    .choose(rng)
                else {
                    bevy::log::debug!("Unable to spawn objects due to lack of available aprons");
                    return None;
                };

                let (segment, &ground::SegmentOf(aerodrome_entity)) =
                    self.segment_query.log_get(apron_entity)?;
                let aerodrome = self.aerodrome_query.log_get(aerodrome_entity)?;

                let (intersect_entity, parked_entity) =
                    segment.by_direction(APRON_FORWARD_HEADING_DIRECTION);
                let intersect = self.endpoint_query.log_get(intersect_entity)?.position;
                let parked = self.endpoint_query.log_get(parked_entity)?.position;

                Some(ResolvedLocation {
                    position:   parked.with_altitude(aerodrome.elevation),
                    speed:      Speed::ZERO,
                    heading:    (parked - intersect).heading(),
                    spawn_type: SpawnType::Ground {
                        segment:   apron_entity,
                        direction: APRON_FORWARD_HEADING_DIRECTION,
                    },
                })
            }
            Location::Runway(segments) => {
                let Some(chosen) = segments
                    .iter()
                    .filter(|taxiway| {
                        self.is_taxiway_available(
                            taxiway.taxiway_segment,
                            object_type.half_length(),
                        )
                    })
                    .choose(rng)
                else {
                    bevy::log::debug!("Unable to spawn objects due to lack of available runways");
                    return None;
                };

                let (segment, &ground::SegmentOf(aerodrome_entity)) =
                    self.segment_query.log_get(chosen.taxiway_segment)?;
                let aerodrome_elevation = self.aerodrome_query.log_get(aerodrome_entity)?.elevation;
                let (endpoint_behind, endpoint_ahead) = segment.by_direction(chosen.direction);
                let endpoint_behind_position =
                    self.endpoint_query.log_get(endpoint_behind)?.position;
                let endpoint_ahead_position = self.endpoint_query.log_get(endpoint_ahead)?.position;
                // TODO adjust position if taxiway is occupied

                Some(ResolvedLocation {
                    position:   endpoint_ahead_position.with_altitude(aerodrome_elevation),
                    speed:      Speed::ZERO,
                    heading:    (endpoint_ahead_position - endpoint_behind_position).heading(),
                    spawn_type: SpawnType::Ground {
                        segment:   chosen.taxiway_segment,
                        direction: chosen.direction,
                    },
                })
            }
            &Location::Airborne { waypoint, altitude, speed, heading } => {
                // TODO detect possible conflicts and adjust altitude
                let &Waypoint { position, .. } = self.waypoint_query.log_get(waypoint)?;
                let position = position.horizontal().with_altitude(altitude);
                Some(ResolvedLocation { position, speed, heading, spawn_type: SpawnType::Airborne })
            }
        }
    }

    /// Checks if the apron is available for spawning.
    ///
    /// An apron is available if it is currently marked as occupied by a virtual object.
    #[expect(clippy::unused_self, reason = "TODO")]
    fn is_apron_available(&self, _apron: Entity) -> bool {
        // TODO implement
        true
    }

    /// Checks if the taxiway is available for spawning an object of the specified length.
    ///
    /// A taxiway is available if it has enough length unoccupied by other objects.
    #[expect(clippy::unused_self, reason = "TODO")]
    fn is_taxiway_available(&self, _taxiway: Entity, _half_length: Length<f32>) -> bool {
        // TODO implement
        true
    }
}

struct ResolvedLocation {
    position:   Position<Vec3>,
    speed:      Speed<f32>,
    heading:    Heading,
    spawn_type: SpawnType,
}

enum SpawnType {
    Airborne,
    Ground { segment: Entity, direction: ground::SegmentDirection },
}
