use std::borrow::Cow;
use std::io;

use bevy::math::bounding::Aabb3d;
use bevy::prelude::{
    BuildChildren, ChildBuild, Command as BevyCommand, DespawnRecursiveExt, Entity, EntityCommand,
    With, World,
};
use bevy::utils::HashMap;

use crate::level::runway::Runway;
use crate::level::waypoint::{self, Waypoint};
use crate::level::{aerodrome, nav, object, plane, runway, wind};
use crate::math::SEA_ALTITUDE;
use crate::store;
use crate::units::{Angle, Distance, Heading};

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
        .query_filtered::<Entity, With<store::LoadedEntity>>()
        .iter(world)
        .collect::<Vec<_>>()
        .into_iter()
        .for_each(|entity| world.entity_mut(entity).despawn_recursive());

    spawn_winds(world, &file.level.environment.winds);
    let aerodromes = spawn_aerodromes(world, &file.level.aerodromes)?;
    let waypoints = spawn_waypoints(world, &file.level.waypoints)?;

    spawn_objects(world, &aerodromes, &waypoints, &file.level.objects)?;

    Ok(())
}

fn spawn_winds(world: &mut World, winds: &[store::Wind]) {
    for wind in winds {
        let entity = world.spawn((store::LoadedEntity, bevy::core::Name::new("Wind"))).id();
        wind::SpawnCommand {
            bundle: wind::Comps {
                vector:        wind::Vector { bottom: wind.bottom_speed, top: wind.top_speed },
                effect_region: wind::EffectRegion(Aabb3d {
                    min: wind.start.with_altitude(wind.bottom).get().into(),
                    max: wind.end.with_altitude(wind.top).get().into(),
                }),
            },
        }
        .apply(entity, world);
    }
}

fn spawn_aerodromes(
    world: &mut World,
    aerodromes: &[store::Aerodrome],
) -> Result<AerodromeMap, Error> {
    aerodromes
        .iter()
        .enumerate()
        .map(|(id, aerodrome)| {
            let aerodrome_id = u32::try_from(id).map_err(|_| Error::TooManyAerodromes)?;
            let aerodrome_entity = world
                .spawn((
                    store::LoadedEntity,
                    bevy::core::Name::new(format!("Aerodrome: {}", aerodrome.code)),
                    aerodrome::Display {
                        id:   aerodrome_id,
                        code: aerodrome.code.clone(),
                        name: aerodrome.full_name.clone(),
                    },
                ))
                .id();

            let runway_entities = aerodrome
                .runways
                .iter()
                .map(|runway| {
                    (runway.name.clone(), spawn_runway(world, aerodrome, runway, aerodrome_entity))
                })
                .collect::<HashMap<_, _>>();

            Ok((
                aerodrome.code.clone(),
                SpawnedAerodrome {
                    aerodrome_entity,
                    index: aerodrome_id,
                    runways: runway_entities,
                },
            ))
        })
        .collect::<HashMapResult<_, _>>()
        .map(AerodromeMap)
}

struct AerodromeMap(HashMap<String, SpawnedAerodrome>);

impl AerodromeMap {
    fn resolve(&self, code: &str) -> Result<&SpawnedAerodrome, Error> {
        self.0.get(code).ok_or_else(|| Error::UnresolvedAerodrome(code.to_string()))
    }
}

struct SpawnedAerodrome {
    aerodrome_entity: Entity,
    index:            u32,
    runways:          HashMap<String, SpawnedRunway>,
}

struct SpawnedRunway {
    runway:             Entity,
    localizer_waypoint: Entity,
}

fn spawn_runway(
    world: &mut World,
    aerodrome: &store::Aerodrome,
    runway: &store::Runway,
    aerodrome_entity: Entity,
) -> SpawnedRunway {
    let runway_entity = world
        .spawn((
            store::LoadedEntity,
            bevy::core::Name::new(format!("Runway: {}/{}", aerodrome.code, runway.name)),
        ))
        .id();

    runway::SpawnCommand {
        waypoint: Waypoint {
            name:         runway.name.clone(),
            display_type: waypoint::DisplayType::Runway,
            position:     runway.touchdown_position.with_altitude(runway.elevation),
        },
        runway:   Runway {
            aerodrome:     aerodrome_entity,
            usable_length: runway.landing_distance_available * runway.heading.into_dir2(),
            glide_angle:   runway.glide_angle,
            display_start: (runway.touchdown_position
                - runway.touchdown_displacement * runway.heading.into_dir2())
            .with_altitude(runway.elevation),
            display_end:   (runway.touchdown_position
                + runway.landing_distance_available * runway.heading.into_dir2())
            .with_altitude(runway.elevation),
            display_width: runway.width,
        },
    }
    .apply(runway_entity, world);

    world.entity_mut(runway_entity).with_children(|b| {
        spawn_runway_navaids(b, runway);
    });

    let localizer_waypoint = world
        .spawn((
            store::LoadedEntity,
            bevy::core::Name::new(format!("LocalizerWaypoint: {}/{}", aerodrome.code, runway.name)),
            runway::LocalizerWaypoint { runway_ref: runway_entity },
        ))
        .id();
    waypoint::SpawnCommand {
        waypoint: Waypoint {
            name:         format!("ILS:{}/{}", aerodrome.code, runway.name),
            display_type: waypoint::DisplayType::None,
            position:     runway.touchdown_position.with_altitude(runway.elevation)
                + (runway.max_visual_distance * runway.heading.opposite().into_dir2())
                    .projected_from_elevation_angle(runway.glide_angle),
        },
    }
    .apply(localizer_waypoint, world);

    SpawnedRunway { runway: runway_entity, localizer_waypoint }
}

