use math::{Length, Position, Speed};
use serde::{Deserialize, Serialize};

use crate::{RoutePresetRef, RunwayRef, SegmentRef, WaypointRef};

/// A sequence of highest-level actions to execute,
/// describing the route to follow.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub struct Route {
    /// The name of the route currently executing, if any.
    ///
    /// Only affects UI.
    pub id:    Option<String>,
    /// The sequence of actions to execute.
    pub nodes: Vec<RouteNode>,
}

/// A single action in a route.
#[derive(Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
        /// Target airspeed.
        goal:  Speed<f32>,
        /// If `Some`, this node blocks until the airspeed is within `goal` &pm; `error`.
        error: Option<Speed<f32>>,
    },
    /// Start reaching for the target altitude.
    StartPitchToAltitude {
        /// Target altitude.
        goal:     Position<f32>,
        /// If `Some`, this node blocks until the altitude is within `goal` &pm; `error`.
        error:    Option<Length<f32>>,
        /// Whether to use maximum possible climb/descent rate.
        expedite: bool,
    },
    /// Align with the ILS of a runway and land on it.
    RunwayLanding {
        /// Runway to land on.
        runway:          RunwayRef,
        /// Preset to switch to upon missed approach.
        goaround_preset: Option<RoutePresetRef>,
        /// Current phase of the landing.
        ///
        /// When used as a route template,
        /// this can be used to allow visual approach without ILS.
        #[serde(default)]
        current_phase:   LandingPhase,
    },
    /// Take off from a runway.
    RunwayTakeoff {
        /// Runway to line up on.
        runway:          RunwayRef,
        /// Initial altitude clearance after takeoff.
        target_altitude: Position<f32>,
    },
    /// Line up on a runway for takeoff.
    RunwayLineup {
        /// Runway to line up on.
        runway: RunwayRef,
    },
    /// Taxi to a segment on the ground.
    ///
    /// If multiple `Taxi`/`HoldShort` steps are specified contiguously,
    /// the shortest path satisfying all of them is chosen.
    /// If this is the last `Taxi`/`HoldShort` step in a contiguous sequence,
    /// the object stops at the end of the first segment
    /// (a strip of taxiway between two intersection points)
    /// matching the given segment reference.
    Taxi {
        /// Segment to taxi to.
        segment: SegmentRef,
    },
    /// Hold short of a segment on the ground.
    ///
    /// This step completes when the object is stopped at an intersection point
    /// that adjoins a ground path matching the given segment reference.
    /// The object stays clear of the intersection point
    /// and does not enter the segment itself.
    HoldShort {
        /// Segment to hold short of.
        segment: SegmentRef,
    },
    /// Wait for explicit clearance from ATC before proceeding to the next node.
    WaitForClearance,
}

/// Phase of landing, used to load aircraft in the middle of a landing.
#[derive(Default, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
pub enum LandingPhase {
    /// The aircraft is only navigating based on ILS.
    #[default]
    Align,
    /// The aircraft is on short final and started reducing to landing speed.
    ShortFinal,
    /// The aircraft has acquired visual contact with the runway.
    Visual,
}

/// How to handle proximity to a waypoint when navigating to it.
#[derive(Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
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
