use std::collections::HashMap;

use bevy_math::Vec2;
use math::{Angle, Heading, Length, Position};
use serde::{Deserialize, Serialize};

use crate::{ObjectType, ObjectTypeRef, RouteNode, RoutePresetRef, WaypointRef, WeightedList};

mod env;
pub use env::*;

mod aerodrome;
pub use aerodrome::*;

mod spawn;
pub use spawn::*;

/// Contents of a map.
#[derive(Clone, Serialize, Deserialize)]
pub struct Level {
    /// Environmental features of the map.
    pub environment:   Environment,
    /// Types of objects that may exist in the level.
    pub object_types:  HashMap<ObjectTypeRef, ObjectType>,
    /// Aerodromes in the map.
    pub aerodromes:    Vec<Aerodrome>,
    /// Waypoints in the airspace.
    pub waypoints:     Vec<Waypoint>,
    /// Route presets that aircraft may be assigned to.
    pub route_presets: Vec<RoutePreset>,
    /// Spawnpoints for new objects.
    pub spawn_sets:    WeightedList<SpawnSet>,
    /// Determines when new objects may spawn.
    pub spawn_trigger: SpawnTrigger,
}

/// A waypoint in the airspace.
#[derive(Clone, Serialize, Deserialize)]
pub struct Waypoint {
    /// Display name of the waypoint.
    pub name:      String,
    /// Position of the waypoint.
    pub position:  Position<Vec2>,
    /// Elevation of the navaids of the waypoint, if any.
    pub elevation: Option<Position<f32>>,
    /// Navaids provided at this waypoint.
    pub navaids:   Vec<Navaid>,
    /// Whether the waypoint can be observed visually when in proximity.
    pub visual:    Option<VisualWaypoint>,
}

/// A navigation aid provided at a waypoint.
#[derive(Clone, Serialize, Deserialize)]
pub struct Navaid {
    /// Type of navaid.
    #[serde(rename = "type")]
    pub ty:            NavaidType,
    /// Horizontal radial directions from which the navaid can be received.
    ///
    /// The range is taken in clockwise direction. That is,
    /// A receiver at heading `h` from the navaid is within this range
    /// if and only if sweeping from `heading_start` to `h` in clockwise direction
    /// does not cross `heading_end`.
    ///
    /// If `heading_start == heading_end`, there is no heading restriction.
    pub heading_start: Heading,
    /// See `heading_start`.
    pub heading_end:   Heading,

    /// Minimum angle of elevation from the navaid to the receiver.
    pub min_pitch: Angle,

    /// Maximum horizontal distance of the receiver from the navaid.
    pub max_dist_horizontal: Length<f32>,
    /// Maximum vertical distance of the receiver from the navaid.
    pub max_dist_vertical:   Length<f32>,
}

/// The type of navaid.
///
/// Currently this only affects the UI display.
#[derive(Clone, Serialize, Deserialize)]
pub enum NavaidType {
    /// This navaid tells the heading from the aircraft to the waypoint
    Vor,
    /// This navaid tells the distance of the aircraft from the waypoint.
    Dme,
}

/// Conditions under which a waypoint is visible,
/// allowing it to serve as a visual navaid.
#[derive(Clone, Serialize, Deserialize)]
pub struct VisualWaypoint {
    /// Maximum 3D distance from which pilots can see the waypoint.
    pub max_distance: Length<f32>,
}

/// A preset route that an aircraft may be assigned to.
#[derive(Clone, Serialize, Deserialize)]
pub struct RoutePreset {
    /// When is this preset available for use?
    pub trigger: RoutePresetTrigger,
    /// Identifies the preset.
    ///
    /// Different triggers of the same preset can have the same `id`,
    /// used for identifying that a route starting at another trigger
    /// can be considered equivalent when the user selects a route change.
    ///
    /// Different presets from the same trigger must not have duplicate `id`s.
    pub id:      String,
    /// Identifier used to reference the preset from other places in the save file.
    ///
    /// This field is only used in the save file and is not visible to users.
    /// If specified, it MUST be a unique value among all presets,
    /// unlike `id` which may be shared between similar presets.
    /// This field is optional and only useful when the route needs to be referenced,
    /// e.g. to initiate a goaround route.
    ///
    /// It is recommended to compose `ref_id` by appending the name of the first waypoint to `id`.
    pub ref_id:  Option<RoutePresetRef>,
    /// Display name of this route. Not a unique identifier.
    pub title:   String,
    /// Nodes of this route.
    /// If the trigger is a waypoint,
    /// the first node should be [`DirectWaypoint`](RouteNode::DirectWaypoint) to that waypoint.
    pub nodes:   Vec<RouteNode>,
}

/// Generates [`RoutePreset`] starting at each waypoint on the way.
#[must_use]
pub fn route_presets_at_waypoints(
    id: &str,
    title: &str,
    nodes: Vec<RouteNode>,
) -> Vec<RoutePreset> {
    nodes
        .iter()
        .enumerate()
        .rev()
        .filter_map(|(start_index, start_node)| {
            let RouteNode::DirectWaypoint {
                waypoint: waypoint @ WaypointRef::Named(waypoint_name),
                ..
            } = start_node
            else {
                return None;
            };
            Some(RoutePreset {
                trigger: RoutePresetTrigger::Waypoint(waypoint.clone()),
                id:      id.to_owned(),
                ref_id:  Some(RoutePresetRef(format!("{id} {}", &waypoint_name.0))),
                title:   title.to_owned(),
                nodes:   nodes[start_index..].to_vec(),
            })
        })
        .collect()
}

/// Defines when a route preset may be selected.
#[derive(Clone, Serialize, Deserialize)]
pub enum RoutePresetTrigger {
    /// This preset may be selected when the current direct target is the waypoint.
    Waypoint(WaypointRef),
}