fn spawn_runway_navaids(b: &mut impl ChildBuild, runway: &store::Runway) {
    b.spawn((
        waypoint::Navaid {
            heading_range:       Heading::NORTH..Heading::NORTH,
            pitch_range:         Angle(0.)..Angle::RIGHT,
            min_dist_horizontal: Distance(0.),
            min_dist_vertical:   Distance(0.),
            // TODO overwrite these two fields with visibility
            max_dist_horizontal: runway.max_visual_distance,
            max_dist_vertical:   Distance(100.),
        },
        waypoint::Visual { max_range: runway.max_visual_distance },
    ));

    if let Some(ils) = &runway.ils {
        b.spawn((
            waypoint::Navaid {
                heading_range:       (runway.heading.opposite() - ils.half_width)
                    ..(runway.heading.opposite() + ils.half_width),
                pitch_range:         ils.min_pitch..ils.max_pitch,
                min_dist_horizontal: ils.visual_range,
                min_dist_vertical:   ils.decision_height,
                max_dist_horizontal: ils.horizontal_range,
                max_dist_vertical:   ils.vertical_range,
            },
            waypoint::HasCriticalRegion {},
        ));
    }
}

fn spawn_waypoints(world: &mut World, waypoints: &[store::Waypoint]) -> Result<WaypointMap, Error> {
    waypoints
        .iter()
        .map(|waypoint| {
            let waypoint_entity = world
                .spawn((
                    store::LoadedEntity,
                    bevy::core::Name::new(format!("Waypoint: {}", waypoint.name)),
                ))
                .id();
            waypoint::SpawnCommand {
                waypoint: Waypoint {
                    name:         waypoint.name.clone(),
                    display_type: choose_waypoint_display_type(&waypoint.navaids),
                    position:     waypoint
                        .position
                        .with_altitude(waypoint.elevation.unwrap_or(SEA_ALTITUDE)),
                },
            }
            .apply(waypoint_entity, world);

            world.entity_mut(waypoint_entity).with_children(|b| {
                waypoint.navaids.iter().for_each(|navaid| spawn_waypoint_navaid(b, navaid));
            });

            Ok((waypoint.name.clone(), waypoint_entity))
        })
        .collect::<HashMapResult<_, _>>()
        .map(WaypointMap)
}

fn choose_waypoint_display_type(navaids: &[store::Navaid]) -> waypoint::DisplayType {
    let has_vor = navaids.iter().any(|navaid| matches!(navaid.ty, store::NavaidType::Vor));
    let has_dme = navaids.iter().any(|navaid| matches!(navaid.ty, store::NavaidType::Dme));
    if has_vor && has_dme {
        waypoint::DisplayType::VorDme
    } else if has_vor {
        waypoint::DisplayType::Vor
    } else if has_dme {
        waypoint::DisplayType::Dme
    } else {
        waypoint::DisplayType::Waypoint
    }
}

fn spawn_waypoint_navaid(b: &mut impl ChildBuild, navaid: &store::Navaid) {
    b.spawn((waypoint::Navaid {
        heading_range:       navaid.heading_start..navaid.heading_end,
        pitch_range:         navaid.min_pitch..Angle::RIGHT,
        min_dist_horizontal: Distance::ZERO,
        min_dist_vertical:   Distance::ZERO,
        max_dist_horizontal: navaid.max_dist_horizontal,
        max_dist_vertical:   navaid.max_dist_vertical,
    },));
}

struct WaypointMap(HashMap<String, Entity>);

impl WaypointMap {
    fn resolve(&self, name: &str) -> Result<Entity, Error> {
        self.0.get(name).copied().ok_or_else(|| Error::UnresolvedWaypoint(name.to_string()))
    }
}

fn spawn_objects(
    world: &mut World,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    objects: &[store::Object],
) -> Result<(), Error> {
    for object in objects {
        match object {
            store::Object::Plane(plane) => spawn_plane(world, aerodromes, waypoints, plane)?,
        }
    }

    Ok(())
}

