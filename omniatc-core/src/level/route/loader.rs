use std::collections::HashMap;
use std::iter;

use bevy::ecs::entity::Entity;
use bevy::ecs::name::Name;
use bevy::ecs::world::World;
use either::Either;

use crate::level::aerodrome::loader::AerodromeMap;
use crate::level::route;
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
    presets: &[store::RoutePreset],
) -> Result<RoutePresetMap, load::Error> {
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
        entity_ref.insert(route::Preset {
            id:    preset.id.clone(),
            title: preset.title.clone(),
            nodes: convert_route(aerodromes, waypoints, &route_preset_map, &preset.nodes)
                .collect::<Result<_, load::Error>>()?,
        });
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
                } => route::node_vec(route::DirectWaypointNode {
                    waypoint: waypoints.resolve_ref(aerodromes, waypoint)?,
                    distance,
                    proximity,
                    altitude,
                }),
                store::RouteNode::SetAirSpeed { goal, error } => {
                    route::node_vec(route::SetAirspeedNode { speed: goal, error })
                }
                store::RouteNode::StartPitchToAltitude { goal, error, expedite } => {
                    route::node_vec(route::StartSetAltitudeNode { altitude: goal, error, expedite })
                }
                store::RouteNode::RunwayLanding { ref runway, ref goaround_preset } => {
                    let runway = aerodromes.resolve_runway_ref(runway)?.runway.runway;
                    let goaround_preset = if let Some(goaround_preset) = goaround_preset {
                        Some(route_presets.resolve(goaround_preset)?)
                    } else {
                        None
                    };
                    Vec::<route::Node>::from([
                        route::AlignRunwayNode { runway, expedite: true, goaround_preset }.into(),
                        route::ShortFinalNode { runway, goaround_preset }.into(),
                        route::VisualLandingNode { runway, goaround_preset }.into(),
                    ])
                }
                store::RouteNode::Taxi { ref segment } => route::node_vec(route::TaxiNode {
                    label:      aerodromes.resolve_segment(segment)?,
                    hold_short: false,
                }),
                store::RouteNode::HoldShort { ref segment } => route::node_vec(route::TaxiNode {
                    label:      aerodromes.resolve_segment(segment)?,
                    hold_short: true,
                }),
            })
        })
        .flat_map(|result| match result {
            Ok(nodes) => Either::Left(nodes.into_iter().map(Ok)),
            Err(err) => Either::Right(iter::once(Err(err))),
        })
}
