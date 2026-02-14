use std::collections::HashMap;
use std::num::NonZero;
use std::{iter, mem};

use bevy::ecs::entity::Entity;
use bevy::ecs::name::Name;
use bevy::ecs::world::World;
use either::Either;

use crate::level::aerodrome::loader::AerodromeMap;
use crate::level::ground;
use crate::level::route::{self, TaxiStopMode};
use crate::level::waypoint::loader::WaypointMap;
use crate::load::{self, StoredEntity};

pub struct RoutePresetMap(HashMap<store::RoutePresetRef, Entity>);

impl RoutePresetMap {
    /// Resolves a route preset by name.
    ///
    /// # Errors
    /// If the route preset name does not exist.
    pub fn resolve(&self, name: &store::RoutePresetRef) -> Result<Entity, load::Error> {
        self.0.get(name).copied().ok_or_else(|| load::Error::UnresolvedRoutePreset(name.0.clone()))
    }
}

/// Spawns route presets declared in a store into the world.
///
/// # Errors
/// If the stored route presets contain invalid references.
pub fn spawn_presets(
    world: &mut World,
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    next_standby_id: &mut NonZero<u32>,
    presets: &[store::RoutePreset],
) -> Result<RoutePresetMap, load::Error> {
    // spawn route preset entities in advance to allow route_preset_map used in convert_route.
    let route_preset_entities: Vec<_> = presets
        .iter()
        .map(|preset| world.spawn((StoredEntity, Name::new(format!("Preset: {}", preset.id)))).id())
        .collect();
    let route_preset_map = RoutePresetMap(
        presets
            .iter()
            .zip(&route_preset_entities)
            .filter_map(|(preset, &entity)| Some((preset.ref_id.clone()?, entity)))
            .collect(),
    );

    for (preset, entity) in presets.iter().zip(route_preset_entities) {
        let mut entity_ref = world.entity_mut(entity);
        entity_ref.insert((
            route::Preset {
                id:    preset.id.clone(),
                title: preset.title.clone(),
                nodes: convert_route(
                    aerodromes,
                    waypoints,
                    &route_preset_map,
                    next_standby_id,
                    &preset.nodes,
                )
                .collect::<Result<_, load::Error>>()?,
            },
            resolve_destination_matcher(aerodromes, waypoints, &preset.destinations)?,
        ));
        match &preset.trigger {
            store::RoutePresetTrigger::Waypoint(waypoint) => {
                let waypoint = waypoints.resolve_ref(aerodromes, waypoint)?;
                entity_ref.insert(route::PresetFromWaypoint(waypoint));
            }
        }
    }

    Ok(route_preset_map)
}

/// Converts a list of stored route nodes into runtime route nodes.
///
/// # Errors
/// If any of the route nodes contain invalid references.
pub fn convert_route<'a>(
    aerodromes: &'a AerodromeMap,
    waypoints: &'a WaypointMap,
    route_presets: &'a RoutePresetMap,
    next_standby_id: &'a mut NonZero<u32>,
    route_nodes: &'a [store::RouteNode],
) -> impl Iterator<Item = Result<route::Node, load::Error>> + use<'a> {
    route_nodes
        .iter()
        .map(|node| {
            Ok(match *node {
                store::RouteNode::DirectWaypoint {
                    ref waypoint,
                    distance,
                    proximity,
                    altitude,
                } => node_vec(route::DirectWaypointNode {
                    waypoint: waypoints.resolve_ref(aerodromes, waypoint)?,
                    distance,
                    proximity,
                    altitude,
                }),
                store::RouteNode::SetAirSpeed { goal, error } => {
                    node_vec(route::SetAirspeedNode { speed: goal, error })
                }
                store::RouteNode::StartPitchToAltitude { goal, error, expedite } => {
                    node_vec(route::StartSetAltitudeNode { altitude: goal, error, expedite })
                }
                store::RouteNode::RunwayLanding {
                    ref runway,
                    ref goaround_preset,
                    current_phase,
                } => {
                    let runway = aerodromes.resolve_runway_ref(runway)?.runway.runway;
                    let goaround_preset = if let Some(goaround_preset) = goaround_preset {
                        Some(route_presets.resolve(goaround_preset)?)
                    } else {
                        None
                    };

                    let mut out_nodes = Vec::<route::Node>::with_capacity(3);
                    if let store::LandingPhase::Align = current_phase {
                        out_nodes.push(
                            route::AlignRunwayNode { runway, expedite: true, goaround_preset }
                                .into(),
                        );
                    }
                    if let store::LandingPhase::Align | store::LandingPhase::ShortFinal =
                        current_phase
                    {
                        out_nodes.push(route::ShortFinalNode { runway, goaround_preset }.into());
                    }
                    out_nodes.push(route::VisualLandingNode { runway, goaround_preset }.into());
                    out_nodes
                }
                store::RouteNode::RunwayTakeoff { runway: _, target_altitude } => {
                    node_vec(route::TakeoffNode { target_altitude })
                }
                store::RouteNode::RunwayLineup { ref runway } => {
                    let runway_pair = aerodromes.resolve_runway_ref(runway)?;
                    node_vec(route::TaxiNode {
                        label:     ground::SegmentLabel::RunwayPair([
                            runway_pair.runway.runway,
                            runway_pair.paired,
                        ]),
                        direction: Some(runway_pair.direction),
                        stop:      TaxiStopMode::LineUp,
                    })
                }
                store::RouteNode::Taxi { ref segment } => node_vec(route::TaxiNode {
                    label:     aerodromes.resolve_segment(segment)?,
                    direction: None,
                    stop:      TaxiStopMode::Exhaust,
                }),
                store::RouteNode::HoldShort { ref segment } => node_vec(route::TaxiNode {
                    label:     aerodromes.resolve_segment(segment)?,
                    direction: None,
                    stop:      TaxiStopMode::HoldShort,
                }),
                store::RouteNode::WaitForClearance => node_vec(route::StandbyNode {
                    skip_id: Some({
                        let next = next_standby_id.checked_add(1).expect("too many standby nodes");
                        mem::replace(next_standby_id, next)
                    }),
                }),
            })
        })
        .flat_map(|result| match result {
            Ok(nodes) => Either::Left(nodes.into_iter().map(Ok)),
            Err(err) => Either::Right(iter::once(Err(err))),
        })
}

fn node_vec(node: impl Into<route::Node>) -> Vec<route::Node> { Vec::from([node.into()]) }

fn resolve_destination_matcher(
    aerodromes: &AerodromeMap,
    waypoints: &WaypointMap,
    dests: &[store::PresetDestination],
) -> Result<route::DestinationMatcher, load::Error> {
    let items = dests
        .iter()
        .map(|dest| {
            Ok(match dest {
                store::PresetDestination::Arrival(dest) => {
                    if let Some(aerodrome) = &dest.aerodrome {
                        let aerodrome = aerodromes.resolve(aerodrome)?.aerodrome_entity;
                        route::DestinationMatcherItem::Arrival { aerodrome }
                    } else {
                        route::DestinationMatcherItem::AnyArrival
                    }
                }
                store::PresetDestination::Departure(dest) => {
                    if let Some(waypoint) = &dest.waypoint {
                        let waypoint = waypoints.resolve(waypoint)?;
                        route::DestinationMatcherItem::Departure { waypoint }
                    } else {
                        route::DestinationMatcherItem::AnyDeparture
                    }
                }
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(route::DestinationMatcher { items })
}