fn spawn_plane(
    world: &mut World,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    plane: &store::Plane,
) -> Result<(), Error> {
    let plane_entity = world
        .spawn((
            store::LoadedEntity,
            bevy::core::Name::new(format!("Plane: {}", plane.aircraft.name)),
        ))
        .id();

    let destination = match &plane.aircraft.dest {
        store::Destination::Departure { aerodrome_code, dest_waypoint } => {
            let aerodrome = aerodromes.resolve(aerodrome_code)?;
            let waypoint = waypoints.resolve(dest_waypoint)?;
            object::Destination::Departure {
                aerodrome:     aerodrome.aerodrome_entity,
                dest_waypoint: waypoint,
            }
        }
        store::Destination::Arrival { aerodrome_code } => {
            let aerodrome = aerodromes.resolve(aerodrome_code)?;
            object::Destination::Arrival { aerodrome: aerodrome.aerodrome_entity }
        }
        store::Destination::Ferry { source_aerodrome_code, dest_aerodrome_code } => {
            let source = aerodromes.resolve(source_aerodrome_code)?;
            let dest = aerodromes.resolve(dest_aerodrome_code)?;
            object::Destination::Ferry {
                from_aerodrome: source.aerodrome_entity,
                to_aerodrome:   dest.aerodrome_entity,
            }
        }
    };

    object::SpawnCommand {
        position: plane.aircraft.position.with_altitude(plane.aircraft.altitude),
        ground_speed: (plane.aircraft.ground_speed * plane.aircraft.ground_dir.into_dir2())
            .with_vertical(plane.aircraft.vert_rate),
        display: object::Display { name: plane.aircraft.name.clone() },
        destination,
    }
    .apply(plane_entity, world);

    if let store::NavTarget::Airborne(..) = plane.nav_target {
        object::SetAirborneCommand.apply(plane_entity, world);
    }

    plane::SpawnCommand {
        control: Some(plane::Control {
            heading:     plane.control.heading,
            yaw_speed:   plane.control.yaw_speed,
            horiz_accel: plane.control.horiz_accel,
        }),
        limits:  plane.plane_limits.clone(),
    }
    .apply(plane_entity, world);

    world.entity_mut(plane_entity).insert(plane.nav_limits.clone());

    if let store::NavTarget::Airborne(target) = &plane.nav_target {
        if let Some(target_altitude) = &target.target_altitude {
            world.entity_mut(plane_entity).insert(nav::TargetAltitude {
                altitude: target_altitude.altitude,
                expedite: target_altitude.expedite,
            });
        }

        if let Some(target_waypoint) = &target.target_waypoint {
            let waypoint_entity =
                resolve_waypoint_ref(aerodromes, waypoints, &target_waypoint.waypoint)?;
            world.entity_mut(plane_entity).insert(nav::TargetWaypoint { waypoint_entity });
        }

        if let Some(target_alignment) = &target.target_alignment {
            let start_waypoint =
                resolve_waypoint_ref(aerodromes, waypoints, &target_alignment.start_waypoint)?;
            let end_waypoint =
                resolve_waypoint_ref(aerodromes, waypoints, &target_alignment.end_waypoint)?;
            world.entity_mut(plane_entity).insert(nav::TargetAlignment {
                start_waypoint,
                end_waypoint,
                lookahead: target_alignment.lookahead,
                activation_range: target_alignment.activation_range,
            });
        }
    }

    Ok(())
}

fn resolve_waypoint_ref(
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    waypoint_ref: &store::WaypointRef,
) -> Result<Entity, Error> {
    match waypoint_ref {
        store::WaypointRef::Named(name) => waypoints.resolve(name),
        store::WaypointRef::RunwayThreshold(store::RunwayRef { aerodrome_code, runway_name }) => {
            let aerodrome = aerodromes.resolve(aerodrome_code)?;
            let Some(runway) = aerodrome.runways.get(runway_name) else {
                return Err(Error::UnresolvedRunway {
                    aerodrome: aerodrome_code.clone(),
                    runway:    runway_name.clone(),
                });
            };
            Ok(runway.runway)
        }
        store::WaypointRef::LocalizerStart(store::RunwayRef { aerodrome_code, runway_name }) => {
            let aerodrome = aerodromes.resolve(aerodrome_code)?;
            let Some(runway) = aerodrome.runways.get(runway_name) else {
                return Err(Error::UnresolvedRunway {
                    aerodrome: aerodrome_code.clone(),
                    runway:    runway_name.clone(),
                });
            };
            Ok(runway.localizer_waypoint)
        }
    }
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
}

type VecResult<T> = Result<Vec<T>, Error>;
type HashMapResult<K, V> = Result<HashMap<K, V>, Error>;
