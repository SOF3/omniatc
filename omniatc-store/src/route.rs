use math::{Length, Position, Speed};
use serde::{Deserialize, Serialize};

use crate::{RunwayRef, SegmentRef, WaypointRef};

#[derive(Clone, Serialize, Deserialize)]
pub struct Route {
    pub id:    Option<String>,
    pub nodes: Vec<RouteNode>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum RouteNode {
    /// Direct to a waypoint.
    DirectWaypoint {
        /// Waypoint to horizontally navigate to.
        waypoint:  WaypointRef,
        /// The node is considered complete when
        /// the horizontal distance between the object and the waypoint is less than this value.
        distance:  Length<f32>,
        /// Whether the object is allowed to complete this node early when in proximity.
        proximity: WaypointProximity,
        /// Start pitching at standard rate *during or before* this node,
        /// approximately reaching this altitude by the time the specified waypoint is reached.
        altitude:  Option<Position<f32>>,
    },
    /// Adjust throttle until the airspeed is reached.
    SetAirSpeed {
        goal:  Speed<f32>,
        /// If `Some`, this node blocks until the airspeed is within `goal` &pm; `error`.
        error: Option<Speed<f32>>,
    },
    /// Pitch until the altitude is reached.
    StartPitchToAltitude {
        goal:     Position<f32>,
        /// If `Some`, this node blocks until the altitude is within `goal` &pm; `error`.
        error:    Option<Length<f32>>,
        expedite: bool,
        // TODO pressure altitude?
    },
    RunwayLanding {
        /// Runway to land on.
        runway:          RunwayRef,
        /// Preset to switch to upon missed approach.
        goaround_preset: Option<String>,
    },
    Taxi {
        segment: SegmentRef,
    },
    HoldShort {
        segment: SegmentRef,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum WaypointProximity {
    /// Turn to the next waypoint before arriving at the waypoint,
    /// such that the position after the turn is exactly between the two waypoints.
    ///
    /// The step is always completed when the proximity range is entered,
    /// allowing smooth transition when the next waypoint has the same heading.
    FlyBy,
    /// Enter the horizontal distance range of the waypoint before turning to the next one.
    FlyOver,
}
