use std::collections::{HashMap, HashSet};

use bevy::ecs::entity::Entity;
use bevy::ecs::world::World;

use crate::level::spawn::{self, Sets};
use crate::level::{aerodrome, ground, object, route, waypoint};
use crate::load;

pub fn spawn(
    world: &mut World,
    object_types: &object::loader::ObjectTypeMap,
    aerodromes: &aerodrome::loader::AerodromeMap,
    waypoints: &waypoint::loader::WaypointMap,
    route_presets: &route::loader::RoutePresetMap,
    spawn_sets: &store::WeightedList<store::SpawnSet>,
) -> load::Result<()> {
    world.resource_mut::<Sets>().0 = spawn_sets.try_map_ref(|set| {
        Ok(spawn::Set {
            gen_name: set.gen_name.clone(),
            types:    set.types.try_map_ref(|ty| object_types.resolve(ty))?,
            route:    set.route.try_map_ref(|route| {
                Ok(spawn::Route {
                    preset:      route_presets.resolve(&route.preset)?,
                    destination: object::loader::resolve_destination(
                        aerodromes,
                        waypoints,
                        &route.destination,
                    )?,
                    score:       route.score,
                })
            })?,
            position: set
                .position
                .try_map_ref(|position| resolve_position(aerodromes, waypoints, position))?,
        })
    })?;
    Ok(())
}

fn resolve_position(
    aerodromes: &aerodrome::loader::AerodromeMap,
    waypoints: &waypoint::loader::WaypointMap,
    position: &store::SpawnPosition,
) -> load::Result<spawn::Location> {
    match position {
        store::SpawnPosition::Aprons { aerodrome, aprons } => {
            let aerodrome = aerodromes.resolve(aerodrome)?;
            let aprons = aprons.as_ref().map(|vec| vec.iter().collect::<HashSet<_>>());
            let apron_entities = aerodrome
                .spawned_segments
                .iter()
                .filter(|(key, _)| match aprons {
                    None => key.is_apron(),
                    Some(ref set) => {
                        if let ground::SegmentLabel::Apron { name } = key {
                            set.contains(name)
                        } else {
                            false
                        }
                    }
                })
                .flat_map(|(_, segments)| segments.iter().map(|segment| segment.entity))
                .collect();
            Ok(spawn::Location::Aprons(apron_entities))
        }
        store::SpawnPosition::Runway { runway, taxiways } => {
            let aerodrome = aerodromes.resolve(&runway.aerodrome)?;
            let spawned_runway = &aerodromes.resolve_runway_ref(runway)?.runway;
            let taxiways = taxiways
                .iter()
                .map(|taxiway_name| {
                    match aerodrome
                        .spawned_segments
                        .get(&ground::SegmentLabel::Taxiway { name: taxiway_name.clone() })
                    {
                        None => Err(load::Error::UnresolvedSegment {
                            variant:   "taxiway",
                            value:     taxiway_name.clone(),
                            aerodrome: runway.aerodrome.0.clone(),
                        }),
                        Some(taxiway_segments) => {
                            let closest_segment = taxiway_segments
                                .iter()
                                .min_by_key(|&segment| {
                                    segment
                                        .alpha_position
                                        .distance_cmp(spawned_runway.start_pos)
                                        .min(
                                            segment
                                                .beta_position
                                                .distance_cmp(spawned_runway.start_pos),
                                        )
                                })
                                .expect("taxiway_segments should not be empty");

                            Ok(spawn::LocationRunwayTaxiway {
                                taxiway_segment: closest_segment.entity,
                                direction:       if closest_segment
                                    .alpha_position
                                    .distance_cmp(spawned_runway.start_pos)
                                    < closest_segment
                                        .beta_position
                                        .distance_cmp(spawned_runway.start_pos)
                                {
                                    ground::SegmentDirection::BetaToAlpha
                                } else {
                                    ground::SegmentDirection::AlphaToBeta
                                },
                            })
                        }
                    }
                })
                .collect::<load::VecResult<_>>()?;
            Ok(spawn::Location::Runway(taxiways))
        }
        store::SpawnPosition::Airborne { waypoint, altitude, speed, heading } => {
            Ok(spawn::Location::Airborne {
                waypoint: waypoints.resolve(waypoint)?,
                altitude: *altitude,
                speed:    *speed,
                heading:  *heading,
            })
        }
    }
}
